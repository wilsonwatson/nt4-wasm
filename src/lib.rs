use std::{cell::RefCell, collections::HashMap, fmt::Debug, future::Future, rc::Rc};

use serde::{ser::Error, Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

thread_local! {
    static STATE: Rc<RefCell<State>> = Rc::new(RefCell::new(State::new()));
}

type Uidty = i64;

#[derive(Debug, Deserialize, Serialize)]
struct Properties {
    #[serde(default)]
    persistent: bool,
    #[serde(default)]
    retained: bool,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct SubscribeOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    periodic: Option<f64>,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    topicsonly: bool,
    #[serde(default)]
    prefix: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", tag = "method", content = "params")]
enum TextFrame {
    Publish {
        name: String,
        pubuid: Uidty,
        #[serde(rename = "type")]
        ty: String,
        properties: Properties,
    },
    Unpublish {
        pubuid: Uidty,
    },
    SetProperties {
        name: String,
        update: Properties,
    },
    Subscribe {
        topics: Vec<String>,
        subuid: Uidty,
        options: SubscribeOptions,
    },
    Unsubscribe {
        subuid: Uidty,
    },
    Announce {
        name: String,
        id: i64,
        #[serde(rename = "type")]
        ty: String,
        #[serde(default)]
        pubuid: Uidty,
        properties: Properties,
    },
    Unannounce {
        name: String,
        id: i64,
    },
    Properties {
        name: String,
        ack: bool,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct BinaryFrame<T = rmpv::Value>(i64, u32, u32, T);

#[derive(Debug)]
struct State {
    name: String,
    ws: Option<WebSocket>,
    offs: u32,
    ready: bool,
    announce_callbacks: Vec<js_sys::Function>,
    unannounce_callbacks: Vec<js_sys::Function>,
    properties_callbacks: Vec<js_sys::Function>,
    sub_callbacks: HashMap<i64, js_sys::Function>,
    topic_names: HashMap<String, i64>,
}

// As described here
fn timesync() {
    log::trace!("timesync");
    let perf = web_sys::window().unwrap().performance().unwrap();
    let now = perf.now() * 1000.0;
    send_binary(BinaryFrame(-1, 0, 1, now)).expect("timesync error!");
}

fn send_binary<T: Serialize + Debug>(v: BinaryFrame<T>) -> Result<(), rmp_serde::encode::Error> {
    STATE.with(|st| st.borrow().send_binary(v))
}

fn send_json(v: TextFrame) -> Result<(), serde_json::error::Error> {
    STATE.with(|st| st.borrow().send_json(v))
}

impl State {
    fn new() -> Self {
        Self {
            name: String::new(),
            ws: None,
            offs: 0,
            ready: false,
            announce_callbacks: Vec::new(),
            unannounce_callbacks: Vec::new(),
            properties_callbacks: Vec::new(),
            sub_callbacks: HashMap::new(),
            topic_names: HashMap::new(),
        }
    }

    fn now(&self) -> u32 {
        let perf = web_sys::window().unwrap().performance().unwrap();
        let now = perf.now() * 1000.0;
        now.round() as u32 + self.offs
    }

    pub fn send_json(&self, v: TextFrame) -> Result<(), serde_json::error::Error> {
        let json = serde_json::to_string(&vec![v])?; // wpilib expects an array at the top level https://github.com/wpilibsuite/allwpilib/blob/d3c9316a972e7652cd946c7ad3de7ebe68cd57d8/ntcore/src/main/native/cpp/net/WireDecoder.cpp#LL121C18-L121C21
        log::trace!("send json {}", json);
        self.ws
            .as_ref()
            .unwrap()
            .send_with_str(&json)
            .map_err(|x| serde_json::error::Error::custom(format!("{:?}", x)))
    }

    pub fn send_binary<T: Serialize + Debug>(
        &self,
        v: BinaryFrame<T>,
    ) -> Result<(), rmp_serde::encode::Error> {
        log::trace!("send_binary {:?}", v);
        self.ws
            .as_ref()
            .unwrap()
            .send_with_u8_array(&rmp_serde::to_vec(&v)?)
            .map_err(|x| rmp_serde::encode::Error::custom(format!("{:?}", x)))
    }

    pub fn start(&mut self, name: String, s: String) {
        log::debug!("Starting server {} on {:?}", name, s);
        self.name = name;

        let ws = WebSocket::new(&format!("{}nt/{}", s, self.name))
            .expect(&format!("Unable to start websocket connection to {}", s));
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let on_message_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(abuf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                let data = js_sys::Uint8Array::new(&abuf).to_vec();
                if let Ok(data) = rmp_serde::from_slice::<BinaryFrame>(&data) {
                    println!("binary {:?}", data);
                    if data.0 == -1 {
                        // timesync
                        let server_time = data.1;
                        let local_time = data.3.as_f64().unwrap();
                        let perf = web_sys::window().unwrap().performance().unwrap();
                        let now = perf.now() * 1000.0;
                        let rtt_2 = ((now - local_time) / 2.0).round() as u32;
                        // local_time + offs + rtt_2 = server_time
                        // offs = server_time - rtt_2 - local_time
                        let new_time = STATE.with(|st| {
                            st.borrow_mut().offs = server_time - rtt_2 - local_time.round() as u32;
                            st.borrow_mut().ready = true;
                            st.borrow().now()
                        });
                        log::trace!("time synced: now = {}, server = {}", new_time, server_time);
                    } else {
                        log::trace!("data in {}", data.0);
                        if let Ok(value) = serde_wasm_bindgen::to_value(&data.3) {
                            STATE.with(|st| {
                                if let Some(f) = st.borrow_mut().sub_callbacks.get(&data.0) {
                                    _ = f.call1(&JsValue::null(), &value);
                                }
                            })
                        }
                    }
                }
            } else if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                let data = text.as_string().unwrap();
                let data = serde_json::from_str::<Vec<TextFrame>>(&data);
                if let Ok(data) = data {
                    for frame in data {
                        println!("text {:?}", &frame);
                        match frame {
                            TextFrame::Announce {
                                name,
                                id,
                                ty,
                                ..
                            } => STATE.with(|st| {
                                st.borrow_mut().topic_names.insert(name.clone(), id);
                                for callback in st.borrow().announce_callbacks.iter() {
                                    _ = callback.call2(
                                        &JsValue::null(),
                                        &JsValue::from(&name),
                                        &JsValue::from(&ty),
                                    );
                                }
                            }),
                            _ => {}
                        }
                    }
                } else {
                    log::error!("text error {:?}", data);
                }
            } else {
                log::warn!("Unrecognized packet recieved {:?}", e.data());
            }
        });

        ws.set_onmessage(Some(on_message_callback.as_ref().unchecked_ref()));
        on_message_callback.forget(); // technically leaks. Needs to check if this is actually an issue

        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            log::info!("error {:?}", e);
        });

        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            log::trace!("websocket opened");
            let a = Closure::<dyn Fn()>::new(|| {
                timesync();
            });
            web_sys::window()
                .unwrap()
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    a.as_ref().unchecked_ref(),
                    3000,
                )
                .unwrap(); // Configure timeout?
            a.forget();
            timesync();
        });

        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        self.ws = Some(ws);
    }
}

