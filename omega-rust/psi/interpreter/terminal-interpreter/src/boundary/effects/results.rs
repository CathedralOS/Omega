//! Host result carriers and their exact binding to a verified boundary result.

use crate::boundary::effects::TerminalEffect;
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::results::{TerminalCrash, TerminalCrashSite};
use crate::boundary::values::{
    TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralValue,
};
use crate::engine::execution::TerminalExecution;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::BoundaryMachineDeclaration;
use terminal_psi::BoundaryMachineResult;
use terminal_psi::CrashCause;
use terminal_psi::OperationResult;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;

/// An immutable host result owns accounted storage. It carries no source
/// identity, qualification or permission to return a mutable reference.
#[derive(Clone, PartialEq, Eq)]
pub struct TerminalByteSequenceValue(crate::storage::byte_sequences::ByteSequenceView);

impl std::fmt::Debug for TerminalByteSequenceValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalByteSequenceValue")
            .field("length", &self.0.bytes().len())
            .finish()
    }
}

impl TerminalByteSequenceValue {
    /// Copy a stable read-only snapshot, reserving its payload before allocation.
    pub fn copy(
        bytes: &[u8],
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let storage =
            crate::storage::byte_sequences::storage::ByteSequenceStorage::copy(bytes, account)?;
        Ok(Self(crate::storage::byte_sequences::ByteSequenceView::new(
            storage,
        )))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffectResult {
    /// A no-successor outcome, permitted only by the boundary's published
    /// same-cause route under this invocation's actual scalar arguments.
    Crash(CrashCause),
    Unit,
    Scalar(TerminalScalarValue),
    /// An exact scalar record or selected sum case and its actual scalar
    /// payload. The payload supplies no authority: normal completion and the
    /// admitted boundary declaration own any result claims or qualifications.
    ScalarAggregate(TerminalScalarAggregateValue),
    /// The same opaque, target-neutral value used for structural entry inputs.
    /// This does not provide a sum discriminator or materialized payload fields.
    Structural(TerminalStructuralValue),
    /// An immutable, storage-backed byte view, not an opaque structural atom.
    ByteSequence(TerminalByteSequenceValue),
    /// An authenticated endpoint of this invocation's captured input storage.
    CapturedReference(super::storage_inputs::TerminalBoundaryReferenceSelection),
    /// A host-produced reference whose referent is materialized guest storage,
    /// never an opaque token. The interpreter mints the carrier and commits
    /// the supplied payload beneath it, so the result's exclusive or shared
    /// loan is real storage the caller's projections can read and write.
    Reference(TerminalReferenceResult),
}

/// The referent payload a host reference result supplies. The carrier itself
/// is minted at commit; only the referent's declared contents cross the
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalReferencePayload {
    /// The referent's declared type and complete scalar contents, in the same
    /// canonical-path form `TerminalScalarAggregateValue` uses for results.
    ScalarAggregate(TerminalScalarAggregateValue),
    /// The operation's contract is uniformly zeroed storage (calloc/memset
    /// semantics): the interpreter materializes zeroed cells for the full
    /// declared referent shape instead of an exact scalar payload.
    Zeroed,
}

/// The materialized referent a host reference result supplies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalReferenceResult {
    /// What the host reports about the referent beneath the minted carrier.
    pub referent: TerminalReferencePayload,
    /// `&`-typed fields beneath the referent: canonical field paths select
    /// each nested carrier; its own referent is materialized recursively.
    /// Every `&` field the referent declares must appear exactly once.
    pub references: Vec<(
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        TerminalReferenceResult,
    )>,
}

impl TerminalExecution<'_> {
    pub(crate) fn admit_boundary_crash(
        &self,
        declaration: &BoundaryMachineDeclaration,
        effect: &TerminalEffect,
        cause: CrashCause,
    ) -> Result<TerminalCrash, TerminalInterpretError> {
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            ..
        } = effect
        else {
            return Err(malformed());
        };
        if *boundary != declaration.id {
            return Err(malformed());
        }
        // Use the already evaluated invocation snapshot, not caller value IDs
        // that happen to equal the declaration's positional scalar formal IDs.
        let arguments = super::boundary_predicates::scalar_literals(arguments);
        super::crash_outcome::validate_boundary_crash_outcome(declaration, &arguments, cause)
            .map_err(|reason| TerminalInterpretError::BoundaryCrashNotPermitted {
                operation: *operation,
                boundary: *boundary,
                cause,
                reason,
            })?;
        Ok(TerminalCrash {
            site: TerminalCrashSite::BoundaryCall {
                machine: self.current_machine,
                block: self.current,
                operation: *operation,
                boundary: *boundary,
            },
            cause,
            site_guard: Vec::new(),
            frontier_lower_bound: self.live_claims.keys().copied().collect(),
        })
    }

    pub(crate) fn preflight_boundary_result(
        &self,
        boundary: &BoundaryMachineDeclaration,
        result: &OperationResult,
    ) -> Result<(), TerminalInterpretError> {
        // Both explicit and automatically discovered imports reach this gate
        // before the host handler runs. Checked providers returned through the
        // ordinary call frame above it, with actual qualification custody.
        if boundary.requires_checked_result(&self.program.catalogs.structural_domains) {
            return Err(TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary: boundary.id,
                reason: "host import cannot establish result provenance owned by another issuer",
            });
        }
        if let Some(result) = result.as_structural()
            && ((requires_materialized_host_custody(&self.structural_types(), result.structural_type)
                && !self.supports_scalar_aggregate_result(result)
                && !self.supports_byte_sequence_result(result)
                && !self.supports_captured_reference_result(boundary, result)
                && !(self.supports_host_reference_result(result)
                    && boundary.result_reference_regions.iter().any(|region|
                        region.path.is_empty() && matches!(region.origin,
                            terminal_psi::TerminalDynamicReferenceOrigin::Invocation { .. }))))
                || self.structural_values.contains_key(&result.place)
                || self
                    .live_affine_frontier
                    .iter()
                    .any(|value| value.place == result.place)
                // A claimed result is admissible only at Linear custody: an
                // unrestricted or affine boundary result owns no claims.
                || (!result.claims.is_empty()
                    && result.multiplicity != StructuralMultiplicity::Linear)
                || !result.projected_qualifications.is_empty())
        {
            return Err(malformed());
        }
        Ok(())
    }

    pub(crate) fn supports_captured_reference_result(
        &self,
        boundary: &BoundaryMachineDeclaration,
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && !boundary.result_reference_regions.is_empty()
            && boundary.result_reference_regions.iter().all(|region| {
                region.path.is_empty()
                    && matches!(
                        region.origin,
                        terminal_psi::TerminalDynamicReferenceOrigin::Parameter { .. }
                    )
            })
            && matches!(
                self.structural_types()
                    .get(&result.structural_type)
                    .map(|row| &row.shape),
                Some(StructuralTypeShape::Reference {
                    access: terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow,
                    ..
                })
            )
    }

    pub(crate) fn commit_boundary_reference_result(
        &mut self,
        result: &terminal_psi::StructuralOperationResult,
        prepared: crate::storage::reference::PreparedReference,
    ) -> Result<(), TerminalInterpretError> {
        let mut carrier = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        carrier.projected_qualifications = prepared.projected_qualifications;
        self.reference_referents.insert(
            crate::boundary::values::StructuralRuntimePlace::from(&carrier),
            prepared.referent,
        )?;
        self.structural_values.insert(result.place, carrier)?;
        self.track_affine_result(result);
        Ok(())
    }

    fn supports_byte_sequence_result(
        &self,
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && matches!(
                self.structural_types()
                    .get(&result.structural_type)
                    .map(|definition| &definition.shape),
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }
                ))
            )
    }

    /// A reference result is admissible from a host only when it carries no
    /// claims and its referent materializes as a finite scalar aggregate —
    /// the exact payload the host's `TerminalReferenceResult` commits.
    fn supports_host_reference_result(
        &self,
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && self
                .reference_contract(result.structural_type)
                .is_ok_and(|(referent, _)| {
                    self.structural_types()
                        .get(&referent)
                        .is_some_and(|declaration| {
                            crate::boundary::values::ScalarAggregatePlan::new(
                                declaration,
                                |identity| self.structural_types().get(&identity),
                            )
                            .is_ok()
                        })
                        || terminal_semantics::zeroed_structural_leaves_modulo_references(
                            self.structural_types().values(),
                            referent,
                        )
                        .is_some()
                })
    }

    fn supports_scalar_aggregate_result(
        &self,
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        result.projected_qualifications.is_empty()
            && (result.claims.is_empty() || result.multiplicity == StructuralMultiplicity::Linear)
            && self
                .structural_types()
                .get(&result.structural_type)
                .is_some_and(|declaration| {
                    let scalar_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
                        fields.iter().all(|field| {
                            !field.relevance.is_erased() && field.field_type.scalar_type().is_some()
                        })
                    };
                    match &declaration.shape {
                        StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::PrimitiveScalar(_) => {
                            crate::boundary::values::ScalarAggregatePlan::new(
                                declaration,
                                |identity| self.structural_types().get(&identity),
                            )
                            .is_ok()
                        }
                        StructuralTypeShape::Sum { cases } => {
                            cases.iter().all(|case| scalar_fields(&case.fields))
                        }
                        _ => false,
                    }
                })
    }
}

