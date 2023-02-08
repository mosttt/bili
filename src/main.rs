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
        println!("{}", e);
        panic!("{}", e)
    }
    cli::run().await.unwrap();
}
