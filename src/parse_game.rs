use prost::Message;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::Read;

use log::{debug, info, trace, warn};

use crate::bitreader::*;
use crate::cmd::Cmd;
use crate::csgo::netmessages_public::csvc_msg_game_event::KeyT;
use crate::csgo::*;
use crate::game_event::{Event, EventContext};
use crate::header::Header;
use crate::packet::{CmdType, DemoCmdInfo, PacketHeader};
use crate::player::Player;
use crate::stringtables::{create_string_table, update_string_table, StringTable};

const TRADE_TIME_LIMIT_IN_SECONDS: f32 = 18.2;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerState {
    Killed,
    Assisted,
    Survived,
    Traded,
    Died(i32, i32),
}

#[derive(Clone, Debug)]
struct State {
    header: Header,
    player_table: Option<StringTable>,
    table_id: usize,
    current_tick: i32,
    current_round: i32,
    score: (i32, i32),

    events: EventContext,
    players: HashMap<i32, Player>,
    current_round_player_state: HashMap<i32, PlayerState>,
    teams: HashMap<i32, bool>,
}

#[rustfmt::skip]
#[allow(dead_code)]
pub fn show_key(k: &KeyT) -> String {
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

fn as_seconds<N>(ticks: N, tickrate: i32) -> f32
where
    i64: TryFrom<N>,
{
    (i64::try_from(ticks).ok().unwrap() as f32) / tickrate as f32
}

impl State {
    pub fn new(header: Header) -> State {
        State {
            header,
            player_table: None,
            table_id: 0,
            current_tick: 0,
            current_round: 0,
            score: (0, 0),

            events: EventContext::new(HashMap::new()),
            players: HashMap::new(),
            current_round_player_state: HashMap::new(),
            teams: HashMap::new(),
        }
    }

    fn find_player_by_xuid(&self, xuid: i64) -> Option<i32> {
        self.players
            .iter()
            .filter_map(|(i, p)| if p.info.xuid == xuid { Some(*i) } else { None })
            .next()
    }

    pub fn current_time(&self) -> String {
        let second = self.current_tick / self.header.tickrate();

        format!("{}m {}s", second / 60, second % 60)
    }

    pub fn handle_command(&mut self, cmd: Cmd) -> std::io::Result<()> {
        match cmd {
            Cmd::CreateStringTable(table) => {
                if let Some((table, players)) = create_string_table(table)? {
                    self.player_table = Some(table);
                    for (i, info) in players {
                        self.players.insert(i, Player::new(info));
                    }
                } else if self.player_table.is_none() {
                    self.table_id += 1;
                }
            }
            Cmd::UpdateStringTable(table) => {
                if table.table_id == Some(self.table_id as i32) {
                    for (i, info) in
                        update_string_table(self.player_table.as_mut().unwrap(), table)?
                    {
                        if let Some(p) = self.find_player_by_xuid(info.xuid) {
                            let mut player = self.players.remove(&p).unwrap();
                            player.info = info;
                            self.players.insert(i, player);
                        } else {
                            self.players.insert(i, Player::new(info));
                        }
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
                self.events = EventContext::new(read_event_names(event_list));
            }
        }
        Ok(())
    }

    fn handle_round_start(&mut self) {
        self.current_round += 1;

        debug!("--\nRound {}\n{}", self.current_round, self.current_time());
        if self.current_round == 16 {
            debug!("Swapping sides");
            self.score = (self.score.1, self.score.0);
        }
    }

    fn update_player_kast_score(&mut self) {
        debug!("{} {:?}", 50, &self.current_round_player_state[&50]);
        debug!("{} {:?}", 56, &self.current_round_player_state[&56]);
        for (i, state) in &self.current_round_player_state {
            match state {
                PlayerState::Killed
                | PlayerState::Assisted
                | PlayerState::Survived
                | PlayerState::Traded => {
                    self.players.get_mut(&i).unwrap().kast += 1;
                }
                PlayerState::Died(_, _) => {}
            }
        }
    }

    fn clear_kast(&mut self) {
        self.current_round_player_state = self
            .players
            .keys()
            .map(|k| (*k, PlayerState::Survived))
            .collect();
    }

    fn handle_game_event(&mut self, ev: netmessages_public::CsvcMsgGameEvent) {
        match self.events.parse_game_event(ev) {
            Event::Filtered => {}
            Event::BeginNewMatch => self.clear_stats(),
            Event::RoundStart => self.handle_round_start(),
            Event::RoundOfficiallyEnded => {
                self.update_player_kast_score();
                self.clear_kast();
            }
            Event::RoundEnd(winner) => {
                if winner {
                    debug!("T win");
                    self.score.0 += 1;
                } else {
                    debug!("CT win");
                    self.score.1 += 1;
                }
                if self.score.0 == 16 || self.score.1 == 16 {
                    self.update_player_kast_score();
                }
                debug!("Score: {:?}", self.score);
            }
            Event::ItemEquip(userid, item) => {
                self.equip(userid, item);
            }
            Event::PlayerSpawn(userid, team) => {
                self.teams.insert(userid, team);
            }
            Event::PlayerDeath {
                victim,
                killer,
                assist,
                flash_assist,
                weapon,
            } => {
                if let Some((muna, tick)) = self.muna_in_hand(victim) {
                    debug!(
                        "{}, (muna in hand = {}, age = {:.1}s)",
                        self.players[&victim].name, muna, tick
                    );
                }
                self.update_stats(victim, killer, assist, flash_assist, &weapon);
            }
            Event::Other(name) => {
                trace!("{} {}", name, self.current_time());
            }
        }
    }

    fn clear_stats(&mut self) {
        debug!("------\n\n");
        self.current_round = 0;
        self.score = (0, 0);
        for (_, player) in self.players.iter_mut() {
            *player = Player::new(player.info.clone());
        }
    }

    pub fn update_kast(&mut self, killer: Option<i32>, assister: Option<i32>, victim: i32) {
        if let Some(killer) = killer {
            if killer != victim {
                self.current_round_player_state
                    .insert(killer, PlayerState::Killed);
            }
        }

        if let Some(assister) = assister {
            self.current_round_player_state
                .insert(assister, PlayerState::Assisted);
        }

        if self
            .current_round_player_state
            .get(&victim)
            .filter(|s| **s != PlayerState::Survived)
            .is_none()
        {
            self.current_round_player_state.insert(
                victim,
                PlayerState::Died(killer.unwrap_or(0), self.current_tick),
            );
        }

        for (traded_id, state) in self.current_round_player_state.iter_mut() {
            if let PlayerState::Died(maybe_traded_killer, tick) = *state {
                let trade_time = as_seconds(self.current_tick - tick, self.header.tickrate());
                if maybe_traded_killer == victim && trade_time < TRADE_TIME_LIMIT_IN_SECONDS {
                    debug!(
                        "[{}]{} traded [{}]{} by killing [{}]{} ({}s)",
                        killer.unwrap(),
                        &self.players[&killer.unwrap()].name,
                        traded_id,
                        &self.players[&traded_id].name,
                        victim,
                        &self.players[&victim].name,
                        trade_time
                    );
                    *state = PlayerState::Traded;
                } else if maybe_traded_killer == victim {
                    debug!(
                        "[{}]{} was too late to trade [{}]{} by killing [{}]{} ({}s)",
                        killer.unwrap(),
                        &self.players[&killer.unwrap()].name,
                        traded_id,
                        &self.players[&traded_id].name,
                        victim,
                        &self.players[&victim].name,
                        trade_time
                    );
                }
            }
        }
    }

    pub fn update_stats(
        &mut self,
        death: i32,
        killer: Option<i32>,
        assist: Option<i32>,
        assist_flash: bool,
        _weapon: &str,
    ) -> Option<()> {
        self.update_kast(killer, assist, death);
        let kill = killer.unwrap_or(death);

        let killer_team = *self.teams.get(&kill)?;
        let victim_team = *self.teams.get(&death)?;
        let assist_team = assist.and_then(|id| self.teams.get(&id).copied());

        if kill == death || killer_team == victim_team {
            // self.players.get_mut(&kill).unwrap().kills -= 1;
        } else if let Some(killer) = self.players.get_mut(&kill) {
            killer.kills += 1;
        } else {
            warn!("Did not find player who killed with id {}", kill);
        }

        //dbg!(assist, assistflash, assist_team);
        if let (Some(assist), Some(assist_team)) = (assist, assist_team) {
            if let Some(assister) = self.players.get_mut(&assist) {
                match (assist_flash, assist_team == victim_team) {
                    (true, true) => {} //assister.flash_assists -= 1,
                    (true, false) => {
                        assister.flash_assists += 1;
                    }
                    (false, true) => {
                        assister.assists -= 1;
                    }
                    (false, false) => {
                        assister.assists += 1;
                    }
                }
            } else {
                warn!("Did not find player who assisted with id {}", assist);
            }
        }

        if let Some(victim) = self.players.get_mut(&death) {
            victim.deaths += 1;
        } else {
            warn!("Did not find player who died with id {}", death);
        }
        Some(())
    }

    fn equip(&mut self, id: i32, item: String) {
        let munas = [
            "hegrenade",
            "incgrenade",
            "smokegrenade",
            "flashbang",
            "molotov",
        ];

        let mut player = self.players.get_mut(&id).unwrap();
        if munas.contains(&item.as_str()) {
            player.latest_muna = Some(item.clone());
            player.muna_tick = self.current_tick;
        }
        player.equipped = item;
    }

    fn as_seconds(&self, ticks: i32) -> f32 {
        as_seconds(ticks, self.header.tickrate())
    }

    fn muna_in_hand(&self, id: i32) -> Option<(String, f32)> {
        self.players
            .get(&id)
            .map(|p| {
                (
                    p.equipped.to_string(),
                    self.as_seconds(self.current_tick - p.muna_tick),
                )
            })
            .filter(|(_, time)| time < &2.5)
    }

    pub fn print_stats(&self) {
        info!("Score: {} - {}", self.score.0, self.score.1);

        let mut current_team = None;
        let mut player_list = self
            .players
            .iter()
            .filter(|(id, _)| self.teams.contains_key(id))
            .map(|(id, player)| (self.teams[id], player))
            .collect::<Vec<_>>();
        player_list
            .sort_by_key(|(team, player)| (*team, -player.kills, -player.assists, player.deaths));
        for (team, player) in player_list {
            if current_team != Some(team) {
                current_team = Some(team);
                info!("Team {}:", if !team { 1 } else { 2 });
            }
            info!(
                "[{:2}]{:16}(k/a/d {:3} {:3} {:3} ({} f) KAST: {:.0}%)",
                self.find_player_by_xuid(player.info.xuid).unwrap(),
                player.name,
                player.kills,
                player.assists,
                player.deaths,
                player.flash_assists,
                100.0 * (player.kast as f32) / ((self.score.0 + self.score.1) as f32),
            );
        }
    }
}

pub fn parse_game<R: Read>(
    mut reader: R,
) -> Result<(Header, Vec<Player>, Vec<Player>), std::io::Error> {
    let header = Header::new(&mut reader);
    info!("Tickrate: {} ticks/second", header.tickrate());

    let mut state = State::new(header.clone());

    loop {
        let header = PacketHeader::new(&mut reader);
        state.current_tick = header.tick;
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

    let mut team_a = state
        .players
        .iter()
        .filter(|(id, _)| state.teams.get(id) == Some(&true))
        .map(|(_, p)| p)
        .cloned()
        .collect::<Vec<_>>();

    let mut team_b = state
        .players
        .iter()
        .filter(|(id, _)| state.teams.get(id) == Some(&false))
        .map(|(_, p)| p)
        .cloned()
        .collect::<Vec<_>>();

    team_a.sort_by_key(|p| p.info.xuid);
    team_b.sort_by_key(|p| p.info.xuid);

    if state.score.0 > state.score.1 {
        info!("Winner team: team with {}", team_a[0].name);
        Ok((header, team_a, team_b))
    } else {
        info!("Winner team: team with {}", team_b[0].name);
        Ok((header, team_b, team_a))
    }
}

#[cfg(test)]
mod test {
    use crate::bitreader::*;
    use crate::parse_game::*;
    use crate::playerinfo::PlayerInfo;

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

    fn gen_player(state: &mut State, id: i32, team: bool) -> i32 {
        let info = PlayerInfo {
            version: 0,
            xuid: 0,
            name: format!("Player {}", id),
            user_id: id,
            guid: format!("STEAMGUID-{}", id),
            friends_id: 0,
            friends_name: format!("FriendsName{}", id),
            fake: false,
            proxy: false,
            custom_files_crc: [0, 0, 0, 0],
            files_downloaded: 0,
            entity_id: 0,
        };
        state.players.insert(id, Player::new(info));
        state.teams.insert(id, team);
        id
    }

    fn stat(state: &State, id: i32) -> (i32, i32, i32, i32) {
        let player = &state.players[&id];
        (
            player.kills,
            player.assists,
            player.deaths,
            player.flash_assists,
        )
    }

    #[test]
    fn kills() {
        let mut state = State::new(Header {
            playback_time: 1.0,
            playback_ticks: 1,
            ..Default::default()
        });
        let killer = Some(gen_player(&mut state, 1, false));
        let victim = gen_player(&mut state, 2, true);
        let assister = gen_player(&mut state, 3, false);
        let friendly_assister = gen_player(&mut state, 4, true);

        assert_eq!(stat(&state, killer.unwrap()), (0, 0, 0, 0));
        assert_eq!(stat(&state, victim), (0, 0, 0, 0));
        assert_eq!(stat(&state, assister), (0, 0, 0, 0));
        assert_eq!(stat(&state, friendly_assister), (0, 0, 0, 0));

        state.update_stats(victim, killer, None, false, "knife");

        assert_eq!(stat(&state, killer.unwrap()), (1, 0, 0, 0));
        assert_eq!(stat(&state, victim), (0, 0, 1, 0));
        assert_eq!(stat(&state, assister), (0, 0, 0, 0));
        assert_eq!(stat(&state, friendly_assister), (0, 0, 0, 0));

        state.update_stats(victim, killer, Some(assister), false, "knife");

        assert_eq!(stat(&state, killer.unwrap()), (2, 0, 0, 0));
        assert_eq!(stat(&state, victim), (0, 0, 2, 0));
        assert_eq!(stat(&state, assister), (0, 1, 0, 0));
        assert_eq!(stat(&state, friendly_assister), (0, 0, 0, 0));

        state.update_stats(victim, killer, Some(assister), true, "knife");

        assert_eq!(stat(&state, killer.unwrap()), (3, 0, 0, 0));
        assert_eq!(stat(&state, victim), (0, 0, 3, 0));
        assert_eq!(stat(&state, assister), (0, 1, 0, 1));
        assert_eq!(stat(&state, friendly_assister), (0, 0, 0, 0));

        state.update_stats(victim, killer, Some(friendly_assister), false, "knife");

        assert_eq!(stat(&state, killer.unwrap()), (4, 0, 0, 0));
        assert_eq!(stat(&state, victim), (0, 0, 4, 0));
        assert_eq!(stat(&state, assister), (0, 1, 0, 1));
        assert_eq!(stat(&state, friendly_assister), (0, -1, 0, 0));
    }
}
