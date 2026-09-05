//! Input and value homes, writes, and stack execution evidence.

pub mod frame_application;
pub mod frame_identity;
pub mod parameters;
pub mod scalars;
pub mod stack;
pub mod stores;

pub use frame_application::*;
pub use frame_identity::*;
pub use parameters::*;
pub use scalars::*;
pub use stack::*;
pub use stores::*;
