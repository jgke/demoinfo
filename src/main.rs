mod bitreader;
mod cmd;
mod csgo;
mod header;
mod packet;
mod playerinfo;
mod stringtables;

use prost::Message;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::BufReader;

use crate::bitreader::*;
use crate::cmd::Cmd;
use crate::csgo::*;
use crate::header::Header;
use crate::netmessages_public::csvc_msg_game_event::KeyT;
use crate::packet::{CmdType, DemoCmdInfo, PacketHeader};
use crate::playerinfo::PlayerInfo;
use crate::stringtables::create_string_table;

#[derive(Clone, Debug)]
struct Player {
    name: String,

    kills: i32,
    assists: i32,
    flash_assists: i32,
    deaths: i32,

    equipped: String,

    info: PlayerInfo,
}

impl Player {
    pub fn new(info: PlayerInfo) -> Player {
        Player {
            name: info.name.clone(),
            kills: 0,
            assists: 0,
            flash_assists: 0,
            deaths: 0,
            equipped: "knife".to_string(),
            info,
        }
    }
}

#[derive(Clone, Debug)]
struct State {
    events: HashMap<i32, (String, HashMap<usize, String>)>,
    players: HashMap<i32, Player>,
    teams: HashMap<i32, bool>,
}

#[rustfmt::skip]
fn show_key(k: &KeyT) -> String {
    match k {
        KeyT { val_string: Some(s), .. } => format!("[string] {}", s),
        KeyT { val_float: Some(n), .. } => format!("[float] {}", n),
        KeyT { val_long: Some(n), .. } => format!("[long] {}", n),
        KeyT { val_short: Some(n), .. } => format!("[short] {}", n),
        KeyT { val_byte: Some(n), .. } => format!("[byte] {}", n),
        KeyT { val_bool: Some(n), .. } => format!("[bool] {}", n),
        KeyT { val_uint64: Some(n), .. } => format!("[u64] {}", n),
        KeyT { val_wstring: Some(n), .. } => format!("[wstring] {}", String::from_utf8_lossy(&n).to_string()),
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

    // See: protos/cstrike15_usermessages_public.proto::ECstrike15UserMessages
    if cmd == 6 {
        let _msg = cstrike15_usermessages_public::CcsUsrMsgSayText2::decode(data).unwrap();
        //dbg!(msg);
    }
}

fn read_event_names(
    list: netmessages_public::CsvcMsgGameEventList,
) -> HashMap<i32, (String, HashMap<usize, String>)> {
    let mut result = HashMap::new();

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
            result.insert(id, (name, inner));
        }
    }

    result
}

impl State {
    pub fn new() -> State {
        State {
            events: HashMap::new(),
            players: HashMap::new(),
            teams: HashMap::new(),
        }
    }

    pub fn handle_command(&mut self, cmd: Cmd) -> std::io::Result<()> {
        match cmd {
            Cmd::CreateStringTable(table) => {
                if let Some(table) = create_string_table(table)? {
                    for (i, info) in table {
                        self.players.insert(i, Player::new(info));
                    }
                }
            }
            Cmd::UserMessage(message) => {
                handle_user_message(message);
            }
            Cmd::GameEvent(event) => {
                self.handle_game_event(event);
            }
            Cmd::GameEventList(event_list) => {
                self.events = read_event_names(event_list);
            }
        }
        Ok(())
    }

