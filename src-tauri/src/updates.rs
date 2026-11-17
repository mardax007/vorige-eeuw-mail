//! Silent updates from GitHub Releases and Martijn's log requests (docs/PLAN.md sections 5, 6).
//!
//! An update downloads in the background and installs only when the app is idle: nothing
//! being written, sent or imported. The release URL, the update key and the log-request URL
//! come from build-time settings; a local build without them never updates.

#[cfg(desktop)]
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
#[cfg(desktop)]
use tauri_plugin_updater::UpdaterExt;
use vem_core::model::now;
use vem_core::store::Store;

use crate::AppState;

#[cfg(desktop)]
const FIRST_CHECK: Duration = Duration::from_secs(120);
/// A phone is opened for a minute at a time; look soon after start.
#[cfg(mobile)]
const FIRST_CHECK: Duration = Duration::from_secs(15);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
#[cfg(desktop)]
const IDLE_POLL: Duration = Duration::from_secs(60);
/// Someone who saved a draft this recently may still be writing.
#[cfg(desktop)]
const WRITING_SECS: i64 = 10 * 60;
/// "Bijgewerkt naar …" shows for one day.
const SHOW_UPDATE_SECS: i64 = 24 * 60 * 60;
const UPDATED_KEY: &str = "updated";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Updated {
    pub version: String,
    pub notes: Vec<String>,
    pub at: i64,
}

pub fn recent_update(store: &Store) -> Option<Updated> {
    let u: Updated = store.kv_get(UPDATED_KEY).ok().flatten()?;
    // Shown only once the new version runs, for a day.
    (u.version == vem_core::version() && now() - u.at < SHOW_UPDATE_SECS).then_some(u)
}

#[cfg(desktop)]
fn idle(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let writing = now() - state.last_draft_save.load(Ordering::Relaxed) < WRITING_SECS;
    let importing = !state.imports.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
    !writing && !importing && state.engine.idle_for_update()
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check_log_requests(&app).await;
            #[cfg(desktop)]
            if let Err(detail) = check_update(&app).await {
                // A failed update goes to Martijn like any other error (docs/errors.md).
                let e = vem_core::VemError::new(vem_core::error::Code::UpdateCmd01).with_detail(detail);
                vem_core::log::error("update.failed", &e);
                app.state::<AppState>().engine.report(None, &e).await;
            }
            // Android never installs an update silently from outside a store: it only offers
            // "Bijwerken" (phase 5 task 8). A failed look is logged; the next one comes later.
            #[cfg(mobile)]
            if let Err(e) = check_android_update(&app).await {
                vem_core::log::error("update.check_failed", &e);
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// The release text as separate points ("- Zoeken gaat sneller." becomes "Zoeken gaat sneller.").
fn note_lines(body: &str) -> Vec<String> {
    body.lines()
        .map(|l| l.trim().trim_start_matches(['-', '*']).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// What the screens show with "Bijwerken" on a phone.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOffer {
    pub version: String,
    pub notes: Vec<String>,
}

fn offer_of(r: &vem_core::apk_update::AndroidRelease) -> UpdateOffer {
    UpdateOffer { version: r.version.clone(), notes: note_lines(&r.notes) }
}

#[cfg(mobile)]
async fn check_android_update(app: &AppHandle) -> vem_core::Result<()> {
    use tauri::Emitter;
    let (Some(windows_url), Some(_key)) = (option_env!("VEM_UPDATE_URL"), option_env!("VEM_UPDATER_PUBKEY")) else {
        return Ok(());
    };
    let Some(url) = vem_core::apk_update::release_url(windows_url) else { return Ok(()) };
    let Some(release) = vem_core::apk_update::check(&url).await? else { return Ok(()) };
    vem_core::log::event("update.found", &[("version", &release.version)]);
    let offer = offer_of(&release);
    *app.state::<AppState>().android_update.lock().unwrap_or_else(|e| e.into_inner()) = Some(release);
    let _ = app.emit("update:available", offer);
    Ok(())
}

/// A newer version waiting for "Bijwerken" (Android); always none on Windows, which updates itself.
#[tauri::command]
pub fn update_offer(state: tauri::State<'_, AppState>) -> Option<UpdateOffer> {
    state.android_update.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(offer_of)
}

/// "Bijwerken": download the APK from GitHub, check it against the update key and open Android's
/// own install screen. Android then asks the person, and keeps the mailboxes (same signing key).
#[tauri::command]
pub async fn install_update(app: AppHandle) -> vem_core::Result<()> {
    #[cfg(target_os = "android")]
    {
        use vem_core::error::Code;
        let get = |detail: String| vem_core::VemError::new(Code::UpdateGet01).with_detail(detail);
        let state = app.state::<AppState>();
        let release = state.android_update.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let release = release.ok_or_else(|| get("no update".into()))?;
        let key = option_env!("VEM_UPDATER_PUBKEY").ok_or_else(|| get("no update key".into()))?;
        let bytes = vem_core::apk_update::download(&release, key).await?;
        let dir = app.path().app_cache_dir().map_err(|e| get(e.to_string()))?;
        std::fs::create_dir_all(&dir).map_err(|e| get(e.kind().to_string()))?;
        let path = dir.join("vorige-eeuw-mail-update.apk");
        std::fs::write(&path, bytes).map_err(|e| get(e.kind().to_string()))?;
        // "Bijgewerkt naar …" shows once the new version runs (recent_update checks the version).
        let notes = note_lines(&release.notes);
        let _ = state.engine.store.kv_set(UPDATED_KEY, &Updated { version: release.version.clone(), notes, at: now() });
        vem_core::log::event("update.install", &[("version", &release.version)]);
        open_installer(&path).map_err(get)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err(vem_core::VemError::new(vem_core::error::Code::AppCmd01).with_detail("android only"))
    }
}

/// Calls `tech.mardax.vem.ApkInstaller.install(context, path)`, which opens Android's install
/// screen for the APK through the app's FileProvider.
#[cfg(target_os = "android")]
fn open_installer(path: &std::path::Path) -> Result<(), String> {
    use jni::objects::{JObject, JValue};
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return Err("android context missing".into());
    }
    // SAFETY: Tauri put a valid JavaVM pointer in ndk-context at start (checked non-null).
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    let path = path.to_string_lossy().into_owned();
    // The Java exception's own text, for the error details: "class not found" alone says little.
    let mut java_error: Option<String> = None;
    let outcome = vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: a global reference to the app context, valid for the whole process; JObject
        // does not delete it on drop.
        let context = unsafe { JObject::from_raw(env, ctx.context().cast()) };
        // A native thread cannot find the app's own classes by name, and this context is the
        // Application, whose class comes from the framework's loader. Context.getClassLoader()
        // is the app's own loader, the one that holds ApkInstaller.
        let loader = env
            .call_method(&context, jni::jni_str!("getClassLoader"), jni::jni_sig!("()Ljava/lang/ClassLoader;"), &[])?
            .l()?;
        let loader = env.cast_local::<jni::objects::JClassLoader>(loader)?;
        let class = match jni::refs::LoaderContext::Loader(&loader).load_class(
            env,
            jni::jni_str!("tech.mardax.vem.ApkInstaller"),
            true,
        ) {
            Ok(class) => class,
            Err(jni::errors::Error::NoClassDefFound { requested, cause }) => {
                if let Some(cause) = &cause {
                    java_error = java_text(env, cause);
                }
                return Err(jni::errors::Error::NoClassDefFound { requested, cause });
            }
            Err(e) => return Err(e),
        };
        let jpath = env.new_string(&path)?;
        let result = env.call_static_method(
            &class,
            jni::jni_str!("install"),
            jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;)V"),
            &[JValue::Object(&context), JValue::Object(&jpath)],
        );
        if let Some(thrown) = env.exception_occurred() {
            env.exception_clear();
            java_error = java_text(env, &thrown);
        }
        result.map(|_| ())
    });
    outcome.map_err(|e| match java_error {
        Some(text) => format!("installer: {e}: {text}"),
        None => format!("installer: {e}"),
    })
}

