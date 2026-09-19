//! Deriving the physical evidence of one artifact: walking its children,
//! validating their coordinates, and the identities the evidence carries.

use crate::NativeByteSpan;
use crate::NativeOptimizationProjection;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalEvidence;
use crate::NativePhysicalOccurrence;
pub(crate) use crate::NormalizedForeignCallbackRelocations;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::native_artifact::boundary_application_coverage_identity;
use crate::physical::derivation::children::{
    derive_admitted_provider_settlement_child, derive_exit_group_child,
    derive_normalized_foreign_child, derive_read_byte_child, derive_write_byte_child,
};
use crate::physical::derivation::hashing::{
    canonical_usize, hash_callback_relocation, hash_machine_function_identity, hash_object_symbol,
    hash_relocation_origin, physical_evidence_gap_identity, relocation_kind_tag,
};
use crate::physical::model::native_optimization_projection;
use crate::physical::model::native_physical_evidence;
use crate::physical::model::native_physical_evidence_gap;
use crate::physical::model::optimized_boundary_occurrence;
use crate::physical::model::optimized_operator_occurrence;
use crate::physical::model::{NativePhysicalEvidenceGap, NativePhysicalEvidenceGapSubject};
use crate::physical::operator_applications::derive_operator_physical_span;
use crate::{NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use machine_code::BoundaryExecutionRecord;
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use sha2::Digest;
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{BoundaryRealization, CallSiteOwner, CompilerBuiltinExecution};
use terminal_psi::OperationKind;

/// How one scoped physical-evidence derivation finished. `Blocked` names the
/// first subject the derivation could not bind, so an artifact without
/// complete evidence never fails silently.
pub(crate) enum NativePhysicalEvidenceDerivation {
    /// The artifact's scope admits no physical-evidence derivation at all.
    Unavailable,
    /// Every surviving occurrence bound exactly one physical child.
    Complete(NativePhysicalEvidence),
    /// Derivation stopped at the first subject it could not bind.
    Blocked(NativePhysicalEvidenceGap),
}

fn blocked(subject: NativePhysicalEvidenceGapSubject) -> NativePhysicalEvidenceDerivation {
    NativePhysicalEvidenceDerivation::Blocked(native_physical_evidence_gap(
        subject,
        physical_evidence_gap_identity(&subject),
    ))
}

pub(crate) fn derive_physical_evidence(
    scope: &NativePhysicalEvidenceScope,
    terminal_artifact: &terminal_codec::CanonicalTerminalArtifact,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    final_image_symbol_digest: [u8; 32],
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Result<NativePhysicalEvidenceDerivation, &'static str> {
    if matches!(scope, NativePhysicalEvidenceScope::Unavailable) {
        return Ok(NativePhysicalEvidenceDerivation::Unavailable);
    }
    if let NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) = scope
        && let Some(publication) = optimized.fragment_publication()
    {
        publication.validate_object(object)?;
    }
    let module = terminal_codec::decode_module(terminal_artifact.semantic_bytes())
        .map_err(|_| "native physical evidence cannot decode Terminal semantics")?;
    let boundary_application_coverage = boundary_application_coverage
        .ok_or("native physical evidence requires exact boundary-application coverage custody")?;

    let projection = match scope {
        NativePhysicalEvidenceScope::Unavailable => unreachable!("unavailable returned above"),
        NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence => {
            derive_identity_projection(
                terminal_artifact.manifest().semantic(),
                &module,
                boundary_application_coverage,
            )?
        }
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) => {
            if optimized.projection().terminal() != terminal_artifact.manifest().semantic()
                || optimized.boundary_application_coverage()
                    != &boundary_application_coverage_identity(Some(boundary_application_coverage))
                        .expect("present boundary-application coverage has an identity")
            {
                return Err(
                    "optimized physical scope is detached from its Terminal or D29 coverage",
                );
            }
            optimized.projection().clone()
        }
    };
    let mut settlements = BTreeMap::new();
    for installed in object.boundary_settlements() {
        let key = (
            installed.machine,
            installed.settlement.psi_operation,
            installed.settlement.boundary,
            installed.settlement.operation_ordinal,
        );
        if settlements.insert(key, installed).is_some() {
            return Err("native physical evidence found duplicate boundary settlements");
        }
    }

    let occurrence_keys = projection
        .boundary_occurrences()
        .iter()
        .map(|occurrence| {
            (
                occurrence.machine(),
                occurrence.operation(),
                occurrence.boundary(),
                occurrence.operation_ordinal(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    if settlements.keys().any(|key| !occurrence_keys.contains(key)) {
        return Err("native physical evidence found a stale boundary settlement");
    }

    let mut foreign_calls = BTreeMap::new();
    for foreign in object.foreign_calls() {
        let CallSiteOwner::Operation(operation) = foreign.owner else {
            return Ok(blocked(
                NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner {
                    machine: foreign.machine,
                    owner: foreign.owner,
                },
            ));
        };
        let key = (foreign.machine, operation);
        if foreign_calls.insert(key, foreign).is_some() {
            return Err("native physical evidence found duplicate normalized foreign calls");
        }
    }
    let foreign_occurrence_keys = projection
        .boundary_occurrences()
        .iter()
        .map(|occurrence| (occurrence.machine(), occurrence.operation()))
        .collect::<BTreeSet<_>>();
    if foreign_calls
        .keys()
        .any(|key| !foreign_occurrence_keys.contains(key))
    {
        return Err("native physical evidence found a stale normalized foreign call");
    }

    let boundary_identities = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut children = Vec::new();
    let mut consumed_port_effects = BTreeSet::new();
    for occurrence in projection.boundary_occurrences() {
        let key = (
            occurrence.machine(),
            occurrence.operation(),
            occurrence.boundary(),
            occurrence.operation_ordinal(),
        );
        let installed = settlements.get(&key);
        let foreign = foreign_calls.get(&(occurrence.machine(), occurrence.operation()));
        if installed.is_some() && foreign.is_some() {
            return Err("native physical evidence found two realizations for one boundary call");
        }
        let requirement = boundary_identities
            .get(&occurrence.boundary())
            .copied()
            .ok_or("native physical evidence names an absent boundary requirement")?;
        let matching_plans = selected_provider_plans
            .iter()
            .filter(|plan| {
                plan.requirement_identities()
                    .binary_search_by(|identity| identity.as_str().cmp(requirement))
                    .is_ok()
            })
            .collect::<Vec<_>>();
        let [selected_plan] = matching_plans.as_slice() else {
            return Err("native physical evidence cannot rejoin one exact selected provider plan");
        };
        match (installed, foreign) {
            (Some(installed), None)
                if matches!(
                    (
                        installed.settlement.execution,
                        &installed.settlement.realization,
                    ),
                    (
                        BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::HostedExitProcessI32
                        ),
                        BoundaryRealization::HostedExitProcessI32(_),
                    )
                ) =>
            {
                if installed.settlement.byte_count == 0 {
                    return Err(
                        "Hosted process-exit physical child requires a nonempty emitted span",
                    );
                }
                children.push(derive_exit_group_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan.plan_digest(),
                    target,
                    object,
                    image,
                    installed,
                )?);
            }
            (Some(installed), None)
                if matches!(
                    (
                        installed.settlement.execution,
                        &installed.settlement.realization,
                    ),
                    (
                        BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::HostedWriteByteI32
                        ),
                        BoundaryRealization::HostedWriteByteI32(_),
                    )
                ) =>
            {
                children.push(derive_write_byte_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan.plan_digest(),
                    target,
                    object,
                    image,
                    installed,
                )?);
            }
            (Some(installed), None)
                if matches!(
                    (
                        installed.settlement.execution,
                        &installed.settlement.realization,
                    ),
                    (
                        BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::HostedReadByte
                        ),
                        BoundaryRealization::HostedReadByte(_),
                    )
                ) =>
            {
                children.push(derive_read_byte_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan.plan_digest(),
                    target,
                    object,
                    image,
                    installed,
                )?);
            }
            (Some(installed), None) => {
                let Some(child) = derive_admitted_provider_settlement_child(
                    &module,
                    object,
                    image,
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan,
                    provider_executions,
                    target,
                    installed,
                    &mut consumed_port_effects,
                )?
                else {
                    return Ok(blocked(
                        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization {
                            occurrence: *occurrence,
                        },
                    ));
                };
                children.push(child);
            }
            (None, Some(foreign)) => {
                let Some(child) = derive_normalized_foreign_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan,
                    provider_executions,
                    target,
                    &module,
                    object,
                    image,
                    final_image_symbol_digest,
                    foreign,
                )?
                else {
                    return Ok(blocked(
                        NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall {
                            occurrence: *occurrence,
                        },
                    ));
                };
                children.push(child);
            }
            _ => {
                return Ok(blocked(
                    NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence {
                        occurrence: *occurrence,
                    },
                ));
            }
        }
    }
    // Every retained privileged port effect must have been consumed by an
    // exact `MetadataOnlyPort` settlement join above. An unowned privileged
    // effect cannot be attributed to a surviving occurrence, so the artifact
    // remains valid without claiming complete physical coverage.
    if consumed_port_effects.len() != object.port_effects().len() {
        let Some(effect) = object
            .port_effects()
            .iter()
            .enumerate()
            .find(|(index, _)| !consumed_port_effects.contains(index))
            .map(|(_, effect)| effect)
        else {
            return Err("native physical evidence port-effect custody count drifted");
        };
        return Ok(blocked(
            NativePhysicalEvidenceGapSubject::UnownedPortEffect {
                machine: effect.machine,
                psi_operation: effect.effect.psi_operation,
                service: effect.effect.service,
                port: effect.effect.port,
                value: effect.effect.value,
                operation_ordinal: effect.effect.operation_ordinal,
                code_offset: effect.effect.code_offset,
                byte_count: effect.effect.byte_count,
            },
        ));
    }
    for occurrence in projection.operator_occurrences() {
        let matching_references = boundary_application_coverage
            .references()
            .iter()
            .filter(|reference| reference.terminal_operation() == occurrence.operation())
            .collect::<Vec<_>>();
        let [reference] = matching_references.as_slice() else {
            return Err("D29 physical occurrence does not rejoin one coverage reference");
        };
        let matching_realizations = boundary_application_coverage
            .realizations()
            .rows()
            .iter()
            .filter(|realization| realization.terminal_operation() == occurrence.operation())
            .collect::<Vec<_>>();
        let [realization] = matching_realizations.as_slice() else {
            return Err("D29 physical occurrence does not rejoin one realization companion");
        };
        let Some(span) = derive_operator_physical_span(
            occurrence,
            realization.realization(),
            &module,
            target,
            object,
            image,
            matches!(scope, NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized)
                if optimized.fragment_publication().is_some()),
        )?
        else {
            // Unsupported compiler-intrinsic or call mechanics leave the
            // artifact valid, but the gap names the exact surviving
            // occurrence the span derivation declined.
            return Ok(blocked(
                NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan {
                    occurrence: *occurrence,
                },
            ));
        };
        let parent = PhysicalChildParent::OperatorApplicationCoverage(**reference);
        let physical_occurrence = NativePhysicalOccurrence::Operator(occurrence.identity());
        let identity = physical_child_identity(
            &parent,
            projection.identity(),
            physical_occurrence,
            span.machine,
            span.object,
            span.final_image,
            span.machine_bytes_digest,
            span.object_bytes_digest,
            span.final_image_bytes_digest,
            span.relocation,
        );
        children.push(
            NativePhysicalChildParts {
                parent,
                projection: projection.identity(),
                occurrence: physical_occurrence,
                machine_span: span.machine,
                object_span: span.object,
                final_image_span: span.final_image,
                machine_bytes_digest: span.machine_bytes_digest,
                object_bytes_digest: span.object_bytes_digest,
                final_image_bytes_digest: span.final_image_bytes_digest,
                relocation: span.relocation,
                identity,
            }
            .into(),
        );
    }
    children.sort_by_key(|child| child.occurrence());
    validate_exact_physical_children(&projection, &children)?;
    let identity = physical_evidence_identity(projection.identity(), &children);
    Ok(NativePhysicalEvidenceDerivation::Complete(
        native_physical_evidence(projection, children, identity),
    ))
}

