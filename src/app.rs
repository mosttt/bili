use crate::{download, user};
use clap::{CommandFactory, Parser, Subcommand};
use once_cell::sync::OnceCell;

static CLI: OnceCell<Cli> = OnceCell::new();

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
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
        /// url to download from bilibili
        #[arg(value_parser = check_download_url)]
        url: String,

        /// 断点续传，必须选择和上次一样的清晰度，否则会出现视频无法使用的情况。
        #[arg(short,long,action = clap::ArgAction::SetTrue)]
        resume: bool,
    },
}

fn check_download_url(s: &str) -> crate::Result<String> {
    if !(s.contains("http://") || s.contains("https://")) {
        return Err(anyhow::Error::msg("not valid url"));
    };
    Ok(s.replace("http://", "https://"))
}

pub(crate) async fn run() -> crate::Result<()> {
    CLI.set(Cli::parse()).unwrap();
    
    match &cli().command {
        Some(Commands::Login { console }) => {
            user::login(console).await?;
        }
        Some(Commands::User) => {
            user::user_info().await?;
        }
        Some(Commands::Download { url, resume: _ }) => {
            download::download(url.clone()).await?;
        }
        None => {
            let mut factory = Cli::command();
            factory.print_help().unwrap();
        }
    }
    Ok(())
}
fn cli() -> &'static Cli {
    CLI.get().unwrap()
}
pub(crate) fn resume_download_value() -> bool {
    if let Some(Commands::Download { url: _, resume }) = cli().command {
        return resume;
    }
    false
}
