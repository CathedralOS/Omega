//! Exact declaration selection that lexical lookup alone cannot settle.
//!
//! `Lowerer::finish` runs these in a fixed order. Operator homes are selected
//! before symbol assignment; the rest run once every declaration has a
//! symbol: the authored-selection ledger, nominal machine-parameter
//! requirements, closed conformance rows, domain establishment routes, and
//! service reaches. `signature_free_requirements` is the shared law for paths
//! that name a trait requirement without a call signature.

pub(crate) mod authored_selections;
pub(crate) mod conformance_blocks;
pub(crate) mod domain_establishment;
pub(crate) mod domain_operator_homes;
pub(crate) mod machine_parameter_requirements;
pub(crate) mod service_reaches;
pub(crate) mod signature_free_requirements;