fn start_ip(name: String, s: String) {
    log::trace!("start_ip({:?})", s);
    STATE.with(|st| {
        st.borrow_mut().start(name, format!("ws://{}:5810/", s));
    });
}

fn start_team_no(name: String, team_number: u64) {
    log::trace!("start_team_no({})", team_number);
    let high = team_number / 100;
    let low = team_number % 100;
    start_ip(name, format!("10.{}.{}.2", high, low))
}

fn start(name: String, s: &JsValue) {
    match s.as_string() {
        Some(x) => start_ip(name, x),
        None => {
            let team_number = s.as_f64().unwrap().round() as u64;
            start_team_no(name, team_number);
        }
    }
}

struct StateFuture;

impl Future for StateFuture {
    type Output = bool;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        STATE.with(|st| {
            if st.borrow().ready {
                std::task::Poll::Ready(true)
            } else {
                let waker = cx.waker().clone();
                let next = Closure::<dyn Fn()>::new(move || {
                    waker.wake_by_ref();
                });
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        next.as_ref().unchecked_ref(),
                        200,
                    )
                    .unwrap();
                next.forget();
                std::task::Poll::Pending
            }
        })
    }
}

#[wasm_bindgen]
pub async fn _nt4_start(name: String, x: &JsValue) {
    log::trace!("_nt4_start({:?}, {:?})", name, x);
    start(name, x);
    assert!(StateFuture.await);
}

