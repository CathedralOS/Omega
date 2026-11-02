//! Settling native evidence against already-produced Terminal artifacts:
//! `entry_settlement` and `callback_settlement` admit native entry and callback
//! sidecars, `boundary_application_settlement` selects realizations from
//! canonical Terminal demands, and `comparison_settlement` binds Terminal
//! comparisons to the boundary executions those settled.

pub(crate) mod boundary_application_settlement;
pub(crate) mod callback_settlement;
pub(crate) mod comparison_settlement;
pub(crate) mod entry_settlement;
