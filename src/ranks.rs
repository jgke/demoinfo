use rusqlite::{params, Connection};
use std::hash::{Hash, Hasher};

use crate::header::Header;
use crate::player::Player;
use crate::stable_hasher::StableHasher;

pub struct RankManager {
    connection: Connection,
}

type Rank = i32;

pub fn player_rank(player: &Player) -> Rank {
    return player.kills + player.assists - player.deaths;
}

pub fn team_rank(team: &[Player]) -> Rank {
    team.iter().map(player_rank).sum()
}

const tables: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS game (
       id INTEGER PRIMARY KEY
     )",
    "CREATE TABLE IF NOT EXISTS player (
       xuid INTEGER PRIMARY KEY,
       name TEXT NOT NULL
     )",
];

impl RankManager {
    pub fn new() -> rusqlite::Result<RankManager> {
        let connection = Connection::open("ranks.db")?;

        for table in tables {
            connection.execute(table, params![]).unwrap();
        }

        Ok(RankManager { connection })
    }

    pub fn update_ranks(
        &mut self,
        header: &Header,
        team_a: &[Player],
        team_b: &[Player],
    ) -> rusqlite::Result<()> {
        let mut hasher = StableHasher::new();
        (
            &header.server_name,
            &header.client_name,
            &header.map,
            &header.playback_ticks,
            team_a,
            team_b,
        )
            .hash(&mut hasher);
        let game_hash: i64 = hasher.finish() as i64;

        let tx = self.connection.transaction()?;

        let game: Option<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM game WHERE id=?")?;

            let res = stmt
                .query_map(params![&game_hash], |row| {
                    let id: i64 = row.get(0)?;
                    Ok(id)
                })?
                .map(|row| row.unwrap())
                .next();
            res
        };

        if game.is_none() {
            tx.execute("INSERT INTO game (id) VALUES (?)", params![&game_hash])?;
        } else {
            panic!("Already found game");
        }

        for player in team_a.iter().chain(team_b.iter()) {
            tx.execute("INSERT OR REPLACE INTO player (xuid, name) VALUES (?, ?)", params![&(player.info.xuid as i64), &player.name])?;
        }

        tx.commit()?;

        Ok(())
    }
}
