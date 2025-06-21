use color_eyre::eyre::{OptionExt, Result};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::time::Duration;
use tokio::sync::mpsc;

const TICK_FPS: f64 = 30.0;

#[derive(Clone, Debug)]
pub enum Event {
    Tick,
    Crossterm(CrosstermEvent),
    App(AppEvent),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Quit,
    UpdateContainers,
    UpdateContainerInfo(String),
    RestartContainer(String),
    StopContainer(String),
    KillContainer(String),
    RemoveContainer(String),
    GoToContainerDetails(String),
    UpdateVolumes,
    RemoveVolume(String, bool),
    UpdateNetworks,
    RemoveNetwork(String),
    UpdateImages,
    RemoveImage(String, bool),
    Back,
}

#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> Result<Event> {
        self.receiver.recv().await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(Event::App(app_event));
    }
}

struct EventTask {
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    async fn run(self) -> Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);

        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
                _ = self.sender.closed() => break,
                _ = tick_delay => self.send(Event::Tick),
                Some(Ok(event)) = crossterm_event => self.send(Event::Crossterm(event)),
            };
        }

        Ok(())
    }

    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}