/// Opaque host identities establish neither bounded contents nor a reference's
/// captured loan and original backing, nor a fused binding's service authority.
pub(crate) fn requires_materialized_host_custody(
    types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        // Array consumers need actual element backing, even for an empty
        // carrier. An opaque host identity cannot supply that payload; checked
        // providers instead return through the ordinary suspended call frame.
        if terminal_semantics::scalar_array_leaf_shape(types.values(), current).is_some() {
            return true;
        }
        let Some(declaration) = types.get(&current) else {
            return true;
        };
        let mut fields = Vec::new();
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                return true;
            }
            StructuralTypeShape::ByteSequence(_) | StructuralTypeShape::AtomicCell(_) => {
                return true;
            }
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element, .. } => pending.push(*element),
            StructuralTypeShape::Record {
                fields: members, ..
            } => fields.extend(members),
            // A discriminator and actual payload are required even when every
            // field is an unrestricted scalar. Opaque identity is not a case.
            StructuralTypeShape::Sum { .. } => return true,
            StructuralTypeShape::Mixed {
                fields: members,
                cases,
            } => {
                fields.extend(members);
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
        }
        for field in fields {
            match field.field_type {
                terminal_psi::StructuralFieldType::BoundedInteger(_)
                | terminal_psi::StructuralFieldType::RefinedScalar(_) => return true,
                terminal_psi::StructuralFieldType::Structural(child) => pending.push(child),
                terminal_psi::StructuralFieldType::Scalar(_)
                | terminal_psi::StructuralFieldType::Atomic(_)
                | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::Erased { .. } => {}
                terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::PredicateByteSequence { .. } => return true,
            }
        }
    }
    false
}

