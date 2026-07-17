//! Abstract persistence-adjacent services required by application code.

/// A clock injected into application services.
pub trait Clock {
    /// Returns Unix time in milliseconds.
    fn now_unix_ms(&self) -> u64;
}

/// An identifier source injected into application services.
pub trait UuidGenerator {
    /// Returns a canonical UUID string supplied by the implementation.
    fn next_uuid(&mut self) -> String;
}
