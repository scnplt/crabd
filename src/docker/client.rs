use bollard::Docker;
use bollard::container::{KillContainerOptions, ListContainersOptions, RemoveContainerOptions, RestartContainerOptions, StopContainerOptions};
use bollard::models::ContainerSummary;

#[derive(Clone)]
pub struct DockerClient {
    client: Docker,
}

impl DockerClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self { client: Docker::connect_with_local_defaults()? })
    }

    pub async fn list_containers(&self) -> Result<Vec<ContainerSummary>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.client.list_containers(Some(ListContainersOptions::<String> { 
            all: true, 
            ..Default::default() 
        })).await?)
    }

    pub async fn stop_container(&self, container_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client.stop_container(container_id, None::<StopContainerOptions>).await?;
        Ok(())
    }

    pub async fn restart_container(&self, container_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client.restart_container(container_id, None::<RestartContainerOptions>).await?;
        Ok(())
    }

    pub async fn kill_container(&self, container_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client.kill_container(container_id, None::<KillContainerOptions<String>>).await?;
        Ok(())
    }

    pub async fn remove_container(&self, container_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.client.remove_container(container_id, Some(RemoveContainerOptions {
            force: true,
            ..Default::default()
        })).await?)
    }
}
