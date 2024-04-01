mod args;
mod commands;
mod hooks;
mod style;
mod utils;

use args::ARGS;

#[tokio::main]
async fn main() {
    let Some(command) = &ARGS.command else {
        println!("No command specified");
        return;
    };
    match command {
        args::Command::List { updates } => {
            if *updates {
                commands::list_updates().await;
            } else {
                commands::list().await;
            }
        }
        args::Command::Search { query } => {
            commands::search(query).await;
        }
        args::Command::Install { query } => {
            commands::install(query).await;
        }
        args::Command::Uninstall { query } => {
            commands::uninstall(query).await;
        }
        args::Command::Update { query, list, count } => {
            if *list {
                commands::list_updates().await;
            } else if *count {
                commands::count_updates().await;
            } else {
                commands::update(query.as_deref()).await;
            }
        }
    }
}
