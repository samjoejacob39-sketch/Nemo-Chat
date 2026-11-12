//! Nook mobile — Rust application logic (UI lives in `ui/*.slint`).
//!
//! The same binary runs on desktop (for preview) and on Android. On Android
//! the entry point is `android_main`, which initialises the Slint backend
//! before handing off to the shared `run()`.

slint::include_modules!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::android_activity::AndroidApp) {
    slint::android::init(app).expect("failed to initialise Slint Android backend");
    run();
}

/// Shared entry point used by both desktop and Android.
pub fn run() {
    // Make sure a TLS crypto provider is installed before any HTTPS call.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let app = AppWindow::new().expect("failed to create Nook window");
    load_demo_data(&app);
    app.set_demo_mode(true);
    app.run().expect("Nook window crashed");
}

fn gradient_image(width: usize, height: usize, from: [u8; 3], to: [u8; 3]) -> slint::Image {
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width as u32, height as u32);
    let total = (width + height) as f32;
    for (i, px) in buffer.make_mut_slice().iter_mut().enumerate() {
        let x = (i % width) as f32;
        let y = (i / width) as f32;
        let t = (x + y) / total;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        *px = slint::Rgba8Pixel::new(mix(from[0], to[0]), mix(from[1], to[1]), mix(from[2], to[2]), 255);
    }
    slint::Image::from_rgba8(buffer)
}

