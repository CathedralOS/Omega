//! Contract clauses on machines, signatures and states: `parse_contract_clauses`
//! reads a machine's clauses, `signature` a signature's, `state_arrival` a
//! state's arrival contracts, `facts` the proof-fact lists they share, and
//! `conformance` the `satisfies` clauses and external provider bindings.

pub(crate) mod conformance;
pub(crate) mod facts;
pub(crate) mod parse_contract_clauses;
pub(crate) mod signature;
pub(crate) mod state_arrival;
