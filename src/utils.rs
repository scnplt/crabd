use bollard::secret::ContainerStateStatusEnum;

pub fn is_container_running(state: ContainerStateStatusEnum) -> bool {
    state == ContainerStateStatusEnum::RUNNING
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_container_running_only_true_for_running() {
        assert!(is_container_running(ContainerStateStatusEnum::RUNNING));
        assert!(!is_container_running(ContainerStateStatusEnum::RESTARTING));
        assert!(!is_container_running(ContainerStateStatusEnum::REMOVING));
    }
}
