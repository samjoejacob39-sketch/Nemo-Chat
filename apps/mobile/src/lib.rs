//! Nemo mobile — Rust application logic (UI lives in `ui/*.slint`).

slint::include_modules!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: slint::android::android_activity::AndroidApp) {
    slint::android::init(app).expect("failed to initialise Slint Android backend");
    run();
}

pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let app = AppWindow::new().expect("failed to create Nemo window");
    load_demo_data(&app);
    app.set_demo_mode(true);
    wire_update_callbacks(&app);
    app.run().expect("Nemo window crashed");
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

    let chats = vec![
        UiChat { index: 0, title: "Maya Chen".into(), subtitle: "Perfect. I'll bring the good beans 🙂".into(), time: "10:12".into(), unread: 2, initials: "MC".into(), variant: 1, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 1, title: "Trail Crew 🚀".into(), subtitle: "Sam: pushing the fix now".into(), time: "09:58".into(), unread: 0, initials: "TC".into(), variant: 2, has_avatar: false, avatar: slint::Image::default(), online: false, muted: true, pinned: true },
        UiChat { index: 2, title: "Sam Okafor".into(), subtitle: "see you at practice".into(), time: "Yesterday".into(), unread: 0, initials: "SO".into(), variant: 3, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 3, title: "Nadia Farouk".into(), subtitle: "typing…".into(), time: "Yesterday".into(), unread: 1, initials: "NF".into(), variant: 4, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { index: 4, title: "Alex Rivera".into(), subtitle: "sent a photo".into(), time: "Mon".into(), unread: 0, initials: "AR".into(), variant: 5, has_avatar: false, avatar: slint::Image::default(), online: false, muted: false, pinned: false },
    ];
    app.set_chats(ModelRc::from(Rc::new(VecModel::from(chats))));

    let img = gradient_image(240, 160, [70, 120, 220], [140, 90, 200]);
    let messages = vec![
        UiBubble { index: 0, kind: 0, text: "Hey! Are we still on for coffee tomorrow?".into(), mine: false, read: true, time: "09:42".into(), day: "Today".into(), is_day_sep: true, sender: "Maya".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 1, kind: 0, text: "Absolutely — 10am at the usual spot?".into(), mine: true, read: true, time: "09:43".into(), day: "".into(), is_day_sep: false, sender: "".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 2, kind: 0, text: "Perfect. I'll bring the good beans 🙂".into(), mine: false, read: true, time: "09:45".into(), day: "".into(), is_day_sep: false, sender: "Maya".into(), has_image: false, image: slint::Image::default() },
        UiBubble { index: 3, kind: 1, text: "".into(), mine: true, read: false, time: "10:02".into(), day: "".into(), is_day_sep: false, sender: "".into(), has_image: true, image: img.clone() },
    ];
    app.set_messages(ModelRc::from(Rc::new(VecModel::from(messages))));
    app.set_typing_name("".into());

    let codes = vec!["NOOK-ABCD-1234".into(), "NOOK-EFGH-5678".into(), "NOOK-IJKL-9012".into(), "NOOK-MNOP-3456".into()];
    app.set_recovery_codes(ModelRc::from(Rc::new(VecModel::from(codes))));

    app.set_policy_title("Terms of Service".into());
    let sections = vec![
        PolicySection { heading: "1. Welcome to Nemo".into(), body: "Nemo is a privacy-first messaging app. By using it you agree to these terms.".into() },
        PolicySection { heading: "2. Your account".into(), body: "You are responsible for keeping your password and recovery codes safe. Nemo has no email or phone recovery.".into() },
        PolicySection { heading: "3. Acceptable use".into(), body: "Be kind. No spam, harassment, or illegal content.".into() },
    ];
    app.set_policy_sections(ModelRc::from(Rc::new(VecModel::from(sections))));

    let requests = vec![
        UiPerson { id: 101, name: "Nadia Farouk".into(), handle: "nadia.f".into(), initials: "NF".into(), variant: 4, online: true, subtitle: "3 mutual friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 102, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, online: false, subtitle: "Sent 2 hours ago".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
    ];
    app.set_requests(ModelRc::from(Rc::new(VecModel::from(requests))));

    let activity = vec![
        UiActivity { id: 201, name: "Maya Chen".into(), initials: "MC".into(), variant: 1, action: "reacted ❤️ to your message".into(), time: "10:20".into(), unread: true },
        UiActivity { id: 202, name: "Sam Okafor".into(), initials: "SO".into(), variant: 3, action: "added you to Trail Crew 🚀".into(), time: "09:58".into(), unread: true },
        UiActivity { id: 203, name: "Nadia Farouk".into(), initials: "NF".into(), variant: 4, action: "sent you a friend request".into(), time: "Yesterday".into(), unread: false },
    ];
    app.set_activity(ModelRc::from(Rc::new(VecModel::from(activity))));

    let find = vec![
        UiPerson { id: 301, name: "Jordan Blake".into(), handle: "jordan.b".into(), initials: "JB".into(), variant: 5, online: false, subtitle: "2 mutual friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 302, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, online: false, subtitle: "Friend of Maya".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 303, name: "Priya Raman".into(), handle: "priya".into(), initials: "PR".into(), variant: 3, online: true, subtitle: "Friends of friends".into(), state: 0, has_avatar: false, avatar: slint::Image::default() },
        UiPerson { id: 304, name: "Maya Chen".into(), handle: "maya.c".into(), initials: "MC".into(), variant: 1, online: true, subtitle: "".into(), state: 2, has_avatar: false, avatar: slint::Image::default() },
    ];
    app.set_find_results(ModelRc::from(Rc::new(VecModel::from(find))));

    app.set_me_name("Noah Bennett".into());
    app.set_me_handle("noah.b".into());
    app.set_me_bio("Coffee, trails, and good playlists.".into());
    app.set_me_initials("NB".into());
    app.set_me_variant(0);
    app.set_friend_count(24);
    app.set_group_count(5);

    let friends = vec![
        UiFriendPick { id: 401, name: "Maya Chen".into(), handle: "maya.c".into(), initials: "MC".into(), variant: 1, selected: true },
        UiFriendPick { id: 402, name: "Sam Okafor".into(), handle: "sam.o".into(), initials: "SO".into(), variant: 3, selected: true },
        UiFriendPick { id: 403, name: "Nadia Farouk".into(), handle: "nadia.f".into(), initials: "NF".into(), variant: 4, selected: false },
        UiFriendPick { id: 404, name: "Theo Park".into(), handle: "theopark".into(), initials: "TP".into(), variant: 2, selected: false },
        UiFriendPick { id: 405, name: "Priya Raman".into(), handle: "priya".into(), initials: "PR".into(), variant: 3, selected: true },
    ];
    app.set_friend_picks(ModelRc::from(Rc::new(VecModel::from(friends))));

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

    // Update channel initial state
    app.set_update_current_version("0.1.0".into());
    app.set_update_current_build(format!("#{}", BUILD_NUMBER).into());
}

// ---------------------------------------------------------------------------
// Update channel (Nemo UI preview)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ReleaseInfo {
    build: i64,
    version: String,
    notes: String,
    download_url: String,
    sha256: String,
}

static DOWNLOADED_APK: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

const BUILD_NUMBER: &str = match option_env!("BUILD_NUMBER") {
    Some(v) => v,
    None => "0",
};

fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Update-Client/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let url = "https://api.github.com/repos/samjoejacob39-sketch/Nemo-Chat/releases?per_page=20";
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", resp.status()));
    }
    let releases: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let arr = releases.as_array().ok_or("unexpected response")?;
    for r in arr {
        if r.get("prerelease").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        let assets = match r.get("assets").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => continue,
        };
        let apk_asset = assets.iter().find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n.ends_with(".apk"))
                .unwrap_or(false)
        });
        let Some(asset) = apk_asset else { continue };
        let download_url = asset
            .get("browser_download_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sha256 = asset
            .get("digest")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_start_matches("sha256:")
            .to_string();
        let tag = r.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
        let build: i64 = tag.trim_start_matches("ui-preview-").parse().unwrap_or(0);
        let version = r
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(tag)
            .to_string();
        let notes = r.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
        return Ok(ReleaseInfo {
            build,
            version,
            notes,
            download_url,
            sha256,
        });
    }
    Err("No UI preview release with an APK asset found.".to_string())
}

