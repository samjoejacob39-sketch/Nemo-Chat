//! Onboarding policy documents (Terms & Conditions, Privacy Policy,
//! Community Guidelines).
//!
//! These are honest drafts that describe what the app actually does today.
//! They are explicitly marked as drafts that require legal review before a
//! public release. `POLICY_VERSION` must be bumped whenever any text changes;
//! users are then asked to re-accept onboarding once.

/// Bump whenever any policy text changes.
pub const POLICY_VERSION: &str = "2026-11-11";

pub const DRAFT_NOTICE: &str =
    "DRAFT — This document is an honest, plain-language draft describing Nook's \
current behaviour. It must be reviewed by a qualified professional before any \
public release.";

/// One document: title + paragraphs (first line is the heading).
pub struct PolicyDoc {
    pub title: &'static str,
    /// Sections rendered as (heading, body) pairs.
    pub sections: &'static [(&'static str, &'static str)],
}

pub const TERMS: PolicyDoc = PolicyDoc {
    title: "Terms & Conditions",
    sections: &[
        (
            "About this draft",
            "This document describes the Terms under which you may use Nook in its \
current development state. It is a draft: before any public release it must be \
reviewed by a qualified legal professional. Nothing here is legal advice.",
        ),
        (
            "1. What Nook is",
            "Nook is a private messaging app for friends. You create an account with a \
username and password — no email address or phone number is required. You can find \
other people by searching for their exact username, send friend requests, and chat \
one-to-one or in groups once a request is accepted.",
        ),
        (
            "2. Your account",
            "You are responsible for keeping your password and your recovery code safe. \
The recovery code is the only way to recover an account if the password is lost; \
it is shown once at sign-up and cannot be shown again. Accounts may be deleted at \
any time from the Profile screen, which removes your data from the service.",
        ),
        (
            "3. Acceptable use",
            "You agree not to harass, threaten, impersonate or spam other users, not to \
upload unlawful content, and not to attempt to break or abuse the service. We may \
suspend accounts that violate these rules or the Community Guidelines.",
        ),
        (
            "4. Content",
            "You keep ownership of the content you send. You grant Nook the limited \
rights needed to store and deliver your messages and images to the people you chat \
with. Messages you send are stored on our servers so they can be delivered and \
synced; you can delete your messages for yourself, or for everyone within one hour \
of sending.",
        ),
        (
            "5. Availability",
            "Nook is provided \u{201c}as is\u{201d}, without warranties of any kind. Features may \
change or be withdrawn during development. We do not guarantee uninterrupted or \
error-free service.",
        ),
        (
            "6. Changes",
            "If these terms change materially, we bump the policy version and ask you to \
review and accept the new version before continuing to use the app.",
        ),
    ],
};

pub const PRIVACY: PolicyDoc = PolicyDoc {
    title: "Privacy Policy",
    sections: &[
        (
            "About this draft",
            "This draft describes exactly what data Nook collects today and why. It \
must be reviewed by a qualified professional before any public release.",
        ),
        (
            "1. Data we store",
            "Account data: your username, display name, optional bio, optional profile \
picture, a salted and hashed copy of your password (Argon2id), a hashed copy of \
your recovery code, and your privacy settings.",
            ),
        (
            "2. Messages and media",
            "Messages you send, images you share, reactions and read receipts are \
stored in our PostgreSQL database so chats stay in sync across sessions. Group \
messages are visible to all members of the group. Nook does not claim end-to-end \
encryption: data is protected in transit with TLS and at rest by our storage \
security, but our servers can technically read message content.",
        ),
        (
            "3. What we do not collect",
            "Nook does not ask for your email address or phone number, does not upload \
your address book or contacts, does not collect your location, and does not use \
third-party analytics or advertising SDKs.",
        ),
        (
            "4. Who can see what",
            "Your username is searchable by others according to your searchability \
setting (everyone, friends, or nobody). Your online status and last-seen time are \
shown to friends only according to your privacy settings. Read receipts can be \
turned off in Settings; other users will then simply not see when you read their \
messages.",
        ),
        (
            "5. Sessions and security",
            "Login sessions are random tokens stored hashed on our servers and sent \
over TLS. On your device the session is stored in app-private storage. You can log \
out of a session at any time, which invalidates it.",
        ),
        (
            "6. Deletion",
            "Deleting your account removes your profile, messages, images, friend \
relationships and notifications from our database. This cannot be undone.",
        ),
        (
            "7. Contact",
            "This is a self-hostable development application; contact the operator of \
the server you use for any data requests.",
        ),
    ],
};

pub const GUIDELINES: PolicyDoc = PolicyDoc {
    title: "Community Guidelines",
    sections: &[
        (
            "Be kind",
            "Nook is for chatting with people you know. Treat others with respect. \
Harassment, hate speech, threats and sexual content involving minors are strictly \
forbidden.",
        ),
        (
            "No impersonation",
            "Do not pretend to be another person, brand or member of the Nook team. \
Usernames that impersonate others may be removed.",
        ),
        (
            "No spam or abuse",
            "Do not send bulk unwanted messages, flood chats, or attempt to overwhelm \
the service. Automated abuse is rate-limited and may result in suspension.",
        ),
        (
            "Report and block",
            "You can block anyone at any time from a chat; blocked users cannot message \
you or find you in search. Use the Report option in a chat to flag abusive \
behaviour to the operator.",
        ),
        (
            "Enforcement",
            "Reports are reviewed by the server operator. Accounts that break these \
guidelines may be suspended or removed.",
        ),
    ],
};

/// All documents, in onboarding order.
pub const ALL_DOCS: &[PolicyDoc] = &[TERMS, PRIVACY, GUIDELINES];
