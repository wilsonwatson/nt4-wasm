use binrw::{binread, until_eof, BinRead};
use itertools::Itertools;
use serde_bytes::ByteBuf;

use crate::types::{Nt4Data, Nt4TypeId};

#[binread]
#[derive(Debug)]
pub enum ControlRecord {
    #[br(magic = b"\x00")]
    Start {
        entry_id: u32,
        #[br(temp)]
        entry_name_len: u32,
        #[br(count = entry_name_len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
        entry_name: String,
        #[br(temp)]
        entry_type_len: u32,
        #[br(count = entry_type_len, try_map = |x: Vec<u8>| crate::types::Nt4TypeId::from_name(&String::from_utf8_lossy(&x).into_owned()))]
        entry_type: crate::types::Nt4TypeId,
        #[br(temp)]
        entry_meta_len: u32,
        #[br(count = entry_meta_len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
        entry_meta: String,
    },
    #[br(magic = b"\x01")]
    Finish { entry_id: u32 },
    #[br(magic = b"\x02")]
    SetMetadata {
        entry_id: u32,
        #[br(temp)]
        entry_meta_len: u32,
        #[br(count = entry_meta_len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
        entry_meta: String,
    },
}

#[derive(Debug)]
pub struct RawData {
    entry_id: u32,
    data: Vec<u8>,
}

impl RawData {
    pub fn get_data(&self, ty: Nt4TypeId) -> (u32, Nt4Data) {
        (
            self.entry_id,
            match ty {
                Nt4TypeId::Boolean => Nt4Data::Boolean(self.data[0] != 0),
                Nt4TypeId::Double => Nt4Data::Double(f64::from_be_bytes([
                    self.data[0],
                    self.data[1],
                    self.data[2],
                    self.data[3],
                    self.data[4],
                    self.data[5],
                    self.data[6],
                    self.data[7],
                ])),
                Nt4TypeId::Int => Nt4Data::Int(i64::from_be_bytes([
                    self.data[0],
                    self.data[1],
                    self.data[2],
                    self.data[3],
                    self.data[4],
                    self.data[5],
                    self.data[6],
                    self.data[7],
                ])),
                Nt4TypeId::Float => Nt4Data::Float(f32::from_be_bytes([
                    self.data[0],
                    self.data[1],
                    self.data[2],
                    self.data[3],
                ])),
                Nt4TypeId::String | Nt4TypeId::Json => {
                    Nt4Data::String(String::from_utf8_lossy(&self.data).into_owned())
                }
                Nt4TypeId::Raw | Nt4TypeId::Rpc | Nt4TypeId::MsgPack | Nt4TypeId::Protobuf => {
                    Nt4Data::Raw(ByteBuf::from(self.data.clone()))
                }
                Nt4TypeId::BooleanArray => {
                    Nt4Data::BooleanArray(self.data.iter().map(|x| *x != 0).collect())
                }
                Nt4TypeId::DoubleArray => Nt4Data::DoubleArray(
                    self.data
                        .iter()
                        .tuples()
                        .map(|(v0, v1, v2, v3, v4, v5, v6, v7)| {
                            f64::from_be_bytes([*v0, *v1, *v2, *v3, *v4, *v5, *v6, *v7])
                        })
                        .collect(),
                ),
                Nt4TypeId::IntArray => Nt4Data::IntArray(
                    self.data
                        .iter()
                        .tuples()
                        .map(|(v0, v1, v2, v3, v4, v5, v6, v7)| {
                            i64::from_be_bytes([*v0, *v1, *v2, *v3, *v4, *v5, *v6, *v7])
                        })
                        .collect(),
                ),
                Nt4TypeId::FloatArray => Nt4Data::FloatArray(
                    self.data
                        .iter()
                        .tuples()
                        .map(|(v0, v1, v2, v3)| f32::from_be_bytes([*v0, *v1, *v2, *v3]))
                        .collect(),
                ),
                Nt4TypeId::StringArray => {
                    let len = u32::from_be_bytes([
                        self.data[0],
                        self.data[1],
                        self.data[2],
                        self.data[3],
                    ]);
                    let mut offs = 4;
                    let mut strs = Vec::new();
                    for _ in 0..len {
                        let strlen = u32::from_be_bytes([
                            self.data[offs + 0],
                            self.data[offs + 1],
                            self.data[offs + 2],
                            self.data[offs + 3],
                        ]);
                        let strn = String::from_utf8_lossy(
                            &self.data[offs + 4..offs + 4 + strlen as usize],
                        )
                        .into_owned();
                        offs += 4 + strlen as usize;
                        strs.push(strn);
                    }
                    Nt4Data::StringArray(strs)
                }
            },
        )
    }
}

#[derive(Debug)]
pub enum Payload {
    Control(ControlRecord),
    Data(RawData),
}

impl BinRead for Payload {
    type Args<'a> = (u32, usize);

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        (entry_id, count): Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        if entry_id == 0 {
            Ok(Self::Control(ControlRecord::read_options(
                reader,
                endian,
                (),
            )?))
        } else {
            let mut data = vec![0u8; count];
            reader.read_exact(&mut data)?;
            Ok(Self::Data(RawData { entry_id, data }))
        }
    }
}

pub struct WpiLogRecordHeader {
    entry_id_len: u8,
    payload_size_len: u8,
    timestamp_len: u8,
}

impl BinRead for WpiLogRecordHeader {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let bitfield = u8::read_options(reader, endian, ())?;
        let entry_id_len = (bitfield & 0x03) + 1;
        let payload_size_len = (bitfield & 0x0c) + 1;
        let timestamp_len = (bitfield & 0x70) + 1;
        Ok(Self {
            entry_id_len,
            payload_size_len,
            timestamp_len,
        })
    }
}

#[binread]
#[derive(Debug)]
pub struct WpiLogRecord {
    #[br(temp)]
    len: WpiLogRecordHeader,
    #[br(parse_with = read_varlen_u32, args(len.entry_id_len as usize,))]
    pub entry_id: u32,
    #[br(temp, parse_with = read_varlen_u32, args(len.payload_size_len as usize,))]
    payload_size: u32,
    #[br(parse_with = read_varlen_u64, args(len.timestamp_len as usize,))]
    pub timestamp: u64,
    #[br(args(entry_id, payload_size as usize,))]
    pub payload: Payload,
}

#[binread]
#[derive(Debug)]
#[br(magic = b"WPILOG")]
pub struct WpiLog {
    pub version: u16,
    #[br(temp)]
    len: u32,
    #[br(count = len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
    pub extra_header: String,
}

fn read_varlen_u32<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    (count,): (usize,),
) -> binrw::BinResult<u32> {
    let mut bytes = vec![0u8; count];
    reader.read_exact(&mut bytes)?;
    bytes.reverse();
    let mut actual_bytes = [0u8; 4];
    for i in 0..count {
        actual_bytes[i] = bytes[i];
    }
    actual_bytes.reverse();
    u32::read_options(&mut std::io::Cursor::new(actual_bytes), endian, ())
}

fn read_varlen_u64<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    (count,): (usize,),
) -> binrw::BinResult<u64> {
    let mut bytes = vec![0u8; count];
    reader.read_exact(&mut bytes)?;
    bytes.reverse();
    let mut actual_bytes = [0u8; 8];
    for i in 0..count {
        actual_bytes[i] = bytes[i];
    }
    actual_bytes.reverse();
    u64::read_options(&mut std::io::Cursor::new(actual_bytes), endian, ())
}
