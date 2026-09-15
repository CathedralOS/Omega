//! Input-only reconstruction shared by legalization and selection: structural
//! storage and borrowed-pointer geometry, structural parameter shape with
//! incoming ABI storage, and bounded no-observation eligibility. None of it
//! confers ownership or access authority.

pub(crate) mod structural_reference_input;
pub(crate) mod structural_unit_input;
pub(crate) mod unobserved_owned_input;
