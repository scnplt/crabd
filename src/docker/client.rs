use bollard::Docker;
use bollard::container::{
    InspectContainerOptions, KillContainerOptions, ListContainersOptions, RemoveContainerOptions,
    RestartContainerOptions, StopContainerOptions,
};
use bollard::models::ContainerSummary;
use bollard::secret::ContainerInspectResponse;
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
}
