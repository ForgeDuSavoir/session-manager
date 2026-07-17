//! Generic adapter boundary.
//!
//! Concrete adapter protocol DTOs are defined in M3. This module is the only place
//! where application code will depend on adapter behavior.

/// A typed boundary for future adapter coordination.
pub trait AdapterPort: Send + Sync {}
