//! Every structural input one artifact execution starts with, and the
//! binding of each kind before invocation custody is committed.
//!
//! [`TerminalStructuralInputs`] is the record the entries in
//! `terminal_interpreter` accept. `scalar_fields` binds host field contents
//! by runtime referent and validates entry restrictions, `byte_arrays` the
//! explicit initialized fixed-array contents, `case_membership` the selected
//! case of a bounded sum, and `placed_views` the established placed-view
//! referents; each is bound only when it is given.

pub(crate) mod byte_arrays;
pub(crate) mod case_membership;
pub(crate) mod placed_views;
pub(crate) mod scalar_fields;
pub(crate) mod zero_initialized;

use crate::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
use crate::structural_inputs::case_membership::TerminalStructuralCaseValue;
use crate::structural_inputs::placed_views::TerminalPlacedViewEstablishment;
use crate::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
use crate::values::{
    TerminalStructuralBooleanFieldValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

/// Actual host-established erased custody beside zero-backed entry data.
/// The host must justify these exact occurrences from admitted root receipts;
/// carrier identity or zero bytes alone never supplies their terms/bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEstablishedErasedField {
    pub argument_index: u32,
    pub field_path: Vec<String>,
    pub carrier_type_identity: String,
}

/// Exact initialized contents supplied by the embedding host for structural
/// entry arguments. All paths remain rooted in the original referents.
/// Every structural input one artifact execution starts with. The default is
/// no structural input at all; each field is bound only when it is given.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    /// Dense positions whose supplied backing the host actually established
    /// with zero contents. A type declaration alone never initializes backing.
    /// Identities and qualifications still come from the supplied carriers;
    /// this lane establishes neither grants nor capability bindings.
    pub zero_initialized_arguments: &'input [u32],
    pub established_erased_fields: &'input [TerminalEstablishedErasedField],
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
    /// Explicit checked demand to construct original private typed entry storage.
    pub private_entry_ram: &'input [terminal_installation::CheckedPrivateEntryRamRecipe],
}
