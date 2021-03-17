use rusqlite::{params, Connection, Transaction};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::header::Header;
use crate::player::Player;
use crate::stable_hasher::StableHasher;

use log::warn;

pub struct RankManager {
    connection: Connection,
}

type Rank = f64;
const DEFAULT_RANK: Rank = 1000.0;

#[derive(Debug, Clone)]
pub struct DbPlayer {
    xuid: i64,
    name: String,
    rank: Rank,
    game_count: i64,
}

pub fn fetch_player(tx: &Transaction, xuid: i64) -> rusqlite::Result<DbPlayer> {
    let mut stmt = tx.prepare("SELECT xuid, name, rank, game_count FROM player WHERE xuid=?")?;

    let res = stmt
        .query_map(params![&xuid], |row| {
            Ok(DbPlayer {
                xuid: row.get(0)?,
                name: row.get(1)?,
                rank: row.get(2)?,
                game_count: row.get(3)?,
            })
        })?
        .map(|row| row.unwrap())
        .next()
        .unwrap_or(DbPlayer {
            xuid: 0,
            name: "".to_string(),
            rank: DEFAULT_RANK,
            game_count: 0,
        });
    Ok(res)
}

pub fn team_rank(
    tx: &Transaction,
    team: &[Player],
) -> rusqlite::Result<(Rank, HashMap<i64, DbPlayer>)> {
    let ranks = team
        .iter()
        .map(|p| Ok((p.info.xuid, fetch_player(tx, p.info.xuid as i64)?)))
        .collect::<rusqlite::Result<HashMap<i64, DbPlayer>>>()?;

    let rank_sum: Rank = ranks.values().map(|p| p.rank).sum();
    let rank_max: Rank = ranks.values().map(|p| p.rank).fold(-1. / 0., Rank::max);

    Ok((rank_sum + rank_max, ranks))
}

const TABLES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS game (
       id INTEGER PRIMARY KEY
     )",
    "CREATE TABLE IF NOT EXISTS player (
       xuid INTEGER PRIMARY KEY,
       name TEXT NOT NULL,
       rank INTEGER NOT NULL,
       game_count INTEGER NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS team (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       name TEXT UNIQUE NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS team__player (
       player_id INTEGER NOT NULL REFERENCES player(xuid),
       team_id INTEGER NOT NULL REFERENCES team(id),
       PRIMARY KEY (player_id, team_id)
     )",
];
const RUSH_A: &[i64] = &[
    76561197972046672,
    76561197977088451,
    76561197978366738,
    76561197979975809,
    76561197985393858,
    76561197997347168,
    76561198202704517,
];

impl RankManager {
    pub fn new() -> rusqlite::Result<RankManager> {
        let connection = Connection::open("ranks.db")?;

        for table in TABLES {
            connection.execute(table, params![]).unwrap();
        }

        connection
            .execute(
                "INSERT OR REPLACE INTO team (name) VALUES ('Solita Rush A')",
                params![],
            )
            .unwrap();
        let team_id: i64 = connection
            .query_row(
                "SELECT id FROM team WHERE name='Solita Rush A'",
                params![],
                |row| row.get(0),
            )
            .unwrap();

        for xuid in RUSH_A {
            connection
                .execute(
                    "INSERT INTO player (xuid, name, rank, game_count)
                                VALUES (?, '', ?, 0)
                                ON CONFLICT DO NOTHING",
                    params![xuid, DEFAULT_RANK],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT OR REPLACE INTO team__player (player_id, team_id) VALUES (?, ?)",
                    params![xuid, team_id],
                )
                .unwrap();
        }

        Ok(RankManager { connection })
    }

    fn update_team_ranks(
        tx: &Transaction,
        ranks: HashMap<i64, DbPlayer>,
        players: &[Player],
        points: Rank,
    ) -> rusqlite::Result<()> {
        let mut total_kills = players.iter().map(|p| p.kills).sum();
        if total_kills == 0 {
            total_kills = 1;
        }

        for player in players {
            let dbplayer = &ranks[&player.info.xuid];

            let share_of_points = if points < 0.0 {
                ((total_kills - player.kills) as Rank) / (total_kills as Rank)
            } else {
                (player.kills as Rank) / (total_kills as Rank)
            };

            tx.execute(
                "INSERT OR REPLACE INTO player (xuid, name, rank, game_count) VALUES (?, ?, ?, ?)",
                params![
                    &(player.info.xuid as i64),
                    &player.name,
                    dbplayer.rank + points * share_of_points,
                    dbplayer.game_count + 1
                ],
            )?;
        }

        Ok(())
    }

    pub fn update_ranks(
        &mut self,
        header: &Header,
        winners: &[Player],
        losers: &[Player],
    ) -> rusqlite::Result<()> {
        let mut hasher = StableHasher::new();
        (
            &header.server_name,
            &header.client_name,
            &header.map,
            &header.playback_ticks,
            winners,
            losers,
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

            let (winner_rank, winner_ranks) = team_rank(&tx, winners)?;
            let (loser_rank, loser_ranks) = team_rank(&tx, losers)?;

            let surprise = loser_rank / winner_rank;

            let winner_gains = 100.0 * surprise;
            let loser_loses = -100.0 * surprise;

            dbg!(winner_rank, loser_rank, surprise, winner_gains, loser_loses);

            RankManager::update_team_ranks(&tx, winner_ranks, winners, winner_gains)?;
            RankManager::update_team_ranks(&tx, loser_ranks, losers, loser_loses)?;
        } else {
            warn!("Already found game");
        }

        tx.commit()?;

        Ok(())
    }
}
