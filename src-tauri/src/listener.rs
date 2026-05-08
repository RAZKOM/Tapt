//! Single-threaded dispatcher that hosts the one `rdev::listen` tap
//! allowed per process and fans events out to the keyboard hook.

use std::sync::Mutex;
use std::sync::OnceLock;

use rdev::Event;

type Handler = Box<dyn Fn(&Event) + Send + Sync + 'static>;

static HANDLERS: OnceLock<Mutex<Vec<Handler>>> = OnceLock::new();
static THREAD_STARTED: OnceLock<()> = OnceLock::new();

fn handlers() -> &'static Mutex<Vec<Handler>> {
    HANDLERS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a callback for the global rdev stream. Uses a single-process
/// hook; `register_handler` from `start_keylistener` wires the dispatcher.
pub fn register_handler(h: Handler) {
    if let Ok(mut guard) = handlers().lock() {
        guard.push(h);
    }
}

/// Spawn the rdev::listen thread once. Subsequent calls are no-ops.
pub fn ensure_listener_thread() {
    if THREAD_STARTED.set(()).is_err() {
        return;
    }

    std::thread::Builder::new()
        .name("rdev-dispatch".into())
        .spawn(move || {
            let cb = move |event: Event| {
                if let Ok(guard) = handlers().lock() {
                    for h in guard.iter() {
                        h(&event);
                    }
                }
            };
            if let Err(err) = rdev::listen(cb) {
                eprintln!("[listener] rdev failed: {err:?}");
            }
        })
        .expect("could not spawn rdev dispatcher thread");
}