fn load_demo_data(app: &AppWindow) {
    use slint::{ModelRc, VecModel};
    use std::rc::Rc;

    // --- Chats list ---
    let chats = vec![
        UiChat { index: 0, title: "Maya Chen".into(), subtitle: "Perfect. I'll bring the good beans 🙂".into(), time: "10:12".into(), unread: 2, initials: "MC".into(), variant: 1, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 1, title: "Trail Crew 🚀".into(), subtitle: "Sam: pushing the fix now".into(), time: "09:58".into(), unread: 0, initials: "TC".into(), variant: 2, has_avatar: false, avatar: slint::Image::default(), online: false, muted: true, pinned: true },
        UiChat { index: 2, title: "Sam Okafor".into(), subtitle: "see you at practice".into(), time: "Yesterday".into(), unread: 0, initials: "SO".into(), variant: 3, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 3, title: "Nadia Farouk".into(), subtitle: "typing…".into(), time: "Yesterday".into(), unread: 1, initials: "NF".into(), variant: 4, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 4, title: "Alex Rivera".into(), subtitle: "sent a photo".into(), time: "Mon".into(), unread: 0, initials: "AR".into(), variant: 5, has_avatar: false, avatar: slint::Image::default(), online: false, muted: false, pinned: false },
    ];
    app.set_chats(ModelRc::from(Rc::new(VecModel::from(chats))));

    // --- Messages ---
    let img = gradient_image(240, 160, [70, 120, 220], [140, 90, 200]);
    let messages = vec![
        UiBubble { index: 0, kind: 0, text: "Hey! Are we still on for coffee tomorrow?".into(), mine: false, read: true, time: "09:42".into(), day: "Today".into(), is_day_sep: true, sender: "Maya".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 1, kind: 0, text: "Absolutely — 10am at the usual spot?".into(), mine: true, read: true, time: "09:43".into(), day: "".into(), is_day_sep: false, sender: "".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 2, kind: 0, text: "Perfect. I'll bring the good beans 🙂".into(), mine: false, read: true, time: "09:45".into(), day: "".into(), is_day_sep: false, sender: "Maya".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 3, kind: 1, text: "".into(), mine: true, read: false, time: "10:02".into(), day: "".into(), is_day_sep: false, sender: "".into(), has_image: true, image: img.clone() },
        UiBubble { index: 4, kind: 2, text: "Maya is typing…".into(), mine: false, read: false, time: "".into(), day: "".into(), is_day_sep: false, sender: "".into(), has_image: false, image: slint::Image::default() },
    ];
    app.set_messages(ModelRc::from(Rc::new(VecModel::from(messages))));
    app.set_typing_name("".into());

    // --- Recovery codes ---
    let codes = vec!["NOOK-ABCD-1234".into(), "NOOK-EFGH-5678".into(), "NOOK-IJKL-9012".into(), "NOOK-MNOP-3456".into()];
    app.set_recovery_codes(ModelRc::from(Rc::new(VecModel::from(codes))));

    // --- Terms ---
    app.set_policy_title("Terms of Service".into());
    let sections = vec![
        PolicySection { heading: "1. Welcome to Nook".into(), body: "Nook is a privacy-first messaging app. By using it you agree to these terms.".into() },
        PolicySection { heading: "2. Your account".into(), body: "You are responsible for keeping your password and recovery codes safe. Nook has no email or phone recovery.".into() },
        PolicySection { heading: "3. Acceptable use".into(), body: "Be kind. No spam, harassment, or illegal content.".into() },
    ];
    app.set_policy_sections(ModelRc::from(Rc::new(VecModel::from(sections))));

    // --- Requests ---
    let requests = vec![
        UiPerson { id: 101, name: "Nadia Farouk".into(), handle: "nadia.f".into(), initials: "NF".into(), variant: 4, online: true, subtitle: "3 mutual friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 102, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, online: false, subtitle: "Sent 2 hours ago".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
    ];
    app.set_requests(ModelRc::from(Rc::new(VecModel::from(requests))));

    // --- Activity ---
    let activity = vec![
        UiActivity { id: 201, name: "Maya Chen".into(), initials: "MC".into(), variant: 1, action: "reacted ❤️ to your message".into(), time: "10:20".into(), unread: true },
        UiActivity { id: 202, name: "Sam Okafor".into(), initials: "SO".into(), variant: 3, action: "added you to Trail Crew 🚀".into(), time: "09:58".into(), unread: true },
        UiActivity { id: 203, name: "Nadia Farouk".into(), initials: "NF".into(), variant: 4, action: "sent you a friend request".into(), time: "Yesterday".into(), unread: false },
    ];
    app.set_activity(ModelRc::from(Rc::new(VecModel::from(activity))));

    // --- Find ---
    let find = vec![
        UiPerson { id: 301, name: "Jordan Blake".into(), handle: "jordan.b".into(), initials: "JB".into(), variant: 5, online: false, subtitle: "2 mutual friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 302, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, online: false, subtitle: "Friend of Maya".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 303, name: "Priya Raman".into(), handle: "priya".into(), initials: "PR".into(), variant: 3, online: true, subtitle: "Friends of friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 304, name: "Maya Chen".into(), handle: "maya.c".into(), initials: "MC".into(), variant: 1, online: true, subtitle: "".into(), state: 2, has_avatar: false, avatar: slint::Image::default() },
    ];
    app.set_find_results(ModelRc::from(Rc::new(VecModel::from(find))));

    // --- Profile ---
    app.set_me_name("Noah Bennett".into());
    app.set_me_handle("noah.b".into());
    app.set_me_bio("Coffee, trails, and good playlists.".into());
    app.set_me_initials("NB".into());
    app.set_me_variant(0);
    app.set_friend_count(24);
    app.set_group_count(5);

    // --- Create group picker ---
    let friends = vec![
        UiFriendPick { id: 401, name: "Maya Chen".into(), handle: "maya.c".into(), initials: "MC".into(), variant: 1, selected: true },
        UiFriendPick { id: 402, name: "Sam Okafor".into(), handle: "sam.o".into(), initials: "SO".into(), variant: 3, selected: true },
        UiFriendPick { id: 403, name: "Nadia Farouk".into(), handle: "nadia.f".into(), initials: "NF".into(), variant: 4, selected: false },
        UiFriendPick { id: 404, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, selected: false },
        UiFriendPick { id: 405, name: "Priya Raman".into(), handle: "priya".into(), initials: "PR".into(), variant: 3, selected: true },
    ];
    app.set_friend_picks(ModelRc::from(Rc::new(VecModel::from(friends))));

    // --- Group info members ---
    let members = vec![
        UiMember { id: 501, name: "Maya Chen".into(), handle: "maya.c".into(), initials: "MC".into(), variant: 1, admin: true },
        UiMember { id: 502, name: "Sam Okafor".into(), handle: "sam.o".into(), initials: "SO".into(), variant: 3, admin: true },
        UiMember { id: 503, name: "Noah Bennett".into(), handle: "noah.b".into(), initials: "NB".into(), variant: 0, admin: false },
        UiMember { id: 504, name: "Nadia Farouk".into(), handle: "nadia.f".into(), initials: "NF".into(), variant: 4, admin: false },
        UiMember { id: 505, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, admin: false },
        UiMember { id: 506, name: "Priya Raman".into(), handle: "priya".into(), initials: "PR".into(), variant: 3, admin: false },
    ];
    app.set_group_members(ModelRc::from(Rc::new(VecModel::from(members))));
    app.set_group_name("Trail Crew 🚀".into());
}

// Re-exported so the desktop binary can call into the library.
pub use slint::ComponentHandle;
