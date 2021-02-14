mod bitreader;
mod csgo;
mod header;
mod packet;

use lazy_static::lazy_static;
use prost::Message;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::Mutex;

use crate::bitreader::*;
use crate::csgo::*;
use crate::netmessages_public::csvc_msg_game_event::KeyT;
use crate::header::Header;
use crate::packet::{DemoCmdInfo, PacketHeader, CmdType};

#[allow(dead_code)]
struct PlayerInfo {
    pub version: u64,
    pub xuid: u64,
    pub name: String,
    pub user_id: i32,
    pub guid: String,
    pub friends_id: u32,
    pub friends_name: String,
    pub fake: bool,
    pub proxy: bool,
    pub custom_files_crc: [u32; 4],
    pub files_downloaded: u8,
    pub entity_id: i64,
}

impl PlayerInfo {
    fn new(entry_index: i64, buf: &[u8]) -> std::io::Result<PlayerInfo> {
        let buf_ptr = &mut buf.as_ref();
        let mut buf = BitReader::new(buf_ptr);

        let version = buf.read_u64_be()?;
        let xuid = buf.read_u64_be()?;
        let name = buf.read_fixed_c_string(128)?;
        let user_id = buf.read_i32_be()?;
        let guid = buf.read_fixed_c_string(32)?;
        let friends_id = buf.read_u32_be()?;
        let friends_name = buf.read_fixed_c_string(128)?;
        let fake = buf.read_u8()?;
        let proxy = buf.read_u8()?;
        let custom_files_crc = [
            buf.read_u32_be().unwrap(),
            buf.read_u32_be().unwrap(),
            buf.read_u32_be().unwrap(),
            buf.read_u32_be().unwrap(),
        ];
        let files_downloaded = buf.read_u8().unwrap();

        let entity_id = entry_index;

        Ok(PlayerInfo {
            version,
            xuid,
            name,
            user_id,
            guid,
            friends_id,
            friends_name,
            fake: fake != 0,
            proxy: proxy != 0,
            custom_files_crc,
            files_downloaded,
            entity_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cmd {
    pub cmd: u32,
    pub data: Vec<u8>,
}

impl Cmd {
    fn parse<R: Read>(r: &mut R) {
        loop {
            let cmd = r.read_var_u32();
            if cmd.is_err() {
                break;
            }
            let size = r.read_var_u32().unwrap();
            let data = r.read_u8_vec(size as usize).unwrap();

            match cmd.unwrap() {
                // 0 => { () }
                // 1 => { (netmessages_public::CnetMsgDisconnect::decode(&*data)).unwrap(); () }
                // 2 => { (netmessages_public::CnetMsgFile::decode(&*data)).unwrap(); () }
                // 4 => { (netmessages_public::CnetMsgTick::decode(&*data)).unwrap(); () }
                // 5 => { (netmessages_public::CnetMsgStringCmd::decode(&*data)).unwrap(); () }
                // 6 => { (netmessages_public::CnetMsgSetConVar::decode(&*data)).unwrap(); () }
                // 7 => { (netmessages_public::CnetMsgSignonState::decode(&*data)).unwrap(); () }
                // 8 => { (netmessages_public::CsvcMsgServerInfo::decode(&*data)).unwrap(); () }
                // 9 => { (netmessages_public::CsvcMsgSendTable::decode(&*data)).unwrap(); () }
                // 10 => { (netmessages_public::CsvcMsgClassInfo::decode(&*data)).unwrap(); () }
                // 11 => { (netmessages_public::CsvcMsgSetPause::decode(&*data)).unwrap(); () }
                12 => {
                    let ev = netmessages_public::CsvcMsgCreateStringTable::decode(&*data).unwrap();
                    create_string_table(ev).unwrap();
                }

                // 13 => { (netmessages_public::CsvcMsgUpdateStringTable::decode(&*data)).unwrap(); () }
                // 14 => { (netmessages_public::CsvcMsgVoiceInit::decode(&*data)).unwrap(); () }
                // 15 => { (netmessages_public::CsvcMsgVoiceData::decode(&*data)).unwrap(); () }
                // 16 => { (netmessages_public::CsvcMsgPrint::decode(&*data)).unwrap(); () }
                // 17 => { (netmessages_public::CsvcMsgSounds::decode(&*data)).unwrap(); () }
                // 18 => { (netmessages_public::CsvcMsgSetView::decode(&*data)).unwrap(); () }
                // 19 => { (netmessages_public::CsvcMsgFixAngle::decode(&*data)).unwrap(); () }
                // 20 => { (netmessages_public::CsvcMsgCrosshairAngle::decode(&*data)).unwrap(); () }
                // 21 => { (netmessages_public::CsvcMsgBspDecal::decode(&*data)).unwrap(); () }
                23 => {
                    let msg = netmessages_public::CsvcMsgUserMessage::decode(&*data).unwrap();
                    handle_user_message(msg);
                }
                25 => {
                    let ev = netmessages_public::CsvcMsgGameEvent::decode(&*data).unwrap();
                    handle_game_event(ev);
                }

                // 26 => { (netmessages_public::CsvcMsgPacketEntities::decode(&*data)).unwrap(); () }
                // 27 => { (netmessages_public::CsvcMsgTempEntities::decode(&*data)).unwrap(); () }
                // 28 => { (netmessages_public::CsvcMsgPrefetch::decode(&*data)).unwrap(); () }
                // 29 => { (netmessages_public::CsvcMsgMenu::decode(&*data)).unwrap(); () }

                30 => {
                    let list = netmessages_public::CsvcMsgGameEventList::decode(&*data).unwrap();
                    read_event_names(list);
                }

                // 31 => { (netmessages_public::CsvcMsgGetCvarValue::decode(&*data)).unwrap(); () }
                _other => {} // other => unimplemented!("{}", other)
            }
        }
    }
}


lazy_static! {
    static ref EVENTS: Mutex<HashMap<i32, (String, HashMap<usize, String>)>> =
        Mutex::new(HashMap::new());
    static ref NAMES: Mutex<HashMap<i32, String>> = Mutex::new(HashMap::new());
    static ref EQUIPS: Mutex<HashMap<i32, String>> = Mutex::new(HashMap::new());
    static ref PLAYERS: Mutex<HashMap<i32, String>> = Mutex::new(HashMap::new());
}

fn equip(id: i32, item: String) {
    EQUIPS.lock().unwrap().insert(id, item);
}

fn muna_in_hand(id: i32) -> Option<String> {
    let equips = EQUIPS.lock().unwrap();
    let item = equips.get(&id)?;
    let munas = [
        "hegrenade",
        "incgrenade",
        "smokegrenade",
        "flashbang",
        "molotov",
    ];
    Some(item).filter(|i| munas.contains(&i.as_ref())).cloned()
}

#[rustfmt::skip]
fn show_key(k: &KeyT) -> String {
    match k {
        KeyT { val_string: Some(s), .. } => s.clone(),
        KeyT { val_float: Some(n), .. } => n.to_string(),
        KeyT { val_long: Some(n), .. } => n.to_string(),
        KeyT { val_short: Some(n), .. } => n.to_string(),
        KeyT { val_byte: Some(n), .. } => n.to_string(),
        KeyT { val_bool: Some(n), .. } => n.to_string(),
        KeyT { val_uint64: Some(n), .. } => n.to_string(),
        KeyT { val_wstring: Some(n), .. } => String::from_utf8_lossy(&n).to_string(),
        KeyT {
            val_string: None,
            val_float: None,
            val_long: None,
            val_short: None,
            val_byte: None,
            val_bool: None,
            val_uint64: None,
            val_wstring: None,
            ..
        } => "[empty]".to_string()
    }
}

fn handle_user_message(msg: netmessages_public::CsvcMsgUserMessage) {
    let data: &[u8] = &msg.msg_data.unwrap();
    let cmd = msg.msg_type.unwrap();

    match cmd {
        6 => {
            let msg = cstrike15_usermessages_public::CcsUsrMsgSayText2::decode(data).unwrap();
            dbg!(msg);
        }
        _ => {}
    }
}

fn handle_game_event(ev: netmessages_public::CsvcMsgGameEvent) {
    let ignored = [
        "player_footstep",
        "weapon_fire",
        "weapon_reload",
        "player_hurt",
        "item_pickup",
    ];
    if let Some((name, key_data)) = ev.eventid.and_then(|id| EVENTS.lock().unwrap().get(&id).cloned()) {
        if ignored.contains(&name.as_str()) {
            return;
        }
        if name == "item_equip" {
            let mut item = None;
            let mut userid = None;
            for (i, key) in ev.keys.iter().enumerate() {
                let key_name = &key_data[&i];
                if key_name == "item" {
                    item = Some(show_key(&key));
                } else if key_name == "userid" {
                    userid = key.val_short;
                }
                //println!("- {} = {}", key_name, show_key(&key));
            }
            if let (Some(item), Some(userid)) = (item, userid) {
                equip(userid, item);
            }
        } else if name == "player_death" {
            let mut userid = None;
            for (i, key) in ev.keys.iter().enumerate() {
                let key_name = &key_data[&i];
                //println!("- {} = {}", key_name, show_key(&key));
                if key_name == "userid" {
                    userid = key.val_short;
                }
            }
            if let Some(id) = userid {
                if let Some(muna) = muna_in_hand(id) {
                    println!(
                        "{}, (muna in hand = {})",
                        PLAYERS.lock().unwrap()[&id],
                        muna
                        );
                }
            }
        } else if name == "player_info"
            || name == "player_connect"
                || name == "player_connect_full"
                {
                    for (i, key) in ev.keys.iter().enumerate() {
                        let key_name = &key_data[&i];
                        //if key_name == "item" {
                        //    item = Some(show_key(&key));
                        //} else if key_name == "userid" {
                        //    userid = key.val_short;
                        //}
                        println!("- {} = {}", key_name, show_key(&key));
                    }
                } else {
                    dbg!(&name);
                    /*
                       for (i, key) in ev.keys.iter().enumerate() {
                       let key_name = &key_data[&i];
                       if key_name == "item" {
                       item = Some(show_key(&key));
                       } else if key_name == "userid" {
                       userid = key.val_short;
                       }
                       println!("- {} = {}", key_name, show_key(&key));
                       }
                       */
                }
    } else if let Some(name) = ev.event_name {
        println!("{}", name);
        for key in ev.keys {
            println!("- {:?} = {}", key, show_key(&key));
        }
    } else {
        dbg!(&ev);
    }
}

fn read_event_names(list: netmessages_public::CsvcMsgGameEventList) {
    for event in list.descriptors {
        if let (Some(id), Some(name)) = (event.eventid, event.name) {
            let inner: HashMap<usize, String> = event
                .keys
                .into_iter()
                .enumerate()
                .map(|(i, key)| {
                    if let Some(name) = key.name {
                        Some((i, name))
                    } else {
                        None
                    }
                })
            .flatten()
                .collect();
            EVENTS.lock().unwrap().insert(id, (name, inner));
        }
    }
}

fn create_string_table(msg: netmessages_public::CsvcMsgCreateStringTable) -> std::io::Result<()> {
    let name = msg.name.unwrap();
    if name != "userinfo" {
        return Ok(())
    }
    let max_entries = msg.max_entries.unwrap();
    let num_entries = msg.num_entries.unwrap();
    let user_data_fixed_size = msg.user_data_fixed_size.filter(|n| *n).is_some();
    let user_data_size = msg.user_data_size.filter(|n| *n != 0);
    let user_data_size_bits = msg.user_data_size_bits.filter(|n| *n != 0);

    if user_data_fixed_size {
        assert!(user_data_size.is_some());
        assert!(user_data_size_bits.is_some());
    }

    let _flags = msg.flags.unwrap();
    let string_data: &[u8] = &msg.string_data.unwrap();
    let reader_buf = &mut string_data.as_ref();
    let mut reader = BitReader::new(reader_buf);

    let entry_bits = (max_entries as f64).log2().ceil() as usize;
    let mut entries: HashMap<i64, (Vec<u8>, Vec<u8>)> = HashMap::new();
    let mut history: VecDeque<Vec<u8>> = VecDeque::new();

    assert!(!reader.read_bit()?, "Dictionary encoding unsupported");

    let mut entry_index = -1;
    for _i in 0..num_entries {
        entry_index += 1;
        if !reader.read_bit()? {
            entry_index = reader.read_bits_u32(entry_bits as u8)? as i64;
        }

        assert!(entry_index >= 0 && entry_index < (max_entries as i64));

        let entry: Vec<u8>;
        let mut userdata: Vec<u8> = Vec::new();

        if reader.read_bit()? {
            // substring check
            if reader.read_bit()? {
                let index = reader.read_bits_u32(5)? as usize;
                assert!(index < history.len(), "History index too large");
                let bytes_to_copy = reader.read_bits_u32(5)? as usize;

                let last = &history[index];

                let substr = last.split_at(bytes_to_copy).0;
                let suffix = reader.read_c_string()?;

                entry = substr.iter().chain(suffix.iter()).copied().collect();
            } else {
                entry = reader.read_c_string()?;
            }
        } else {
            // If the string itself hasn't changed, this entry must already exist
            let tuple = entries
                .get(&entry_index)
                .cloned()
                .unwrap_or((vec![], vec![]));
            entry = tuple.0;
            userdata = tuple.1;
        }

        if reader.read_bit()? {
            // don't read the length, it's fixed length and the length was networked down already
            if user_data_fixed_size {
                userdata = vec![reader
                    .read_bits_u32(user_data_size_bits.unwrap() as u8)?
                    .try_into()
                    .unwrap()];
            } else {
                let bytes = reader.read_bits_u32(14)? as usize;
                let mut buf = vec![0; bytes];
                reader.read_exact(&mut buf)?;
                userdata = buf;
            }

            if name == "userinfo" {
                let info = PlayerInfo::new(entry_index, &userdata)?;
                PLAYERS.lock().unwrap().insert(info.user_id, info.name);
            }
        }

        entries.insert(entry_index, (entry.clone(), userdata));

        // add to history
        if history.len() > 31 {
            history.pop_front();
        }

        history.push_back(entry);
    }

    // // parse client-side entries
    // if reader.read_bit()? {
    //   let numStrings = reader.read_u16().unwrap();

    //   for i in 0..numStrings {
    //     let entry = reader.read_c_string();

    //     if reader.read_bit()? {
    //       let userDataSize = reader.read_u16().unwrap();
    //       let mut buf = vec![0; userDataSize.into()];
    //       reader.read_exact(&mut buf)?;

    //       dbg!(String::from_utf8_lossy(&buf));
    //       unimplemented!();
    //       // tslint:disable-next-line no-dead-store
    //       //userData =
    //       //  userDataCallback === undefined
    //       //    ? userDataBuf
    //       //    : userDataCallback(userDataBuf);
    //     }

    //     // TODO: do something with client-side entries
    //   }
    // }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(&args[1])?;
    let mut reader = BufReader::new(file);
    let _header = Header::new(&mut reader);

    loop {
        let header = PacketHeader::new(&mut reader);
        match header.cmd_type {
            CmdType::SyncTick => {
                continue;
            }
            CmdType::Stop => {
                break;
            }
            CmdType::SignOn | CmdType::Packet => {
                let _split1 = DemoCmdInfo::new(&mut reader);
                let _split2 = DemoCmdInfo::new(&mut reader);

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
                let _slice = reader.read_u8_vec(size as usize).unwrap();
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
        assert_eq!(None, (&mut (&[]).as_ref()).read_var_u32().ok());
        assert_eq!(Some(1), (&mut (&[1]).as_ref()).read_var_u32().ok());
        assert_eq!(None, (&mut (&[255]).as_ref()).read_var_u32().ok());
        assert_eq!(Some(4), (&mut (&[4]).as_ref()).read_var_u32().ok());
        assert_eq!(Some(2226), (&mut (&[178, 17]).as_ref()).read_var_u32().ok());
    }
}
