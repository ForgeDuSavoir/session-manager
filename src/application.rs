//! Use-case orchestration over typed ports and domain values.
//!
//! Implementations are added once M1 defines application behavior.

use crate::storage_port::{Clock, UuidGenerator};

/// Dependencies available to application services.
pub struct ApplicationPorts<C, U> {
    /// Injected source of wall-clock time.
    pub clock: C,
    /// Injected source of identifiers.
    pub uuid_generator: U,
}

impl<C: Clock, U: UuidGenerator> ApplicationPorts<C, U> {
    /// Constructs a port bundle without performing I/O.
    #[must_use]
    pub fn new(clock: C, uuid_generator: U) -> Self {
        Self {
            clock,
            uuid_generator,
        }
    }
}
