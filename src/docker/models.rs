use std::collections::HashMap;
use std::fmt;

use bollard::secret::{
    ContainerStateStatusEnum, MountPoint, MountPointTypeEnum, Port, PortBinding, PortTypeEnum,
};

/// Parses the loosely-typed `state` string coming from the Docker API into the
/// typed enum, falling back to `EMPTY` when missing or unrecognized.
pub fn parse_container_state(state: Option<&str>) -> ContainerStateStatusEnum {
    state
        .and_then(|s| s.parse().ok())
        .unwrap_or(ContainerStateStatusEnum::EMPTY)
}

/// Human-readable label for a container state, used only for table rendering.
pub fn state_label(state: ContainerStateStatusEnum) -> String {
    if state == ContainerStateStatusEnum::EMPTY {
        "-".to_string()
    } else {
        state.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortMapping {
    pub private: u16,
    pub public: u16,
    pub protocol: PortTypeEnum,
}

impl fmt::Display for PortMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/{}", self.private, self.public, self.protocol)
    }
}

impl PortMapping {
    pub fn from_summary_ports(ports: &[Port]) -> Vec<PortMapping> {
        let mut filtered_ports: Vec<PortMapping> = ports
            .iter()
            .filter_map(|p| {
                Some(PortMapping {
                    private: p.private_port,
                    public: p.public_port?,
                    protocol: p.typ?,
                })
            })
            .collect();

        filtered_ports.sort_by_key(|p| p.private);
        filtered_ports.dedup();

        filtered_ports
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    pub host_ip: String,
    pub host_port: String,
}

impl fmt::Display for HostBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host_ip, self.host_port)
    }
}

impl HostBinding {
    fn from_binding(binding: &PortBinding) -> HostBinding {
        HostBinding {
            host_ip: binding.host_ip.clone().unwrap_or_default(),
            host_port: binding.host_port.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortConfig {
    pub container_port: String,
    pub protocol: String,
    pub ipv4: Option<HostBinding>,
    pub ipv6: Option<HostBinding>,
}

impl fmt::Display for PortConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.ipv4, &self.ipv6) {
            (Some(ipv4), Some(ipv6)) => write!(
                f,
                "{ipv4} | {ipv6} -> {}/{}",
                self.container_port, self.protocol
            ),
            (Some(ipv4), None) => {
                write!(f, "{ipv4} -> {}/{}", self.container_port, self.protocol)
            }
            (None, Some(ipv6)) => {
                write!(f, "{ipv6} -> {}/{}", self.container_port, self.protocol)
            }
            (None, None) => write!(f, ""),
        }
    }
}

