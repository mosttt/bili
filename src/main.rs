pub(crate) use anyhow::Result;

mod app;
mod user;
mod local;
mod entities;
mod download;


#[tokio::main]
async fn main(){
    app::run().await.unwrap();
}

