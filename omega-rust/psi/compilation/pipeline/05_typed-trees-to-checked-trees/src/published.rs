//! What checking publishes beside the typed program: `facts` (CheckFacts, the
//! checked program's knowledge of its own values), `proof` (evidence,
//! proof-output and outcome facts), `semantic` (semantic facts), `places` (the
//! place a value expression denotes in one state) and `labels` (the ordered
//! label rosters those tables are keyed by).

pub(crate) mod facts;
pub(crate) mod labels;
pub(crate) mod places;
pub(crate) mod proof;
pub(crate) mod semantic;
