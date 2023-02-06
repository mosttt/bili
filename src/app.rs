use std::time::Duration;
use clap::{CommandFactory, Parser, Subcommand};
use qrcode::QrCode;
use crate::{download, user};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Login in
    Login {
        /// whether output QR in console,default false
        #[arg(short, long)]
        console: bool,
    },
    /// print user information
    User,

    /// download from url
    Download {
        url: String
    },
}

pub(crate) async fn run() -> crate::Result<()> {

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Login { console }) => {
            user::login(console).await?;
        }
        Some(Commands::User) => {
            user::user_info().await?;
        }
        Some(Commands::Download { url }) => {
            download::download(url.clone()).await?;
        }
        None => {
            let mut factory = Cli::command();
            factory.print_help().expect("TODO: panic message");
        }
    }
    Ok(())
}

