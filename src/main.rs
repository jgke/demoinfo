mod bitreader;
mod cmd;
mod csgo;
mod header;
mod packet;
mod player;
mod playerinfo;
mod parse_game;
mod ranks;
mod stringtables;
mod stable_hasher;

use std::env;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(&args[1])?;
    let reader = BufReader::new(file);

    let mut rankmanager = ranks::RankManager::new()?;

    let (header, team_a, team_b) = parse_game::parse_game(reader)?;
    rankmanager.update_ranks(&header, &team_a, &team_b)?;

    Ok(())
}
