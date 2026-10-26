//! Scalar consumers retain a checked value dependency, never a physical home.
//! The common producer publishes the complete binding at its actual operand
//! position; observations and calls then consume that binding.

use super::super::StructuralTypeId;
use super::{CheckedTrees, LoweringError};
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarEffect,
};
use crate::scalar_lowering::computations::Expansion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueRequest {
    pub(crate) value: checked_trees::CheckedStructuralValueHandle,
    pub(crate) structural_type: StructuralTypeId,
    captured: bool,
}

impl ValueRequest {
    pub(crate) fn prepare(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        value: checked_trees::CheckedStructuralValueHandle,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Self, LoweringError> {
        let root = checked
            .facts
            .values
            .structural_values
            .root_for_value(value)
            .ok_or(LoweringError::Unsupported(
                "computed value lost its complete checked root",
            ))?;
        let identity = validation::stored_structural_type_identity(
            &checked.typed,
            root.type_reference,
            &[],
            &[],
        );
        let declaration = crate::terminal_identities::structural_declaration_by_identity(
            types,
            identities,
            checked,
            identity,
            "computed value has no produced carrier",
        )?;
        Ok(Self {
            value,
            structural_type: declaration.id,
            captured: root.captured_copy.is_valid(),
        })
    }

    pub(crate) fn finish(
        self,
        source_types: &[semantic_vocabulary::QualifiedScalarType],
        target: usize,
        expansion: &mut Expansion<'_>,
    ) -> usize {
        self.finish_with_alias(
            typed_trees::expression::ExpressionHandle::invalid(),
            source_types,
            target,
            expansion,
        )
    }

    /// Bind a lexical saved-subject name only after its real value completes.
    /// The alias names the same home; it creates no storage, loan or owner.
    pub(crate) fn finish_with_alias(
        self,
        alias: typed_trees::expression::ExpressionHandle,
        source_types: &[semantic_vocabulary::QualifiedScalarType],
        target: usize,
        expansion: &mut Expansion<'_>,
    ) -> usize {
        let mut effects = Vec::new();
        if !self.captured && !expansion.has_established_value(self.value) {
            effects.push(LoweredScalarEffect::ProduceStructuralValue {
                value: self.value,
                structural_type: self.structural_type,
            });
        }
        if alias.is_valid() {
            effects.push(LoweredScalarEffect::BindCompletedValueAlias {
                value: self.value,
                alias,
            });
        }
        if effects.is_empty() {
            return target;
        }
        expansion.push(LoweredScalarBranchState::jumping(
            target,
            Vec::new(),
            source_types.to_vec(),
            source_types,
            effects,
        ))
    }
}
