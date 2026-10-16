//! Core logic for managing named hosts-file sets.
//!
//! The on-disk layout mirrors GasMask (`~/Library/Gas Mask/{Local,Remote,Combined}/*.hst`)
//! so existing sets can be reused as-is.

pub mod combine;
pub mod manager;
pub mod platform;
pub mod remote;
pub mod settings;
pub mod store;

pub use combine::{combine, CombinedSource};
pub use manager::{Manager, ManagerError};
pub use remote::{RemoteEntry, RemoteRegistry};
pub use settings::Settings;
pub use store::{HostsSet, SetKind, Store, StoreError};
