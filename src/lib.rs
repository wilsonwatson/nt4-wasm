use chrono::Duration;

use instant::Instant;
use js_sys::JsString;
use wasm_bindgen::prelude::*;

mod binary;
mod text;
mod types;
mod instant;

use text::*;
use types::*;

#[wasm_bindgen]
pub struct Nt4Connection {
    send_binary_fn: Option<js_sys::Function>,
    send_text_fn: Option<js_sys::Function>,
    announce_fn: Option<js_sys::Function>,
    unannounce_fn: Option<js_sys::Function>,
    ready_fn: Option<js_sys::Function>,
    unready_fn: Option<js_sys::Function>,
    on_data_fn: Option<js_sys::Function>,
    start_time: Instant,
    offs: i64,
    uid_cnt: i32,
}

macro_rules! set_fns {
    ($($name:ident),* $(,)?) => {
        paste::paste! {
            #[wasm_bindgen]
            impl Nt4Connection {
                #[wasm_bindgen(constructor)]
                pub fn new() -> Nt4Connection {
                    Self {
                        $(
                            $name: None,
                        )*
                        start_time: Instant::now(),
                        offs: 0,
                        uid_cnt: 0,
                    }
                }
                $(
                    pub fn [<set_ $name>](&mut self, f: js_sys::Function) {
                        self.$name = Some(f);
                    }
                )*
            }
        }
    };
}

set_fns! {
    send_binary_fn,
    send_text_fn,
    announce_fn,
    unannounce_fn,
    ready_fn,
    unready_fn,
    on_data_fn,
}

macro_rules! expect_available {
    ($self:ident $b:block) => {
        $b
    };
    ($self:ident $name:ident $b:block) => {
        if let Some($name) = $self.$name.clone() {
            $b
        } else {
            Err(JsString::from(format!("{} not implemented!", stringify!($name))).into())
        }
    };
    ($self:ident $name:ident, $($names:ident),* $b:block) => {
        if let Some($name) = $self.$name.clone() {
            expect_available! { $self $($names),* $b }
        } else {
            Err(JsString::from(format!("{} not implemented!", stringify!($name))).into())
        }
    };
}

impl Nt4Connection {
    fn now(&mut self) -> Result<i64, JsValue> {
        let now = Duration::from_std(Instant::now().duration_since(self.start_time))
            .map_err(|x| JsString::from(format!("{:?}", x)))?;
        Ok(if let Some(us) = now.num_microseconds() {
            us
        } else {
            self.start_time = Instant::now();
            let now = Duration::from_std(Instant::now().duration_since(self.start_time))
                .map_err(|x| JsString::from(format!("{:?}", x)))?;
            now.num_microseconds().unwrap()
        })
    }

    fn new_uid(&mut self) -> i32 {
        let next = self.uid_cnt;
        self.uid_cnt += 1;
        next
    }
}

