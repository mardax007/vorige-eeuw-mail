//! Android, decision 24: new mail in the background through an own foreground service.
//!
//! The app and the service are two processes on one data folder. The app's process runs while
//! its screen is open; the service runs in its own process (`:mail`, see `MailService.kt`) for
//! as long as "Nieuwe mail direct melden" is on. Only one of them works on the mailboxes at a
//! time (`vem_core::handoff`): the app while it is on screen, the service otherwise. The app lets
//! go when it leaves the screen and takes over again when it comes back; the service notices
//! within a few seconds and pauses.
//!
//! The Kotlin side calls in through `tech.mardax.vem.MailCore` (the `Java_…` functions below)
//! and is called back through `tech.mardax.vem.MailService`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::{Env, EnvUnowned};
use vem_core::engine::{Engine, Sink};
use vem_core::handoff;
use vem_core::model::{MessageSummary, OutboxItem};
use vem_core::net::Trust;
use vem_core::report::AppFacts;
use vem_core::secrets::Secrets;
use vem_core::store::Store;
use vem_core::VemError;

use crate::shell::{self, ShellTexts, SHELL_TEXTS_KEY};

/// A file in the data folder that says the service should run. Kotlin looks at it after a
/// restart of the phone, after an update and on the alarm, before any Rust has started.
const FLAG_FILE: &str = "background-mail";
/// How often the working service looks whether the app came back on screen.
const LOOK_EVERY: Duration = Duration::from_secs(3);
/// How long the app leaving the screen waits for a service that is still starting.
const SERVICE_START_WAIT: Duration = Duration::from_secs(5);

fn background_on(dir: &Path) -> bool {
    dir.join(FLAG_FILE).exists()
}

// ---- The app's side ----

struct Ui {
    engine: Arc<Engine>,
    dir: PathBuf,
    /// Tells the screens to reload: the service may have fetched mail while they were away.
    on_resume: Box<dyn Fn() + Send + Sync>,
    /// The app's turn on the mailboxes. The lock also keeps two hand-overs from overlapping.
    turn: Mutex<Option<handoff::Turn>>,
}

static UI: OnceLock<Ui> = OnceLock::new();
/// The app's process only starts with its screen, so it starts as on screen.
static ON_SCREEN: AtomicBool = AtomicBool::new(true);

/// Called once from the app's setup instead of starting the engine directly.
pub fn attach(engine: Arc<Engine>, dir: PathBuf, on_resume: Box<dyn Fn() + Send + Sync>) {
    let _ = UI.set(Ui { engine, dir, on_resume, turn: Mutex::new(None) });
    settle_soon();
}

/// "Nieuwe mail direct melden" changed: the flag file for Kotlin, then the service itself.
pub fn set_background(dir: &Path, on: bool) {
    let flag = dir.join(FLAG_FILE);
    let written = if on { std::fs::write(&flag, b"1") } else { std::fs::remove_file(&flag).or_else(not_found_is_fine) };
    if let Err(e) = written {
        vem_core::log::event("background.flag_failed", &[("detail", &e.to_string())]);
    }
    let result = with_app_class(jni::jni_str!("tech.mardax.vem.MailService"), |env, context, class| {
        env.call_static_method(
            class,
            jni::jni_str!("apply"),
            jni::jni_sig!("(Landroid/content/Context;Z)V"),
            &[JValue::Object(context), JValue::Bool(on)],
        )
        .map(|_| ())
    });
    if let Err(e) = result {
        vem_core::log::event("background.apply_failed", &[("detail", &e)]);
    }
}

fn not_found_is_fine(e: std::io::Error) -> std::io::Result<()> {
    if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }
}

fn settle_soon() {
    std::thread::spawn(settle);
}

/// Brings the app's turn in line with where the app is: on screen it works on the mailboxes;
/// away from the screen it lets the service work, when the service is on.
fn settle() {
    let Some(ui) = UI.get() else { return };
    let mut turn = ui.turn.lock().unwrap_or_else(|e| e.into_inner());
    let wanted = ON_SCREEN.load(Ordering::SeqCst) || !background_on(&ui.dir);
    match (wanted, turn.is_some()) {
        (true, false) => {
            // Waits a few seconds at most, while the service pauses.
            match handoff::app_take(&ui.dir) {
                Ok(t) => {
                    *turn = Some(t);
                    let engine = ui.engine.clone();
                    if let Err(e) = tauri::async_runtime::block_on(async move { engine.start() }) {
                        vem_core::log::error("handoff.start_failed", &e);
                    }
                    (ui.on_resume)();
                    vem_core::log::event("handoff.app_works", &[]);
                }
                Err(e) => vem_core::log::event("handoff.failed", &[("detail", &e.to_string())]),
            }
        }
        (false, true) => {
            // Let go only when a service is there to take over; a service that Android refused
            // to start would leave nobody fetching mail.
            if !wait_for_service(&ui.dir) {
                vem_core::log::event("handoff.no_service", &[]);
                return;
            }
            let engine = ui.engine.clone();
            tauri::async_runtime::block_on(async move { engine.pause().await });
            *turn = None;
            vem_core::log::event("handoff.app_yields", &[]);
        }
        _ => {}
    }
}

