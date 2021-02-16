use prost::Message;
use std::io::Read;

use crate::bitreader::ReadExtras;
use crate::netmessages_public;

#[derive(Clone, Debug)]
pub enum Cmd {
    CreateStringTable(netmessages_public::CsvcMsgCreateStringTable),
    UserMessage(netmessages_public::CsvcMsgUserMessage),
    GameEvent(netmessages_public::CsvcMsgGameEvent),
    GameEventList(netmessages_public::CsvcMsgGameEventList),
}

impl Cmd {
    pub fn parse<R: Read>(r: &mut R) -> Option<Cmd> {
        loop {
            let cmd = r.read_var_u32();
            if cmd.is_err() {
                return None;
            }
            let size = r.read_var_u32().unwrap();
            let data = r.read_u8_vec(size as usize).unwrap();

            // See: protos/netmessages_public.proto::SVC_Messages
            match cmd.unwrap() {
                12 => {
                    return Some(Cmd::CreateStringTable(
                        netmessages_public::CsvcMsgCreateStringTable::decode(&*data).unwrap(),
                    ));
                }

                23 => {
                    return Some(Cmd::UserMessage(
                        netmessages_public::CsvcMsgUserMessage::decode(&*data).unwrap(),
                    ));
                }
                25 => {
                    return Some(Cmd::GameEvent(
                        netmessages_public::CsvcMsgGameEvent::decode(&*data).unwrap(),
                    ));
                }

                30 => {
                    return Some(Cmd::GameEventList(
                        netmessages_public::CsvcMsgGameEventList::decode(&*data).unwrap(),
                    ));
                }

                _other => {}
            }
        }
    }
}
