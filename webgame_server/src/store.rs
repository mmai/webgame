use chrono::{Utc, DateTime};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use lazy_static::lazy_static;

use regex::Regex;
use std::convert::From;

use crate::protocol::GameState;
use crate::game::{Game, UniverseGame};
use crate::store_sled::SledStore;
use crate::store_print::PrintStore;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameRecord<State: GameState> {
    date_started: DateTime<Utc>,
    date_updated: DateTime<Utc>,
    #[serde(deserialize_with = "State::deserialize")] //cf. https://github.com/serde-rs/serde/issues/1296
    state: State,
}

impl<State: GameState> From<State> for GameRecord<State> {
    fn from(state: State) -> Self {
        let date_started = Utc::now();//TODO don't update for existing
        let date_updated = Utc::now();
        GameRecord { 
            date_started,
            date_updated,
            state
        }
    }
}

#[async_trait]
pub trait GameStore<GameStateT: GameState> {
    // fn new( path: &str ) -> Self;
    async fn save(&self, game: &dyn UniverseGame<GameStateT> ) -> bool;
}

pub struct GameStoreFactory {}

impl GameStoreFactory {
    pub fn create<GameStateT: GameState>(path: &str) -> dyn GameStore<GameStateT> {
        let (db_type, db_conf) = Self::parse(path);
        match db_type {
            "sled" => SledStore::new(db_conf),
            _ => PrintStore::new(db_conf),
        }
    }

    fn parse(input: &str) -> Option<&str> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(?x)
            ^(?P<db_type>[^:]+):
            (?P<db_conf>.*)$
            ").unwrap();
        }
        RE.captures(input).and_then(|cap| {
            cap.name("login").map(|login| login.as_str())
        })
    }

}
