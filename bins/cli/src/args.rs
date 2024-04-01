use clap::{Args, Parser, Subcommand};
use once_cell::sync::Lazy;
use unipac_macros::{for_all, for_all_attrs};

pub static ARGS: Lazy<UnipacArgs> = Lazy::new(parse);

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct UnipacArgs {
    #[arg(short, long)]
    pub no_interactive: bool,

    #[command(flatten)]
    pub managers: Managers,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    List {
        #[arg(short, long)]
        updates: bool,
    },
    Search {
        query: String,
    },
    Install {
        query: String,
    },
    Uninstall {
        query: String,
    },
    Update {
        query: Option<String>,
        #[arg(short, long)]
        list: bool,
        #[arg(short, long)]
        count: bool,
    },
}

#[for_all_attrs]
#[derive(Args, Default)]
pub struct Managers {
    #[arg(short, long)]
    pub __manager: bool,
}

impl Managers {
    pub fn any(&self) -> bool {
        for_all! {
            if self.__manager {
                return true;
            }
        }
        false
    }
}

fn parse() -> UnipacArgs {
    let mut args = UnipacArgs::parse();
    if !args.managers.any() {
        for_all! {
            args.managers.__manager = true;
        }
    }
    args
}
