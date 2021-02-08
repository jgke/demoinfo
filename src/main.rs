mod csgo;

use byteorder::{ByteOrder, LittleEndian};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::Mutex;
use prost::Message;

use crate::csgo::*;

trait BufReaderRaw {
    fn read_u8(&mut self) -> Option<u8>;
    fn read_u16(&mut self) -> Option<u16>;
    fn read_u32(&mut self) -> Option<u32>;
    fn read_i32(&mut self) -> Option<i32>;
    fn read_f32(&mut self) -> Option<f32>;
    fn read_var_u32(&mut self) -> Option<u32>;
    fn read_u8_vec(&mut self, size: usize) -> Option<Vec<u8>>;
}

impl<R: Read> BufReaderRaw for R {
    fn read_u8(&mut self) -> Option<u8> {
        let mut slice = [0u8; 1];
        self.read_exact(&mut slice).map(|_| slice[0]).ok()
    }
    fn read_u16(&mut self) -> Option<u16> {
        let mut slice = [0u8; 2];
        self.read_exact(&mut slice).map(|_| LittleEndian::read_u16(&slice)).ok()
    }
    fn read_u32(&mut self) -> Option<u32> {
        let mut slice = [0u8; 4];
        self.read_exact(&mut slice).map(|count| LittleEndian::read_u32(&slice)).ok()
    }
    fn read_i32(&mut self) -> Option<i32> {
        let mut slice = [0u8; 4];
        self.read_exact(&mut slice).map(|count| LittleEndian::read_i32(&slice)).ok()
    }
    fn read_f32(&mut self) -> Option<f32> {
        let mut slice = [0u8; 4];
        self.read_exact(&mut slice).map(|count| LittleEndian::read_f32(&slice)).ok()
    }
    fn read_var_u32(&mut self) -> Option<u32> {
        let mut res = 0;
        for byte in 0..=4 {
            let num = self.read_u8()?;
            res |= ((num as u32) & 0x7F) << (byte * 7);
            if num & 0b1000_0000 == 0 {
                return Some(res);
            }
        }
        Some(res)
    }
    fn read_u8_vec(&mut self, size: usize) -> Option<Vec<u8>> {
        let mut vec = vec![0; size];
        self.read_exact(&mut vec).map(|_| vec).ok()
    }
}

#[derive(Clone, Copy, Debug)]
enum CmdType {
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
struct PacketHeader {
    cmd_type: CmdType,
    tick: i32,
    player_slot: u8,
}

#[derive(Clone, Copy, Debug)]
struct DemoCmdInfo {
    flags: i32,

    view_origin: (f32, f32, f32),
    view_angle: (f32, f32, f32),
    local_view_angles: (f32, f32, f32),