/// `throwable.toString()`, or None when even that fails.
#[cfg(target_os = "android")]
fn java_text(env: &mut jni::Env<'_>, throwable: &jni::objects::JObject<'_>) -> Option<String> {
    let text = env
        .call_method(throwable, jni::jni_str!("toString"), jni::jni_sig!("()Ljava/lang/String;"), &[])
        .ok()?
        .l()
        .ok()?;
    let text = env.cast_local::<jni::objects::JString>(text).ok()?;
    let out = text.try_to_string(env).ok();
    if env.exception_check() {
        env.exception_clear();
    }
    out
}

async fn check_log_requests(app: &AppHandle) {
    let Some(url) = option_env!("VEM_LOG_REQUESTS_URL") else { return };
    let Ok(client) = vem_core::lookup::http() else { return };
    let Ok(resp) = client.get(url).send().await else { return };
    let Ok(list) = resp.json::<vem_core::report::LogRequests>().await else { return };
    let state = app.state::<AppState>();
    if let Err(e) = state.engine.answer_log_request(&list) {
        vem_core::log::error("logrequest.failed", &e);
    }
}

#[cfg(desktop)]
async fn check_update(app: &AppHandle) -> Result<(), String> {
    let (Some(url), Some(key)) = (option_env!("VEM_UPDATE_URL"), option_env!("VEM_UPDATER_PUBKEY")) else {
        return Ok(());
    };
    let endpoint = url.parse().map_err(|_| "update url".to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(()) };
    vem_core::log::event("update.found", &[("version", &update.version)]);
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    while !idle(app) {
        tokio::time::sleep(IDLE_POLL).await;
    }
    let notes = note_lines(&update.body.clone().unwrap_or_default());
    let state = app.state::<AppState>();
    let _ = state.engine.store.kv_set(UPDATED_KEY, &Updated { version: update.version.clone(), notes, at: now() });
    vem_core::log::event("update.install", &[("version", &update.version)]);
    // On Windows the installer closes the app, installs quietly and starts it again.
    update.install(bytes).map_err(|e| e.to_string())?;
    Ok(())
}
