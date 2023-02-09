use crate::{download, user};
use clap::{CommandFactory, Parser, Subcommand};
use dialoguer::Input;
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
        url: Option<String>,

        /// 断点续传，必须选择和上次一样的清晰度，否则会出现视频无法使用的情况。
        #[arg(short,long,action = clap::ArgAction::SetTrue)]
        resume: bool,

        ///  使用url解析剧集数据而不是id, 有的剧集下不了加上这个试试 (对集合类视频有效，对BV无效)
        #[arg(short,long,action = clap::ArgAction::SetTrue)]
        parse_input_url: bool,

        /// 加上这个可以选择要下载的seasons, 而不是全部的seasons
        #[arg(short,long,action = clap::ArgAction::SetTrue)]
        choose_seasons: bool,
    },
}

fn check_download_url(s: &str) -> crate::Result<String> {
    if !(s.contains("http://") || s.contains("https://")) {
        return Err(anyhow::Error::msg("not valid url"));
    };
    Ok(s.replace("http://", "https://"))
}

pub(crate) async fn run() -> crate::Result<()> {
    CLI.set(Cli::parse())?;

    match &cli().command {
        Some(Commands::Login { console }) => {
            user::login(console).await?;
        }
        Some(Commands::User) => {
            user::user_info().await?;
        }
        Some(Commands::Download {
            url,
            resume: _,
            parse_input_url: _,
            choose_seasons: _,
        }) => {
            let url = if let Some(url) = url {
                url.to_string()
            } else {
                check_download_url(
                    Input::<String>::new()
                        .with_prompt("请输入视频网址")
                        .interact_text()?
                        .as_str(),
                )?
            };
            download::download(url).await?;
        }
        None => {
            let mut factory = Cli::command();
            factory.print_help()?;
        }
    }
    Ok(())
}
fn cli() -> &'static Cli {
    CLI.get().unwrap()
}
pub(crate) fn resume_download_value() -> bool {
    if let Some(Commands::Download {
        url: _,
        resume,
        parse_input_url: _,
        choose_seasons: _,
    }) = cli().command
    {
        return resume;
    }
    false
}

pub(crate) fn parse_input_url_value() -> bool {
    if let Some(Commands::Download {
        url: _,
        resume: _,
        parse_input_url,
        choose_seasons: _,
    }) = cli().command
    {
        return parse_input_url;
    }
    false
}

pub(crate) fn choose_seasons_value() -> bool {
    if let Some(Commands::Download {
        url: _,
        resume: _,
        parse_input_url: _,
        choose_seasons,
    }) = cli().command
    {
        return choose_seasons;
    }
    false
}
