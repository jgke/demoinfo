use crate::csgo::netmessages_public;
use log::{log_enabled, trace, Level};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Event {
    Filtered,
    BeginNewMatch,
    RoundStart,
    RoundOfficiallyEnded,
    RoundEnd(bool),
    ItemEquip(i32, String),
    PlayerSpawn(i32, bool),
    PlayerDeath {
        victim: i32,
        killer: Option<i32>,
        assist: Option<i32>,
        flash_assist: bool,
        weapon: String,
    },
    Other(String),
}

#[derive(Clone, Debug)]
pub struct EventContext {
    events: HashMap<i32, (String, HashMap<usize, String>)>,
}

impl EventContext {
    pub fn new(events: HashMap<i32, (String, HashMap<usize, String>)>) -> EventContext {
        EventContext { events }
    }
    pub fn parse_game_event(&self, ev: netmessages_public::CsvcMsgGameEvent) -> Event {
        let ignored = [
            "player_footstep",
            "weapon_fire",
            "weapon_reload",
            "player_hurt",
            "item_pickup",
        ];
        if let Some((name, key_data)) = ev.eventid.and_then(|id| self.events.get(&id)) {
            if ignored.contains(&name.as_str()) {
                return Event::Filtered;
            }
            if log_enabled!(Level::Trace) {
                trace!("{}", &name);
                for (i, key) in ev.keys.iter().enumerate() {
                    let key_name = &key_data[&i];
                    trace!("- {} = {}", key_name, crate::parse_game::show_key(&key));
                }
            }
            match name.as_str() {
                "begin_new_match" => Event::BeginNewMatch,
                "round_announce_match_start" | "round_start" => Event::RoundStart,
                "round_officially_ended" => Event::RoundOfficiallyEnded,
                "round_end" => {
                    let mut winner_team = None;
                    for (i, key) in ev.keys.iter().enumerate() {
                        let key_name = &key_data[&i];
                        if key_name == "winner" {
                            winner_team = key.val_byte;
                        }
                    }
                    let team = winner_team.filter(|t| *t == 2 || *t == 3).unwrap();
                    Event::RoundEnd(team == 2)
                }
                "item_equip" => {
                    let mut item = None;
                    let mut userid = None;
                    for (i, key) in ev.keys.iter().enumerate() {
                        let key_name = &key_data[&i];
                        if key_name == "item" {
                            item = Some(key.val_string.clone().unwrap());
                        } else if key_name == "userid" {
                            userid = key.val_short;
                        }
                    }
                    Event::ItemEquip(userid.unwrap(), item.unwrap())
                }
                "player_spawn" => {
                    let mut userid = None;
                    let mut teamnum = None;
                    for (i, key) in ev.keys.iter().enumerate() {
                        let key_name = &key_data[&i];
                        if key_name == "userid" {
                            userid = key.val_short;
                        } else if key_name == "teamnum" {
                            teamnum = key.val_short;
                        }
                    }
                    if teamnum.unwrap() == 2 || teamnum.unwrap() == 3 {
                        Event::PlayerSpawn(userid.unwrap(), teamnum.unwrap() == 2)
                    } else {
                        Event::Filtered
                    }
                }
                "player_death" => {
                    let mut userid = None;
                    let mut attackerid = None;
                    let mut assisterid = None;
                    let mut assisterflash = None;
                    let mut weapon: Option<&str> = None;
                    for (i, key) in ev.keys.iter().enumerate() {
                        let key_name = &key_data[&i];
                        if key_name == "userid" {
                            userid = key.val_short;
                        } else if key_name == "attacker" {
                            attackerid = key.val_short;
                        } else if key_name == "assister" {
                            assisterid = key.val_short;
                        } else if key_name == "assistedflash" {
                            assisterflash = key.val_bool;
                        } else if key_name == "weapon" {
                            weapon = key.val_string.as_ref().map(|s| s.as_str());
                        }
                    }
                    let id = userid.unwrap();
                    Event::PlayerDeath {
                        victim: id,
                        killer: attackerid.filter(|id| *id > 0),
                        assist: assisterid.filter(|id| *id > 0),
                        flash_assist: assisterflash.unwrap_or(false),
                        weapon: weapon.unwrap_or("").to_string(),
                    }
                }
                name => Event::Other(name.to_string()),
            }
        } else if let Some(name) = ev.event_name {
            trace!("{}", &name);
            Event::Other(name)
        } else {
            panic!("{:?}", ev);
        }
    }
}
