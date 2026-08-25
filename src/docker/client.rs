use bollard::Docker;
use bollard::container::{
    InspectContainerOptions, KillContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
    RemoveContainerOptions, RestartContainerOptions, StopContainerOptions,
};
use bollard::image::{ListImagesOptions, RemoveImageOptions};
use bollard::models::ContainerSummary;
use bollard::network::ListNetworksOptions;
use bollard::secret::{ContainerInspectResponse, ImageSummary, Network, VolumeListResponse};
use bollard::volume::{ListVolumesOptions, RemoveVolumeOptions};
use color_eyre::eyre::Result;
use futures::{Stream, StreamExt};
use regex::Regex;
use std::future::Future;
use std::sync::LazyLock;

use crate::docker::error::{DockerError, DockerResult};

/// Matches ANSI escape sequences (CSI and simple two-byte forms) so raw
/// container log output can be shown as plain text.
static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[@-_]").unwrap());

#[derive(Clone)]
pub struct DockerClient {
    client: Docker,
}

impl DockerClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Docker::connect_with_local_defaults()?,
        })
    }
}

/// Crate-internal Docker surface. Methods return `Send` futures because `App`
/// spawns them as background tasks (see `App::spawn_docker`).
pub trait DockerApi: Clone + Send + Sync + 'static {
    fn list_containers(&self) -> impl Future<Output = DockerResult<Vec<ContainerSummary>>> + Send;
    fn stop_container(&self, container_id: &str) -> impl Future<Output = DockerResult<()>> + Send;
    fn restart_container(
        &self,
        container_id: &str,
    ) -> impl Future<Output = DockerResult<()>> + Send;
    fn kill_container(&self, container_id: &str) -> impl Future<Output = DockerResult<()>> + Send;
    fn remove_container(&self, container_id: &str)
    -> impl Future<Output = DockerResult<()>> + Send;
    fn inspect_container(
        &self,
        container_id: &str,
    ) -> impl Future<Output = DockerResult<ContainerInspectResponse>> + Send;
    fn list_volumes(&self) -> impl Future<Output = DockerResult<VolumeListResponse>> + Send;
    fn remove_volume(
        &self,
        name: &str,
        force: bool,
    ) -> impl Future<Output = DockerResult<()>> + Send;
    fn list_networks(&self) -> impl Future<Output = DockerResult<Vec<Network>>> + Send;
    fn remove_network(&self, name: &str) -> impl Future<Output = DockerResult<()>> + Send;
    fn list_images(&self) -> impl Future<Output = DockerResult<Vec<ImageSummary>>> + Send;
    fn remove_image(&self, id: &str, force: bool) -> impl Future<Output = DockerResult<()>> + Send;
    fn container_logs(
        &self,
        container_id: &str,
        tail: usize,
    ) -> impl Stream<Item = DockerResult<String>> + Send;
}

impl DockerApi for DockerClient {
    async fn list_containers(&self) -> DockerResult<Vec<ContainerSummary>> {
        Ok(self
            .client
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await?)
    }

    async fn stop_container(&self, container_id: &str) -> DockerResult<()> {
        self.client
            .stop_container(container_id, None::<StopContainerOptions>)
            .await?;
        Ok(())
    }

    async fn restart_container(&self, container_id: &str) -> DockerResult<()> {
        self.client
            .restart_container(container_id, None::<RestartContainerOptions>)
            .await?;
        Ok(())
    }

    async fn kill_container(&self, container_id: &str) -> DockerResult<()> {
        self.client
            .kill_container(container_id, None::<KillContainerOptions<String>>)
            .await?;
        Ok(())
    }

    async fn remove_container(&self, container_id: &str) -> DockerResult<()> {
        self.client
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;
        Ok(())
    }

    async fn inspect_container(
        &self,
        container_id: &str,
    ) -> DockerResult<ContainerInspectResponse> {
        Ok(self
            .client
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await?)
    }

    async fn list_volumes(&self) -> DockerResult<VolumeListResponse> {
        Ok(self
            .client
            .list_volumes(Some(ListVolumesOptions::<String>::default()))
            .await?)
    }

    async fn remove_volume(&self, name: &str, force: bool) -> DockerResult<()> {
        Ok(self
            .client
            .remove_volume(name, Some(RemoveVolumeOptions { force }))
            .await?)
    }

    async fn list_networks(&self) -> DockerResult<Vec<Network>> {
        Ok(self
            .client
            .list_networks(Some(ListNetworksOptions::<String>::default()))
            .await?)
    }

    async fn remove_network(&self, name: &str) -> DockerResult<()> {
        Ok(self.client.remove_network(name).await?)
    }

    async fn list_images(&self) -> DockerResult<Vec<ImageSummary>> {
        let options = Some(ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        });
        Ok(self.client.list_images(options).await?)
    }

    async fn remove_image(&self, id: &str, force: bool) -> DockerResult<()> {
        let options = Some(RemoveImageOptions {
            force,
            ..Default::default()
        });
        self.client.remove_image(id, options, None).await?;
        Ok(())
    }

    fn container_logs(
        &self,
        container_id: &str,
        tail: usize,
    ) -> impl Stream<Item = DockerResult<String>> + Send {
        self.client
            .logs(
                container_id,
                Some(LogsOptions::<String> {
                    follow: true,
                    stdout: true,
                    stderr: true,
                    tail: tail.to_string(),
                    ..Default::default()
                }),
            )
            .map(|r| r.map_err(DockerError::from).map(log_output_to_string))
    }
}

/// Converts a single `LogOutput` frame to a plain-text line: lossily decodes
/// the bytes, strips ANSI escape sequences and carriage returns. StdOut,
/// StdErr and Console frames are treated alike.
fn log_output_to_string(output: LogOutput) -> String {
    let text = String::from_utf8_lossy(&output.into_bytes()).into_owned();
    let text = ANSI_ESCAPE_RE.replace_all(&text, "");
    text.replace('\r', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_output_to_string_strips_ansi_and_carriage_returns() {
        let message = b"\x1b[32mhello\x1b[0m world\r\n".as_slice().into();
        let text = log_output_to_string(LogOutput::StdOut { message });
        assert_eq!(text, "hello world\n");
    }

    #[test]
    fn log_output_to_string_lossy_decodes_invalid_utf8() {
        let message = [b'a', 0xff, b'b'].as_slice().into();
        let text = log_output_to_string(LogOutput::StdErr { message });
        assert!(text.starts_with('a'));
        assert!(text.ends_with('b'));
    }

    #[test]
    fn log_output_to_string_treats_all_frame_kinds_alike() {
        let message = b"line\n".as_slice();
        assert_eq!(
            log_output_to_string(LogOutput::Console {
                message: message.into()
            }),
            "line\n"
        );
        assert_eq!(
            log_output_to_string(LogOutput::StdIn {
                message: message.into()
            }),
            "line\n"
        );
    }
}
