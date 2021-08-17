//! Error types

use thiserror::Error;

/// An error caused by a lock being poisoned.
/// This indicates an issue somewhere else, in another thread.
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

    #[error("Memory pool lock poisoned")]
    MemoryPool,
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

    #[error("Memory pool missing")]
    MemoryPoolMissing,
}

/// Indicates invalid usage of an API.
#[derive(Debug, Error)]
pub enum UsageError {
    #[error("Attempt to create mappable memory block from non-mappable memory")]
    NonMappableMemory,

    #[error("Called get_queue without properly requesting the queue beforehand.")]
    QueueNegotiatorMisuse,
}

/// Displays an error with full backtrace
pub fn full_error_display(err: anyhow::Error) -> String {
    let cont = err
        .chain()
        .skip(1)
        .map(|cause| format!("    caused by: {}", cause))
        .collect::<Vec<String>>()
        .join("\n");

    format!("Error: {}\n{}", err, cont)
}