    fn handle_game_event(&mut self, ev: netmessages_public::CsvcMsgGameEvent) {
        let ignored = [
            "player_footstep",
            "weapon_fire",
            "weapon_reload",
            "player_hurt",
            "item_pickup",
        ];
        if let Some((name, key_data)) = ev.eventid.and_then(|id| self.events.get(&id)) {
            if ignored.contains(&name.as_str()) {
                return;
            }
            //println!("{}", name);
            if name == "round_announce_match_start" {
                self.clear_stats();
            } else if name == "item_equip" {
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
                    //dbg!(&self.players.get(&userid));
                    if let Some(player) = self.players.get_mut(&userid) {
                        player.equipped = item;
                    }
                }
            } else if name == "player_spawn" {
                let mut userid = None;
                let mut teamnum = None;
                for (i, key) in ev.keys.iter().enumerate() {
                    let key_name = &key_data[&i];
                    //println!("- {} = {}", key_name, show_key(&key));
                    if key_name == "userid" {
                        userid = key.val_short;
                    } else if key_name == "teamnum" {
                        teamnum = key.val_short;
                    }
                }
                if let (Some(userid), Some(teamnum)) = (userid, teamnum) {
                    if teamnum == 2 || teamnum == 3 {
                        self.teams.insert(userid, teamnum == 2);
                    }
                }
            } else if name == "player_death" {
                //println!("\n{}", name);
                let mut userid = None;
                let mut attackerid = None;
                let mut assisterid = None;
                let mut assisterflash = None;
                for (i, key) in ev.keys.iter().enumerate() {
                    let key_name = &key_data[&i];
                    //println!("- {} = {}", key_name, show_key(&key));
                    if key_name == "userid" {
                        userid = key.val_short;
                    } else if key_name == "attacker" {
                        attackerid = key.val_short;
                    } else if key_name == "assister" {
                        assisterid = key.val_short;
                    } else if key_name == "assistedflash" {
                        assisterflash = key.val_bool;
                    }
                }
                if let Some(id) = userid {
                    if let Some(muna) = self.muna_in_hand(id) {
                        println!("{}, (muna in hand = {})", self.players[&id].name, muna);
                    }
                }
                if let (Some(kill), Some(death), assist) =
                    (attackerid, userid, assisterid.unwrap_or(0))
                {
                    self.update_stats(kill, death, assist, assisterflash);
                } else {
                    panic!("{:?} {:?} {:?}", attackerid, userid, assisterid);
                }
            } else {
                //println!("{}", name);
                //for (i, key) in ev.keys.iter().enumerate() {
                //    let key_name = &key_data[&i];
                //    //if key_name == "item" {
                //    //    item = Some(show_key(&key));
                //    //} else if key_name == "userid" {
                //    //    userid = key.val_short;
                //    //}
                //    println!("- {} = {}", key_name, show_key(&key));
                //}
            }
        } else if let Some(_name) = ev.event_name {
            //println!("{}", name);
            //for key in ev.keys {
            //    println!("- {:?} = {}", key, show_key(&key));
            //}
        } else {
            dbg!(&ev);
        }
    }

    fn clear_stats(&mut self) {
        for (_, player) in self.players.iter_mut() {
            player.kills = 0;
            player.deaths = 0;
            player.assists = 0;
            player.flash_assists = 0;
        }
    }

    pub fn update_stats(
        &mut self,
        mut kill: i32,
        death: i32,
        assist: i32,
        assistflash: Option<bool>,
    ) {
        if kill == 0 {
            kill = death;
            //println!("{} ate it", &self.players[&death].name);
        } else if false {
            if assist > 0 {
                println!(
                    "{} killed {} with {}assist from {}",
                    &self.players[&kill].name,
                    &self.players[&death].name,
                    if assistflash == Some(true) {
                        "flash "
                    } else {
                        ""
                    },
                    &self.players[&assist].name
                );
            } else {
                println!(
                    "{} killed {}",
                    &self.players[&kill].name, &self.players[&death].name
                );
            }
        }
        //println!("");

        let killer_team = self.teams[&kill];
        let victim_team = self.teams[&death];
        let assist_team = if assist > 0 {
            Some(self.teams[&assist])
        } else {
            None
        };

        if kill == death || killer_team == victim_team {
            // self.players.get_mut(&kill).unwrap().kills -= 1;
        } else {
            self.players.get_mut(&kill).unwrap().kills += 1;
        }

        //dbg!(assist, assistflash, assist_team);
        if let Some(assist_team) = assist_team {
            let assister = self.players.get_mut(&assist).unwrap();
            match (assistflash, assist_team == victim_team) {
                (Some(true), true) => assister.flash_assists -= 1,
                (Some(true), false) => {} //assister.flash_assists += 1,
                (_, true) => assister.assists -= 1,
                (_, false) => assister.assists += 1,
            }
        }

        self.players.get_mut(&death).unwrap().deaths += 1;
    }

    fn muna_in_hand(&self, id: i32) -> Option<String> {
        let item = self.players.get(&id).map(|p| &p.equipped);
        let munas = [
            "hegrenade",
            "incgrenade",
            "smokegrenade",
            "flashbang",
            "molotov",
        ];
        item.filter(|i| munas.contains(&i.as_ref())).cloned()
    }

    pub fn print_stats(&self) {
        //dbg!(&self.teams);
        //dbg!(&self.players);
        let mut player_list = self
            .players
            .iter()
            .filter(|(id, _)| self.teams.contains_key(id))
            .map(|(id, player)| (self.teams[id], player))
            .collect::<Vec<_>>();
        player_list.sort_by_key(|(team, player)| (*team, -player.kills));
        for (_, player) in player_list {
            println!(
                "{:16} (k/a/d {:3} {:3} {:3} ({} f))",
                player.name, player.kills, player.assists, player.deaths, player.flash_assists
            );
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(&args[1])?;
    let mut reader = BufReader::new(file);
    let _header = Header::new(&mut reader);

    let mut state = State::new();

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
                let mut read = (*slice).as_ref();
                while let Some(cmd) = Cmd::parse(&mut read) {
                    state.handle_command(cmd)?;
                }
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

    state.print_stats();

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
