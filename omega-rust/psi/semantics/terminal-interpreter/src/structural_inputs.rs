//! Every structural input one artifact execution starts with, and the
//! binding of each kind before invocation custody is committed.
//!
//! [`TerminalStructuralInputs`] is the record the entries in
//! `terminal_interpreter` accept. `scalar_fields` binds host field contents
//! by runtime referent and validates entry restrictions, `byte_arrays` the
//! explicit initialized fixed-array contents, `case_membership` the selected
//! case of a bounded sum, and `placed_views` the established placed-view
//! referents; each is bound only when it is given.

#[cfg(test)]
mod argument_binding_tests;
pub(crate) mod byte_arrays;
pub(crate) mod case_membership;
#[cfg(test)]
mod placed_view_tests;
pub(crate) mod placed_views;
pub(crate) mod scalar_fields;

use crate::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
use crate::structural_inputs::case_membership::TerminalStructuralCaseValue;
use crate::structural_inputs::placed_views::TerminalPlacedViewEstablishment;
use crate::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
use crate::values::{
    TerminalStructuralBooleanFieldValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

/// Exact initialized contents supplied by the embedding host for structural
/// entry arguments. All paths remain rooted in the original referents.
/// Every structural input one artifact execution starts with. The default is
/// no structural input at all; each field is bound only when it is given.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    pub scalar_fields: &'input [TerminalStructuralScalarFieldValue],
    /// Boolean fields are bound as scalar fields; a rejected boolean field is
    /// reported as a boolean-field argument error.
    pub boolean_fields: &'input [TerminalStructuralBooleanFieldValue],
    pub primitive_values: &'input [TerminalStructuralPrimitiveValue],
    pub cases: &'input [TerminalStructuralCaseValue],
    pub byte_arrays: &'input [TerminalStructuralByteArrayValue],
    /// Established placed-view inputs: one exact establishment per direct
    /// entry roster row lends its qualified referent backing for the
    /// invocation's duration. An entry declaring no placed-view inputs leaves
    /// this empty.
    pub placed_view_establishments: &'input [TerminalPlacedViewEstablishment],
}