impl PortConfig {
    pub fn from_port_map(ports: &HashMap<String, Option<Vec<PortBinding>>>) -> Vec<PortConfig> {
        ports
            .iter()
            .filter_map(|(port, bindings)| {
                let (ipv4_binding, ipv6_binding) = bindings
                    .as_ref()
                    .map(|b| {
                        let ipv4 = b
                            .iter()
                            .find(|pb| pb.host_ip == Some("0.0.0.0".to_string()));
                        let ipv6 = b.iter().find(|pb| pb.host_ip == Some("::".to_string()));
                        (ipv4, ipv6)
                    })
                    .unwrap_or((None, None));

                let ipv4 = ipv4_binding.map(HostBinding::from_binding);
                let ipv6 = ipv6_binding.map(HostBinding::from_binding);

                if ipv4.is_none() && ipv6.is_none() {
                    return None;
                }

                let container_port = port.split('/').next().unwrap_or("").to_string();
                let protocol = port.split('/').nth(1).unwrap_or("").to_string();

                Some(PortConfig {
                    container_port,
                    protocol,
                    ipv4,
                    ipv6,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub source: String,
    pub destination: String,
}

impl fmt::Display for Mount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.source, self.destination)
    }
}

impl Mount {
    pub fn from_mount_points(mount_points: &[MountPoint]) -> Vec<Mount> {
        mount_points
            .iter()
            .map(|mp| {
                let source = match mp.typ {
                    Some(MountPointTypeEnum::VOLUME) => {
                        mp.name.clone().unwrap_or_else(|| "-".to_string())
                    }
                    Some(_) => mp.source.clone().unwrap_or_else(|| "-".to_string()),
                    None => "-".to_string(),
                };

                let destination = mp.destination.clone().unwrap_or_else(|| "-".to_string());

                Mount {
                    source,
                    destination,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_container_state_recognizes_known_values() {
        assert_eq!(
            parse_container_state(Some("running")),
            ContainerStateStatusEnum::RUNNING
        );
        assert_eq!(
            parse_container_state(Some("restarting")),
            ContainerStateStatusEnum::RESTARTING
        );
    }

    #[test]
    fn parse_container_state_falls_back_to_empty() {
        assert_eq!(parse_container_state(None), ContainerStateStatusEnum::EMPTY);
        assert_eq!(
            parse_container_state(Some("bogus")),
            ContainerStateStatusEnum::EMPTY
        );
    }

    #[test]
    fn state_label_formats_correctly() {
        assert_eq!(state_label(ContainerStateStatusEnum::EMPTY), "-");
        assert_eq!(state_label(ContainerStateStatusEnum::RUNNING), "running");
    }

    #[test]
    fn port_mapping_filters_sorts_and_dedups() {
        let ports = vec![
            Port {
                private_port: 8080,
                public_port: Some(80),
                typ: Some(PortTypeEnum::TCP),
                ..Default::default()
            },
            Port {
                private_port: 22,
                public_port: None,
                typ: Some(PortTypeEnum::TCP),
                ..Default::default()
            },
            Port {
                private_port: 53,
                public_port: Some(53),
                typ: None,
                ..Default::default()
            },
            Port {
                private_port: 443,
                public_port: Some(443),
                typ: Some(PortTypeEnum::TCP),
                ..Default::default()
            },
            Port {
                private_port: 8080,
                public_port: Some(80),
                typ: Some(PortTypeEnum::TCP),
                ..Default::default()
            },
        ];

        let mappings = PortMapping::from_summary_ports(&ports);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].private, 443);
        assert_eq!(mappings[1].private, 8080);
        assert_eq!(mappings[1].to_string(), "8080:80/tcp");
    }

    #[test]
    fn port_config_from_port_map_handles_ipv4_only() {
        let mut ports = HashMap::new();
        ports.insert(
            "80/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("8080".to_string()),
            }]),
        );

        let configs = PortConfig::from_port_map(&ports);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].to_string(), "0.0.0.0:8080 -> 80/tcp");
    }

    #[test]
    fn port_config_from_port_map_handles_ipv6_only() {
        let mut ports = HashMap::new();
        ports.insert(
            "80/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("::".to_string()),
                host_port: Some("8080".to_string()),
            }]),
        );

        let configs = PortConfig::from_port_map(&ports);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].to_string(), ":::8080 -> 80/tcp");
    }

    #[test]
    fn port_config_from_port_map_handles_both() {
        let mut ports = HashMap::new();
        ports.insert(
            "80/tcp".to_string(),
            Some(vec![
                PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some("8080".to_string()),
                },
                PortBinding {
                    host_ip: Some("::".to_string()),
                    host_port: Some("8080".to_string()),
                },
            ]),
        );

        let configs = PortConfig::from_port_map(&ports);
        assert_eq!(configs.len(), 1);
        let text = configs[0].to_string();
        assert!(text.contains(" | "));
        assert!(text.ends_with(" -> 80/tcp"));
    }

    #[test]
    fn port_config_from_port_map_skips_missing_bindings() {
        let mut ports = HashMap::new();
        ports.insert("80/tcp".to_string(), None);
        ports.insert("80".to_string(), Some(vec![]));

        let configs = PortConfig::from_port_map(&ports);
        assert!(configs.is_empty());
    }

    #[test]
    fn port_config_from_port_map_handles_key_without_slash() {
        let mut ports = HashMap::new();
        ports.insert(
            "80".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("8080".to_string()),
            }]),
        );

        let configs = PortConfig::from_port_map(&ports);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].protocol, "");
        assert_eq!(configs[0].container_port, "80");
    }

    #[test]
    fn mount_from_mount_points_selects_source_by_type() {
        let mount_points = vec![
            MountPoint {
                typ: Some(MountPointTypeEnum::VOLUME),
                name: Some("my-volume".to_string()),
                source: Some("/var/lib/docker/volumes/my-volume".to_string()),
                destination: Some("/data".to_string()),
                ..Default::default()
            },
            MountPoint {
                typ: Some(MountPointTypeEnum::BIND),
                name: None,
                source: Some("/host/path".to_string()),
                destination: Some("/container/path".to_string()),
                ..Default::default()
            },
            MountPoint {
                typ: None,
                name: None,
                source: None,
                destination: None,
                ..Default::default()
            },
        ];

        let mounts = Mount::from_mount_points(&mount_points);
        assert_eq!(mounts[0].source, "my-volume");
        assert_eq!(mounts[0].destination, "/data");
        assert_eq!(mounts[1].source, "/host/path");
        assert_eq!(mounts[1].destination, "/container/path");
        assert_eq!(mounts[2].source, "-");
        assert_eq!(mounts[2].destination, "-");
    }
}
