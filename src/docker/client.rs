use bollard::Docker;
use bollard::container::{
    InspectContainerOptions, KillContainerOptions, ListContainersOptions, RemoveContainerOptions,
    RestartContainerOptions, StopContainerOptions,
};
use bollard::image::{ListImagesOptions, RemoveImageOptions};
use bollard::models::ContainerSummary;
use bollard::network::ListNetworksOptions;
use bollard::secret::{ContainerInspectResponse, ImageSummary, Network, VolumeListResponse};
use bollard::volume::{ListVolumesOptions, RemoveVolumeOptions};
use color_eyre::eyre::Result;
use std::future::Future;

use crate::docker::error::DockerResult;

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
}
