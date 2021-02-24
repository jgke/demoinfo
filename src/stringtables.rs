use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::io::Read;

use crate::bitreader::{BitReader, ReadExtras};
use crate::csgo::netmessages_public;
use crate::playerinfo::PlayerInfo;

#[derive(Debug, Clone, Copy)]
pub struct StringTable {
    max_entries: i32,
    entry_index: i64,
    user_data_fixed_size: bool,
    user_data_size_bits: Option<i32>,
}

fn calculate_string_table(table: &mut StringTable, table_entries: i32, data: &[u8]) -> std::io::Result<HashMap<i32, PlayerInfo>> {
    let mut players = HashMap::new();

    if table.user_data_fixed_size {
        assert!(table.user_data_size_bits.is_some());
    }

    let reader_buf = &mut &*data;
    let mut reader = BitReader::new(reader_buf);

    let entry_bits = (table.max_entries as f64).log2().ceil() as usize;
    let mut entries: HashMap<i64, (Vec<u8>, Vec<u8>)> = HashMap::new();
    let mut history: VecDeque<Vec<u8>> = VecDeque::new();

    assert!(!reader.read_bit()?, "Dictionary encoding unsupported");

    for _i in 0..table_entries {
        table.entry_index += 1;
        if !reader.read_bit()? {
            table.entry_index = reader.read_bits_u32(entry_bits as u8)? as i64;
        }

        assert!(table.entry_index >= 0 && table.entry_index < (table.max_entries as i64));

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
                .get(&table.entry_index)
                .cloned()
                .unwrap_or((vec![], vec![]));
            entry = tuple.0;
            userdata = tuple.1;
        }

        if reader.read_bit()? {
            // don't read the length, it's fixed length and the length was networked down already
            if table.user_data_fixed_size {
                userdata = vec![reader
                    .read_bits_u32(table.user_data_size_bits.unwrap() as u8)?
                    .try_into()
                    .unwrap()];
            } else {
                let bytes = reader.read_bits_u32(14)? as usize;
                let mut buf = vec![0; bytes];
                reader.read_exact(&mut buf)?;
                userdata = buf;
            }

            let info = PlayerInfo::new(table.entry_index, &userdata)?;
            players.insert(info.user_id, info);
        }

        entries.insert(table.entry_index, (entry.clone(), userdata));

        // add to history
        if history.len() > 31 {
            history.pop_front();
        }

        history.push_back(entry);
    }

    Ok(players)
}

pub fn create_string_table(
    msg: netmessages_public::CsvcMsgCreateStringTable,
) -> std::io::Result<Option<(StringTable, HashMap<i32, PlayerInfo>)>> {
    let name = msg.name.unwrap();
    //println!("Stringtables: {}", name);
    if name != "userinfo" {
        return Ok(None);
    }

    let mut table = StringTable {
        max_entries: msg.max_entries.unwrap(),
        entry_index: -1,
        user_data_fixed_size: msg.user_data_fixed_size.filter(|n| *n).is_some(),
        user_data_size_bits: msg.user_data_size_bits.filter(|n| *n != 0),
    };

    if table.user_data_fixed_size {
        assert!(table.user_data_size_bits.is_some());
    }

    let _flags = msg.flags.unwrap();
    let string_data: &[u8] = &msg.string_data.unwrap();

    let players = calculate_string_table(&mut table, msg.num_entries.unwrap(), string_data)?;

    Ok(Some((table, players)))
}

pub fn update_string_table(
    table: &mut StringTable,
    msg: netmessages_public::CsvcMsgUpdateStringTable,
) -> std::io::Result<HashMap<i32, PlayerInfo>> {
    let string_data: &[u8] = &msg.string_data.unwrap();

    calculate_string_table(table, msg.num_changed_entries.unwrap(), string_data)
}
