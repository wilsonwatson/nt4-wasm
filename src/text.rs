use crate::types::{SubscriptionOptions, Properties, PartialProperties};

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct PublishParams {
    pub name: String,
    pub pubuid: i32,
    #[serde(rename = "type")]
    pub ty: crate::types::Nt4TypeId,
    pub properties: Properties,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct UnpublishParams {
    pub pubuid: i32,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct SetPropertiesParams {
    pub name: String,
    pub update: PartialProperties,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct SubscribeParams {
    pub topics: Vec<String>,
    pub subuid: i32,
    pub options: SubscriptionOptions,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct UnsubscribeParams {
    pub subuid: i32,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct AnnounceParams {
    pub name: String,
    pub id: i32,
    #[serde(rename = "type")]
    pub ty: crate::types::Nt4TypeId,
    #[serde(default)]
    pub pubuid: Option<i32>,
    pub properties: Properties,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct UnannounceParams {
    pub name: String,
    pub id: i32,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
pub struct PropertiesParams {
    pub name: String,
    #[serde(default)]
    pub ack: Option<bool>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
#[
    serde(tag = "method", content = "params", rename_all = "lowercase")
]
pub enum ClientToServerTextDataFrame {
    Publish(PublishParams),
    Unpublish(UnpublishParams),
    SetProperties(SetPropertiesParams),
    Subscribe(SubscribeParams),
    Unsubscribe(UnsubscribeParams),
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Debug)]
#[
    serde(tag = "method", content = "params", rename_all = "lowercase")
]
pub enum ServerToClientTextDataFrame {
    Announce(AnnounceParams),
    Unannounce(UnannounceParams),
    Properties(PropertiesParams),
}