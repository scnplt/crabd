mod docker;
mod views;
mod app;

use crate::app::App;
use crate::docker::client::DockerClient;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
    let docker = DockerClient::new()?;
    let containers = Arc::new(Mutex::new(docker.list_containers().await.unwrap()));
    let mut app = App::new(containers).await;
    let (tx, mut rx) = mpsc::channel(32);

    tokio::spawn(async move {
        loop {
            let new_containers = docker.list_containers().await.unwrap();
            let _ = tx.send(new_containers).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let mut terminal = ratatui::init();
    let _ = app.run(&mut terminal, &mut rx).await;

    ratatui::restore();
    Ok(())
}
