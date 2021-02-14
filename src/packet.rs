use std::io::Read;

use crate::bitreader::ReadExtras;

#[derive(Clone, Copy, Debug)]
pub enum CmdType {
    SignOn = 1,
    Packet = 2,
    SyncTick = 3,
    ConsoleCmd = 4,
    UserCmd = 5,
    DataTables = 6,
    Stop = 7,
    CustomData = 8,
    StringTables = 9,
}

#[derive(Clone, Copy, Debug)]
pub struct PacketHeader {
    pub cmd_type: CmdType,
    pub tick: i32,
    pub player_slot: u8,
}

impl PacketHeader {
    pub fn new<R: Read>(reader: &mut R) -> PacketHeader {
        let cmd_type = reader.read_u8().unwrap();
        let tick = reader.read_i32().unwrap();
        let player_slot = reader.read_u8().unwrap();
        PacketHeader {
            cmd_type: match cmd_type {
                1 => CmdType::SignOn,
                2 => CmdType::Packet,
                3 => CmdType::SyncTick,
                4 => CmdType::ConsoleCmd,
                5 => CmdType::UserCmd,
                6 => CmdType::DataTables,
                7 => CmdType::Stop,
                8 => CmdType::CustomData,
                9 => CmdType::StringTables,
                other => panic!("Unexpected command type: {}", other),
            },
            tick,
            player_slot,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DemoCmdInfo {
    flags: i32,

    view_origin: (f32, f32, f32),
    view_angle: (f32, f32, f32),
    local_view_angles: (f32, f32, f32),

    view_origin2: (f32, f32, f32),
    view_angle2: (f32, f32, f32),
    local_view_angles2: (f32, f32, f32),
}

impl DemoCmdInfo {
    pub fn new<R: Read>(r: &mut R) -> DemoCmdInfo {
        DemoCmdInfo {
            flags: r.read_i32().unwrap(),
            view_origin: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
            view_angle: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
            local_view_angles: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
            view_origin2: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
            view_angle2: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
            local_view_angles2: (
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
                r.read_f32().unwrap(),
            ),
        }
    }
}
