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
    wire_auth_callbacks(&app);
    wire_conversation_callbacks(&app);
    wire_send_callbacks(&app);
    wire_friends_callbacks(&app);
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
        UiChat { id: "demo-0".into(), index: 0, title: "Maya Chen".into(), subtitle: "Perfect. I'll bring the good beans 🙂".into(), time: "10:12".into(), unread: 2, initials: "MC".into(), variant: 1, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { id: "demo-1".into(), index: 1, title: "Trail Crew 🚀".into(), subtitle: "Sam: pushing the fix now".into(), time: "09:58".into(), unread: 0, initials: "TC".into(), variant: 2, has_avatar: false, avatar: slint::Image::default(), online: false, muted: true, pinned: true },
        UiChat { id: "demo-2".into(), index: 2, title: "Sam Okafor".into(), subtitle: "see you at practice".into(), time: "Yesterday".into(), unread: 0, initials: "SO".into(), variant: 3, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { id: "demo-3".into(), index: 3, title: "Nadia Farouk".into(), subtitle: "typing…".into(), time: "Yesterday".into(), unread: 1, initials: "NF".into(), variant: 4, has_avatar: false, avatar: slint::Image::default(), online: true, muted: false, pinned: false },
        UiChat { id: "demo-4".into(), index: 4, title: "Alex Rivera".into(), subtitle: "sent a photo".into(), time: "Mon".into(), unread: 0, initials: "AR".into(), variant: 5, has_avatar: false, avatar: slint::Image::default(), online: false, muted: false, pinned: false },
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
// ---------------------------------------------------------------------------
// Authentication (wired to the Nemo backend)
// ---------------------------------------------------------------------------

fn api_base() -> String {
    if let Ok(v) = std::env::var("NOOK_API_BASE") {
        if !v.is_empty() {
            return v.trim_end_matches('/').to_string();
        }
    }
    #[cfg(target_os = "android")]
    {
        "http://10.0.2.2:8080".to_string()
    }
    #[cfg(not(target_os = "android"))]
    {
        "http://127.0.0.1:8080".to_string()
    }
}

fn token_path() -> std::path::PathBuf {
    std::env::temp_dir().join("nemo_session_token.txt")
}

fn save_token(token: &str) {
    let _ = std::fs::write(token_path(), token);
}

fn load_token() -> Option<String> {
    std::fs::read_to_string(token_path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn register_request(username: &str, display_name: &str, password: &str) -> Result<(String, String), String> {
    use nook_shared::models::RegisterRequest;
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = RegisterRequest {
        username: username.to_string(),
        display_name: display_name.to_string(),
        password: password.to_string(),
    };
    let resp = client
        .post(format!("{}/api/auth/register", api_base()))
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(if text.is_empty() { "Registration failed.".to_string() } else { text });
    }
    let parsed: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    store_user_id(&parsed);
    let token = parsed
        .get("session")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let recovery = parsed
        .get("recovery_code")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return Err("Server did not return a session token.".to_string());
    }
    Ok((token, recovery))
}

fn login_request(username: &str, password: &str) -> Result<(String, bool), String> {
    use nook_shared::models::LoginRequest;
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = LoginRequest {
        username: username.to_string(),
        password: password.to_string(),
    };
    let resp = client
        .post(format!("{}/api/auth/login", api_base()))
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(if text.is_empty() { "Login failed.".to_string() } else { text });
    }
    let parsed: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    store_user_id(&parsed);
    let token = parsed
        .get("session")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let onboarding = parsed
        .get("onboarding_required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if token.is_empty() {
        return Err("Server did not return a session token.".to_string());
    }
    Ok((token, onboarding))
}

fn accept_terms_request(token: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    // Accept the current policy version advertised by the server.
    let body = serde_json::json!({ "version": nook_shared::policies::POLICY_VERSION });
    let resp = client
        .post(format!("{}/api/me/terms", api_base()))
        .bearer_auth(token)
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(if text.is_empty() { "Could not accept terms.".to_string() } else { text });
    }
    Ok(())
}

