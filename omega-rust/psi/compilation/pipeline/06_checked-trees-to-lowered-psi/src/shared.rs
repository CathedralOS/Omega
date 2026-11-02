//! What every plan family reads: `lowering_error` (the failure vocabulary),
//! `terminal_identities` (fixed Terminal identity allocation and
//! checked-to-lowered lookup), `producer_result` (what a family hands back),
//! `declarations` (a checked state symbol to its machine and state),
//! `direct_calls` (the one mapping from a direct call's facts to its Terminal
//! operation), `structural_types` (checked structural plans as Terminal
//! declarations), `checked_facts`, `content_classes`, `reference_custody`,
//! `call_sites`, `call_operands` and `activation_storage` (readers over the
//! checked tables), and `source_guards` (what lowering may read and refuse).

pub(crate) mod activation_storage;
pub(crate) mod call_operands;
pub(crate) mod call_sites;
pub(crate) mod checked_facts;
pub(crate) mod content_classes;
pub(crate) mod declarations;
pub(crate) mod direct_calls;
pub(crate) mod lowering_error;
pub(crate) mod producer_result;
pub(crate) mod reference_custody;
#[cfg(test)]
mod source_guards;
pub(crate) mod structural_types;
pub(crate) mod terminal_identities;
