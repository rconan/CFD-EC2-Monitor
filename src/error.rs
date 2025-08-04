//! Error types for EC2 Monitor

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("AWS SDK error: {0}")]
    AwsSdk(String),

    #[error("SSH connection error: {0}")]
    SshConnection(#[from] ssh2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Environment variable error: {0}")]
    Env(#[from] std::env::VarError),

    #[error("Parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Parse float error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("No public IP available for instance")]
    NoPublicIp,

    #[error("SSH key file not found: {path}")]
    KeyFileNotFound { path: String },

    #[error("SSH authentication failed")]
    AuthenticationFailed,

    #[error("Invalid wind speed: {speed}. Valid speeds are 2m/s, 7m/s, 12m/s, or 17m/s")]
    InvalidWindSpeed { speed: String },

    #[error("SSH command failed with exit code {code}: {stderr}")]
    SshCommandFailed { code: i32, stderr: String },

    #[error("Timestep parsing failed: {reason}")]
    TimestepParsing { reason: String },

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Tmux session launch failed: {reason}")]
    TmuxLaunchFailed { reason: String },
}