fn wire_auth_callbacks(app: &AppWindow) {
    use slint::ComponentHandle;
    let weak = app.as_weak();

    // Sign up
    let w = weak.clone();
    app.on_submit_signup(move |u, d, p| {
        let username = u.to_string();
        let display_name = d.to_string();
        let password = p.to_string();
        if let Some(ui) = w.upgrade() {
            ui.set_signup_busy(true);
            ui.set_signup_error("".into());
        }
        let w2 = w.clone();
        std::thread::spawn(move || {
            let result = register_request(&username, &display_name, &password);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w2.upgrade() {
                    match result {
                        Ok((token, recovery)) => {
                            save_token(&token);
                            let tok = token.clone();
                            ui.set_auth_token(token.into());
                            if let Ok(chats) = fetch_conversations(&tok) {
                                ui.set_chats(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(chats))));
                            }
                            let mut codes: Vec<slint::SharedString> = Vec::new();
                            if !recovery.is_empty() {
                                codes.push(recovery.into());
                            }
                            use slint::{ModelRc, VecModel};
                            use std::rc::Rc;
                            ui.set_recovery_codes(ModelRc::from(Rc::new(VecModel::from(codes))));
                            ui.set_signup_busy(false);
                            ui.set_screen("recovery".into());
                        }
                        Err(e) => {
                            ui.set_signup_error(e.into());
                            ui.set_signup_busy(false);
                        }
                    }
                }
            });
        });
    });

    // Login
    let w = weak.clone();
    app.on_submit_login(move |u, p| {
        let username = u.to_string();
        let password = p.to_string();
        if let Some(ui) = w.upgrade() {
            ui.set_login_busy(true);
            ui.set_login_error("".into());
        }
        let w2 = w.clone();
        std::thread::spawn(move || {
            let result = login_request(&username, &password);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w2.upgrade() {
                    match result {
                        Ok((token, onboarding)) => {
                            save_token(&token);
                            let tok = token.clone();
                            ui.set_auth_token(token.into());
                            if let Ok(chats) = fetch_conversations(&tok) {
                                ui.set_chats(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(chats))));
                            }
                            ui.set_login_busy(false);
                            if onboarding {
                                ui.set_screen("terms".into());
                            } else {
                                ui.set_screen("chats".into());
                            }
                        }
                        Err(e) => {
                            ui.set_login_error(e.into());
                            ui.set_login_busy(false);
                        }
                    }
                }
            });
        });
    });

    // Accept terms
    let w = weak.clone();
    app.on_accept_terms(move || {
        let token = load_token().unwrap_or_default();
        if token.is_empty() {
            if let Some(ui) = w.upgrade() {
                ui.set_screen("chats".into());
            }
            return;
        }
        let w2 = w.clone();
        std::thread::spawn(move || {
            let result = accept_terms_request(&token);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w2.upgrade() {
                    let _ = result;
                    ui.set_screen("chats".into());
                }
            });
        });
    });
}

// ---------------------------------------------------------------------------
// Conversations & messages (REST)
// ---------------------------------------------------------------------------

static CONVERSATION_IDS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static CURRENT_USER_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static CURRENT_CONVERSATION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn store_user_id(parsed: &serde_json::Value) {
    if let Some(id) = parsed.get("session").and_then(|s| s.get("user")).and_then(|u| u.get("id")).and_then(|v| v.as_str()) {
        *CURRENT_USER_ID.lock().unwrap() = Some(id.to_string());
    }
}

fn initials_for(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn fetch_conversations(token: &str) -> Result<Vec<UiChat>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("{}/api/conversations", api_base()))
        .bearer_auth(token)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let parsed: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let arr = parsed.as_array().cloned().unwrap_or_default();
    let mut ids = Vec::new();
    let mut chats = Vec::new();
    for (i, c) in arr.iter().enumerate() {
        let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        ids.push(id.clone());
        let title = c.get("title").and_then(|v| v.as_str()).unwrap_or("Chat").to_string();
        let last = c.get("last_message");
        let subtitle = last
            .and_then(|m| m.get("body"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let time = last
            .and_then(|m| m.get("created_at"))
            .and_then(|v| v.as_str())
            .map(|s| s.get(11..16).unwrap_or(s).to_string())
            .unwrap_or_default();
        let unread = c.get("unread_count").and_then(|v| v.as_u64()).unwrap_or(0) as i32;
        let pinned = c.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
        let muted = c.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
        let online = c.get("other_online").and_then(|v| v.as_bool()).unwrap_or(false);
        chats.push(UiChat {
            id: id.into(),
            index: i as i32,
            title: title.clone().into(),
            subtitle: subtitle.into(),
            time: time.into(),
            unread,
            initials: initials_for(&title).into(),
            variant: (i % 6) as i32,
            has_avatar: false,
            avatar: slint::Image::default(),
            online,
            muted,
            pinned,
        });
    }
    *CONVERSATION_IDS.lock().unwrap() = ids;
    Ok(chats)
}

fn fetch_messages(token: &str, conversation_id: &str) -> Result<Vec<UiBubble>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("{}/api/conversations/{}/messages", api_base(), conversation_id))
        .bearer_auth(token)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let parsed: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    // Response may be { messages: [...] } or bare array.
    let arr = parsed
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .or_else(|| parsed.as_array().cloned())
        .unwrap_or_default();
    let me = CURRENT_USER_ID.lock().unwrap().clone().unwrap_or_default();
    let mut bubbles = Vec::new();
    for (i, m) in arr.iter().enumerate() {
        let text = m.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let sender_name = m
            .get("sender")
            .and_then(|s| s.get("display_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sender_id = m
            .get("sender")
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mine = !me.is_empty() && sender_id == me;
        let time = m
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.get(11..16).unwrap_or(s).to_string())
            .unwrap_or_default();
        let kind = match m.get("kind").and_then(|v| v.as_str()).unwrap_or("text") {
            "image" => 1,
            "system" => 2,
            _ => 0,
        };
        let has_image = m.get("attachment").is_some();
        bubbles.push(UiBubble {
            index: i as i32,
            kind,
            text: text.into(),
            mine,
            read: true,
            time: time.into(),
            day: "".into(),
            is_day_sep: false,
            sender: sender_name.into(),
            has_image,
            image: slint::Image::default(),
        });
    }
    Ok(bubbles)
}