#[wasm_bindgen]
impl Nt4Connection {
    #[doc = " unsubscribe(int id)\n"]
    #[doc = " @param {number} id - topic id recieved from a {@link subscribe} call."]
    #[wasm_bindgen(skip_jsdoc)]
    pub fn unsubscribe(&mut self, id: i32) -> Result<(), JsValue> {
        expect_available! { self send_text_fn {
            let data = text::ClientToServerTextDataFrame::Unsubscribe(UnsubscribeParams { subuid: id });
            let data = serde_json::to_string(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            send_text_fn.call1(&JsValue::NULL, &JsString::from(data))?;
            Ok(())
        } }
    }

    pub fn subscribe(&mut self, path: &str, options: JsValue) -> Result<i32, JsValue> {
        let options = serde_wasm_bindgen::from_value(options)?;
        expect_available! { self send_text_fn {
            let id = self.new_uid();
            let data = text::ClientToServerTextDataFrame::Subscribe(
                SubscribeParams {
                    topics: vec![path.to_string()],
                    subuid: id,
                    options,
                },
            );
            let data = serde_json::to_string(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            send_text_fn.call1(&JsValue::NULL, &JsString::from(data))?;
            Ok(id)
        } }
    }

    pub fn unpublish(&mut self, id: i32) -> Result<(), JsValue> {
        expect_available! { self send_text_fn {
            let data = text::ClientToServerTextDataFrame::Unpublish(UnpublishParams {
                pubuid: id
            });
            let data = serde_json::to_string(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            send_text_fn.call1(&JsValue::NULL, &JsString::from(data))?;
            Ok(())
        } }
    }

    pub fn publish(
        &mut self,
        name: &str,
        ty: JsValue,
        properties: JsValue,
    ) -> Result<i32, JsValue> {
        let ty = serde_wasm_bindgen::from_value(ty)?;
        let properties = serde_wasm_bindgen::from_value(properties)?;
        expect_available! { self send_text_fn {
            let id = self.new_uid();
            let data = text::ClientToServerTextDataFrame::Publish(PublishParams {
                name: name.to_string(),
                properties,
                pubuid: id,
                ty,
            });
            let data = serde_json::to_string(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            send_text_fn.call1(&JsValue::NULL, &JsString::from(data))?;
            Ok(id)
        } }
    }

    pub fn set_properties(&mut self, name: &str, update: JsValue) -> Result<(), JsValue> {
        let update = serde_wasm_bindgen::from_value(update)?;
        expect_available! { self send_text_fn {
            let data = text::ClientToServerTextDataFrame::SetProperties(SetPropertiesParams {
                name: name.to_string(),
                update
            });
            let data = serde_json::to_string(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            send_text_fn.call1(&JsValue::NULL, &JsString::from(data))?;
            Ok(())
        } }
    }

    pub fn timesync(&mut self) -> Result<(), JsValue> {
        expect_available! { self send_binary_fn {
            let now = self.now()?;
            let data = binary::BinaryDataFrame::timesync(now);
            let data = rmp_serde::to_vec(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            let data = serde_wasm_bindgen::to_value(&data)?;
            send_binary_fn.call1(&JsValue::NULL, &data)?;
            Ok(())
        } }
    }

    pub fn on_binary(&mut self, data_frame: Vec<u8>) -> Result<(), JsValue> {
        let data_frame: binary::BinaryDataFrame =
            rmp_serde::from_slice(&data_frame).map_err(|x| JsString::from(format!("{:?}", x)))?;
        expect_available! { self on_data_fn, ready_fn {
            if data_frame.topic_id == -1 {
                if let Some(local_time) = data_frame.data.as_int() {
                    let local_time = Duration::microseconds(*local_time);
                    let server_time = Duration::microseconds(data_frame.timestamp);
                    let now = Duration::microseconds(self.now()?);
                    let rtt_2 = (now - local_time) / 2;
                    self.offs = (server_time - rtt_2 - local_time).num_microseconds().unwrap();
                    ready_fn.call0(&JsValue::NULL)?;
                    Ok(())
                } else {
                    Err(JsString::from(format!("Invalid timesync dataframe: {:?}", data_frame)).into())
                }
            } else {
                let data = serde_wasm_bindgen::to_value(&data_frame.data)?;
                on_data_fn.call3(&JsValue::NULL, &JsValue::from(data_frame.topic_id), &JsValue::from(data_frame.timestamp), &data)?;
                Ok(())
            }
        }}
    }

    pub fn on_text(&mut self, data_frame: String) -> Result<(), JsValue> {
        let data_frame: text::ServerToClientTextDataFrame = serde_json::from_str(&data_frame).map_err(|x| JsString::from(format!("{:?}", x)))?;
        match data_frame {
            text::ServerToClientTextDataFrame::Announce(ann) => {
                expect_available! { self announce_fn {
                    let data = serde_wasm_bindgen::to_value(&Topic { name: ann.name, ty: ann.ty })?;
                    announce_fn.call1(&JsValue::NULL, &data)?;
                    Ok(())
                } }
            },
            text::ServerToClientTextDataFrame::Unannounce(unann) => {
                expect_available! { self unannounce_fn {
                    let data = JsString::from(unann.name);
                    unannounce_fn.call1(&JsValue::NULL, &data)?;
                    Ok(())
                } }
            },
            text::ServerToClientTextDataFrame::Properties(_) => {
                /* IDK what happens here */
                Ok(())
            },
            
        }
    }

    pub fn on_disconnect(&mut self) -> Result<(), JsValue> {
        expect_available! { self unready_fn {
            unready_fn.call0(&JsValue::NULL)?;
            Ok(())
        } }
    }

    pub fn send_data(&mut self, topic_id: i32, data: JsValue) -> Result<(), JsValue> {
        let inner_data: types::Nt4Data = serde_wasm_bindgen::from_value(data)?;
        expect_available! { self send_binary_fn {
            let now = self.now()?;
            let data = binary::BinaryDataFrame { data: inner_data, timestamp: now + self.offs, topic_id };
            let data = rmp_serde::to_vec(&data).map_err(|x| JsString::from(format!("{:?}", x)))?;
            let data = serde_wasm_bindgen::to_value(&data)?;
            send_binary_fn.call1(&JsValue::NULL, &data)?;
            Ok(())
        } }
    }
}

#[wasm_bindgen(start)]
pub fn run() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}
