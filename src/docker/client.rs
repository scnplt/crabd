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

    pub async fn list_containers(&self) -> Result<Vec<ContainerSummary>> {
        Ok(self
            .client
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await?)
    }

    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        self.client
            .stop_container(container_id, None::<StopContainerOptions>)
            .await?;
        Ok(())
    }

    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        self.client
            .restart_container(container_id, None::<RestartContainerOptions>)
            .await?;
        Ok(())
    }

    pub async fn kill_container(&self, container_id: &str) -> Result<()> {
        self.client
            .kill_container(container_id, None::<KillContainerOptions<String>>)
            .await?;
        Ok(())
    }

    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
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

    pub async fn inspect_container(&self, container_id: &str) -> Result<ContainerInspectResponse> {
        Ok(self
            .client
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await?)
    }

    pub async fn list_volumes(&self) -> Result<VolumeListResponse> {
        Ok(self.client.list_volumes(Some(ListVolumesOptions::<String>::default())).await?)
    }

    pub async fn remove_volume(&self, name: &str, force: bool) -> Result<()> {
        Ok(self.client.remove_volume(name, Some(RemoveVolumeOptions { force })).await?)
    }

    pub async fn list_networks(&self) -> Result<Vec<Network>> {
        Ok(self.client.list_networks(Some(ListNetworksOptions::<String>::default())).await?)
    }

    pub async fn remove_network(&self, name: &str) -> Result<()> {
        Ok(self.client.remove_network(name).await?)
    }

    pub async fn list_images(&self) -> Result<Vec<ImageSummary>> {
        let options = Some(ListImagesOptions::<String> { all: true, ..Default::default() });
        Ok(self.client.list_images(options).await?)
    }

    pub async fn remove_image(&self, id: &str, force: bool) -> Result<()> {
        let options = Some(RemoveImageOptions { force, ..Default::default() });
        self.client.remove_image(id, options, None).await?;
        Ok(())
    }
}
