use std::io::Read;

use crate::bitreader::{string_from_nilslice, ReadExtras};

#[derive(Clone, Debug)]
pub struct Header {
    pub magic: String,
    pub demo_protocol: i32,
    pub network_protocol: i32,
    pub server_name: String,
    pub client_name: String,
    pub map: String,
    pub directory: String,
    pub playback_time: f32,
    pub playback_ticks: i32,
    pub playback_frames: i32,
    pub signon_length: i32,
}

impl Header {
    pub fn new<R: Read>(reader: &mut R) -> Header {
        let magic = string_from_nilslice(&reader.read_u8_vec(8).unwrap());
        let demo_protocol = reader.read_i32().unwrap();
        let network_protocol = reader.read_i32().unwrap();
        let server_name = string_from_nilslice(&reader.read_u8_vec(260).unwrap());
        let client_name = string_from_nilslice(&reader.read_u8_vec(260).unwrap());
        let map = string_from_nilslice(&reader.read_u8_vec(260).unwrap());
        let directory = string_from_nilslice(&reader.read_u8_vec(260).unwrap());
        let playback_time = reader.read_f32().unwrap();
        let playback_ticks = reader.read_i32().unwrap();
        let playback_frames = reader.read_i32().unwrap();
        let signon_length = reader.read_i32().unwrap();
        let header = Header {
            magic,
            demo_protocol,
            network_protocol,
            server_name,
            client_name,
            map,
            directory,
            playback_time,
            playback_ticks,
            playback_frames,
            signon_length,
        };

        assert_eq!(header.magic, "HL2DEMO");
        assert_eq!(header.demo_protocol, 4);

        header
    }
}
