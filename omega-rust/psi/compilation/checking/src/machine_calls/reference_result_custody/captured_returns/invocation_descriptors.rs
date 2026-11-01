//! A completed independent result descriptor is not an input-relative loan.
//! Its actual invocation establishes the descriptor under the checked public
//! lifetime contract. Whole-value copies retain that occurrence; its pointee
//! remains opaque. This publishes neither a caller storage address nor access
//! beyond the returned carrier's already checked qualifications.

use super::{
    Captures, CheckedReferenceOriginSourcePlan, CheckedReferenceResultSourcePlan, SymbolHandle,
};

impl Captures<'_> {
    pub(super) fn completed_invocation_descriptor(
        &self,
        capture: &checked_trees::FlowOperandFact,
        leaf: &CheckedReferenceResultSourcePlan,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Option<CheckedReferenceOriginSourcePlan>> {
        let source = self.completed_operand_source(capture)?;
        if self
            .program
            .expression_table
            .call_node(source.expression)
            .is_none()
        {
            return Some(None);
        }
        let source_state = self.program.state_by_symbol(source.state_symbol)?;
        let call = self.return_call(
            source_state,
            source.statement_index,
            Default::default(),
            source.expression,
        )?;
        if call.authored_result == checked_trees::FlowCallResult::Discarded {
            return None;
        }
        let returned = self.target_sources(call.target_symbol, active)?;
        let independent = returned
            .iter()
            .filter(|returned| {
                returned.path == leaf.path
                    && matches!(
                        returned.source.root,
                        checked_trees::CheckedReferenceOriginRoot::Invocation { .. }
                    )
            })
            .collect::<Vec<_>>();
        if independent.is_empty() {
            return Some(None);
        }
        // This query establishes independent descriptors only. A mixed route
        // also owes exact input substitution; its independent alternative must
        // never make an unaccounted parameter-relative source disappear.
        if returned.iter().any(|returned| {
            returned.path == leaf.path
                && matches!(
                    returned.source.root,
                    checked_trees::CheckedReferenceOriginRoot::Parameter { .. }
                )
        }) {
            return None;
        }
        if independent.iter().any(|returned| {
            returned.source.type_identity != leaf.source.type_identity
                || !super::super::reference_access_can_supply(
                    returned.source.access,
                    leaf.source.access,
                )
        }) {
            return None;
        }
        self.invocation_descriptor_origin(source.type_reference, leaf)
            .map(Some)
    }

    /// Re-root the independent descriptor at this receiving invocation. Its
    /// concrete pointer/extent remain in the returned value. Callee-local
    /// invocation identities cannot escape into the caller's namespace.
    pub(super) fn invocation_descriptor_origin(
        &self,
        result: typed_trees::types::TypeReferenceHandle,
        leaf: &CheckedReferenceResultSourcePlan,
    ) -> Option<CheckedReferenceOriginSourcePlan> {
        let segments = super::super::loan_place_segments(self.program, result, &leaf.path)?;
        let carrier = crate::project_type_reference_from_segments(self.program, result, &segments)?;
        let shell = crate::reference_shell(self.program, carrier)?;
        if self.program.normalized_type_identity(shell.referee) != leaf.source.type_identity
            || !super::super::reference_access_can_supply(shell.access.into(), leaf.source.access)
        {
            return None;
        }
        Some(CheckedReferenceOriginSourcePlan {
            root: checked_trees::CheckedReferenceOriginRoot::Invocation {
                carrier_type: self.program.normalized_type_identity(carrier),
            },
            entry_reference_hops: 0,
            path: vec![checked_trees::CheckedReferenceOriginProjection::Exact(
                checked_trees::CheckedUnitStructuralPathSegment::Referent,
            )],
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            type_identity: leaf.source.type_identity,
            access: leaf.source.access,
        })
    }
}
