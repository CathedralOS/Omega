//! Ordered custody boundaries from target selection through native artifacts.

pub(crate) mod allocation;
pub(crate) mod artifacts;
pub(crate) mod encoding;
pub(crate) mod layout;
pub(crate) mod machine;
pub(crate) mod realization;
pub(crate) mod selection;

pub use allocation::*;
pub use artifacts::*;
pub use encoding::*;
pub use layout::*;
pub use machine::*;
pub use realization::*;
pub use selection::*;
