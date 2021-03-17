mod bitreader;
mod cmd;
mod csgo;
mod game_event;
mod header;
mod packet;
mod parse_game;
mod player;
mod playerinfo;
mod ranks;
mod stable_hasher;
mod stringtables;

use std::env;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args: Vec<String> = env::args().collect();
    let file = File::open(&args[1])?;
    let reader = BufReader::new(file);

    let mut rankmanager = ranks::RankManager::new()?;

    let (header, team_a, team_b) = parse_game::parse_game(reader)?;
    rankmanager.update_ranks(&header, &team_a, &team_b)?;

    Ok(())
}
