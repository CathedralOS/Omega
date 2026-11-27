//! A wait on unchanged activation-private storage has no external releaser.
//! Follow definite execution from the selected entry. Stop where another
//! operation might publish an alias, change the storage, or divert execution.

mod component;
mod facts;
mod private_cells;
mod progress;
mod selected;
mod spin;

pub use component::{component_guard_order, component_spin_wait};
pub use private_cells::{GuardOrder, private_wait_failure};
pub use progress::{PrivateWaitFailure, ProgressFailure, ProgressScope};
pub use selected::{SelectedCall, selected_spin_wait};
pub use spin::{SpinWait, spin_wait};
