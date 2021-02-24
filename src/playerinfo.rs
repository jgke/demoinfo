use crate::bitreader::{BitReader, ReadExtras};

#[allow(dead_code)]
#[derive(Clone, Debug, Hash)]
pub struct PlayerInfo {
    pub version: u64,
    pub xuid: i64,
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
    pub fn new(entry_index: i64, buf: &[u8]) -> std::io::Result<PlayerInfo> {
        let buf_ptr = &mut &*buf;
        let mut buf = BitReader::new(buf_ptr);

        let version = buf.read_u64_be()?;
        let xuid = buf.read_i64_be()?;
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
