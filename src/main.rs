use std::process::exit;

pub(crate) use anyhow::Result;

mod cli;
mod download;
mod entities;
mod ffmpeg;
mod local;
mod user;

#[tokio::main]
async fn main() {
    if let Err(e) = ffmpeg::ffmpeg_run_version() {
        if cfg!(debug_assertions) {
            panic!("{}", e);
        }
        println!("{}", e);
        exit(0);
    }
    if let Err(e) = cli::run().await {
        if cfg!(debug_assertions) {
            panic!("{}", e);
        }
        println!("{}", e);
        exit(0);
    }
}