fn send_message_request(token: &str, conversation_id: &str, body: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Nemo-Mobile/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "kind": "text", "body": body });
    let resp = client
        .post(format!("{}/api/conversations/{}/messages", api_base(), conversation_id))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(())
}

fn wire_conversation_callbacks(app: &AppWindow) {
    use slint::ComponentHandle;
    let weak = app.as_weak();

    // Load conversations into chats list (call after auth).
    let w = weak.clone();
    app.on_open_chat(move |idx| {
        let token = load_token().unwrap_or_default();
        let conv_id = CONVERSATION_IDS
            .lock()
            .unwrap()
            .get(idx as usize)
            .cloned()
            .unwrap_or_default();
        if conv_id.is_empty() {
            if let Some(ui) = w.upgrade() {
                ui.set_screen("chat".into());
            }
            return;
        }
        // Set current conversation id in a global-ish way via token file? Store in static.
        *CURRENT_CONVERSATION.lock().unwrap() = Some(conv_id.clone());
        if let Some(ui) = w.upgrade() {
            if let Ok(msgs) = fetch_messages(&token, &conv_id) {
                ui.set_messages(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(msgs))));
            }
            ui.set_screen("chat".into());
        }
    });

    // Send message
    let w = weak.clone();
    // ChatScreen send callback not wired at root level yet; expose via a new root callback.
}