fn wire_update_callbacks(app: &AppWindow) {
    use slint::ComponentHandle;
    let weak = app.as_weak();

    app.on_check_updates(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_update_status("checking".into());
            ui.set_update_error("".into());
        }
        let weak2 = weak.clone();
        std::thread::spawn(move || {
            let result = fetch_latest_release();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak2.upgrade() {
                    match result {
                        Ok(info) => {
                            ui.set_update_latest_version(info.version.into());
                            ui.set_update_latest_build(format!("#{}", info.build).into());
                            ui.set_update_notes(info.notes.into());
                            let current: i64 = BUILD_NUMBER.parse().unwrap_or(0);
                            if info.build > current {
                                ui.set_update_status("available".into());
                            } else {
                                ui.set_update_status("up-to-date".into());
                            }
                        }
                        Err(e) => {
                            ui.set_update_status("error".into());
                            ui.set_update_error(e.into());
                        }
                    }
                }
            });
        });
    });

    let weak = app.as_weak();
    app.on_download_update(move || {
        let weak2 = weak.clone();
        std::thread::spawn(move || {
            let w = weak2.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w.upgrade() {
                    ui.set_update_status("downloading".into());
                    ui.set_update_downloading(true);
                    ui.set_update_progress(0);
                }
            });

            let info = match fetch_latest_release() {
                Ok(i) => i,
                Err(e) => {
                    let w = weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_update_status("error".into());
                            ui.set_update_error(e.into());
                            ui.set_update_downloading(false);
                        }
                    });
                    return;
                }
            };

            let client = match reqwest::blocking::Client::builder()
                .user_agent("Nemo-Update-Client/1.0")
                .timeout(std::time::Duration::from_secs(300))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let msg = e.to_string();
                    let w = weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_update_status("error".into());
                            ui.set_update_error(msg.into());
                            ui.set_update_downloading(false);
                        }
                    });
                    return;
                }
            };
            let resp = match client.get(&info.download_url).send() {
                Ok(r) => r,
                Err(e) => {
                    let msg = e.to_string();
                    let w = weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_update_status("error".into());
                            ui.set_update_error(msg.into());
                            ui.set_update_downloading(false);
                        }
                    });
                    return;
                }
            };
            let bytes = match resp.bytes() {
                Ok(b) => b,
                Err(e) => {
                    let msg = e.to_string();
                    let w = weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_update_status("error".into());
                            ui.set_update_error(msg.into());
                            ui.set_update_downloading(false);
                        }
                    });
                    return;
                }
            };
            if !info.sha256.is_empty() {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(&bytes);
                let hex: String = h.finalize().iter().map(|b| format!("{:02x}", b)).collect();
                if hex != info.sha256.to_lowercase() {
                    let w = weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_update_status("error".into());
                            ui.set_update_error("Checksum mismatch - download aborted.".into());
                            ui.set_update_downloading(false);
                        }
                    });
                    return;
                }
            }
            let path = std::env::temp_dir().join("nemo-update.apk");
            if let Err(e) = std::fs::write(&path, &bytes) {
                let msg = e.to_string();
                let w = weak2.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = w.upgrade() {
                        ui.set_update_status("error".into());
                        ui.set_update_error(msg.into());
                        ui.set_update_downloading(false);
                    }
                });
                return;
            }
            *DOWNLOADED_APK.lock().unwrap() = Some(path);
            let w = weak2.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w.upgrade() {
                    ui.set_update_progress(100);
                    ui.set_update_downloading(false);
                    ui.set_update_status("ready".into());
                }
            });
        });
    });

    let weak = app.as_weak();
    app.on_install_update(move || {
        let path = DOWNLOADED_APK.lock().unwrap().clone();
        if let Some(p) = path {
            #[cfg(target_os = "android")]
            {
                let uri = format!("file://{}", p.display());
                let _ = std::process::Command::new("am")
                    .args([
                        "start",
                        "-a",
                        "android.intent.action.VIEW",
                        "-d",
                        &uri,
                        "-t",
                        "application/vnd.android.package-archive",
                    ])
                    .spawn();
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = p;
            }
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_update_status("idle".into());
        }
    });
}