//! The checked body a scalar call lowers.
use super::super::CheckedUnitStructuralParameterPlan;
use super::{CheckedTrees, LoweringError, PrimitiveType, unsupported};
use crate::unit::attached_unit::bodies::UnitBody;

/// The Unit body that owns a scalar callee's completion: an ordinary
/// single-state body's scalar result or control, or a state graph's scalar
/// result. The shared Unit assembler emits it with the closure. A machine
/// without such a body has no executable scalar lowering; its Unit plan gap
/// names the construct planning stopped at.
pub(crate) struct CheckedScalarCallee<'checked>(UnitBody<'checked>);

impl<'checked> CheckedScalarCallee<'checked> {
    pub(crate) fn find(
        checked: &'checked CheckedTrees,
        source: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let callee = Self::find_for_unit_call(checked, source)?;
        if !callee.structural_parameters().is_empty() || !callee.entry_claims().is_empty() {
            return unsupported("scalar callee requires structural call custody");
        }
        Ok(callee)
    }

    /// Select a body whose caller retains an explicit structural transfer lane.
    /// Scalar-only edges must continue to use the narrower `find` entry.
    pub(crate) fn find_for_unit_call(
        checked: &'checked CheckedTrees,
        source: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let mut operation_bodies = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .chain(
                checked
                    .facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
            .filter(|plan| plan.machine == source);
        let operation_body = operation_bodies.next();
        let mut graph_bodies = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .filter(|plan| plan.machine == source);
        let graph_body = graph_bodies.next();
        if operation_bodies.next().is_some()
            || graph_bodies.next().is_some()
            || (operation_body.is_some() && graph_body.is_some())
        {
            return unsupported("scalar callee has duplicate checked body ownership");
        }
        if let Some(plan) = graph_body.filter(|plan| {
            matches!(
                plan.result,
                checked_trees::CheckedControlResultPlan::Scalar { .. }
            )
        }) {
            return Ok(Self(UnitBody::Composed(plan)));
        }
        if let Some(plan) = operation_body.filter(|plan| {
            matches!(
                plan.result,
                checked_trees::CheckedControlResultPlan::Scalar { .. }
            )
        }) {
            if plan.structural_result.is_some() {
                return unsupported("scalar callee has conflicting checked result ownership");
            }
            return Ok(Self(UnitBody::Ordinary(plan)));
        }
        Err(LoweringError::InvalidUnitMachinePlan {
            machine: checked.symbols.display_path(source, "::"),
            reason: "scalar callee has no checked executable body",
            gap: checked
                .facts
                .flow
                .terminal_unit_effects
                .gap_diagnostic(source, &checked.symbols),
        })
    }

    pub(crate) fn body(&self) -> UnitBody<'checked> {
        self.0
    }

    pub(crate) fn structural_parameters(&self) -> &'checked [CheckedUnitStructuralParameterPlan] {
        self.0
            .entry()
            .map_or(&[][..], |entry| entry.structural_parameters)
    }

    pub(crate) fn entry_claims(&self) -> &[checked_trees::CheckedUnitEntryClaimPlan] {
        self.0.entry().map_or(&[][..], |entry| entry.entry_claims)
    }

    pub(crate) fn entry_state(&self) -> Result<symbols::SymbolHandle, LoweringError> {
        Ok(self.0.entry()?.state)
    }

    pub(crate) fn parameter_types(&self) -> Result<Vec<PrimitiveType>, LoweringError> {
        Ok(self
            .0
            .entry()?
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.primitive_type)
            .collect())
    }

    /// Proof-only erased scalar formals as (authored position, primitive type)
    /// pairs in authored order. Empty for callees with no erased roster.
    pub(crate) fn erased_parameters(&self) -> Vec<(u32, PrimitiveType)> {
        self.0
            .entry()
            .map(|entry| {
                entry
                    .erased_scalar_parameters
                    .iter()
                    .map(|parameter| (parameter.source_position, parameter.primitive_type))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Proof-only erased formals in authored order. Empty for callees with
    /// no erased-proof roster.
    pub(crate) fn erased_proof_parameters(
        &self,
    ) -> Vec<&checked_trees::CheckedErasedProofParameterPlan> {
        self.0
            .entry()
            .map(|entry| entry.erased_proof_parameters.iter().collect())
            .unwrap_or_default()
    }
}
