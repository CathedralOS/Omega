//! Compile accepted package inputs without reconstructing package review.
//!
//! The 2026-10-03 owner ruling makes review an explicit acceptance operation.
//! Matching source and compiler identities select retained compilation inputs;
//! they do not prove generated code, supply host grants, or waive Psi checking.

mod identity;
pub(crate) use identity::compiler_input_identity;








