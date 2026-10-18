//! A scalar observation's checked completion keeps its snapshot, not its loan.
//!
//! The receiver has completed when this continuation runs. Resolve the source
//! action against that exact home and use ordinary cleanup control; a selected
//! arm never creates or retires the other arm's receiver. An already captured
//! operand remains with its outer consumer and does not enter this operation.

use crate::lowering_error::{LoweringError, unsupported};
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use crate::scalar_lowering::branch_states::structural_values::PreparedValue;
use crate::unit::unit_cleanup::PreparedNormalCleanupAction;

impl super::Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observation_completion(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        receiver: checked_trees::CheckedStructuralValueHandle,
        prepared: &PreparedValue,
        selected: arena::HandleSpan<checked_trees::CheckedNormalCleanupAction>,
        parameter_types: Vec<super::QualifiedScalarType>,
        target: usize,
    ) -> Result<usize, LoweringError> {
        if selected.is_empty() {
            return Ok(target);
        }
        let plans = &self.checked.facts.values.scalar_computations;
        let actions =
            plans
                .temporary_cleanup_actions
                .span(selected)
                .ok_or(LoweringError::Unsupported(
                    "scalar observation has stale completion actions",
                ))?;
        let owner = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(receiver)
            .expression;
        let type_ids = actions
            .iter()
            .map(|action| {
                crate::terminal_identities::structural_declaration_by_identity(
                    self.structural_types,
                    identities,
                    self.checked,
                    action.type_identity(),
                    "scalar observation completion has no retained carrier type",
                )
                .map(|declaration| (action.type_identity(), declaration.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let regions = crate::unit::unit_cleanup::lower_checked_cleanup_regions(
            identities,
            self.checked,
            actions,
            &[],
            &self
                .checked
                .facts
                .flow
                .terminal_unit_effects
                .cleanup_control,
            self.structural_types,
            &type_ids,
            |identities, source, path| {
                if *source != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    expression: owner,
                }) {
                    return unsupported("scalar observation completion lost its exact receiver");
                }
                let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                    identities, path, prepared.structural_type, self.structural_types,
                    None, &mut std::iter::empty(), None,
                )?;
                Ok((prepared.place, path))
            },
            |place| {
                if place != prepared.place {
                    return unsupported("scalar observation completion changed its receiver home");
                }
                Ok(prepared.multiplicity)
            },
            // These are intrinsic permission-carrier deaths. Nominal backing
            // owners keep their independently selected source continuations.
            |_, _, _, _| unsupported("descriptor observation completion contains a nominal action"),
        )?;
        Ok(self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            bindings: Vec::new(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            terminator: LoweredScalarBranchTerminator::jump_with_cleanup(
                target,
                super::parameters(&parameter_types),
                regions
                    .into_iter()
                    .map(PreparedNormalCleanupAction::Resolved)
                    .collect(),
            ),
            parameter_types,
        }))
    }
}
