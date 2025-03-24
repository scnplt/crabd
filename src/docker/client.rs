use bollard::Docker;
use bollard::container::{ListContainersOptions, StopContainerOptions, RestartContainerOptions, KillContainerOptions, StartContainerOptions};
use bollard::models::ContainerSummary;

#[derive(Clone)]
pub struct DockerClient {
    client: Docker,
}

impl DockerClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self { client: Docker::connect_with_local_defaults()? })
    }

    pub async fn list_containers(&self, show_all: bool) -> Result<Vec<ContainerSummary>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.client.list_containers(Some(ListContainersOptions::<String> { 
            all: show_all, 
            ..Default::default() 
        })).await?)
    }

    pub async fn start_container(&self, container_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client.start_container(container_id, None::<StartContainerOptions<String>>).await?;
        Ok(())
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
}
