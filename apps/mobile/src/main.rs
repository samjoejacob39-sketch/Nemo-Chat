//! Desktop preview entry point for Nook.
//!
//! All application logic lives in the `nook_mobile` library so the same
//! code path is shared with the Android build. This binary is just a thin
//! launcher.

fn main() {
    nook_mobile::run();
}