#[derive(Clone, Copy)]
pub(crate) struct PhysicalChildCoordinate {
    pub(crate) projection: NativeOptimizationProjectionIdentity,
    pub(crate) occurrence: NativePhysicalOccurrence,
    pub(crate) parent_role: u8,
}

fn validate_exact_physical_children(
    projection: &NativeOptimizationProjection,
    children: &[NativePhysicalChild],
) -> Result<(), &'static str> {
    validate_exact_physical_child_coordinates(
        projection,
        children.iter().map(|child| PhysicalChildCoordinate {
            projection: child.projection(),
            occurrence: child.occurrence(),
            parent_role: child.parent().role_tag(),
        }),
    )
}

pub(crate) fn validate_exact_physical_child_coordinates(
    projection: &NativeOptimizationProjection,
    children: impl IntoIterator<Item = PhysicalChildCoordinate>,
) -> Result<(), &'static str> {
    let expected = projection
        .operator_occurrences()
        .iter()
        .map(|occurrence| (NativePhysicalOccurrence::Operator(occurrence.identity()), 1))
        .chain(
            projection
                .boundary_occurrences()
                .iter()
                .map(|occurrence| (NativePhysicalOccurrence::Boundary(occurrence.identity()), 2)),
        )
        .collect::<BTreeMap<_, _>>();
    if expected.len()
        != projection
            .operator_occurrences()
            .len()
            .checked_add(projection.boundary_occurrences().len())
            .ok_or("native physical evidence occurrence count overflow")?
    {
        return Err("native physical evidence projection repeats an optimized occurrence");
    }

    let mut observed = BTreeMap::new();
    for child in children {
        if child.projection != projection.identity() {
            return Err("native physical child is detached from its optimized projection");
        }
        if child.parent_role != child.occurrence.role_tag()
            || expected.get(&child.occurrence) != Some(&child.parent_role)
        {
            return Err("native physical child swapped or substituted its semantic parent role");
        }
        if observed
            .insert(child.occurrence, child.parent_role)
            .is_some()
        {
            return Err("native physical evidence contains duplicate optimized occurrences");
        }
    }
    if observed != expected {
        return Err("native physical evidence does not cover the exact surviving occurrence set");
    }
    Ok(())
}

