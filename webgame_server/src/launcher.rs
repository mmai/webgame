use clap::{Arg, App};
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;
use std::sync::Arc;

use std::thread;
use futures::executor::block_on;
use std::time::Duration;
use chrono::{Utc, DateTime};
use std::path::Path;
use std::fs::File;
use std::fs;

use webgame_protocol::{GameState, GameRecord};
use crate::server;
use crate::store::GameStore;
use crate::store_sled::SledStore;

pub async fn launch<
    GamePlayCommand:Debug+Send+DeserializeOwned+'static,
    SetPlayerRoleCommand: Debug+Send+DeserializeOwned+'static,
    GameStateType: GameState+'static,
    PlayEventT: Serialize+Send+Sync+'static,
    >(
        name: &'static str,
        version: String,
        // version: &'static str,
        author: &'static str,
        on_gameplay: server::GamePlayHandler<GamePlayCommand, GameStateType, PlayEventT>,
        on_setplayerrole: server::SetPlayerRoleHandler<SetPlayerRoleCommand, GameStateType, PlayEventT>
    ) 

    where GameStateType::VariantParameters: Debug+DeserializeOwned+Serialize+Send+Sync+'static
{
// pub async fn launch(dispatcher: impl server::GameDispatcher) {
    pretty_env_logger::init();


    let app = App::new("Webtarot")
        .version(version.as_str())
        .author(author)
        .about(name)
        .arg(Arg::with_name("directory")
             .short("d")
             .long("directory")
             .value_name("ROOT")
             .help("Directory path of the static files")
             .takes_value(true))
        .arg(Arg::with_name("archives")
             .short("c")
             .long("archives-directory")
             .value_name("ARCHIVES")
             .help("Directory path where game archives are stored")
             .takes_value(true))
        .arg(Arg::with_name("archive_delay")
             .long("archive-delay")
             .value_name("ARCHIVEDELAY")
             .help("Retention period in hours after wich the game is archived")
             .takes_value(true))
        .arg(Arg::with_name("archive_check")
             .long("archive-check")
             .value_name("ARCHIVECHECK")
             .help("Archivage check period in minutes")
             .takes_value(true))
        .arg(Arg::with_name("address")
             .short("a")
             .long("ip address")
             .value_name("IP")
             .help("IP address the server listen to")
             .takes_value(true))
        .arg(Arg::with_name("port")
             .short("p")
             .long("port")
             .value_name("PORT")
             .help("Port the server listen to")
             .takes_value(true))
        .arg(Arg::with_name("databaseuri")
             .short("u")
             .long("db-uri")
             .value_name("DBURI")
             .help("Uri of the database storing game states")
             .takes_value(true))
        ;
    let matches = app.get_matches();

    let mut default_public_dir = get_current_dir();
    default_public_dir.push_str("/public");
    let public_dir = matches.value_of("directory").unwrap_or(&default_public_dir);
    // let pdir = std::path::PathBuf::from(public_dir);

    let str_port = matches.value_of("port").unwrap_or("8002"); 
    // let port = str_port.parse::<u16>().unwrap();
    let str_ip = matches.value_of("address").unwrap_or("127.0.0.1"); 

    let db_uri = matches.value_of("databaseuri").unwrap_or("webgame_db");
    let archives_dir = matches.value_of("archives_directory").unwrap_or("webgame_archives");
    let cleaner_archive_after = matches.value_of("archive_delay").and_then(|val| val.parse::<i64>().ok()).unwrap_or(24);
    let cleaner_check_interval = matches.value_of("archives_check").and_then(|val| val.parse::<u64>().ok()).unwrap_or(120);
    let store = Arc::new(SledStore::new(&db_uri));

    let str_socket = format!("{}:{}", str_ip, str_port);
    if let Ok(socket) = str_socket.parse() {

        // XXX Cleaning task : should be defined in another module ?
        let cleaner_store = Arc::clone(&store);
        let cleaner_archives = String::from(archives_dir);

        if !Path::new(&cleaner_archives).exists(){
            fs::create_dir(&cleaner_archives).unwrap();
        }

        thread::spawn(move || {
            loop {
                let now = Utc::now().time();
                let fgames: Vec<GameRecord<GameStateType>> = cleaner_store.data().iter()
                    .map(|res| res.map(|game| game.1))
                    .filter_map(Result::ok)
                    .filter(|d| (now - d.date_updated.time()).num_hours() > cleaner_archive_after )
                    .collect();
                // println!("{} games to archive", fgames.len());
                for g in fgames {
                    let filename = format!("{}/{}.json", cleaner_archives, &g.info.game_id);
                    if let Ok(_ok) = serde_json::to_writer(&File::create(&filename).unwrap(), &g) {
                        // println!("stored {}", &filename);
                        if block_on(cleaner_store.delete(g.info.game_id)) {
                            // println!("and deleted.. ");
                        }
                    }
                }
                thread::sleep(Duration::from_secs(60 * cleaner_check_interval));
            }
        });

        server::serve(
            String::from(public_dir),
            store,
            socket,
            on_gameplay,
            on_setplayerrole,
            ).await;
    } else {
        println!("Could not parse ip / port {}", str_socket);
    }
}

fn get_current_dir() -> String {
    std::env::current_dir()
    .map( |cd|
          String::from(cd.as_path().to_str().unwrap())
    ).expect("Can't find current path")
}
