
#[derive(Debug)]
pub struct BinaryDataFrame {
    pub topic_id: i32,
    pub timestamp: i64,
    pub data: crate::types::Nt4Data,
}

impl BinaryDataFrame {
    pub fn timesync(time: i64) -> Self {
        Self { topic_id: -1, timestamp: 0, data: crate::types::Nt4Data::Int(time) }
    }
}

impl serde::Serialize for BinaryDataFrame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        use serde::ser::SerializeTuple;

        let data_type = self.data.get_id();
        serializer.serialize_tuple(4).and_then(|mut s| {
            s.serialize_element(&self.topic_id)?;
            s.serialize_element(&self.timestamp)?;
            s.serialize_element(&data_type)?;
            s.serialize_element(&self.data)?;
            s.end()
        })
    }
}

impl<'de> serde::Deserialize<'de> for BinaryDataFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (topic_id, timestamp, data_type, data) =
            <(i32, i64, u8, crate::types::Nt4Data) as serde::Deserialize>::deserialize(
                deserializer,
            )?;
        if data.get_id() != data_type {
            Err(<D::Error as serde::de::Error>::custom(format!(
                "data type does not match type id: got id for {:?}, expected id for {:?}",
                data.get_name(),
                crate::types::Nt4TypeId::from_id(data_type)
                    .map_err(|x| <D::Error as serde::de::Error>::custom(x))?
            )))
        } else {
            Ok(Self {
                topic_id,
                timestamp,
                data,
            })
        }
    }
}