/// True once a service holds its presence lock; waits a little for one that is starting.
fn wait_for_service(dir: &Path) -> bool {
    let started = std::time::Instant::now();
    loop {
        if handoff::service_is_running(dir).unwrap_or(false) {
            return true;
        }
        if started.elapsed() >= SERVICE_START_WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// `MailCore.uiForeground()`, from `MainActivity.onStart`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_tech_mardax_vem_MailCore_uiForeground<'c>(_env: EnvUnowned<'c>, _class: JClass<'c>) {
    ON_SCREEN.store(true, Ordering::SeqCst);
    settle_soon();
}

/// `MailCore.uiBackground()`, from `MainActivity.onStop`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_tech_mardax_vem_MailCore_uiBackground<'c>(_env: EnvUnowned<'c>, _class: JClass<'c>) {
    ON_SCREEN.store(false, Ordering::SeqCst);
    settle_soon();
}

// ---- The service's side ----

struct Service {
    engine: Arc<Engine>,
    dir: PathBuf,
    runtime: tokio::runtime::Runtime,
    /// `service.lock`, held for the life of this process: tells the app a service is there.
    _present: std::fs::File,
}

static SERVICE: OnceLock<Service> = OnceLock::new();
/// Android may deliver several start commands at once, each calling `serviceStart` on its own
/// thread; only one may set up the core and its loop.
static STARTING: Mutex<bool> = Mutex::new(false);

/// `MailCore.serviceStart(context, dataDir)`: starts the headless core once per process and
/// returns the shell texts as JSON, for the service's notifications.
#[unsafe(no_mangle)]
pub extern "system" fn Java_tech_mardax_vem_MailCore_serviceStart<'c>(
    mut env: EnvUnowned<'c>,
    _class: JClass<'c>,
    context: JObject<'c>,
    data_dir: JString<'c>,
) -> JString<'c> {
    env.with_env(|env| -> Result<JString<'c>, jni::errors::Error> {
        let dir = PathBuf::from(data_dir.try_to_string(env)?);
        let mut context_kept = STARTING.lock().unwrap_or_else(|e| e.into_inner());
        if !*context_kept {
            keep_context(env, &context)?;
            *context_kept = true;
        }
        if SERVICE.get().is_none() {
            match start_service(&dir) {
                Ok(s) => {
                    if SERVICE.set(s).is_ok() {
                        std::thread::spawn(|| {
                            if let Some(s) = SERVICE.get() {
                                service_loop(s)
                            }
                        });
                    }
                }
                Err(e) => vem_core::log::event("service.start_failed", &[("detail", &e)]),
            }
        }
        drop(context_kept);
        let texts = SERVICE.get().map(|s| texts(&s.engine.store)).unwrap_or_default();
        JString::from_str(env, serde_json::to_string(&texts).unwrap_or_default())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// `MailCore.serviceWake()`, from the alarm: look at the server now.
#[unsafe(no_mangle)]
pub extern "system" fn Java_tech_mardax_vem_MailCore_serviceWake<'c>(_env: EnvUnowned<'c>, _class: JClass<'c>) {
    if let Some(s) = SERVICE.get() {
        s.engine.wake(None);
    }
}

/// Puts the application context where `vem_core::net` (Android's certificate check) and the
/// calls back into Kotlin find it. Tauri does this in the app's process; here nobody else does.
/// Called once per process (`STARTING`).
fn keep_context(env: &mut Env<'_>, context: &JObject<'_>) -> Result<(), jni::errors::Error> {
    let vm = env.get_java_vm()?;
    let global = env.new_global_ref(context)?;
    // SAFETY: a valid JavaVM pointer and a global reference that is never released (into_raw),
    // so both stay valid for the whole process; this runs once (STARTING).
    unsafe { ndk_context::initialize_android_context(vm.get_raw().cast(), global.into_raw().cast()) };
    Ok(())
}

