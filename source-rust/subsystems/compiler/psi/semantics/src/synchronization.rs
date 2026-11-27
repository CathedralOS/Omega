//! A wait on unchanged activation-private storage has no external releaser.
//! Follow definite execution from the selected entry. Stop where another
//! operation might publish an alias, change the storage, or divert execution.

mod facts;
mod private_cells;
mod progress;
mod selected;
mod spin;

pub use private_cells::{private_wait_failure, stuck_private_wait};
pub use progress::{PrivateWaitFailure, ProgressFailure};
pub use selected::selected_spin_wait;
pub use spin::{SpinWait, spin_wait};
