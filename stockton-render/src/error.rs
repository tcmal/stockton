//! Error types

use anyhow;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockPoisoned {
    #[error("Device lock poisoned")]
    Device,

    #[error("Map lock poisoned")]
    Map,

    #[error("Queue lock poisoned")]
    Queue,

    #[error("Other lock poisoned")]
    Other,
}

/// Indicates the given property has no acceptable values
#[derive(Debug, Error)]
pub enum EnvironmentError {
    #[error("No supported color format")]
    ColorFormat,

    #[error("No supported depth format")]
    DepthFormat,

    #[error("No supported present mode")]
    PresentMode,

    #[error("No supported composite alpha mode")]
    CompositeAlphaMode,

    #[error("No suitable queue families found")]
    NoSuitableFamilies,

    #[error("No suitable memory types found")]
    NoMemoryTypes,

    #[error("Couldn't use shaderc")]
    NoShaderC,

    #[error("No suitable queues")]
    NoQueues,
}

/// Indicates an issue with the level object being used
#[derive(Debug, Error)]
pub enum LevelError {
    #[error("Referential Integrity broken")]
    BadReference,
}

pub fn full_error_display(err: anyhow::Error) -> String {
    let cont = err
        .chain()
        .skip(1)
        .map(|cause| format!("    caused by: {}", cause))
        .collect::<Vec<String>>()
        .join("\n");

    format!("Error: {}\n{}", err, cont)
}
