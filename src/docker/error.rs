#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DockerError {
    #[error("Not found: {message}")]
    NotFound { message: String },
    #[error("Conflict: {message}")]
    Conflict { message: String },
    #[error("Cannot reach the Docker daemon ({details})")]
    DaemonUnreachable { details: String },
    #[error("Docker error {status_code}: {message}")]
    Server { status_code: u16, message: String },
    #[error("Something went wrong: {message}")]
    Other { message: String },
}

pub type DockerResult<T> = std::result::Result<T, DockerError>;

impl From<bollard::errors::Error> for DockerError {
    fn from(err: bollard::errors::Error) -> Self {
        match &err {
            bollard::errors::Error::DockerResponseServerError {
                status_code: 404,
                message,
            } => DockerError::NotFound {
                message: message.clone(),
            },
            bollard::errors::Error::DockerResponseServerError {
                status_code: 409,
                message,
            } => DockerError::Conflict {
                message: message.clone(),
            },
            bollard::errors::Error::DockerResponseServerError {
                status_code,
                message,
            } => DockerError::Server {
                status_code: *status_code,
                message: message.clone(),
            },
            bollard::errors::Error::SocketNotFoundError(_)
            | bollard::errors::Error::UnsupportedURISchemeError { .. }
            | bollard::errors::Error::IOError { .. }
            | bollard::errors::Error::HyperResponseError { .. }
            | bollard::errors::Error::RequestTimeoutError => DockerError::DaemonUnreachable {
                details: err.to_string(),
            },
            _ => DockerError::Other {
                message: err.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_error_404_maps_to_not_found() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            message: "No such volume: foo".into(),
        };
        let mapped: DockerError = err.into();
        assert_eq!(
            mapped,
            DockerError::NotFound {
                message: "No such volume: foo".into()
            }
        );
        assert_eq!(mapped.to_string(), "Not found: No such volume: foo");
    }

    #[test]
    fn server_error_409_maps_to_conflict() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 409,
            message: "remove foo: volume is in use - [abc123def456]".into(),
        };
        let mapped: DockerError = err.into();
        assert_eq!(
            mapped,
            DockerError::Conflict {
                message: "remove foo: volume is in use - [abc123def456]".into()
            }
        );
        assert_eq!(
            mapped.to_string(),
            "Conflict: remove foo: volume is in use - [abc123def456]"
        );
    }

    #[test]
    fn other_status_codes_map_to_server() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 500,
            message: "internal server error".into(),
        };
        let mapped: DockerError = err.into();
        assert_eq!(
            mapped,
            DockerError::Server {
                status_code: 500,
                message: "internal server error".into()
            }
        );
        assert_eq!(
            mapped.to_string(),
            "Docker error 500: internal server error"
        );
    }

    #[test]
    fn socket_not_found_and_timeout_map_to_daemon_unreachable() {
        let socket_err: DockerError =
            bollard::errors::Error::SocketNotFoundError("/var/run/docker.sock".into()).into();
        assert!(matches!(socket_err, DockerError::DaemonUnreachable { .. }));
        assert!(
            socket_err
                .to_string()
                .starts_with("Cannot reach the Docker daemon (")
        );

        let timeout_err: DockerError = bollard::errors::Error::RequestTimeoutError.into();
        assert!(matches!(timeout_err, DockerError::DaemonUnreachable { .. }));
        assert!(
            timeout_err
                .to_string()
                .starts_with("Cannot reach the Docker daemon (")
        );

        let io_err: DockerError = bollard::errors::Error::IOError {
            err: std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
        }
        .into();
        assert!(matches!(io_err, DockerError::DaemonUnreachable { .. }));
        assert!(
            io_err
                .to_string()
                .starts_with("Cannot reach the Docker daemon (")
        );
    }

    #[test]
    fn unmapped_variants_fall_back_to_other() {
        let err: DockerError = bollard::errors::Error::APIVersionParseError {}.into();
        assert!(matches!(err, DockerError::Other { .. }));
        assert!(err.to_string().starts_with("Something went wrong: "));
    }
}