    view_origin2: (f32, f32, f32),
    view_angle2: (f32, f32, f32),
    local_view_angles2: (f32, f32, f32),
}

#[derive(Clone, Debug)]
struct Header {
    magic: String,
    demo_protocol: i32,
    network_protocol: i32,
    server_name: String,
    client_name: String,
    map: String,
    directory: String,
    playback_time: f32,
    playback_ticks: i32,
    playback_frames: i32,
    signon_length: i32,
}

fn string_from_nilslice(s: &[u8]) -> String {
    String::from_utf8(
        s.iter()
            .copied()
            .take_while(|c| *c != 0)
            .collect::<Vec<u8>>(),
    )
    .unwrap()
}

impl Header {
    fn new<R: Read>(reader: &mut R) -> Header {
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

impl PacketHeader {
    fn new<R: Read>(reader: &mut R) -> PacketHeader {
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

impl DemoCmdInfo {
    fn new<R: Read>(r: &mut R) -> DemoCmdInfo {
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cmd {
    cmd: u32,
    data: Vec<u8>,
}

lazy_static! {
    static ref events: Mutex<HashMap<i32, (String, HashMap<i32, String>)>> = Mutex::new(HashMap::new());
}



impl Cmd {
    fn parse<R: Read>(r: &mut R) {
        loop {
            let cmd = r.read_var_u32();
            if cmd.is_none() {
                break;
            }
            let size = r.read_var_u32().unwrap();
            let data = r.read_u8_vec(size as usize).unwrap();

            match cmd.unwrap() {
                0 => { () }
                1 => { (netmessages_public::CnetMsgDisconnect::decode(&*data)).unwrap(); () }
                2 => { (netmessages_public::CnetMsgFile::decode(&*data)).unwrap(); () }
                4 => { (netmessages_public::CnetMsgTick::decode(&*data)).unwrap(); () }
                5 => { (netmessages_public::CnetMsgStringCmd::decode(&*data)).unwrap(); () }
                6 => { (netmessages_public::CnetMsgSetConVar::decode(&*data)).unwrap(); () }
                7 => { (netmessages_public::CnetMsgSignonState::decode(&*data)).unwrap(); () }
                8 => { (netmessages_public::CsvcMsgServerInfo::decode(&*data)).unwrap(); () }
                9 => { (netmessages_public::CsvcMsgSendTable::decode(&*data)).unwrap(); () }
                10 => { (netmessages_public::CsvcMsgClassInfo::decode(&*data)).unwrap(); () }
                11 => { (netmessages_public::CsvcMsgSetPause::decode(&*data)).unwrap(); () }
                12 => { (netmessages_public::CsvcMsgCreateStringTable::decode(&*data)).unwrap(); () }
                13 => { (netmessages_public::CsvcMsgUpdateStringTable::decode(&*data)).unwrap(); () }
                14 => { (netmessages_public::CsvcMsgVoiceInit::decode(&*data)).unwrap(); () }
                15 => { (netmessages_public::CsvcMsgVoiceData::decode(&*data)).unwrap(); () }
                16 => { (netmessages_public::CsvcMsgPrint::decode(&*data)).unwrap(); () }
                17 => { (netmessages_public::CsvcMsgSounds::decode(&*data)).unwrap(); () }
                18 => { (netmessages_public::CsvcMsgSetView::decode(&*data)).unwrap(); () }
                19 => { (netmessages_public::CsvcMsgFixAngle::decode(&*data)).unwrap(); () }
                20 => { (netmessages_public::CsvcMsgCrosshairAngle::decode(&*data)).unwrap(); () }
                21 => { (netmessages_public::CsvcMsgBspDecal::decode(&*data)).unwrap(); () }
                23 => { (netmessages_public::CsvcMsgUserMessage::decode(&*data)).unwrap(); () }
                25 => {
                    let ev = netmessages_public::CsvcMsgGameEvent::decode(&*data).unwrap();
                    if let Some((name, key_data)) = ev.eventid.and_then(|id| events.lock().unwrap().get(&id).cloned()) {
                        if name != "player_footstep" {
                            println!("{}", name);
                            for key in ev.keys {
                                if let Some(id) = key.r#type {
                                    println!("- {}", key_data[&id]);
                                }
                            }
                        }
                        //dbg!(name, ev);
                    }
                    ()
                }
                26 => { (netmessages_public::CsvcMsgPacketEntities::decode(&*data)).unwrap(); () }
                27 => { (netmessages_public::CsvcMsgTempEntities::decode(&*data)).unwrap(); () }
                28 => { (netmessages_public::CsvcMsgPrefetch::decode(&*data)).unwrap(); () }
                29 => { (netmessages_public::CsvcMsgMenu::decode(&*data)).unwrap(); () }
                30 => {
                    let list = netmessages_public::CsvcMsgGameEventList::decode(&*data).unwrap();
                    for event in list.descriptors {
                        dbg!(&event);
                        if let (Some(id), Some(name)) = (event.eventid, event.name) {
                            let inner: HashMap<i32, String> = event.keys
                                .into_iter()
                                .map(|key| if let (Some(id), Some(name)) = (key.r#type, key.name) {
                                    Some((id, name))
                                } else {
                                    None
                                })
                            .flatten()
                            .collect();
                            events.lock().unwrap().insert(id, (name, inner));
                        }
                    }
                    ()
                }
                31 => { (netmessages_public::CsvcMsgGetCvarValue::decode(&*data)).unwrap(); () }

                other => unimplemented!("{}", other)
            }
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(&args[1])?;
    let mut reader = BufReader::new(file);
    let header = Header::new(&mut reader);
    dbg!(&header);

    loop {
        let header = PacketHeader::new(&mut reader);
        match header.cmd_type {
            CmdType::SyncTick => {
                println!("SyncTick");
                continue;
            }
            CmdType::Stop => {
                println!("Demo finished");
                break;
            }
            CmdType::SignOn | CmdType::Packet => {
                let split1 = DemoCmdInfo::new(&mut reader);
                let split2 = DemoCmdInfo::new(&mut reader);

                reader.read_u32().unwrap();
                reader.read_u32().unwrap();

                let size: u32 = reader.read_u32().unwrap();
                let slice = reader.read_u8_vec(size as usize).unwrap();
                Cmd::parse(&mut (*slice).as_ref());
            }
            CmdType::ConsoleCmd => unimplemented!(),
            CmdType::UserCmd => unimplemented!(),
            CmdType::DataTables => {
                let size: u32 = reader.read_u32().unwrap();
                let slice = reader.read_u8_vec(size as usize).unwrap();
            }
            CmdType::CustomData => unimplemented!(),
            CmdType::StringTables => unimplemented!(),
            //cmd => {
            //}
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn header_parse() {
        let data = include_bytes!("example_header");
        let header = Header::new(&mut data.as_ref());
        assert_eq!("HL2DEMO", header.magic);
        assert_eq!(4, header.demo_protocol);
        assert_eq!(13769, header.network_protocol);
        assert_eq!("Kanaliiga #2", header.server_name);
        assert_eq!("GOTV Demo", header.client_name);
        assert_eq!("de_vertigo", header.map);
        assert_eq!("csgo", header.directory);
        assert_eq!(2179.953125, header.playback_time);
        assert_eq!(279034, header.playback_ticks);
        assert_eq!(139406, header.playback_frames);
        assert_eq!(447407, header.signon_length);
    }

    #[test]
    fn eof() {
        let data = &[];
        Cmd::parse(&mut data.as_ref());
    }

    #[test]
    fn read_var() {
        assert_eq!(None, (&mut (&[]).as_ref()).read_var_u32());
        assert_eq!(Some(1), (&mut (&[1]).as_ref()).read_var_u32());
        assert_eq!(None, (&mut (&[255]).as_ref()).read_var_u32());
        assert_eq!(Some(4), (&mut (&[4]).as_ref()).read_var_u32());
        assert_eq!(Some(2226), (&mut (&[178, 17]).as_ref()).read_var_u32());
    }
}
