//! Input and value homes, writes, and stack execution evidence.
//! `frame_layout` holds raw geometry and prerequisite records; the backend
//! validates these before choosing or applying any executable frame protocol.

pub mod frame_application;
pub mod frame_identity;
pub mod frame_layout;
pub mod parameters;
pub mod scalars;
pub mod stack;
pub mod stores;

pub use frame_application::*;
pub use frame_identity::*;
pub use frame_layout::*;
pub use parameters::*;
pub use scalars::*;
pub use stack::*;
pub use stores::*;
