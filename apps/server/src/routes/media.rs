//! Media upload and serving.

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use nook_shared::UploadImageResponse;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, ApiResult};
use crate::media;
use crate::state::{AppState, RATE_UPLOAD};

/// `POST /api/media/upload` — multipart field `file`, optional `purpose`
/// ("image" or "avatar").
pub async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    mut multipart: axum::extract::Multipart,
) -> ApiResult<(StatusCode, Json<UploadImageResponse>)> {
    state
        .limiter
        .check(&user.user_id.to_string(), &RATE_UPLOAD)
        .map_err(AppError::rate_limited)?;

    let mut kind: &'static str = "image";
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("invalid multipart body: {e}")))?
    {
        match field.name() {
            Some("purpose") => {
                if field.text().await.unwrap_or_default() == "avatar" {
                    kind = "avatar";
                }
            }
            Some("file") => {
                bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::bad_request(format!("could not read upload: {e}")))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let bytes = bytes.ok_or_else(|| AppError::validation("Please attach an image file."))?;
    let stored = media::store_image(&state, user.user_id, kind, &bytes).await?;
    Ok((StatusCode::CREATED, Json(UploadImageResponse { attachment: stored.attachment })))
}

/// `GET /api/media/:id` — serves a stored image after an access check.
pub async fn serve(
    State(state): State<AppState>,
    user: AuthUser,
    Path(attachment_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let (bytes, mime) = media::load_attachment(&state, attachment_id, user.user_id).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&mime) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static("private, max-age=3600"));
    Ok(response)
}
