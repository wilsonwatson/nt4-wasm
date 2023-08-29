#![allow(dead_code)]

use std::time::Duration;
use serde_bytes::ByteBuf;

macro_rules! nt4_type {
    ($($name:ident($str:literal, $id:literal, $ty:ty, [$(($other:ident, $other_name:literal)),* $(,)?])),* $(,)?) => {
        #[derive(Debug)]
        #[derive(Clone, Copy)]
        pub enum Nt4TypeId {
            $($name),*
        }

        impl Nt4TypeId {
            #[allow(unreachable_patterns)]
            pub fn from_id(id: u8) -> Result<Self, String> {
                match id {
                    $(
                        $id => Ok(Self::$name),
                    )*
                    x => Err(format!("Unrecognized type id: {}", x))
                }
            }

            pub fn get_id(&self) -> u8 {
                match self {
                    $(
                        Self::$name => $id
                    ),*
                }
            }

            pub fn get_name(&self) -> &'static str {
                match self {
                    $(
                        Self::$name => $str
                    ),*
                }
            }
        }

        impl serde::Serialize for Nt4TypeId {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
                match self {
                    $(
                        Self::$name => serializer.serialize_str($str)
                    ),*
                }
            }
        }

        impl <'de> serde::Deserialize<'de> for Nt4TypeId {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de> {
                let res = <String as serde::Deserialize>::deserialize(deserializer)?;
                match res.as_str() {
                    $(
                        $str => Ok(Self::$name),
                    )*
                    x => Err(<D::Error as serde::de::Error>::custom(format!("Unrecognized type: {:?}", x)))
                }
            }
        }
        
        #[derive(serde::Deserialize, serde::Serialize)]
        #[derive(Debug)]
        #[serde(untagged)]
        pub enum Nt4Data {
            $($name($ty)),*
        }

        impl Nt4Data {
            pub fn get_id(&self) -> u8 {
                match self {
                    $(
                        Self::$name(_) => $id
                    ),*
                }
            }

            pub fn get_name(&self) -> &'static str {
                match self {
                    $(
                        Self::$name(_) => $str
                    ),*
                }
            }

            pub fn get_type_id(&self) -> Nt4TypeId {
                match self {
                    $(
                        Self::$name(_) => Nt4TypeId::$name
                    ),*
                }
            }

            $(
                paste::paste! {
                    pub fn [<as_ $name:snake>](&self) -> Option<&$ty> {
                        match self {
                            Self::$name(y) => Some(y),
                            $(
                                Self::$other(y) => Some(y),
                            )*
                            _ => None
                        }
                    }
                }
            )*

            #[allow(unused_variables)]
            pub fn convert(self, name: &str) -> Result<Self, String> {
                match self {
                    $(
                        Self::$name(y) => {
                            match name {
                                $str => Ok(Self::$name(y)),
                                $(
                                    $other_name => Ok(Self::$other(y)),
                                )*
                                x => Err(format!("Cannot convert from {:?} to {:?}", $str, x))
                            }
                        },
                    )*
                }
            }
        }
    };
}

nt4_type! {
    Boolean("boolean", 0, bool, []),
    Double("double", 1, f64, []),
    Int("int", 2, i64, []),
    Float("float", 3, f32, []),
    String("string", 4, String, [(Json, "json")]),
    Json("json", 4, String, [(String, "string")]),
    Raw("raw", 5, ByteBuf, [(Rpc, "rpc"), (MsgPack, "msgpack"), (Protobuf, "protobuf")]),
    Rpc("rpc", 5, ByteBuf, [(Raw, "raw"), (MsgPack, "msgpack"), (Protobuf, "protobuf")]),
    MsgPack("msgpack", 5, ByteBuf, [(Raw, "raw"), (Rpc, "rpc"), (Protobuf, "protobuf")]),
    Protobuf("protobuf", 5, ByteBuf, [(Raw, "raw"), (Rpc, "rpc"), (MsgPack, "msgpack")]),
    BooleanArray("boolean[]", 16, Vec<bool>, []),
    DoubleArray("double[]", 17, Vec<f64>, []),
    IntArray("int[]", 18, Vec<i64>, []),
    FloatArray("float[]", 19, Vec<f32>, []),
    StringArray("string[]", 20, Vec<String>, []),
}



#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct SubscriptionOptions {
    #[
        serde(
            serialize_with = "utils::serialize_duration_s",
            deserialize_with = "utils::deserialize_duration_s",
            default = "defaults::duration_100ms"
        )
    ]
    pub periodic: Duration,
    #[serde(default = "defaults::def_false")]
    pub all: bool,
    #[serde(default = "defaults::def_false")]
    pub topicsonly: bool,
    #[serde(default = "defaults::def_false")]
    pub prefix: bool,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        Self {
            periodic: defaults::duration_100ms(),
            all: defaults::def_false(),
            topicsonly: defaults::def_false(),
            prefix: defaults::def_false(),
        }
    }
}


mod utils {
    use std::time::Duration;

    use serde::{Deserializer, Serializer};

    pub fn deserialize_duration_s<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Duration::from_secs_f64(
            <f64 as serde::Deserialize>::deserialize(deserializer)?,
        ))
    }

    pub fn serialize_duration_s<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(duration.as_secs_f64())
    }
}

mod defaults {
    use std::time::Duration;

    pub fn duration_100ms() -> Duration {
        Duration::from_millis(100)
    }

    pub fn def_false() -> bool {
        true
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct Properties {
    #[serde(default)]
    pub persistent: bool,
    #[serde(default)]
    pub retained: bool,
}

#[doc = "Properties, but all members are optional. Used for updating properties of a topic."]
#[derive(serde::Deserialize)]
#[derive(Debug)]
pub struct PartialProperties {
    #[serde(default)]
    pub persistent: Option<bool>,
    #[serde(default)]
    pub retained: Option<bool>,
}

impl serde::Serialize for PartialProperties {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        #[derive(serde::Serialize)]
        struct PnR {
            persistent: bool,
        }
        #[derive(serde::Serialize)]
        struct RnP {
            retained: bool
        }
        #[derive(serde::Serialize)]
        struct RP {
            persistent: bool,
            retained: bool,
        }
        #[derive(serde::Serialize)]
        struct N {}

        if let Some(persistent) = self.persistent {
            if let Some(retained) = self.retained {
                (RP { persistent, retained }).serialize(serializer)
            } else {
                (PnR { persistent }).serialize(serializer)
            }
        } else if let Some(retained) = self.retained {
            (RnP { retained }).serialize(serializer)
        } else {
            (N {}).serialize(serializer)
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Topic {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Nt4TypeId,
}