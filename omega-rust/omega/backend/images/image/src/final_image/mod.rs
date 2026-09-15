//! The final image record: sections, memory layout, symbols, relocations, and
//! the placed executable/data region inventories that emission and
//! publication read. The crate root re-exports the public surface by name.

mod data_regions;
mod executable_regions;
mod layout;
mod memory;
mod relocations;
mod root;
mod symbols;

pub use data_regions::*;
pub use executable_regions::*;
pub use layout::*;
pub use memory::*;
pub use relocations::*;
pub use root::*;
pub use symbols::*;
