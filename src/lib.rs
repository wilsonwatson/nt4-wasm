use std::{cell::RefCell, collections::HashMap, fmt::Debug, future::Future, rc::Rc};

use bimap::BiMap;
use js_sys::Array;
use serde::{ser::Error, Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{
    Event, HtmlElement, HtmlInputElement, MessageEvent, MutationObserver, MutationObserverInit,
    MutationRecord, WebSocket,
};

const SUB_TO_TOPIC_ATTR: &'static str = "subscribe";
const SUB_TO_TOPIC_LIST_ATTR: &'static str = "subscribe_list";
const PUB_TOPIC_ATTR: &'static str = "publish";
const PUB_TOPIC_TYPE_ATTR: &'static str = "publish_type";
const PROCESSED_ATTR: &'static str = "nt4_processed";
const PERIOD_ATTR: &'static str = "period";

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
    offs: i32,
    ready: bool,
    announce_callbacks: Vec<js_sys::Function>,
    unannounce_callbacks: Vec<js_sys::Function>,
    properties_callbacks: Vec<js_sys::Function>,
    close_callbacks: Vec<js_sys::Function>,
    ready_callbacks: Vec<js_sys::Function>,
    sub_callbacks: HashMap<String, js_sys::Function>,
    topic_names: BiMap<String, i64>,
    periodic: i32,
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

fn send_binary_fill(id: i64, ty: String, v: rmpv::Value) -> Result<(), rmp_serde::encode::Error> {
    let ty: u32 = match ty.as_str() {
        "boolean" => 0,
        "double" => 1,
        "int" => 2,
        "float" => 3,
        "string" | "json" => 4,
        "raw" | "rpc" | "msgpack" | "protobuf" => 5,
        "boolean[]" => 16,
        "double[]" => 17,
        "int[]" => 18,
        "float[]" => 19,
        "string[]" => 20,
        _ => {
            return Err(rmp_serde::encode::Error::custom(format!(
                "invalid type: {:?}",
                ty
            )))
        }
    };
    STATE.with(|st| {
        st.borrow()
            .send_binary(BinaryFrame(id, st.borrow().now(), ty, v))
    })
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
            close_callbacks: Vec::new(),
            ready_callbacks: Vec::new(),
            sub_callbacks: HashMap::new(),
            topic_names: BiMap::new(),
            periodic: -1,
        }
    }

    fn now(&self) -> u32 {
        let perf = web_sys::window().unwrap().performance().unwrap();
        let now = perf.now() * 1000.0;
        (now.round() as i32 + self.offs) as u32
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
        self.name = name.clone();

        let ws = WebSocket::new(&format!("{}nt/{}", s, self.name))
            .expect(&format!("Unable to start websocket connection to {}", s));
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let on_message_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(abuf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                let data = js_sys::Uint8Array::new(&abuf).to_vec();
                if let Ok(data) = rmp_serde::from_slice::<BinaryFrame>(&data) {
                    log::trace!("binary {:?}", data);
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
                            st.borrow_mut().offs =
                                server_time as i32 - rtt_2 as i32 - local_time.round() as i32;
                            st.borrow_mut().ready = true;
                            for callback in &st.borrow_mut().ready_callbacks {
                                _ = callback.call0(&JsValue::NULL);
                            }
                            st.borrow().now()
                        });
                        log::trace!("time synced: now = {}, server = {}", new_time, server_time);
                    } else {
                        log::trace!("data in {}", data.0);
                        if let Ok(value) = serde_wasm_bindgen::to_value(&data.3) {
                            STATE.with(|st| {
                                let x = if let Some(topic) =
                                    st.borrow_mut().topic_names.get_by_right(&data.0)
                                {
                                    Some(topic.clone())
                                } else {
                                    None
                                };
                                if let Some(topic) = x {
                                    if let Some(f) = st.borrow_mut().sub_callbacks.get(&topic) {
                                        _ = f.call1(&JsValue::null(), &value);
                                    }
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
                            TextFrame::Announce { name, id, ty, .. } => STATE.with(|st| {
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

        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: Event| {
            let ready = STATE.with(|st| st.borrow_mut().ready);
            match e.type_().as_str() {
                "error" => {
                    STATE.with(|st| st.borrow_mut().ready = false);
                    log::info!("error establishing connection, trying again in 3 seconds");
                }
                "close" => {
                    if !ready {
                        return;
                    }
                    log::info!("connection closed, trying again in 3 seconds");
                }
                _ => return,
            }
            let name = name.clone();
            let s = s.clone();
            let a = Closure::<dyn Fn()>::new(move || {
                STATE.with(|st| {
                    st.borrow_mut().start(name.clone(), s.clone());
                });
            });
            STATE.with(|st| {
                st.borrow_mut().ready = false;
                for callback in &st.borrow_mut().close_callbacks {
                    _ = callback.call0(&JsValue::NULL);
                }
                web_sys::window()
                    .unwrap()
                    .clear_interval_with_handle(st.borrow().periodic)
            });
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    a.as_ref().unchecked_ref(),
                    3000,
                )
                .unwrap();
            a.forget();
        });

        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            log::trace!("websocket opened");
            let a = Closure::<dyn Fn()>::new(|| {
                timesync();
            });
            let periodic = web_sys::window()
                .unwrap()
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    a.as_ref().unchecked_ref(),
                    3000,
                )
                .unwrap(); // Configure timeout?
            a.forget();
            STATE.with(|st| st.borrow_mut().periodic = periodic);
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
        "close" => {
            STATE.with(|st| {
                st.borrow_mut().close_callbacks.push(f);
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
        st.borrow_mut().sub_callbacks.insert(name.to_string(), f);
    });
    uid
}

pub fn unsubscribe(uid: i64) {
    send_json(TextFrame::Unsubscribe { subuid: uid }).unwrap();
}

pub fn sub_topic_list(prefix: &str) -> i64 {
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

pub fn publish(name: &str, ty: &str) -> i64 {
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

fn send_data_serde<T : Serialize>(pubid: i64, ty: String, obj: T) {
    let v: rmpv::Value = rmp_serde::from_slice(&rmp_serde::to_vec(&obj).unwrap()).unwrap();
    send_binary_fill(pubid, ty, v).unwrap();
}

#[allow(dead_code)]
fn send_data(pubid: i64, ty: String, obj: JsValue) {
    let v: rmpv::Value = serde_wasm_bindgen::from_value(obj).unwrap();
    send_binary_fill(pubid, ty, v).unwrap();
}

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

fn infill(node: &HtmlElement, topic: String) {
    let _doc = web_sys::window().unwrap().document().unwrap();
    let period = node
        .get_attribute(PERIOD_ATTR)
        .and_then(|x| x.parse::<f64>().ok());

    match node.tag_name().as_str() {
        "INPUT" => {
            let ncopy = node.clone();
            if let Ok(node) = ncopy.dyn_into::<HtmlInputElement>() {
                let a = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
                    if let Ok(v) = serde_wasm_bindgen::from_value::<rmpv::Value>(value) {
                        if let Ok(v) = serde_json::to_string(&v) {
                            node.set_value(&v);
                        }
                    }
                });
                subscribe(
                    &topic,
                    a.as_ref().unchecked_ref::<js_sys::Function>().clone(),
                    period,
                );
                a.forget();
            }
        }
        _ => {
            // simple print
            let ncopy = node.clone();
            let a = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
                if let Ok(v) = serde_wasm_bindgen::from_value::<rmpv::Value>(value) {
                    if let Ok(v) = serde_json::to_string(&v) {
                        ncopy.set_inner_text(&v);
                    }
                }
            });
            subscribe(
                &topic,
                a.as_ref().unchecked_ref::<js_sys::Function>().clone(),
                period,
            );
            a.forget();
        }
    }

    node.set_attribute(PROCESSED_ATTR, "true").unwrap();
}

/// add proper events for topic list
fn infill_list(node: &HtmlElement, root: String) {
    log::warn!("subscribe lists are not yet implemented!");
    let ncopy = node.clone();
    let a = Closure::<dyn Fn(JsValue, JsValue)>::new(move |name: JsValue, ty: JsValue| {
        let doc = web_sys::window().unwrap().document().unwrap();
        let div = doc.create_element("div").unwrap();
        div.set_attribute("name", &name.as_string().unwrap()).unwrap();
        div.set_attribute("type", &ty.as_string().unwrap()).unwrap();
        ncopy.append_child(&div).unwrap();
    });
    add_event_listener("announce", a.as_ref().unchecked_ref::<js_sys::Function>().clone());
    a.forget();
    // TODO unannounce
    sub_topic_list(&root);
    node.set_attribute(PROCESSED_ATTR, "true").unwrap();
}

/// add proper events for publisher
fn infill_pub(node: &HtmlElement, topic: String, ty: String) {
    match node.tag_name().as_str() {
        "INPUT" => {
            let id = publish(&topic, &ty);
            let ncopy = node.clone().dyn_into::<HtmlInputElement>().unwrap();
            let a = Closure::<dyn Fn()>::new(move || match ty.as_str() {
                "string" => {
                    let data = ncopy.value();
                    send_data_serde(id, ty.clone(), data);
                }
                "double" => {
                    if let Ok(data) = ncopy.value().parse::<f64>() {
                        send_data_serde(id, ty.clone(), data);
                    }
                }
                "float" => {
                    if let Ok(data) = ncopy.value().parse::<f32>() {
                        send_data_serde(id, ty.clone(), data);
                    }
                }
                "int" => {
                    if let Ok(data) = ncopy.value().parse::<i64>() {
                        send_data_serde(id, ty.clone(), data);
                    }
                }
                _ => {}
            });
            node.add_event_listener_with_callback("change", a.as_ref().unchecked_ref())
                .unwrap();
            a.forget();
        }
        x => {
            log::warn!("{}", x);
        }
    }
}

/// check if element should be infilled, then infill it
fn maybe_infill<T>(t: T)
where
    T: JsCast + AsRef<JsValue> + Into<JsValue>,
{
    if let Ok(elem) = t.dyn_into::<HtmlElement>() {
        if let Some(x) = elem.get_attribute(PROCESSED_ATTR) {
            if x == "true" {
                return;
            }
        }
        if let Some(id) = elem.get_attribute(PUB_TOPIC_ATTR) {
            if let Some(ty) = elem.get_attribute(PUB_TOPIC_TYPE_ATTR) {
                infill_pub(&elem, id, ty);
            }
        }
        if let Some(id) = elem.get_attribute(SUB_TO_TOPIC_ATTR) {
            if id.starts_with("/") {
                infill(&elem, id);
            }
        } else if let Some(id) = elem.get_attribute(SUB_TO_TOPIC_LIST_ATTR) {
            if id.starts_with("/") && !id.ends_with("/") {
                infill_list(&elem, id);
            }
        }
    }
}

#[wasm_bindgen]
pub async fn _nt4_start(name: String, x: &JsValue) {
    log::trace!("_nt4_start({:?}, {:?})", name, x);
    // ready callbacks
    STATE.with(|st| {
        let ready_callback = Closure::<dyn Fn()>::new(move || {
            let doc = web_sys::window().unwrap().document().unwrap();
            if let Ok(parts) = doc.query_selector_all(".nt_is_ready") {
                for i in 0..parts.length() {
                    if let Some(node) = parts.item(i) {
                        if let Ok(elem) = node.dyn_into::<HtmlElement>() {
                            _ = elem.class_list().add_1("nt_ready");
                            _ = elem.class_list().remove_1("nt_closed");
                        }
                    }
                }
            }
        });
        st.borrow_mut().ready_callbacks.push(
            ready_callback
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone(),
        );
        ready_callback.forget();
        let close_callback = Closure::<dyn Fn()>::new(move || {
            let doc = web_sys::window().unwrap().document().unwrap();
            if let Ok(parts) = doc.query_selector_all(".nt_is_ready") {
                for i in 0..parts.length() {
                    if let Some(node) = parts.item(i) {
                        if let Ok(elem) = node.dyn_into::<HtmlElement>() {
                            _ = elem.class_list().add_1("nt_closed");
                            _ = elem.class_list().remove_1("nt_ready");
                        }
                    }
                }
            }
        });
        st.borrow_mut().close_callbacks.push(
            close_callback
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone(),
        );
        close_callback.forget();
    });

    let doc = web_sys::window().unwrap().document().unwrap();
    if let Ok(parts) = doc.query_selector_all(".nt_is_ready") {
        for i in 0..parts.length() {
            if let Some(node) = parts.item(i) {
                if let Ok(elem) = node.dyn_into::<HtmlElement>() {
                    _ = elem.class_list().add_1("nt_closed");
                }
            }
        }
    }

    start(name, x);
    assert!(StateFuture.await);

    let a = Closure::<dyn Fn(Vec<MutationRecord>, MutationObserver)>::new(
        move |records: Vec<MutationRecord>, _observer| {
            for record in &records {
                match record.type_().as_str() {
                    "attributes" => {
                        if let Some(node) = record.target() {
                            maybe_infill(node);
                        }
                        let doc = web_sys::window().unwrap().document().unwrap();
                        let p = doc.create_element("p").unwrap();
                        p.set_inner_html(&format!(
                            "{:?} {:?}",
                            record.target(),
                            record.attribute_name()
                        ));
                        doc.body().unwrap().append_child(&p).unwrap();
                    }
                    x => {
                        log::warn!("{:?}", x);
                    }
                }
            }
        },
    );
    let mutation_observer = MutationObserver::new(a.as_ref().unchecked_ref()).unwrap();
    a.forget();

    let doc = web_sys::window().unwrap().document().unwrap();

    let elems = doc.get_elements_by_tag_name("*");
    for i in 0..elems.length() {
        if let Some(elem) = elems.item(i) {
            maybe_infill(elem);
        }
    }

    let body = doc.body().unwrap();
    let arr = Array::new_with_length(5);
    // watch any element in body for a change to the following 5 attributes (including creation or destruction)
    arr.set(0, JsValue::from_str(SUB_TO_TOPIC_ATTR));
    arr.set(1, JsValue::from_str(SUB_TO_TOPIC_LIST_ATTR));
    arr.set(2, JsValue::from_str(PROCESSED_ATTR));
    arr.set(3, JsValue::from_str(PUB_TOPIC_ATTR));
    arr.set(4, JsValue::from_str(PUB_TOPIC_TYPE_ATTR));
    mutation_observer
        .observe_with_options(
            &body,
            MutationObserverInit::new()
                .attributes(true)
                .subtree(true)
                .attribute_filter(&arr),
        )
        .unwrap();
}