fn wire_send_callbacks(app: &AppWindow) {
    let weak = app.as_weak();
    app.on_send_message(move |text| {
        let token = load_token().unwrap_or_default();
        let conv = CURRENT_CONVERSATION.lock().unwrap().clone().unwrap_or_default();
        let body = text.to_string();
        if conv.is_empty() || body.trim().is_empty() {
            return;
        }
        let _ = send_message_request(&token, &conv, &body);
        if let Ok(msgs) = fetch_messages(&token, &conv) {
            if let Some(ui) = weak.upgrade() {
                ui.set_messages(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(msgs))));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Friends & Find
// ---------------------------------------------------------------------------

static SUGGESTED_USER_IDS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static REQUEST_IDS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn to_ui_person(u: &serde_json::Value, state: i32) -> UiPerson {
    let id_str = u.get("id").and_then(|v| v.as_str()).unwrap_or("");
    // Map UUID to a stable-ish int via simple hash for UI identity.
    let id = id_str.bytes().fold(0i32, |a, b| a.wrapping_mul(31).wrapping_add(b as i32)).abs();
    let name = u.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let handle = u.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let initials = initials_for(&name);
    UiPerson {
        id,
        name: name.into(),
        handle: handle.into(),
        initials: initials.into(),
        variant: 0,
        online: false,
        subtitle: "".into(),
        state,
        has_avatar: false,
        avatar: slint::Image::default(),
    }
}

fn fetch_search(token: &str, query: &str) -> Vec<UiPerson> {
    let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let url = format!("{}/api/users/search?q={}", api_base(), query.replace(' ', "%20"));
    let resp = match client.get(url).bearer_auth(token).send() { Ok(r) => r, Err(_) => return Vec::new() };
    if !resp.status().is_success() { return Vec::new(); }
    let parsed: serde_json::Value = match resp.json() { Ok(v) => v, Err(_) => return Vec::new() };
    let arr = parsed.get("users").and_then(|v| v.as_array()).cloned()
        .or_else(|| parsed.as_array().cloned()).unwrap_or_default();
    let mut ids = Vec::new();
    let mut people = Vec::new();
    for u in &arr {
        if let Some(id) = u.get("id").and_then(|v| v.as_str()) { ids.push(id.to_string()); }
        people.push(to_ui_person(u, 0));
    }
    *SUGGESTED_USER_IDS.lock().unwrap() = ids;
    people
}

fn fetch_friends(token: &str) -> Vec<UiPerson> {
    let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() {
        Ok(c) => c, Err(_) => return Vec::new(),
    };
    let resp = match client.get(format!("{}/api/friends", api_base())).bearer_auth(token).send() { Ok(r) => r, Err(_) => return Vec::new() };
    if !resp.status().is_success() { return Vec::new(); }
    let parsed: serde_json::Value = match resp.json() { Ok(v) => v, Err(_) => return Vec::new() };
    let arr = parsed.get("friends").and_then(|v| v.as_array()).cloned()
        .or_else(|| parsed.as_array().cloned()).unwrap_or_default();
    arr.iter().map(|u| to_ui_person(u, 2)).collect()
}

fn fetch_friend_requests(token: &str) -> Vec<UiPerson> {
    let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() {
        Ok(c) => c, Err(_) => return Vec::new(),
    };
    let resp = match client.get(format!("{}/api/friend-requests", api_base())).bearer_auth(token).send() { Ok(r) => r, Err(_) => return Vec::new() };
    if !resp.status().is_success() { return Vec::new(); }
    let parsed: serde_json::Value = match resp.json() { Ok(v) => v, Err(_) => return Vec::new() };
    let arr = parsed.get("requests").and_then(|v| v.as_array()).cloned()
        .or_else(|| parsed.as_array().cloned()).unwrap_or_default();
    let mut ids = Vec::new();
    let mut people = Vec::new();
    for r in &arr {
        if let Some(id) = r.get("id").and_then(|v| v.as_str()) { ids.push(id.to_string()); }
        // The other user info may be under "from" or "user"
        let user = r.get("from").or_else(|| r.get("user")).unwrap_or(r);
        people.push(to_ui_person(user, 0));
    }
    *REQUEST_IDS.lock().unwrap() = ids;
    people
}

fn wire_friends_callbacks(app: &AppWindow) {
    use slint::ComponentHandle;
    let weak = app.as_weak();

    // search
    let w = weak.clone();
    app.on_search_friends(move |q| {
        let token = load_token().unwrap_or_default();
        let query = q.to_string();
        let suggested = if query.trim().is_empty() { Vec::new() } else { fetch_search(&token, &query) };
        let friends = fetch_friends(&token);
        let requests = fetch_friend_requests(&token);
        let pending = requests.len() as i32;
        if let Some(ui) = w.upgrade() {
            ui.set_find_results(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(suggested))));
            ui.set_requests(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(requests))));
            ui.set_pending_count(pending);
        }
    });

    // add friend
    let w = weak.clone();
    app.on_add_friend(move |idx| {
        let token = load_token().unwrap_or_default();
        let user_id = SUGGESTED_USER_IDS.lock().unwrap().get(idx as usize).cloned().unwrap_or_default();
        if user_id.is_empty() { return; }
        let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() { Ok(c) => c, Err(_) => return };
        let _ = client.post(format!("{}/api/friend-requests", api_base())).bearer_auth(&token).json(&serde_json::json!({"user_id": user_id})).send();
        // refresh
        let suggested = fetch_search(&token, "");
        let friends = fetch_friends(&token);
        let requests = fetch_friend_requests(&token);
        if let Some(ui) = w.upgrade() {
            ui.set_find_results(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(suggested))));
            let pc = requests.len() as i32;
            ui.set_requests(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(requests))));
            ui.set_pending_count(pc);
        }
    });

    // accept request
    let w = weak.clone();
    app.on_accept_friend_request(move |idx| {
        let token = load_token().unwrap_or_default();
        let rid = REQUEST_IDS.lock().unwrap().get(idx as usize).cloned().unwrap_or_default();
        if rid.is_empty() { return; }
        let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() { Ok(c) => c, Err(_) => return };
        let _ = client.post(format!("{}/api/friend-requests/{}/accept", api_base(), rid)).bearer_auth(&token).send();
        let requests = fetch_friend_requests(&token);
        let friends = fetch_friends(&token);
        if let Some(ui) = w.upgrade() {
            let pc = requests.len() as i32;
            ui.set_requests(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(requests))));
            ui.set_pending_count(pc);
        }
    });

    // decline request
    let w = weak.clone();
    app.on_decline_friend_request(move |idx| {
        let token = load_token().unwrap_or_default();
        let rid = REQUEST_IDS.lock().unwrap().get(idx as usize).cloned().unwrap_or_default();
        if rid.is_empty() { return; }
        let client = match reqwest::blocking::Client::builder().user_agent("Nemo-Mobile/1.0").timeout(std::time::Duration::from_secs(20)).build() { Ok(c) => c, Err(_) => return };
        let _ = client.post(format!("{}/api/friend-requests/{}/decline", api_base(), rid)).bearer_auth(&token).send();
        let requests = fetch_friend_requests(&token);
        if let Some(ui) = w.upgrade() {
            let pc = requests.len() as i32;
            ui.set_requests(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(requests))));
            ui.set_pending_count(pc);
        }
    });
}
