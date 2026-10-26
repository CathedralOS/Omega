//! Reuse an evaluated cleanup discriminator within its unchanged typed scope.
//!
//! The scalar snapshot is ordinary control, not current payload availability.
//! Scope restoration owns dominance; the complete emitted prefix and cleanup
//! actions authenticate storage-epoch preservation. Unknown effects, indirect
//! addressing, whole-home completion and nominal hooks refuse reuse. The caller
//! then emits the original observation with every original ownership check.

use super::{
    CleanupEmission, LoweringError, OperationBuffer, OperationKind, TerminalAffineCleanupAction,
    ValueDeclaration,
};

impl CleanupEmission<'_> {
    /// Rejoin the exact physical declaration before using an observed tag or
    /// eliding a sole-case observation. Declarations survive home retirement;
    /// this metadata query does not assert payload liveness or membership.
    pub(super) fn case_subject(
        &self,
        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
        operations: &OperationBuffer,
    ) -> Result<&terminal_psi::StructuralTypeDeclaration, LoweringError> {
        let mut declarations = self
            .source_parameters
            .iter()
            .map(|(_, declaration)| declaration)
            .chain(self.previous_parameter_homes.iter())
            .chain(self.structural_parameters.iter())
            .chain(
                self.blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|declaration| declaration.place == condition.place)
            .map(|declaration| (declaration.structural_type, declaration.access))
            .chain(
                operations
                    .iter()
                    .chain(self.blocks.iter().flat_map(|block| &block.operations))
                    .filter_map(|operation| operation.result.structural())
                    .filter(|result| result.place == condition.place)
                    .map(|result| {
                        (
                            result.structural_type,
                            terminal_psi::StructuralAccess::Owned,
                        )
                    }),
            );
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "retained cleanup case has no emitted root declaration",
        ))?;
        if declaration.1 != terminal_psi::StructuralAccess::Owned {
            return Err(LoweringError::Unsupported(
                "retained cleanup case has a borrowed root declaration",
            ));
        }
        if declarations.any(|other| other != declaration) {
            return Err(LoweringError::Unsupported(
                "retained cleanup case has inconsistent root declarations",
            ));
        }
        let subject = terminal_semantics::runtime_structural_path_tip(
            self.structural_types.iter(),
            declaration.0,
            &condition.path,
        )
        .ok_or(LoweringError::Unsupported(
            "retained cleanup case has no exact declared path",
        ))?;
        if subject.id != condition.structural_type {
            return Err(LoweringError::Unsupported(
                "retained cleanup case disagrees with its emitted subject type",
            ));
        }
        Ok(subject)
    }

    pub(super) fn declared_single_case(
        &self,
        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
        operations: &OperationBuffer,
    ) -> bool {
        self.case_subject(condition, operations)
            .is_ok_and(|declaration| match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases }
                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                    cases.len() == 1 && cases[0].id == condition.case
                }
                _ => false,
            })
    }

    /// Reuse the completed Boolean, never a tag load on the current payload.
    /// Its scope supplies dominance. The append-only operation prefix also
    /// authenticates the unchanged storage epoch, including effects emitted in
    /// siblings before their observation scopes were restored.
    pub(super) fn retained_case(
        &self,
        condition: &crate::unit::unit_cleanup::CleanupCaseCondition,
        operations: &OperationBuffer,
    ) -> Option<ValueDeclaration> {
        if !inline_case_path(&condition.path) {
            return None;
        }
        self.case_subject(condition, operations).ok()?;
        let captured = operations
            .storage_observations
            .cases
            .iter()
            .rev()
            .find(|captured| {
                captured.source == condition.place
                    && captured.path == condition.path
                    && captured.structural_type == condition.structural_type
                    && captured.case == condition.case
            })?;
        let position = operations.iter().position(|operation| {
            operation.id == captured.producer
                && operation.result.scalar_ref() == Some(&captured.value)
                && matches!(&operation.kind,
                    OperationKind::StructuralCaseMembership { source, path, case }
                    if *source == captured.source && *path == captured.path && *case == captured.case)
        })?;
        if !operations[position + 1..].iter().all(|operation| {
            self.preserves_case_observation(operation, captured.source, operations)
        }) {
            return None;
        }
        // Home completion is distinct from payload descriptor retirement.
        // The latter leaves the discriminator untouched; the former cannot
        // retain a relationship between a scalar snapshot and live storage.
        for action in self.blocks.iter().flat_map(|block| {
            block
                .terminator
                .cleanup_rosters()
                .flat_map(|(_, actions)| actions)
        }) {
            match action {
                TerminalAffineCleanupAction::RetireHome(place)
                | TerminalAffineCleanupAction::DiscardRoot(place)
                    if *place == captured.source =>
                {
                    return None;
                }
                TerminalAffineCleanupAction::DiscardResidual(discard)
                    if discard.place == captured.source
                        && captured.path.starts_with(&discard.path) =>
                {
                    return None;
                }
                TerminalAffineCleanupAction::InvokeNominal(_) => return None,
                _ => {}
            }
        }
        Some(captured.value)
    }

    fn preserves_case_observation(
        &self,
        operation: &terminal_psi::Operation,
        source: semantic_vocabulary::PlaceId,
        operations: &OperationBuffer,
    ) -> bool {
        if matches!(operation.kind, OperationKind::AtomicAccess { .. }) {
            return false;
        }
        if terminal_semantics::is_unconditionally_total_scalar(&operation.kind) {
            return true;
        }
        match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination, path, ..
            }
            | OperationKind::WriteOnlyPrimitiveStore {
                destination, path, ..
            } if inline_case_path(path) && self.fresh_inline_home(*destination, operations) => {
                // A typed scalar leaf store cannot replace a discriminator.
                // Only an actual inline establishment supplies disjoint owned
                // storage; a parameter, pointer, recast or returned address does
                // not obtain that proof from matching types or PlaceIds.
                true
            }
            OperationKind::StoreStructuralField {
                destination, path, ..
            } if *destination != source
                && inline_case_path(path)
                && self.fresh_inline_home(*destination, operations) =>
            {
                true
            }
            _ => {
                // The shared operand inventory distinguishes observations from
                // writes and transfers. Calls remain barriers even when their
                // explicit arguments happen to be shared; an opaque effect is
                // not a storage-independence proof.
                let Ok(row) = terminal_semantics::operation_semantic_row(&operation.kind) else {
                    return false;
                };
                if row.custody() == terminal_semantics::OperationSemanticCustody::CallComposition {
                    return false;
                }
                let mut observed = false;
                let mut readonly = true;
                operation
                    .kind
                    .clone()
                    .map_structural_uses_with_access(&mut |place, access| {
                        observed = true;
                        readonly &= access == terminal_psi::StructuralAccess::SharedBorrow;
                        place
                    });
                observed && readonly
            }
        }
    }

    fn fresh_inline_home(
        &self,
        place: semantic_vocabulary::PlaceId,
        operations: &OperationBuffer,
    ) -> bool {
        let mut producers = operations.iter().filter_map(|operation| {
            let result = operation.result.structural()?;
            (result.place == place).then_some((operation, result))
        });
        let Some((producer, result)) = producers.next() else {
            return false;
        };
        if producers.next().is_some()
            || !matches!(
                producer.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishStructuralCase { .. }
                    | OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::EstablishScalarArray { .. }
                    | OperationKind::EstablishStructuralArray { .. }
            )
        {
            return false;
        }
        terminal_psi::find_structural_type(self.structural_types, result.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                        | terminal_psi::StructuralTypeShape::Record { .. }
                        | terminal_psi::StructuralTypeShape::FixedArray { .. }
                        | terminal_psi::StructuralTypeShape::Sum { .. }
                        | terminal_psi::StructuralTypeShape::Mixed { .. }
                )
            })
    }
}

/// Only exact inline coordinates identify this occurrence's discriminator.
/// Runtime selectors, reference crossings and representation reinterpretations
/// need their own address/alias proof and never inherit this reuse premise.
fn inline_case_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    path.iter().all(|segment| {
        matches!(
            segment,
            terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_)
                | terminal_psi::StructuralPathSegment::Case(_)
        )
    })
}
