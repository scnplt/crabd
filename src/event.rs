use bollard::secret::{
    ContainerInspectResponse, ContainerSummary, ImageSummary, Network, VolumeListResponse,
};
use color_eyre::eyre::{OptionExt, Result};
use crossterm::event::KeyEventKind;
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::{Event::Key, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::docker::error::DockerResult;

const TICK_FPS: f64 = 30.0;

#[derive(Clone, Debug)]
pub enum Event {
    Tick,
    Crossterm(KeyEvent),
    App(AppEvent),
    Docker(DockerOutcome),
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

/// Outcome of a background Docker operation, delivered back through the event channel
/// once the spawned task that ran it completes.
#[derive(Clone, Debug)]
pub enum DockerOutcome {
    ContainersListed(DockerResult<Vec<ContainerSummary>>),
    VolumesListed(DockerResult<VolumeListResponse>),
    NetworksListed(DockerResult<Vec<Network>>),
    ImagesListed(DockerResult<Vec<ImageSummary>>),
    ContainerInspected {
        open_details: bool,
        result: DockerResult<Box<ContainerInspectResponse>>,
    },
    ActionCompleted {
        resource: ResourceKind,
        result: DockerResult<()>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Containers,
    Volumes,
    Networks,
    Images,
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
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(Event::App(app_event));
    }

    /// Clone of the event channel sender, for background tasks that report Docker results.
    pub fn sender(&self) -> mpsc::UnboundedSender<Event> {
        self.sender.clone()
    }

    /// Test-only constructor; skips the crossterm reader task so `App` can be driven
    /// deterministically without a terminal.
    #[cfg(test)]
    pub fn new_without_reader() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self { sender, receiver }
    }

    #[cfg(test)]
    pub fn try_next(&mut self) -> Option<Event> {
        self.receiver.try_recv().ok()
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
                Some(Ok(event)) = crossterm_event => {
                    if let Key(key) = event
                        && key.kind == KeyEventKind::Press
                    {
                        self.send(Event::Crossterm(key))
                    }
                }
            };
        }

        Ok(())
    }

    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_then_next_returns_the_app_event() {
        let mut handler = EventHandler::new_without_reader();

        handler.send(AppEvent::Quit);

        let event = handler.next().await.unwrap();
        assert!(matches!(event, Event::App(AppEvent::Quit)));
    }

    #[tokio::test]
    async fn docker_outcome_is_delivered_through_the_sender_clone() {
        let mut handler = EventHandler::new_without_reader();
        let sender = handler.sender();

        sender
            .send(Event::Docker(DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: Ok(()),
            }))
            .unwrap();

        let event = handler.next().await.unwrap();
        assert!(matches!(
            event,
            Event::Docker(DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: Ok(())
            })
        ));
    }
}
