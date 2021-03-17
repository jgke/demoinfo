use crate::playerinfo::PlayerInfo;

#[derive(Clone, Debug, Hash)]
pub struct Player {
    pub name: String,

    pub kills: i32,
    pub assists: i32,
    pub flash_assists: i32,
    pub deaths: i32,

    pub kast: i32,

    pub equipped: String,

    pub latest_muna: Option<String>,
    pub muna_tick: i32,

    pub info: PlayerInfo,
}

impl Player {
    pub fn new(info: PlayerInfo) -> Player {
        Player {
            name: info.name.clone(),
            kills: 0,
            assists: 0,
            flash_assists: 0,
            deaths: 0,

            kast: 0,

            equipped: "knife".to_string(),
            latest_muna: None,
            muna_tick: 0,

            info,
        }
    }
}