fn start_service(dir: &Path) -> Result<Service, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let present = handoff::service_present(dir)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "another service holds service.lock".to_string())?;
    vem_core::log::init(dir);
    vem_core::log::event("service.start", &[("version", vem_core::version())]);
    vem_core::net::init_android().map_err(|e| format!("{e:?}"))?;
    let store = Arc::new(Store::open(dir).map_err(|e| format!("{e:?}"))?);
    let secrets = Arc::new(Secrets::new(dir));
    let sink = Arc::new(ServiceSink { store: store.clone() });
    let facts = AppFacts { version: vem_core::version().to_string(), os: shell::windows_version() };
    let engine = Engine::new(store, secrets, sink, Trust::Platform, facts);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("vem-mail")
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Service { engine, dir: dir.to_path_buf(), runtime, _present: present })
}

/// Waits for its turn, works until the app is back on screen, pauses, and waits again. Ends
/// with the process (the service's `onDestroy` ends it).
fn service_loop(s: &Service) {
    loop {
        let turn = match handoff::service_take(&s.dir) {
            Ok(t) => t,
            Err(e) => {
                vem_core::log::event("handoff.failed", &[("detail", &e.to_string())]);
                std::thread::sleep(LOOK_EVERY);
                continue;
            }
        };
        let started = {
            let _in_runtime = s.runtime.enter();
            s.engine.start()
        };
        if let Err(e) = started {
            vem_core::log::error("service.start_failed", &e);
        }
        vem_core::log::event("handoff.service_works", &[]);
        // A lock file that cannot be read counts as "app on screen": better a pause than two
        // processes on the mailboxes.
        while !handoff::app_is_on_screen(&s.dir).unwrap_or(true) {
            std::thread::sleep(LOOK_EVERY);
        }
        s.runtime.block_on(s.engine.pause());
        drop(turn);
        vem_core::log::event("handoff.service_yields", &[]);
    }
}

fn texts(store: &Store) -> ShellTexts {
    store.kv_get(SHELL_TEXTS_KEY).ok().flatten().unwrap_or_default()
}

/// The service has no screens: only new mail matters, as a notification.
struct ServiceSink {
    store: Arc<Store>,
}

impl Sink for ServiceSink {
    fn mail_changed(&self, _: &str, _: Option<String>) {}
    fn sync_state(&self, _: &str, _: bool, _: Option<VemError>) {}
    fn outbox_changed(&self, _: Vec<OutboxItem>) {}
    fn new_mail(&self, _account_id: &str, messages: Vec<MessageSummary>) {
        let wanted = self.store.settings().map(|s| s.notifications).unwrap_or(true);
        if !wanted || messages.is_empty() {
            return;
        }
        let (title, body) = shell::new_mail_text(&texts(&self.store), &messages);
        let result = with_app_class(jni::jni_str!("tech.mardax.vem.MailService"), |env, context, class| {
            let title = env.new_string(&title)?;
            let body = env.new_string(&body)?;
            env.call_static_method(
                class,
                jni::jni_str!("showNewMail"),
                jni::jni_sig!("(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V"),
                &[JValue::Object(context), JValue::Object(&title), JValue::Object(&body)],
            )
            .map(|_| ())
        });
        if let Err(e) = result {
            vem_core::log::event("service.notify_failed", &[("detail", &e)]);
        }
    }
}

// ---- Calling Kotlin ----

/// Runs `f` with the application context and one of the app's own classes, on any thread.
fn with_app_class<F>(name: &'static jni::strings::JNIStr, f: F) -> Result<(), String>
where
    F: for<'l> FnOnce(&mut Env<'l>, &JObject<'l>, &JClass<'l>) -> Result<(), jni::errors::Error>,
{
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return Err("android context missing".into());
    }
    // SAFETY: a valid JavaVM pointer, set at start by Tauri or by keep_context (checked non-null).
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    let mut java_error: Option<String> = None;
    let outcome = vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: a global reference to the application context, valid for the whole process;
        // JObject does not delete it on drop.
        let context = unsafe { JObject::from_raw(env, ctx.context().cast()) };
        // A native thread finds the app's own classes only through the app's class loader.
        let loader = env
            .call_method(&context, jni::jni_str!("getClassLoader"), jni::jni_sig!("()Ljava/lang/ClassLoader;"), &[])?
            .l()?;
        let loader = env.cast_local::<jni::objects::JClassLoader>(loader)?;
        let class = jni::refs::LoaderContext::Loader(&loader).load_class(env, name, true)?;
        let result = f(env, &context, &class);
        if let Some(thrown) = env.exception_occurred() {
            env.exception_clear();
            java_error = crate::updates::java_text(env, &thrown);
        }
        result
    });
    outcome.map_err(|e| match java_error {
        Some(text) => format!("{e}: {text}"),
        None => e.to_string(),
    })
}
