//! Image upload pipeline: validate, decode, strip metadata by re-encoding,
//! downscale oversized images, persist to disk, record in `attachments`.

use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use nook_shared::AttachmentMeta;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{AppError, ApiResult};
use crate::state::AppState;

pub const MAX_DIMENSION: u32 = 4096;
/// Longest edge after normalization — keeps bubbles fast on phones.
pub const MAX_USEFUL_EDGE: u32 = 1920;
pub const ALLOWED_MIMES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

pub struct StoredImage {
    pub attachment: AttachmentMeta,
}

/// Validates + stores an uploaded image for `owner`.
pub async fn store_image(
    state: &AppState,
    owner: Uuid,
    kind: &'static str,
    bytes: &[u8],
) -> ApiResult<StoredImage> {
    if bytes.is_empty() {
        return Err(AppError::validation("The file is empty."));
    }
    if bytes.len() > state.config.max_upload_bytes {
        return Err(AppError::validation(
            "That image is too large. Please pick one under 10 MB.",
        ));
    }

    let decoded = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AppError::validation("That file is not a readable image."))
        .and_then(|r| {
            r.decode().map_err(|_| AppError::validation("That file is not a supported image (use PNG, JPEG, WebP or GIF)."))
        })?;

    let source_mime = match decoded {
        _ => guess_source_mime(bytes),
    };
    if !ALLOWED_MIMES.contains(&source_mime) {
        return Err(AppError::validation("Only PNG, JPEG, WebP or GIF images are supported."));
    }

    let (mut rgba, had_alpha) = decoded_to_rgba(decoded);
    let mut width = rgba.width();
    let mut height = rgba.height();
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::validation("Image dimensions are too large (max 4096 px)."));
    }
    if width > MAX_USEFUL_EDGE || height > MAX_USEFUL_EDGE {
        let scale = f64::from(MAX_USEFUL_EDGE) / f64::from(width.max(height));
        let new_w = ((f64::from(width) * scale).round() as u32).max(1);
        let new_h = ((f64::from(height) * scale).round() as u32).max(1);
        let resized = image::imageops::resize(
            &rgba,
            new_w,
            new_h,
            image::imageops::FilterType::Lanczos3,
        );
        width = new_w;
        height = new_h;
        rgba = resized;
    }

    // Re-encode: strips EXIF/metadata. JPEG for opaque photos, PNG otherwise.
    let (ext, mime) = if had_alpha { ("png", "image/png") } else { ("jpg", "image/jpeg") };
    let mut encoded = Vec::with_capacity(rgba.len() / 2);
    if had_alpha {
        PngEncoder::new(std::io::Cursor::new(&mut encoded))
            .write_image(
                rgba.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| AppError::internal(format!("png encode: {e}")))?;
    } else {
        let rgb = image::DynamicImage::ImageRgba8(rgba.clone()).to_rgb8();
        JpegEncoder::new_with_quality(std::io::Cursor::new(&mut encoded), 85)
            .write_image(
                rgb.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| AppError::internal(format!("jpeg encode: {e}")))?;
    }

    let id = Uuid::now_v7();
    let sha = {
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        hex::encode(hasher.finalize())
    };
    let rel_path = format!("attachments/{ext}/{id}.{ext}");
    let abs_path = state.config.media_dir.join(&rel_path);
    if let Some(parent) = abs_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::internal(format!("mkdir: {e}")))?;
    }
    tokio::fs::write(&abs_path, &encoded)
        .await
        .map_err(|e| AppError::internal(format!("write: {e}")))?;

    sqlx::query(
        "INSERT INTO attachments (id, owner_id, kind, mime, byte_size, width, height, storage_path, sha256)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(owner)
    .bind(kind)
    .bind(mime)
    .bind(encoded.len() as i64)
    .bind(width as i32)
    .bind(height as i32)
    .bind(&rel_path)
    .bind(&sha)
    .execute(&state.pool)
    .await?;

    Ok(StoredImage {
        attachment: AttachmentMeta {
            id,
            mime: mime.into(),
            byte_size: encoded.len() as i64,
            width: Some(width),
            height: Some(height),
            url: format!("/api/media/{id}"),
        },
    })
}

fn decoded_to_rgba(img: image::DynamicImage) -> (image::RgbaImage, bool) {
    // Animated GIFs: use the first frame.
    let has_alpha = img.color().has_alpha();
    (img.to_rgba8(), has_alpha)
}

fn guess_source_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// Loads an attachment's bytes from disk after an authorization check.
pub async fn load_attachment(
    state: &AppState,
    attachment_id: Uuid,
    requester: Uuid,
) -> ApiResult<(Vec<u8>, String)> {
    let row: (String, String) = sqlx::query_as::<_, (String, String)>(
        "SELECT storage_path, mime FROM attachments WHERE id = $1",
    )
    .bind(attachment_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("That image no longer exists."))?;

    let (storage_path, mime) = row;

    // Authorization: avatars are viewable by any signed-in user (they are
    // already exposed through search/profiles); message images require
    // membership in a conversation containing the message, or ownership.
    let allowed = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT
            a.kind = 'avatar'
            OR a.owner_id = $2
            OR EXISTS (
                SELECT 1
                FROM messages m
                JOIN conversation_members cm
                  ON cm.conversation_id = m.conversation_id AND cm.user_id = $2
                WHERE m.attachment_id = a.id
            )
        FROM attachments a
        WHERE a.id = $1
        "#,
    )
    .bind(attachment_id)
    .bind(requester)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(false);

    if !allowed {
        return Err(AppError::forbidden("You don't have access to this image."));
    }

    let path = resolve_media_path(&state.config.media_dir, &storage_path)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::not_found("That image is no longer available."))?;
    Ok((bytes, mime))
}

/// Guards against path traversal; storage_path must stay within media_dir.
fn resolve_media_path(media_dir: &Path, storage_path: &str) -> ApiResult<PathBuf> {
    let candidate = media_dir.join(storage_path);
    let canonical_dir = media_dir.canonicalize().unwrap_or_else(|_| media_dir.to_path_buf());
    let canonical = candidate
        .canonicalize()
        .map_err(|_| AppError::not_found("That image is no longer available."))?;
    if !canonical.starts_with(&canonical_dir) {
        return Err(AppError::bad_request("Invalid media path."));
    }
    Ok(canonical)
}