fn derive_identity_projection(
    terminal: terminal_psi::TerminalPsiIdentity,
    module: &terminal_psi::TerminalModule,
    boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
) -> Result<NativeOptimizationProjection, &'static str> {
    let operator_operations = boundary_application_coverage
        .references()
        .iter()
        .map(|reference| reference.terminal_operation())
        .collect::<BTreeSet<_>>();
    if operator_operations.len() != boundary_application_coverage.references().len() {
        return Err("native optimization projection contains duplicate D29 operations");
    }
    let mut operator_occurrences = Vec::with_capacity(operator_operations.len());
    let mut boundary_occurrences = Vec::new();
    for machine in &module.machines {
        let mut operation_ordinal = 0_usize;
        for block in &machine.blocks {
            for operation in &block.operations {
                if operator_operations.contains(&operation.id) {
                    let identity = operator_occurrence_identity(
                        terminal,
                        machine.id,
                        operation.id,
                        operation_ordinal,
                    );
                    operator_occurrences.push(optimized_operator_occurrence(
                        terminal,
                        machine.id,
                        operation.id,
                        operation_ordinal,
                        identity,
                    ));
                }
                if let OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    let identity = boundary_occurrence_identity(
                        terminal,
                        machine.id,
                        operation.id,
                        boundary,
                        operation_ordinal,
                    );
                    boundary_occurrences.push(optimized_boundary_occurrence(
                        terminal,
                        machine.id,
                        operation.id,
                        boundary,
                        operation_ordinal,
                        identity,
                    ));
                }
                operation_ordinal = operation_ordinal
                    .checked_add(1)
                    .ok_or("native optimization projection operation ordinal overflow")?;
            }
            operation_ordinal = operation_ordinal
                .checked_add(1)
                .ok_or("native optimization projection terminator ordinal overflow")?;
        }
    }
    if operator_occurrences.len() != operator_operations.len() {
        return Err("native optimization projection cannot rejoin every D29 operation");
    }
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.push(1); // D29 operator-application occurrences.
    canonical.extend_from_slice(&canonical_usize(operator_occurrences.len()));
    for occurrence in &operator_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    canonical.push(2); // D41 ordinary boundary-trait occurrences.
    canonical.extend_from_slice(&canonical_usize(boundary_occurrences.len()));
    for occurrence in &boundary_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    let identity = NativeOptimizationProjectionIdentity::from_canonical_bytes(&canonical);
    Ok(native_optimization_projection(
        terminal,
        operator_occurrences,
        boundary_occurrences,
        identity,
    ))
}

