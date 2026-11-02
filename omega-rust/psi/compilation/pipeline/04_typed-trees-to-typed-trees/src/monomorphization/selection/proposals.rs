//! The static specialization proposal ledger.
//!
//! One inference round names, per operand occurrence, the candidate and binder
//! ordinal it fills: a proposal. The five binder families are one transport so
//! every producer (explicit machine operands, argument compatibility, declared
//! range endpoints, domain constraints) appends to the family it derives and
//! assembly reads them together, instead of five parallel vectors threaded
//! through every signature. A proposal is evidence for one tuple; admission
//! remains immutable checking's judgment.
//!
//! [`BindingContext`] is the invariant half of the same relation: the program,
//! one candidate's binder rosters, its authored fixed-range positions and the
//! candidate ordinal every proposal of this round names.

use super::{StaticMachineArgument, SymbolHandle, TypeReferenceHandle};

/// Which candidate and binder ordinal an inferred type, const or runtime-value
/// operand fills.
pub(crate) type ValueProposal = (usize, usize, StaticMachineArgument);

/// Which candidate and binder ordinal an inferred type or const operand fills.
pub(crate) type TypeProposal = (usize, usize, TypeReferenceHandle);

/// One candidate's static specialization proposals, kept by binder family.
#[derive(Default)]
pub(crate) struct StaticProposals {
    pub(crate) types: Vec<TypeProposal>,
    pub(crate) constants: Vec<TypeProposal>,
    pub(crate) machines: Vec<ValueProposal>,
    pub(crate) evidence: Vec<ValueProposal>,
    pub(crate) runtime_values: Vec<(usize, usize, crate::monomorphization::RuntimeValueBinding)>,
}

/// The invariant inputs of one candidate's binding inference.
#[derive(Clone, Copy)]
pub(crate) struct BindingContext<'context> {
    pub(crate) program: &'context dyn validation::CheckingInput,
    pub(crate) type_parameters: &'context [(SymbolHandle, String)],
    pub(crate) const_parameters: &'context [(SymbolHandle, String, TypeReferenceHandle)],
    pub(crate) fixed_range_parameters: Option<&'context [usize]>,
    pub(crate) candidate_index: usize,
}

impl<'context> BindingContext<'context> {
    pub(crate) fn new(
        program: &'context dyn validation::CheckingInput,
        type_parameters: &'context [(SymbolHandle, String)],
        const_parameters: &'context [(SymbolHandle, String, TypeReferenceHandle)],
        fixed_range_parameters: Option<&'context [usize]>,
        candidate_index: usize,
    ) -> Self {
        Self {
            program,
            type_parameters,
            const_parameters,
            fixed_range_parameters,
            candidate_index,
        }
    }

    /// The same candidate while a constrained shell is inspected whole: a
    /// peeled range or policy must not expose a different endpoint.
    pub(crate) fn without_fixed_ranges(self) -> Self {
        Self {
            fixed_range_parameters: None,
            ..self
        }
    }
}
