//! The wasm-bindgen exports (see the crate's notes for the JS API).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use method_schedule_session::clock::{self, Clock};
use method_schedule_session::host::{self, Error};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Every open schedule, and the commands on them, as JS sees it.
#[wasm_bindgen(js_name = Host)]
pub struct WebHost {
    host: host::Host,
}

impl Default for WebHost {
    fn default() -> Self {
        WebHost::new()
    }
}

#[wasm_bindgen(js_class = Host)]
impl WebHost {
    /// No schedule open yet. The first one installs the browser's clock.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WebHost {
        install_clock();
        WebHost {
            host: host::Host::new(),
        }
    }

    /// Runs `command` with `args_json` (the arguments object as the page's `invoke` sends
    /// it, as JSON; empty means none) and gives back its reply as JSON. Throws an `Error`
    /// with the page's message.
    pub fn dispatch(&mut self, command: &str, args_json: &str) -> Result<String, JsError> {
        let args = if args_json.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(args_json).map_err(|e| {
                JsError::new(
                    &Error::BadArguments {
                        command: command.into(),
                        message: e.to_string(),
                    }
                    .message(),
                )
            })?
        };
        let reply = self.host.dispatch(command, &args).map_err(error)?;
        Ok(reply.to_string())
    }

    /// Opens `.mtsc` text as a new schedule in front, shown as `name`: `{doc, view}`, as
    /// JSON.
    #[wasm_bindgen(js_name = openMtsc)]
    pub fn open_mtsc(&mut self, text: &str, name: Option<String>) -> Result<String, JsError> {
        let opened = self.host.open_mtsc(text, name.as_deref()).map_err(error)?;
        serde_json::to_string(&opened).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Schedule `doc` (missing: the one in front) as `.mtsc` text: `{name, text}`.
    #[wasm_bindgen(js_name = saveMtsc)]
    pub fn save_mtsc(&mut self, doc: Option<u32>) -> Result<Object, JsError> {
        let saved = self.host.save_mtsc(doc).map_err(error)?;
        let out = Object::new();
        set(
            &out,
            "name",
            &JsValue::from_str(&crate::mtsc_name(&saved.name)),
        )?;
        set(&out, "text", &JsValue::from_str(&saved.text))?;
        Ok(out)
    }

    /// Shows every schedule read-only (a viewer), or lets them be changed.
    #[wasm_bindgen(js_name = setReadOnly)]
    pub fn set_read_only(&mut self, on: bool) {
        self.host.set_read_only(on);
    }

    /// `{kind: "web", version, commands_version, read_only}`, as JSON.
    #[wasm_bindgen(js_name = hostInfo)]
    pub fn host_info(&self) -> String {
        serde_json::to_string(&self.host.host_info()).unwrap_or_default()
    }

    /// Keeps a picture's bytes under the name a schedule stores for it (`images/...`).
    #[wasm_bindgen(js_name = putImage)]
    pub fn put_image(&mut self, name: &str, bytes: &Uint8Array) {
        self.host.put_image(name, bytes.to_vec());
    }

    /// The bytes kept under a picture's name, or `undefined`.
    pub fn image(&self, name: &str) -> Option<Uint8Array> {
        self.host.image(name).map(Uint8Array::from)
    }

    /// Every picture's name in the store.
    #[wasm_bindgen(js_name = imageNames)]
    pub fn image_names(&self) -> Array {
        self.host
            .image_names()
            .iter()
            .map(|n| JsValue::from_str(n))
            .collect()
    }
}

/// Every command `dispatch` runs.
#[wasm_bindgen]
pub fn commands() -> Array {
    host::commands()
        .iter()
        .map(|c| JsValue::from_str(c))
        .collect()
}

fn error(e: Error) -> JsError {
    JsError::new(&e.message())
}

fn set(o: &Object, key: &str, v: &JsValue) -> Result<(), JsError> {
    Reflect::set(o, &JsValue::from_str(key), v)
        .map(|_| ())
        .map_err(|_| JsError::new("Couldn't build the reply."))
}

// ---------------------------------------------------------------------------
// The clock

/// The browser's clock: `Date.now()` for the wall and, held so it never goes back, for the
/// steady clock; unique names from a random seed and a counter.
struct BrowserClock {
    seed: u64,
    last: AtomicU64,
    count: AtomicU64,
}

impl Clock for BrowserClock {
    fn wall_ms(&self) -> u64 {
        now_ms()
    }

    fn mono_ms(&self) -> u64 {
        let now = now_ms();
        self.last.fetch_max(now, Ordering::Relaxed).max(now)
    }

    fn unique(&self) -> String {
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        format!("{:x}-{:x}-{n:x}", self.seed, now_ms())
    }
}

fn now_ms() -> u64 {
    let ms = js_sys::Date::now();
    if ms.is_finite() && ms > 0.0 {
        ms as u64
    } else {
        0
    }
}

/// Installs [`BrowserClock`] once, before the first schedule.
fn install_clock() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        clock::install(Box::new(BrowserClock {
            seed: random_seed(),
            last: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }));
    });
}

/// Eight bytes from `crypto.getRandomValues`, or the time when the page has no `crypto`
/// (never in a current browser).
fn random_seed() -> u64 {
    let from_crypto = || -> Option<u64> {
        let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto")).ok()?;
        let get: Function = Reflect::get(&crypto, &JsValue::from_str("getRandomValues"))
            .ok()?
            .dyn_into()
            .ok()?;
        let bytes = Uint8Array::new_with_length(8);
        get.call1(&crypto, &bytes).ok()?;
        let mut b = [0u8; 8];
        bytes.copy_to(&mut b);
        Some(u64::from_le_bytes(b))
    };
    from_crypto().unwrap_or_else(now_ms)
}