pub(crate) fn operator_occurrence_identity(
    terminal: terminal_psi::TerminalPsiIdentity,
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    operation_ordinal: usize,
) -> OptimizedOperatorOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

pub(crate) fn boundary_occurrence_identity(
    terminal: terminal_psi::TerminalPsiIdentity,
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    boundary: semantic_vocabulary::BoundaryMachineId,
    operation_ordinal: usize,
) -> OptimizedBoundaryOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&boundary.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(&canonical)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn physical_child_identity(
    parent: &PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: PhysicalRelocationDisposition,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-child.sha256.v2\0");
    digest.update([parent.role_tag()]);
    digest.update(parent.identity());
    digest.update(projection.bytes());
    digest.update([occurrence.role_tag()]);
    digest.update(occurrence.identity());
    for span in [machine_span, object_span, final_image_span] {
        digest.update(canonical_usize(span.offset()));
        digest.update(canonical_usize(span.byte_count()));
    }
    digest.update(machine_bytes_digest);
    digest.update(object_bytes_digest);
    digest.update(final_image_bytes_digest);
    digest.update([match relocation {
        PhysicalRelocationDisposition::DirectInstructionBytes => 1,
        PhysicalRelocationDisposition::ResolvedInternalCall => 2,
        PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(_) => 3,
    }]);
    if let PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(relocation) = relocation {
        digest.update(relocation.locator_identity());
        digest.update(relocation.boundary_plan_identity());
        hash_object_symbol(&mut digest, relocation.object_symbol());
        hash_relocation_origin(&mut digest, relocation.origin());
        digest.update(canonical_usize(relocation.offset()));
        digest.update(canonical_usize(relocation.byte_width()));
        digest.update(relocation.addend().to_le_bytes());
        digest.update([relocation_kind_tag(relocation.kind())]);
        match relocation.callback() {
            None => digest.update([0]),
            Some(NormalizedForeignCallbackRelocations::X86_64Relative32 {
                callback_function,
                relocation,
            }) => {
                digest.update([1]);
                hash_machine_function_identity(&mut digest, callback_function);
                hash_callback_relocation(&mut digest, relocation);
            }
            Some(NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                callback_function,
                page,
                page_offset,
            }) => {
                digest.update([2]);
                hash_machine_function_identity(&mut digest, callback_function);
                hash_callback_relocation(&mut digest, page);
                hash_callback_relocation(&mut digest, page_offset);
            }
        }
        digest.update(relocation.final_image_symbol_identity());
    }
    digest.finalize().into()
}

fn physical_evidence_identity(
    projection: NativeOptimizationProjectionIdentity,
    children: &[NativePhysicalChild],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-evidence.sha256.v2\0");
    digest.update(projection.bytes());
    digest.update(canonical_usize(children.len()));
    for child in children {
        digest.update(child.identity());
    }
    digest.finalize().into()
}

fn terminal_identity_bytes(terminal: terminal_psi::TerminalPsiIdentity) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&terminal.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(terminal.program_fingerprint.as_bytes());
    bytes
}

pub(crate) fn span(bytes: &[u8], span: NativeByteSpan) -> Result<&[u8], &'static str> {
    let end = span
        .offset()
        .checked_add(span.byte_count())
        .ok_or("native physical child byte span overflow")?;
    bytes
        .get(span.offset()..end)
        .ok_or("native physical child byte span is out of bounds")
}

pub(crate) fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
}