#[wasm_bindgen(js_name = addEventListener)]
pub fn add_event_listener(name: &str, f: js_sys::Function) {
    log::trace!("add_event_listener");
    match name {
        "announce" => {
            STATE.with(|st| {
                st.borrow_mut().announce_callbacks.push(f);
            });
        }
        "unannounce" => {
            STATE.with(|st| {
                st.borrow_mut().unannounce_callbacks.push(f);
            });
        }
        "properties" => {
            STATE.with(|st| {
                st.borrow_mut().properties_callbacks.push(f);
            });
        }
        x => log::warn!("Unrecognized event {:?}", x),
    }
}

fn newuid() -> i64 {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf[0..2]).unwrap();
    return i64::from_le_bytes(buf);
}

#[wasm_bindgen]
pub fn subscribe(name: &str, f: js_sys::Function, periodic: Option<f64>) -> i64 {
    let uid = newuid();
    log::trace!("subscribe -> {}", uid);
    send_json(TextFrame::Subscribe {
        topics: vec![String::from(name)],
        subuid: uid,
        options: SubscribeOptions {
            periodic,
            all: true,
            topicsonly: false,
            prefix: false,
        },
    })
    .unwrap();
    STATE.with(|st| {
        let topic_id = st.borrow_mut().topic_names.get(name).map(|x| *x).unwrap_or(-1);
        if topic_id >= 0 {
            st.borrow_mut().sub_callbacks.insert(topic_id, f);
        }
    });
    uid
}

#[wasm_bindgen]
pub fn unsubscribe(uid: i64) {
    send_json(TextFrame::Unsubscribe { subuid: uid }).unwrap();
}

#[wasm_bindgen]
pub async fn sub_topic_list(prefix: &str) -> i64 {
    let uid = newuid();
    log::trace!("sub_topic_list -> {}", uid);
    send_json(TextFrame::Subscribe {
        topics: vec![String::from(prefix)],
        subuid: uid,
        options: SubscribeOptions {
            periodic: None,
            all: true,
            topicsonly: true,
            prefix: true,
        },
    })
    .unwrap();
    uid
}

#[wasm_bindgen]
pub async fn publish(name: &str, ty: &str) -> i64 {
    let uid = newuid();
    log::trace!("publish -> {}", uid);
    send_json(TextFrame::Publish {
        name: name.to_string(),
        pubuid: uid,
        ty: ty.to_string(),
        properties: Properties {
            persistent: true,
            retained: true,
        },
    })
    .unwrap();
    uid
}

#[wasm_bindgen]
pub fn unpublish(uid: i64) {
    send_json(TextFrame::Unpublish { pubuid: uid }).unwrap();
}

#[wasm_bindgen(start)]
pub async fn initialize() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    #[cfg(debug_assertions)]
    console_log::init_with_level(log::Level::Trace).expect("Couldn't initialize logger");
    #[cfg(not(debug_assertions))]
    console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
}
