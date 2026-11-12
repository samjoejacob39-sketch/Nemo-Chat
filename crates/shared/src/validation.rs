//! Input validation rules shared by the server (authoritative) and the
//! mobile client (early feedback).

use chrono::Utc;

pub const USERNAME_MIN: usize = 3;
pub const USERNAME_MAX: usize = 24;
pub const PASSWORD_MIN: usize = 8;
pub const PASSWORD_MAX: usize = 72; // bcrypt/argon2 safe limit
pub const DISPLAY_NAME_MAX: usize = 50;
pub const BIO_MAX: usize = 280;
pub const MESSAGE_BODY_MAX: usize = 4000;
pub const GROUP_TITLE_MAX: usize = 64;
pub const SEARCH_QUERY_MAX: usize = 32;
pub const REPORT_REASON_MAX: usize = 280;

/// Usernames are always stored lowercase; matching is case-insensitive.
/// Reserved names cannot be registered.
pub const RESERVED_USERNAMES: &[&str] = &[
    "admin", "administrator", "nook", "nookapp", "support", "help", "root", "system", "moderator",
    "mod", "official", "staff", "team", "security", "abuse", "null", "undefined", "everyone",
    "here", "me", "self",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameError {
    TooShort,
    TooLong,
    InvalidCharacters,
    Reserved,
}

impl core::fmt::Display for UsernameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UsernameError::TooShort => write!(f, "Usernames need at least {USERNAME_MIN} characters."),
            UsernameError::TooLong => write!(f, "Usernames can be at most {USERNAME_MAX} characters."),
            UsernameError::InvalidCharacters => write!(
                f,
                "Only lowercase letters, numbers and underscores are allowed."
            ),
            UsernameError::Reserved => write!(f, "That username is reserved. Please pick another one."),
        }
    }
}

/// Normalizes a username: trims, lowercases.
pub fn normalize_username(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Validates an already-normalized username.
pub fn validate_username(username: &str) -> Result<(), UsernameError> {
    let len = username.chars().count();
    if len < USERNAME_MIN {
        return Err(UsernameError::TooShort);
    }
    if len > USERNAME_MAX {
        return Err(UsernameError::TooLong);
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(UsernameError::InvalidCharacters);
    }
    if username.starts_with('_') || username.ends_with('_') || username.contains("__") {
        return Err(UsernameError::InvalidCharacters);
    }
    if RESERVED_USERNAMES.contains(&username) {
        return Err(UsernameError::Reserved);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
    TooLong,
    NoLetterOrDigit,
}

impl core::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PasswordError::TooShort => write!(f, "Passwords need at least {PASSWORD_MIN} characters."),
            PasswordError::TooLong => write!(f, "Passwords can be at most {PASSWORD_MAX} characters."),
            PasswordError::NoLetterOrDigit => {
                write!(f, "Use at least one letter and one number.")
            }
        }
    }
}

pub fn validate_password(password: &str) -> Result<(), PasswordError> {
    let bytes = password.as_bytes().len();
    if bytes < PASSWORD_MIN {
        return Err(PasswordError::TooShort);
    }
    if bytes > PASSWORD_MAX {
        return Err(PasswordError::TooLong);
    }
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(PasswordError::NoLetterOrDigit);
    }
    Ok(())
}

/// Rough password strength score 0..=3 used for the sign-up meter.
/// 0 = weak, 1 = fair, 2 = good, 3 = strong.
pub fn password_strength(password: &str) -> u8 {
    let mut score = 0u8;
    let len = password.chars().count();
    if validate_password(password).is_ok() {
        score += 1;
    }
    if len >= 12 {
        score += 1;
    }
    let classes = [
        password.chars().any(|c| c.is_uppercase()),
        password.chars().any(|c| !c.is_alphanumeric()),
        password.chars().any(|c| c.is_ascii_digit()),
    ]
    .iter()
    .filter(|b| **b)
    .count() as u8;
    if classes >= 2 {
        score += 1;
    }
    score.min(3)
}

pub fn validate_display_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Please enter a name your friends will see.".into());
    }
    if trimmed.chars().count() > DISPLAY_NAME_MAX {
        return Err(format!("Names can be at most {DISPLAY_NAME_MAX} characters."));
    }
    Ok(())
}

pub fn validate_bio(bio: &str) -> Result<(), String> {
    if bio.chars().count() > BIO_MAX {
        return Err(format!("Bios can be at most {BIO_MAX} characters."));
    }
    Ok(())
}

pub fn validate_message_body(body: &str) -> Result<(), String> {
    if body.chars().count() > MESSAGE_BODY_MAX {
        return Err(format!("Messages can be at most {MESSAGE_BODY_MAX} characters."));
    }
    Ok(())
}

pub fn validate_group_title(title: &str) -> Result<(), String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("Please enter a group name.".into());
    }
    if trimmed.chars().count() > GROUP_TITLE_MAX {
        return Err(format!("Group names can be at most {GROUP_TITLE_MAX} characters."));
    }
    Ok(())
}

pub fn validate_search_query(q: &str) -> Result<(), String> {
    let trimmed = q.trim();
    if trimmed.is_empty() {
        return Err("Type a username to search.".into());
    }
    if trimmed.chars().count() > SEARCH_QUERY_MAX {
        return Err(format!("Search queries can be at most {SEARCH_QUERY_MAX} characters."));
    }
    Ok(())
}

pub fn validate_report_reason(reason: &str) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("Please choose a reason.".into());
    }
    if reason.chars().count() > REPORT_REASON_MAX {
        return Err(format!("Reasons can be at most {REPORT_REASON_MAX} characters."));
    }
    Ok(())
}

/// Time windows for editing / deleting-for-everyone, server-enforced.
pub const EDIT_WINDOW: chrono::Duration = chrono::Duration::minutes(15);
pub const DELETE_FOR_EVERYONE_WINDOW: chrono::Duration = chrono::Duration::hours(1);

pub fn can_edit(created_at: chrono::DateTime<Utc>) -> bool {
    Utc::now() - created_at <= EDIT_WINDOW
}

pub fn can_delete_for_everyone(created_at: chrono::DateTime<Utc>) -> bool {
    Utc::now() - created_at <= DELETE_FOR_EVERYONE_WINDOW
}

/// The exact emoji set the server accepts as reactions and the client offers.
pub const REACTION_EMOJIS: &[&str] = &[
    "❤️", "👍", "👎", "😂", "😮", "😢", "🎉", "🔥", "😍", "🤔", "👏", "🙏",
    "💯", "😅", "🥳", "😴", "🤝", "✨", "⭐", "💡", "👀", "🤗", "😎", "🫶",
];

pub fn is_valid_reaction(emoji: &str) -> bool {
    REACTION_EMOJIS.contains(&emoji)
}