impl TerminalExecution<'_> {
    pub(crate) fn commit_boundary_result(
        &mut self,
        result: &OperationResult,
        expected: &BoundaryMachineResult,
        returned: TerminalEffectResult,
    ) -> Result<(), TerminalInterpretError> {
        match (result, expected, returned) {
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::ByteSequence(bytes),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && expected.qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && self.supports_byte_sequence_result(declaration) =>
            {
                if self.byte_sequence_values.contains_key(&declaration.place) {
                    return Err(malformed());
                }
                let bytes = bytes.0.into_account(self.account.as_ref())?;
                let value = TerminalStructuralValue::minted(
                    self.local_structural_identities.allocate()?,
                    declaration.structural_type,
                    Vec::new(),
                );
                self.byte_sequence_values.insert(
                    declaration.place,
                    crate::storage::byte_sequences::binding::ByteSequenceBinding::Immutable(bytes),
                )?;
                self.structural_values.insert(declaration.place, value)?;
                self.track_affine_result(declaration);
            }
            (OperationResult::Unit, BoundaryMachineResult::Unit, TerminalEffectResult::Unit) => {}
            (
                OperationResult::Scalar(declaration),
                BoundaryMachineResult::Scalar(expected),
                TerminalEffectResult::Scalar(value),
            ) if declaration.value_type() == *expected
                && value.scalar_type() == expected.scalar_type =>
            {
                self.values.insert(declaration.id, value)?;
            }
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::ScalarAggregate(value),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && declaration.qualifications == expected.qualifications
                && declaration.projected_qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && self.supports_scalar_aggregate_result(declaration) =>
            {
                self.publish_scalar_aggregate(declaration, value)?;
            }
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::Structural(value),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && declaration.qualifications == expected.qualifications
                // Host adapters do not return descendant-membership evidence.
                // Checked family members use ordinary structural return custody
                // instead; an annotation cannot grant this authority to a host.
                && declaration.projected_qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && value.structural_type == expected.structural_type
                && value.qualifications == expected.qualifications
                && value.path.is_empty()
                && !requires_materialized_host_custody(
                    &self.structural_types(),
                    declaration.structural_type,
                ) =>
            {
                // The validated response is an actual opaque payload, not
                // absent scalar cells. Store it only after the complete result
                // guard and the caller's freshness reservation have succeeded.
                self.structural_host_atoms.insert(
                    crate::boundary::values::StructuralRuntimePlace::from(&value),
                    crate::storage::structural_storage::HostStructuralPayload::Opaque(
                        value.opaque_identity,
                    ),
                )?;
                self.structural_values.insert(declaration.place, value)?;
                self.track_affine_result(declaration);
            }
            (
                OperationResult::Structural(declaration),
                BoundaryMachineResult::Structural(expected),
                TerminalEffectResult::Reference(supply),
            ) if declaration.structural_type == expected.structural_type
                && declaration.multiplicity == expected.multiplicity
                && declaration.qualifications == expected.qualifications
                && declaration.projected_qualifications.is_empty()
                && expected.projected_qualifications.is_empty()
                && declaration.claims.is_empty()
                && self.supports_host_reference_result(declaration) =>
            {
                self.publish_host_reference_result(declaration, supply)?;
            }
            _ => return Err(malformed()),
        }
        Ok(())
    }
}
