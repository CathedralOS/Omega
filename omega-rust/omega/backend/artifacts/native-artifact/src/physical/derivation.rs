use std::collections::{BTreeMap, BTreeSet};

use boundary_applications::TerminalBoundaryApplicationCoverage;
use installation_evidence::ProviderExecutionEvidence;
use machine_code::{BoundaryExecutionRecord, PortEffectRecord};
use object_file::{RelocationKind, RelocationOrigin, SectionKind};
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::{
    BoundaryRealization, CallSiteOwner, CompilerBuiltinExecution, CompletionClaimSource,
    NormalizedForeignCallBinding, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use terminal_psi::OperationKind;

use super::{model::*, operator_applications::derive_operator_physical_span};
use crate::{
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest, native_artifact::boundary_application_coverage_identity,
};

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
) -> Result<Option<NativePhysicalEvidence>, &'static str> {
    if matches!(scope, NativePhysicalEvidenceScope::Unavailable) {
        return Ok(None);
    }
    if let NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) = scope
        && let Some(publication) = optimized.fragment_publication()
    {
        publication.validate_object(object)?;
    }
    let module = terminal_codec::decode_module(terminal_artifact.semantic_bytes())
        .map_err(|_| "native physical evidence cannot decode Terminal semantics")?;
    if module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        return Ok(None);
    }
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
            return Ok(None);
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
                    return Ok(None);
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
                    return Ok(None);
                };
                children.push(child);
            }
            _ => return Ok(None),
        }
    }
    // Every retained privileged port effect must have been consumed by an
    // exact `MetadataOnlyPort` settlement join above. An unowned privileged
    // effect cannot be attributed to a surviving occurrence, so the artifact
    // remains valid without claiming complete physical coverage.
    if consumed_port_effects.len() != object.port_effects().len() {
        return Ok(None);
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
            // artifact valid without claiming complete physical coverage.
            return Ok(None);
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
    Ok(Some(native_physical_evidence(
        projection, children, identity,
    )))
}

#[derive(Clone, Copy)]
struct PhysicalChildCoordinate {
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    parent_role: u8,
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

fn validate_exact_physical_child_coordinates(
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

fn operator_occurrence_identity(
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

fn boundary_occurrence_identity(
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
fn derive_exit_group_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    installed: &image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    if !target_operations::HostedExitProcessI32Realization::supports_target(target) {
        return Err("Hosted process-exit physical child requires a canonical supported target");
    }
    let expected_destination = match target.architecture {
        Architecture::X86_64 => target_operations::MachineRegister::X86Rdi,
        Architecture::Aarch64 => target_operations::MachineRegister::Aarch64X(0),
    };
    let (role, parent_identity) = match (
        settlement.scalar_arguments.as_slice(),
        settlement.runtime_scalar_arguments.as_slice(),
    ) {
        ([scalar_argument], [])
            if scalar_argument.scalar_type == ScalarType::Integer(i32_type)
                && matches!(scalar_argument.immediate, semantic_vocabulary::IntegerValue::Signed(value) if i32::try_from(value).is_ok())
                && scalar_argument.destination == expected_destination =>
        {
            (
                BoundaryTraitSettlementRole::CompilerBuiltin {
                    catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
                    execution: CompilerBuiltinExecution::HostedExitProcessI32,
                    realization: BoundaryRealization::HostedExitProcessI32(Default::default()),
                    scalar_argument: *scalar_argument,
                },
                builtin_boundary_trait_settlement_identity(
                    occurrence,
                    requirement_identity,
                    selected_plan_digest,
                    target,
                    scalar_argument,
                ),
            )
        }
        ([], [scalar_argument]) if matches!(scalar_argument.source, machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { scalar_type, .. } if scalar_type == ScalarType::Integer(i32_type)) => {
            (
                BoundaryTraitSettlementRole::CompilerBuiltinRuntimeScalar {
                    catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
                    execution: CompilerBuiltinExecution::HostedExitProcessI32,
                    realization: BoundaryRealization::HostedExitProcessI32(Default::default()),
                    scalar_argument: scalar_argument.clone(),
                },
                builtin_runtime_scalar_boundary_trait_settlement_identity(
                    occurrence,
                    requirement_identity,
                    selected_plan_digest,
                    target,
                    scalar_argument,
                ),
            )
        }
        _ => return Err("Hosted process-exit physical child requires one exact i32 source"),
    };
    if !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || !settlement.native_result.is_unit()
    {
        return Err("Hosted process-exit D41 settlement custody is incomplete or substituted");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("Hosted process-exit physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("Hosted process-exit physical child object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("Hosted process-exit physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("Hosted process-exit physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("Hosted process-exit physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("Hosted process-exit physical child unexpectedly contains a relocation");
    }

    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let relocation = PhysicalRelocationDisposition::DirectInstructionBytes;
    let identity = physical_child_identity(
        &parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(NativePhysicalChildParts {
        parent,
        projection,
        occurrence: NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
        identity,
    }
    .into())
}

#[allow(clippy::too_many_arguments)]
fn derive_write_byte_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    installed: &image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let [scalar_argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return Err("hosted write-byte physical child requires one runtime scalar argument");
    };
    if ![
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ]
    .contains(&target)
        || !settlement.scalar_arguments.is_empty()
        || !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || !settlement.native_result.is_unit()
    {
        return Err("hosted write-byte D41 settlement custody is incomplete or substituted");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("hosted write-byte physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("hosted write-byte physical child object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("hosted write-byte physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("hosted write-byte physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("hosted write-byte physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("hosted write-byte physical child unexpectedly contains a relocation");
    }
    let role = BoundaryTraitSettlementRole::CompilerBuiltinRuntimeScalar {
        catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
        execution: CompilerBuiltinExecution::HostedWriteByteI32,
        realization: BoundaryRealization::HostedWriteByteI32(Default::default()),
        scalar_argument: scalar_argument.clone(),
    };
    let parent_identity = builtin_runtime_scalar_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        scalar_argument,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let relocation = PhysicalRelocationDisposition::DirectInstructionBytes;
    let identity = physical_child_identity(
        &parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(NativePhysicalChildParts {
        parent,
        projection,
        occurrence: NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
        identity,
    }
    .into())
}

#[allow(clippy::too_many_arguments)]
fn derive_read_byte_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    installed: &image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let Some(result) = settlement.native_result.structural() else {
        return Err("hosted read-byte physical child requires one structural result");
    };
    if ![
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ]
    .contains(&target)
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || result.defining_operation != occurrence.operation()
        || result.layout.tag_byte_offset != 0
        || result.layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
        || result.layout.shape != calling_conventions::ValueShape::integer(8, 4)
        || result.layout.payload_byte_offset != 4
        || !result.layout.common_fields.is_empty()
        || result.layout.cases.len() != 2
        || !result.layout.cases[0].fields.is_empty()
        || result.layout.cases[1].fields.as_slice()
            != [calling_conventions::PackedFieldLayout {
                shape: calling_conventions::ValueShape::integer(4, 4),
                byte_offset: 4,
            }]
    {
        return Err("hosted read-byte D41 settlement custody is incomplete or substituted");
    }
    let payload_offset = result
        .home_byte_offset
        .checked_add(u32::from(result.layout.payload_byte_offset))
        .ok_or("hosted read-byte physical child result home overflow")?;
    let expected = match target.architecture {
        Architecture::X86_64 => {
            isa_x86_64::encode_linux_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "Linux read-byte x86-64 encoding is not reproducible")?
        }
        Architecture::Aarch64 if target == NativeTarget::macos_arm64() => {
            isa_aarch64::encode_macos_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "macOS read-byte AArch64 encoding is not reproducible")?
        }
        Architecture::Aarch64 => {
            isa_aarch64::encode_linux_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "Linux read-byte AArch64 encoding is not reproducible")?
        }
    };
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("hosted read-byte physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("hosted read-byte physical child object span overflow")?;
    if installed.text_offset != expected_object_offset || expected.len() != settlement.byte_count {
        return Err("hosted read-byte physical child span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != expected
        || machine_bytes != object_bytes
        || object_bytes != final_image_bytes
    {
        return Err("hosted read-byte physical child bytes changed across custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("hosted read-byte physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("hosted read-byte physical child unexpectedly contains a relocation");
    }
    let role = BoundaryTraitSettlementRole::CompilerBuiltinStructural {
        catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
        execution: CompilerBuiltinExecution::HostedReadByte,
        realization: BoundaryRealization::HostedReadByte(Default::default()),
        result: result.clone(),
    };
    let parent_identity = builtin_structural_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        result,
    )?;
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let relocation = PhysicalRelocationDisposition::DirectInstructionBytes;
    let identity = physical_child_identity(
        &parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(NativePhysicalChildParts {
        parent,
        projection,
        occurrence: NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
        identity,
    }
    .into())
}

/// Derive the D41 physical child for one installed provider settlement
/// realized in place by a supported target mechanism.
///
/// `Ok(None)` leaves realizations this lane does not cover without a claimed
/// child; the artifact remains valid but retains no complete physical
/// evidence. Custody drift that would contradict a validated object or its
/// Terminal operation is an error, matching the hosted-builtin derivations.
#[allow(clippy::too_many_arguments)]
fn derive_admitted_provider_settlement_child(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    installed: &image_emission::ObjectBoundarySettlement,
    consumed_port_effects: &mut BTreeSet<usize>,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let settlement = &installed.settlement;
    let BoundaryExecutionRecord::AdmittedProvider(execution_record) = settlement.execution else {
        return Ok(None);
    };
    let supported = matches!(
        settlement.realization,
        BoundaryRealization::MetadataOnlyPort(_)
            | BoundaryRealization::DirectPortReadU8(_)
            | BoundaryRealization::LinuxWriteLine(_)
            | BoundaryRealization::ClaimCompletionOnly(_)
    );
    if !supported {
        return Ok(None);
    }
    // The retained settlement must still rejoin exactly one Terminal boundary
    // call on the occurrence's machine, and that call must carry the
    // settlement's exact structural-argument and completion-receipt custody.
    // None of the supported mechanisms retain a scalar argument.
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("installed D41 settlement does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        structural_arguments,
        completion_receipts,
    } = &operation.kind
    else {
        return Err("installed D41 settlement owner is not a Terminal boundary call");
    };
    let declaration = module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .ok_or("installed D41 settlement names an absent boundary")?;
    if *boundary != occurrence.boundary()
        || settlement.psi_operation != occurrence.operation()
        || settlement.boundary != *boundary
        || declaration.identity != requirement_identity
        || !arguments.is_empty()
        || *structural_arguments != settlement.arguments
        || *completion_receipts != settlement.completion_receipts
    {
        return Err("installed D41 settlement changed its semantic operation custody");
    }
    if !settlement_completion_custody_is_exact(settlement) {
        return Err("installed D41 settlement changed its completion custody");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("installed D41 settlement names an absent object function")?;
    if !function
        .provenance
        .operations
        .contains(&settlement.psi_operation)
    {
        return Err("installed D41 settlement operation left its function provenance");
    }
    let port_effect = match &settlement.realization {
        BoundaryRealization::MetadataOnlyPort(realization) => Some(metadata_port_effect_custody(
            module,
            object,
            image,
            occurrence,
            settlement,
            realization,
            target,
            function,
            operation,
            declaration,
            consumed_port_effects,
        )?),
        BoundaryRealization::DirectPortReadU8(realization) => {
            direct_port_read_custody(
                object,
                image,
                occurrence,
                settlement,
                realization,
                target,
                function,
                operation,
                declaration,
            )?;
            None
        }
        BoundaryRealization::LinuxWriteLine(_) => {
            if !linux_write_line_custody_is_exact(target, settlement, function.bytes(object))
                || function.unit_stack.is_none()
                || function.scalar_stack.is_some()
                || !matches!(operation.result, terminal_psi::OperationResult::Unit)
                || !declaration.result.is_unit()
            {
                return Err("installed D41 write-line custody is incomplete or substituted");
            }
            None
        }
        BoundaryRealization::ClaimCompletionOnly(_) => {
            if !settlement.scalar_arguments.is_empty()
                || !settlement.runtime_scalar_arguments.is_empty()
                || !settlement.byte_sequence_arguments.is_empty()
                || !settlement.native_result.is_unit()
                || settlement.byte_count != 0
                || !matches!(operation.result, terminal_psi::OperationResult::Unit)
                || !declaration.result.is_unit()
            {
                return Err("installed D41 claim-completion custody is incomplete or substituted");
            }
            None
        }
        _ => unreachable!("supported installed realizations were dispatched above"),
    };
    let execution = rejoin_admitted_provider_execution(
        requirement_identity,
        selected_plan,
        provider_executions,
        execution_record,
    )?;

    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("installed D41 settlement object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("installed D41 settlement object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("installed D41 settlement bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("installed D41 settlement relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("installed D41 settlement unexpectedly contains a relocation");
    }

    let role = BoundaryTraitSettlementRole::AdmittedProviderSettlement {
        execution,
        settlement: settlement.clone(),
        port_effect: port_effect.clone(),
    };
    let parent_identity = admitted_provider_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan.plan_digest(),
        target,
        execution,
        settlement,
        port_effect.as_ref(),
    )?;
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest: selected_plan.plan_digest(),
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let relocation = PhysicalRelocationDisposition::DirectInstructionBytes;
    let identity = physical_child_identity(
        &parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(Some(
        NativePhysicalChildParts {
            parent,
            projection,
            occurrence: NativePhysicalOccurrence::Boundary(occurrence.identity()),
            machine_span,
            object_span,
            final_image_span,
            machine_bytes_digest,
            object_bytes_digest,
            final_image_bytes_digest,
            relocation,
            identity,
        }
        .into(),
    ))
}

/// Rejoin the one retained provider execution an installed settlement names.
/// The record must identify the selected plan and match exactly one retained
/// execution on every compact coordinate.
fn rejoin_admitted_provider_execution(
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    execution_record: machine_code::ProviderExecutionRecord,
) -> Result<ProviderExecutionBinding, &'static str> {
    if execution_record.provider_plan_report_identity != selected_plan.report_identity() {
        return Err("installed D41 settlement names the wrong selected provider plan");
    }
    let matching_executions = provider_executions
        .iter()
        .filter(|execution| {
            execution.requirement_identity() == requirement_identity
                && execution.provider_plan_report_identity()
                    == execution_record.provider_plan_report_identity
                && execution.provider_execution_report_identity()
                    == execution_record.provider_execution_report_identity
                && execution.provider_execution_report_fingerprint()
                    == execution_record.provider_execution_report_fingerprint
                && execution.normalized_root_report_identity()
                    == execution_record.normalized_root_report_identity
                && execution.boundary_contract_report_fingerprint()
                    == execution_record.boundary_contract_report_fingerprint
        })
        .count();
    if matching_executions != 1 {
        return Err("installed D41 settlement cannot rejoin one retained provider execution");
    }
    let plan_report_identity =
        ProviderPlanReportIdentity::new(execution_record.provider_plan_report_identity)
            .ok_or("installed D41 settlement has a zero provider-plan report identity")?;
    ProviderExecutionBinding::from_execution_record(
        plan_report_identity,
        execution_record.provider_execution_report_identity,
        execution_record.provider_execution_report_fingerprint,
        execution_record.normalized_root_report_identity,
        execution_record.boundary_contract_report_fingerprint,
    )
    .ok_or("installed D41 settlement has an invalid provider execution")
}

/// Join one `MetadataOnlyPort` settlement to its exact privileged port
/// effect. The effect operation must be the immediately preceding emitted
/// operation whose interval ends where the settlement begins, must still
/// name one exact Terminal port write, and must reproduce its target bytes
/// across machine, object, and final-image custody.
#[allow(clippy::too_many_arguments)]
fn metadata_port_effect_custody(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    settlement: &machine_code::BoundarySettlementRecord,
    realization: &target_operations::MetadataOnlyPortRealization,
    target: NativeTarget,
    function: &image_emission::ObjectFunction,
    operation: &terminal_psi::Operation,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    consumed_port_effects: &mut BTreeSet<usize>,
) -> Result<PortEffectRecord, &'static str> {
    if target.architecture != Architecture::X86_64
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.native_result.is_unit()
        || settlement.byte_count != 0
        || !matches!(operation.result, terminal_psi::OperationResult::Unit)
        || !declaration.result.is_unit()
    {
        return Err("metadata port D41 settlement custody is incomplete or substituted");
    }
    let matching_effects = object
        .port_effects()
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            let effect = &candidate.effect;
            candidate.machine == occurrence.machine()
                && effect.psi_operation == realization.effect_operation
                && effect.service == realization.service
                && effect.port == realization.port
                && effect.value == realization.value
                && effect.operation_ordinal.checked_add(1) == Some(settlement.operation_ordinal)
                && effect.code_offset.checked_add(effect.byte_count) == Some(settlement.code_offset)
        })
        .collect::<Vec<_>>();
    let [(effect_index, object_effect)] = matching_effects.as_slice() else {
        return Err("metadata port D41 settlement does not rejoin one privileged port effect");
    };
    let effect = &object_effect.effect;
    let matching_effect_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == effect.psi_operation)
        .collect::<Vec<_>>();
    let [effect_operation] = matching_effect_operations.as_slice() else {
        return Err("metadata port D41 effect does not rejoin one Terminal operation");
    };
    if !matches!(
        effect_operation.kind,
        OperationKind::PortWrite { service, port, value }
            if service == effect.service && port == effect.port && value == effect.value
    ) {
        return Err("metadata port D41 effect changed its semantic port write");
    }
    if !function
        .provenance
        .operations
        .contains(&effect.psi_operation)
        || effect.byte_count != x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH
    {
        return Err("metadata port D41 effect left its function provenance or width");
    }
    let expected_object_offset = function
        .text_offset
        .checked_add(effect.code_offset)
        .ok_or("metadata port D41 effect object span overflow")?;
    if object_effect.text_offset != expected_object_offset {
        return Err("metadata port D41 effect object span is detached");
    }
    let expected = x86_encoding::encode_immediate_port_write(effect.port, effect.value);
    let machine_span = native_byte_span(effect.code_offset, effect.byte_count);
    let object_span = native_byte_span(object_effect.text_offset, effect.byte_count);
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, object_span)?;
    if machine_bytes != expected.as_slice()
        || machine_bytes != object_bytes
        || object_bytes != final_image_bytes
    {
        return Err("metadata port D41 effect bytes changed across physical custody");
    }
    let object_end = object_effect
        .text_offset
        .checked_add(effect.byte_count)
        .ok_or("metadata port D41 effect relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                object_effect.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("metadata port D41 effect unexpectedly contains a relocation");
    }
    consumed_port_effects.insert(*effect_index);
    Ok(effect.clone())
}

/// Replay the exact direct port-read custody: one 16-byte x86-64 `in al, dx`
/// sequence, one `u8` result in `RAX`, and the exact `0xc3` return edge that
/// returns the read value.
#[allow(clippy::too_many_arguments)]
fn direct_port_read_custody(
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    settlement: &machine_code::BoundarySettlementRecord,
    realization: &target_operations::DirectPortReadU8Realization,
    target: NativeTarget,
    function: &image_emission::ObjectFunction,
    operation: &terminal_psi::Operation,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    let u8_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    let Some(result) = settlement.native_result.scalar() else {
        return Err("direct port-read D41 settlement requires one scalar result");
    };
    if target.architecture != Architecture::X86_64
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || settlement.byte_count != x86_encoding::IMMEDIATE_PORT_READ_U8_WIDTH
        || result.scalar_type != u8_type
        || result.placement.shape != calling_conventions::ValueShape::integer(1, 1)
        || result.placement.locations.as_slice()
            != [calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 1,
            }]
        || function.unit_stack.is_some()
        || function.scalar_stack.is_none()
    {
        return Err("direct port-read D41 settlement custody is incomplete or substituted");
    }
    if !settlement.arguments.iter().all(|argument| {
        argument.path.is_empty()
            && function
                .scalar_structural_parameters
                .iter()
                .any(|parameter| parameter.place == argument.place)
    }) {
        return Err("direct port-read D41 settlement changed a structural argument place");
    }
    let result_matches = matches!(
        &operation.result,
        terminal_psi::OperationResult::Scalar(value)
            if value.id == result.value && value.scalar_type == u8_type
    ) && matches!(
        declaration.result,
        terminal_psi::BoundaryMachineResult::Scalar(scalar) if scalar == u8_type
    );
    if !result_matches {
        return Err("direct port-read D41 settlement changed its semantic scalar result");
    }
    let Some(return_ordinal) = settlement.operation_ordinal.checked_add(1) else {
        return Err("direct port-read D41 return ordinal overflow");
    };
    let Some(return_offset) = settlement.code_offset.checked_add(settlement.byte_count) else {
        return Err("direct port-read D41 return offset overflow");
    };
    let matching_returns = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == occurrence.machine()
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Edge(result.return_edge)
                && attribution.attribution.operation_ordinal == return_ordinal
                && attribution.attribution.code_offset == return_offset
                && attribution.attribution.byte_count == 1
        })
        .collect::<Vec<_>>();
    let [return_attribution] = matching_returns.as_slice() else {
        return Err("direct port-read D41 settlement does not rejoin one return edge");
    };
    let expected_return_object_offset = function
        .text_offset
        .checked_add(return_offset)
        .ok_or("direct port-read D41 return object span overflow")?;
    let expected = x86_encoding::encode_immediate_port_read_u8(realization.port);
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    if span(function.bytes(object), machine_span)? != expected.as_slice()
        || return_attribution.text_offset != expected_return_object_offset
        || function.bytes(object).get(return_offset) != Some(&0xc3)
        || object.text_bytes().get(return_attribution.text_offset) != Some(&0xc3)
        || image.final_text_bytes.get(return_attribution.text_offset) != Some(&0xc3)
    {
        return Err("direct port-read D41 custody changed across physical custody");
    }
    Ok(())
}

/// Replay the exact Linux `write_line` byte custody: one borrowed-view
/// byte-sequence structural argument, the exact target encoder output, and
/// the code/data intervals the settlement retains.
fn linux_write_line_custody_is_exact(
    target: NativeTarget,
    settlement: &machine_code::BoundarySettlementRecord,
    function_bytes: &[u8],
) -> bool {
    let BoundaryRealization::LinuxWriteLine(_) = settlement.realization else {
        return false;
    };
    let [custody] = settlement.byte_sequence_arguments.as_slice() else {
        return false;
    };
    if target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::X86_64 | Architecture::Aarch64
        )
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || settlement.arguments.as_slice() != [custody.argument.clone()]
        || !custody.argument.path.is_empty()
        || !matches!(
            custody.structural_type.shape,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView
            )
        )
        || !settlement.native_result.is_unit()
    {
        return false;
    }
    let encoded = match target.architecture {
        Architecture::X86_64 => isa_x86_64::encode_linux_write_line_literal(&custody.bytes),
        Architecture::Aarch64 => isa_aarch64::encode_linux_write_line_literal(&custody.bytes),
    };
    let Ok((encoded, data)) = encoded else {
        return false;
    };
    settlement.byte_count == encoded.len()
        && settlement.byte_count != 0
        && custody.code_offset == settlement.code_offset
        && custody.code_byte_count == data.start
        && custody.code_byte_count != 0
        && custody.data_offset == settlement.code_offset.saturating_add(data.start)
        && custody.data_byte_count == data.len()
        && custody.data_byte_count == custody.bytes.len().saturating_add(1)
        && encoded
            .get(data.clone())
            .is_some_and(|payload| payload.strip_suffix(b"\n") == Some(custody.bytes.as_slice()))
        && settlement
            .code_offset
            .checked_add(settlement.byte_count)
            .and_then(|end| function_bytes.get(settlement.code_offset..end))
            == Some(encoded.as_slice())
}

/// Replay the object validator's completion-custody responsibility after the
/// verified module has been discarded. The caller already constrained the
/// execution/realization pair, so this mirrors the argument-path,
/// receipt-index, receipt-custody, and provider-custody reconstruction
/// checks exactly.
fn settlement_completion_custody_is_exact(
    settlement: &machine_code::BoundarySettlementRecord,
) -> bool {
    if settlement.arguments.iter().any(|argument| {
        argument.path.iter().any(
            |segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(identity) if identity.is_empty()),
        )
    }) || settlement.completion_receipts.iter().any(|receipt| {
        usize::try_from(receipt.argument_index)
            .map_or(true, |index| index >= settlement.arguments.len())
    }) {
        return false;
    }
    if !settlement_completion_receipts_have_exact_custody(
        &settlement.arguments,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    ) {
        return false;
    }
    let Some(expected) = machine_code::derive_completion_provider_custody(
        settlement.execution,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    ) else {
        return false;
    };
    expected == settlement.completion_provider_custody
}

/// Replay the verifier's exact claim-source matching, claim uniqueness, and
/// canonical receipt ordering for one retained settlement.
fn settlement_completion_receipts_have_exact_custody(
    arguments: &[terminal_psi::StructuralArgument],
    sources: &[CompletionClaimSource],
    receipts: &[terminal_psi::CompletionReceipt],
) -> bool {
    let mut source_claims = BTreeSet::<semantic_vocabulary::ClaimId>::new();
    if sources.windows(2).any(|pair| pair[0] >= pair[1])
        || sources.iter().any(|source| {
            !source_claims.insert(source.claim()) || !claim_source_is_canonical(source)
        })
    {
        return false;
    }

    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            sources.iter().filter_map(move |source| {
                let argument_index = u32::try_from(index).ok()?;
                (source.input() == argument.place
                    && match &source.entry {
                        Some(source) => argument.path.is_empty() || source.path == argument.path,
                        None => true,
                    })
                .then_some((argument_index, source.claim()))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual = receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    let mut receipt_claims = BTreeSet::<semantic_vocabulary::ClaimId>::new();
    receipts.windows(2).all(|pair| pair[0] < pair[1])
        && receipts
            .iter()
            .all(|receipt| receipt_claims.insert(receipt.claim))
        && actual == expected
}

fn claim_source_is_canonical(source: &CompletionClaimSource) -> bool {
    let entry_is_canonical = source.entry.as_ref().is_none_or(|entry| {
        entry.claim == source.claim
            && entry.path.iter().all(|segment| {
                !matches!(segment, terminal_psi::StructuralPathSegment::Field(identity) if identity.is_empty())
            })
    });
    let content_is_canonical = source.content.as_ref().is_none_or(|content| {
        content.claim == source.claim
            && content.input.version == semantic_vocabulary::ContentPlaceVersion::Entry
            && !content.projections.is_empty()
            && !content
                .projections
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            && content.input.segments.iter().all(|segment| {
                !matches!(
                    segment,
                    semantic_vocabulary::ContentPlaceSegment::Case(identity)
                        | semantic_vocabulary::ContentPlaceSegment::Field(identity)
                        if identity.is_empty()
                )
            })
            && content.projections.iter().all(|projection| {
                projection.projection.projection_report_fingerprint != 0
                    && !projection.algebra.parameter.is_empty()
            })
    });
    let paired_sources_match =
        match (&source.entry, &source.content) {
            (Some(entry), Some(content)) => {
                entry.input == content.input.root
                    && entry.path.len() == content.input.segments.len()
                    && entry.path.iter().zip(&content.input.segments).all(
                        |(entry, content)| match (entry, content) {
                            (
                                terminal_psi::StructuralPathSegment::Field(entry),
                                semantic_vocabulary::ContentPlaceSegment::Field(content),
                            ) => entry == content,
                            (
                                terminal_psi::StructuralPathSegment::FixedIndex(entry),
                                semantic_vocabulary::ContentPlaceSegment::FixedIndex(content),
                            ) => entry == content,
                            _ => false,
                        },
                    )
            }
            _ => true,
        };
    (source.entry.is_some() || source.content.is_some())
        && entry_is_canonical
        && content_is_canonical
        && paired_sources_match
}

#[allow(clippy::too_many_arguments)]
fn derive_normalized_foreign_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    final_image_symbol_digest: [u8; 32],
    foreign: &image_emission::ObjectForeignCall,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("normalized foreign D41 occurrence does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        structural_arguments,
        completion_receipts,
    } = &operation.kind
    else {
        return Err("normalized foreign D41 occurrence is not a Terminal boundary call");
    };
    let declaration = module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .ok_or("normalized foreign D41 occurrence names an absent boundary")?;
    if *boundary != occurrence.boundary()
        || declaration.identity != requirement_identity
        || !structural_arguments.is_empty()
        || !completion_receipts.is_empty()
        || !declaration.structural_parameters.is_empty()
    {
        return Ok(None);
    }

    if foreign.operation_ordinal != occurrence.operation_ordinal()
        || arguments.len() != declaration.scalar_parameters.len()
        || foreign.scalar_arguments.len() != arguments.len()
    {
        return Err("normalized foreign D41 child changed its scalar call occurrence");
    }
    let Some(parameter_shapes) = declaration
        .scalar_parameters
        .iter()
        .copied()
        .map(fixed_integer_shape)
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    for ((argument, scalar_type), physical) in arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&foreign.scalar_arguments)
    {
        if physical.source.source_value() != *argument
            || physical.source.scalar_type() != *scalar_type
        {
            return Err("normalized foreign D41 child changed a scalar argument source");
        }
    }
    let result_shape = match (
        &operation.result,
        &declaration.result,
        &foreign.scalar_result,
    ) {
        (terminal_psi::OperationResult::Unit, terminal_psi::BoundaryMachineResult::Unit, None) => {
            None
        }
        (
            terminal_psi::OperationResult::Scalar(value),
            terminal_psi::BoundaryMachineResult::Scalar(declared),
            Some(physical),
        ) if value.scalar_type == *declared
            && physical.home.source_value == value.id
            && physical.home.scalar_type == *declared =>
        {
            let Some(shape) = fixed_integer_shape(*declared) else {
                return Ok(None);
            };
            Some(shape)
        }
        _ => return Err("normalized foreign D41 child changed its scalar result custody"),
    };
    let callback = match (
        foreign.callback_address.as_ref(),
        foreign
            .boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) {
        (None, []) => None,
        (Some(callback), [_]) => Some(callback),
        _ => return Err("normalized foreign D41 child changed its callback-plan custody"),
    };
    let callback_ordinal = callback
        .map(|callback| usize::try_from(callback.target.application.native_ordinal))
        .transpose()
        .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
    if callback_ordinal.is_some_and(|ordinal| ordinal > parameter_shapes.len()) {
        return Err("normalized foreign D41 callback ordinal is outside its native signature");
    }
    let mut native_parameter_shapes = parameter_shapes;
    if let (Some(callback), Some(ordinal)) = (callback, callback_ordinal) {
        native_parameter_shapes.insert(ordinal, callback.target.application.shape);
    }
    let signature = calling_conventions::CallSignature {
        parameters: native_parameter_shapes,
        result: result_shape,
    };

    let validated_plan = match callback {
        Some(callback) => {
            let callback_ordinal = callback_ordinal
                .expect("callback custody establishes one native parameter ordinal");
            let pointer_shape = calling_conventions::ValueShape::integer(
                u16::try_from(target.pointer_size)
                    .map_err(|_| "normalized foreign D41 pointer size does not fit its ABI")?,
                u16::try_from(target.pointer_alignment)
                    .map_err(|_| "normalized foreign D41 pointer alignment does not fit its ABI")?,
            );
            let expected_placement = match callback.destination {
                machine_code::CallbackAddressDestination::Register(register) => {
                    calling_conventions::ValuePlacement {
                        shape: callback.target.application.shape,
                        locations: vec![calling_conventions::ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: callback.target.application.shape.byte_size,
                        }],
                    }
                }
                machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                    calling_conventions::ValuePlacement {
                        shape: callback.target.application.shape,
                        locations: vec![calling_conventions::ValueLocation::Stack {
                            stack_byte_offset: byte_offset,
                            value_byte_offset: 0,
                            byte_size: callback.target.application.shape.byte_size,
                            alignment: callback.target.application.shape.alignment,
                        }],
                    }
                }
            };
            if callback.target.terminal_operation != occurrence.operation()
                || callback.target.registrar_boundary_entry_plan != foreign.boundary_entry_plan
                || callback
                    .target
                    .callback_function
                    .callback_thunk_placement_index()
                    != Some(callback.target.placement_index)
                || callback.target.registrar_application_commitment == [0; 32]
                || callback.target.application.shape != pointer_shape
                || callback.target.application.placement != expected_placement
                || foreign
                    .boundary_entry_plan
                    .call
                    .parameters
                    .get(callback_ordinal)
                    != Some(&callback.target.application.placement)
                || foreign.boundary_entry_plan.call.callback_materializations[0].destination
                    != calling_conventions::NativePlace::Parameter(
                        callback.target.application.parameter,
                    )
            {
                return Err("normalized foreign D41 child changed its callback target custody");
            }
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                foreign.boundary_entry_plan.clone(),
                &signature,
                &callback.target.registrar_context,
            )
            .map_err(
                |_| "normalized foreign D41 child contains an invalid callback boundary entry plan",
            )?
        }
        None => calling_conventions::validate_boundary_entry_plan(
            foreign.boundary_entry_plan.clone(),
            &signature,
        )
        .map_err(|_| "normalized foreign D41 child contains an invalid boundary entry plan")?,
    };
    let boundary_plan_identity = validated_plan.contract_commitment_digest();
    if validated_plan.plan() != &foreign.boundary_entry_plan
        || foreign.locator.target().native_target() != target
        || foreign.same_stack_contribution.requirement_identity() != requirement_identity
        || foreign
            .same_stack_contribution
            .provider_plan_report_identity()
            != selected_plan.report_identity()
        || foreign
            .same_stack_contribution
            .provider_plan_commitment()
            .as_bytes()
            != *selected_plan.plan_digest().as_bytes()
    {
        return Err(
            "normalized foreign D41 child changed its locator, boundary plan, or same-stack admission",
        );
    }

    let execution_record = foreign.provider_execution;
    if execution_record.provider_plan_report_identity != selected_plan.report_identity() {
        return Err("normalized foreign D41 child names the wrong selected provider plan");
    }
    let matching_executions = provider_executions
        .iter()
        .filter(|execution| {
            execution.requirement_identity() == requirement_identity
                && execution.provider_plan_report_identity()
                    == execution_record.provider_plan_report_identity
                && execution.provider_execution_report_identity()
                    == execution_record.provider_execution_report_identity
                && execution.provider_execution_report_fingerprint()
                    == execution_record.provider_execution_report_fingerprint
                && execution.normalized_root_report_identity()
                    == execution_record.normalized_root_report_identity
                && execution.boundary_contract_report_fingerprint()
                    == execution_record.boundary_contract_report_fingerprint
        })
        .count();
    if matching_executions != 1 {
        return Err("normalized foreign D41 child cannot rejoin one retained provider execution");
    }
    let plan_report_identity =
        ProviderPlanReportIdentity::new(execution_record.provider_plan_report_identity)
            .ok_or("normalized foreign D41 child has a zero provider-plan report identity")?;
    let execution = ProviderExecutionBinding::from_execution_record(
        plan_report_identity,
        execution_record.provider_execution_report_identity,
        execution_record.provider_execution_report_fingerprint,
        execution_record.normalized_root_report_identity,
        execution_record.boundary_contract_report_fingerprint,
    )
    .ok_or("normalized foreign D41 child has an invalid provider execution")?;

    let matching_image_calls = object
        .foreign_calls()
        .iter()
        .filter(|candidate| {
            candidate.machine == foreign.machine && candidate.owner == foreign.owner
        })
        .collect::<Vec<_>>();
    let [image_call] = matching_image_calls.as_slice() else {
        return Err("normalized foreign D41 child does not rejoin one final-image call");
    };
    if *image_call != foreign {
        return Err("normalized foreign D41 call custody changed before final-image emission");
    }

    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("normalized foreign D41 child names an absent object function")?;
    let matching_attributions = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == occurrence.machine()
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(occurrence.operation())
                && attribution.attribution.operation_ordinal == occurrence.operation_ordinal()
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        return Err("normalized foreign D41 child does not rejoin one emitted operation interval");
    };
    let code_offset = attribution.attribution.code_offset;
    let byte_count = attribution.attribution.byte_count;
    let object_offset = attribution.text_offset;
    if byte_count == 0
        || function
            .text_offset
            .checked_add(code_offset)
            .filter(|offset| *offset == object_offset)
            .is_none()
    {
        return Err("normalized foreign D41 child has a detached emitted operation interval");
    }
    let object_end = object_offset
        .checked_add(byte_count)
        .ok_or("normalized foreign D41 object end overflow")?;
    let expected_kind = match target.architecture {
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
    };
    let matching_imports = object
        .object()
        .layout
        .normalized_imports
        .iter()
        .filter(|import| import.locator == foreign.locator)
        .collect::<Vec<_>>();
    let [import] = matching_imports.as_slice() else {
        return Err("normalized foreign D41 child does not rejoin one object import");
    };
    let overlapping_relocations = object
        .relocations()
        .records()
        .map(|(_, relocation)| relocation)
        .filter(|relocation| {
            relocation.section == SectionKind::Text
                && ranges_overlap(
                    object_offset,
                    object_end,
                    relocation.offset,
                    relocation.offset.saturating_add(relocation.byte_width),
                )
        })
        .collect::<Vec<_>>();
    let expected_origin = RelocationOrigin::SemanticOperation {
        function_symbol_handle: function.symbol,
        operation_identity: occurrence.operation().get(),
    };
    let matching_import_relocations = overlapping_relocations
        .iter()
        .copied()
        .filter(|relocation| {
            relocation.origin == expected_origin
                && relocation.offset == foreign.text_offset
                && relocation.byte_width == 4
                && relocation.symbol_handle == import.symbol
                && relocation.addend == 0
                && relocation.kind == expected_kind
        })
        .collect::<Vec<_>>();
    let [relocation] = matching_import_relocations.as_slice() else {
        return Err(
            "normalized foreign D41 child changed import owner, symbol, addend, kind, or span",
        );
    };
    let callback_relocations = match callback {
        None => None,
        Some(callback) => {
            let (callback_symbol, _) = object_file::object_function_symbol(
                object.object(),
                callback.target.callback_function,
            )
            .ok_or("normalized foreign D41 callback lost its private object symbol")?;
            let callback_end = callback
                .code_offset
                .checked_add(callback.byte_count)
                .ok_or("normalized foreign D41 callback materialization span overflow")?;
            if callback.code_offset < object_offset || callback_end > object_end {
                return Err(
                    "normalized foreign D41 callback materialization left its operation interval",
                );
            }
            let exact_callback_relocation =
                |offset: usize, kind: RelocationKind| -> Result<_, &'static str> {
                    let matching = overlapping_relocations
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            candidate.origin == expected_origin
                                && candidate.offset == offset
                                && candidate.byte_width == 4
                                && candidate.symbol_handle == callback_symbol
                                && candidate.addend == 0
                                && candidate.kind == kind
                        })
                        .collect::<Vec<_>>();
                    let [matching] = matching.as_slice() else {
                        return Err(
                            "normalized foreign D41 callback changed a private-function relocation",
                        );
                    };
                    Ok(normalized_foreign_callback_relocation(
                        matching.symbol_handle,
                        matching.origin,
                        matching.offset,
                        matching.byte_width,
                        matching.addend,
                        matching.kind,
                    ))
                };
            Some(match callback.encoding {
                machine_code::CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                    NormalizedForeignCallbackRelocations::X86_64Relative32 {
                        callback_function: callback.target.callback_function,
                        relocation: exact_callback_relocation(
                            relocation_offset,
                            RelocationKind::X86_64Relative32,
                        )?,
                    }
                }
                machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                    callback_function: callback.target.callback_function,
                    page: exact_callback_relocation(
                        page_relocation_offset,
                        RelocationKind::Aarch64Page21,
                    )?,
                    page_offset: exact_callback_relocation(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?,
                },
            })
        }
    };
    let expected_relocation_count = 1 + match callback_relocations {
        None => 0,
        Some(NormalizedForeignCallbackRelocations::X86_64Relative32 { .. }) => 1,
        Some(NormalizedForeignCallbackRelocations::Aarch64PageAddress { .. }) => 2,
    };
    if overlapping_relocations.len() != expected_relocation_count
        || overlapping_relocations.iter().any(|relocation| {
            relocation.offset < object_offset
                || relocation
                    .offset
                    .checked_add(relocation.byte_width)
                    .is_none_or(|end| end > object_end)
        })
    {
        return Err("normalized foreign D41 child contains an unowned or out-of-span relocation");
    }

    let machine_span = native_byte_span(code_offset, byte_count);
    let object_span = native_byte_span(object_offset, byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes {
        return Err("normalized foreign D41 child changed before object custody");
    }
    let mutable_intervals = overlapping_relocations
        .iter()
        .map(|relocation| {
            let start = relocation
                .offset
                .checked_sub(object_offset)
                .ok_or("normalized foreign D41 relocation precedes its operation span")?;
            let end = start
                .checked_add(relocation.byte_width)
                .ok_or("normalized foreign D41 relocation span overflow")?;
            (end <= byte_count)
                .then_some((start, end))
                .ok_or("normalized foreign D41 relocation exceeds its operation span")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if object_bytes
        .iter()
        .zip(final_image_bytes)
        .enumerate()
        .any(|(index, (before, after))| {
            before != after
                && !mutable_intervals
                    .iter()
                    .any(|(start, end)| index >= *start && index < *end)
        })
    {
        return Err("normalized foreign D41 bytes changed outside its exact relocation set");
    }

    let realization = NormalizedForeignCallBinding {
        locator: foreign.locator.clone(),
        boundary_entry_plan: foreign.boundary_entry_plan.clone(),
        same_stack_contribution: foreign.same_stack_contribution.clone(),
    };
    let role = BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    };
    let parent_identity = admitted_provider_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan.plan_digest(),
        target,
        execution,
        boundary_plan_identity,
        &foreign.locator,
        foreign.same_stack_contribution.commitment().as_bytes(),
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest: selected_plan.plan_digest(),
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let relocation = PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
        normalized_foreign_call_relocation(
            foreign.locator.identity_digest().as_bytes(),
            boundary_plan_identity,
            import.symbol,
            relocation.origin,
            relocation.offset,
            relocation.byte_width,
            relocation.addend,
            relocation.kind,
            callback_relocations,
            final_image_symbol_digest,
        ),
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let physical_occurrence = NativePhysicalOccurrence::Boundary(occurrence.identity());
    let identity = physical_child_identity(
        &parent,
        projection,
        physical_occurrence,
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(Some(
        NativePhysicalChildParts {
            parent,
            projection,
            occurrence: physical_occurrence,
            machine_span,
            object_span,
            final_image_span,
            machine_bytes_digest,
            object_bytes_digest,
            final_image_bytes_digest,
            relocation,
            identity,
        }
        .into(),
    ))
}

fn fixed_integer_shape(scalar_type: ScalarType) -> Option<calling_conventions::ValueShape> {
    let ScalarType::Integer(integer) = scalar_type else {
        return None;
    };
    let bits = integer.bits();
    if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || !matches!(bits, 8 | 16 | 32 | 64)
    {
        return None;
    }
    let bytes = bits / 8;
    Some(calling_conventions::ValueShape::integer(bytes, bytes))
}

fn builtin_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    scalar_argument: &target_operations::BoundaryScalarArgument,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([1]); // HostedV1 compiler-builtin catalog.
    digest.update([1, 1]); // CompilerBuiltin::HostedExitProcessI32 + realization.
    digest.update(scalar_argument.source_value.get().to_le_bytes());
    digest.update([1]); // exact signed i32 scalar schema
    let semantic_vocabulary::IntegerValue::Signed(value) = scalar_argument.immediate else {
        unreachable!("D41 settlement shape was checked")
    };
    digest.update(i32::try_from(value).expect("checked i32").to_le_bytes());
    digest.finalize().into()
}

fn builtin_runtime_scalar_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    scalar_argument: &machine_code::ForeignCallScalarArgumentRecord,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    if matches!(
        scalar_argument.source,
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
    ) {
        digest.update([1, 1, 1]);
    } else {
        digest.update([1, 2, 2]);
    }
    digest.update(scalar_argument.parameter_index.to_le_bytes());
    match scalar_argument.source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            source_value,
            instruction,
            ..
        } => {
            digest.update([6]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            instruction,
            ..
        } => {
            digest.update([5]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
            source_value,
            instruction,
            scratch_byte_offset,
            ..
        } => {
            digest.update([4]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
            digest.update(scratch_byte_offset.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.update([0]);
            digest.update(parameter_index.to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.update([1]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    digest.update(value.to_le_bytes())
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    digest.update(value.to_le_bytes())
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            digest.update([3]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            digest.update([u8::from(value)]);
            digest.update(
                u64::try_from(definition_ordinal)
                    .expect("validated definition ordinal is u64-representable")
                    .to_le_bytes(),
            );
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.update([2]);
            digest.update(home.defining_operation.get().to_le_bytes());
            digest.update(home.source_value.get().to_le_bytes());
            digest.update(home.byte_offset.to_le_bytes());
        }
    }
    digest.finalize().into()
}

fn builtin_structural_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<[u8; 32], &'static str> {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v3\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([1, 3, 3]); // HostedV1, read-byte execution, structural role.
    digest.update(result.defining_operation.get().to_le_bytes());
    digest.update(result.result.place.get().to_le_bytes());
    digest.update(result.result.structural_type.get().to_le_bytes());
    digest.update([match result.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }]);
    digest.update((result.result.qualifications.len() as u64).to_le_bytes());
    for domain in &result.result.qualifications {
        digest.update(domain.get().to_le_bytes());
    }
    digest.update((result.result.projected_qualifications.len() as u64).to_le_bytes());
    for qualification in &result.result.projected_qualifications {
        hash_structural_path(&mut digest, &qualification.path);
        digest.update(qualification.domain.get().to_le_bytes());
    }
    digest.update((result.result.claims.len() as u64).to_le_bytes());
    for claim in &result.result.claims {
        digest.update(claim.claim.get().to_le_bytes());
        hash_structural_path(&mut digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)
        .map_err(|_| "hosted read-byte result declaration cannot be encoded canonically")?;
    hash_bytes(&mut digest, &declaration);
    hash_sum_layout(&mut digest, &result.layout);
    digest.update(result.home_byte_offset.to_le_bytes());
    Ok(digest.finalize().into())
}

fn hash_structural_path(digest: &mut Sha256, path: &[terminal_psi::StructuralPathSegment]) {
    digest.update((path.len() as u64).to_le_bytes());
    for segment in path {
        match segment {
            terminal_psi::StructuralPathSegment::Referent => digest.update([3]),
            terminal_psi::StructuralPathSegment::Field(identity) => {
                digest.update([1]);
                hash_bytes(digest, identity.as_bytes());
            }
            terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                digest.update([2]);
                digest.update(index.to_le_bytes());
            }
        }
    }
}

fn hash_sum_layout(digest: &mut Sha256, layout: &calling_conventions::ConventionalSumLayout) {
    hash_integer_shape(digest, layout.shape);
    digest.update(layout.tag_byte_offset.to_le_bytes());
    hash_integer_shape(digest, layout.tag_shape);
    hash_packed_fields(digest, &layout.common_fields);
    digest.update(layout.payload_byte_offset.to_le_bytes());
    digest.update((layout.cases.len() as u64).to_le_bytes());
    for case in &layout.cases {
        hash_packed_fields(digest, &case.fields);
    }
}

fn hash_packed_fields(digest: &mut Sha256, fields: &[calling_conventions::PackedFieldLayout]) {
    digest.update((fields.len() as u64).to_le_bytes());
    for field in fields {
        digest.update(field.byte_offset.to_le_bytes());
        hash_integer_shape(digest, field.shape);
    }
}

fn hash_integer_shape(digest: &mut Sha256, shape: calling_conventions::ValueShape) {
    debug_assert_eq!(shape.class, calling_conventions::ValueClass::Integer);
    digest.update([1]);
    digest.update(shape.byte_size.to_le_bytes());
    digest.update(shape.alignment.to_le_bytes());
}

#[allow(clippy::too_many_arguments)]
fn admitted_provider_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    execution: ProviderExecutionBinding,
    boundary_plan_identity: [u8; 32],
    locator: &target::NormalizedForeignLocator,
    same_stack_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([2]); // AdmittedProvider::NormalizedForeignCall.
    digest.update(
        execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.update(execution.provider_execution_report_identity().to_le_bytes());
    digest.update(
        execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(execution.normalized_root_report_identity().to_le_bytes());
    digest.update(
        execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(locator.identity_digest().as_bytes());
    digest.update(boundary_plan_identity);
    digest.update(same_stack_identity);
    digest.finalize().into()
}

/// Strong identity for one exact installed provider settlement realized in
/// place. The hash binds the complete retained settlement row — execution,
/// realization, scalar/structural/byte-sequence argument custody, completion
/// claim sources, receipts, provider custody, result placement, and source
/// coordinates — plus the joined privileged port effect when one exists.
#[allow(clippy::too_many_arguments)]
fn admitted_provider_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    execution: ProviderExecutionBinding,
    settlement: &machine_code::BoundarySettlementRecord,
    port_effect: Option<&PortEffectRecord>,
) -> Result<[u8; 32], &'static str> {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v4\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([3]); // AdmittedProviderSettlement role.
    digest.update(
        execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.update(execution.provider_execution_report_identity().to_le_bytes());
    digest.update(
        execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(execution.normalized_root_report_identity().to_le_bytes());
    digest.update(
        execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    hash_boundary_settlement_record(&mut digest, settlement)?;
    match port_effect {
        None => digest.update([0]),
        Some(effect) => {
            digest.update([1]);
            hash_port_effect_record(&mut digest, effect);
        }
    }
    Ok(digest.finalize().into())
}

fn hash_port_effect_record(digest: &mut Sha256, effect: &PortEffectRecord) {
    digest.update(effect.psi_operation.get().to_le_bytes());
    digest.update(effect.service.get().to_le_bytes());
    digest.update(effect.port.to_le_bytes());
    digest.update([effect.value]);
    digest.update(canonical_usize(effect.operation_ordinal));
    digest.update(canonical_usize(effect.code_offset));
    digest.update(canonical_usize(effect.byte_count));
}

/// Hash the complete retained settlement row so no realization, argument,
/// completion, result, or coordinate field can be substituted underneath a
/// parent identity.
fn hash_boundary_settlement_record(
    digest: &mut Sha256,
    settlement: &machine_code::BoundarySettlementRecord,
) -> Result<(), &'static str> {
    digest.update(settlement.psi_operation.get().to_le_bytes());
    digest.update(settlement.boundary.get().to_le_bytes());
    match settlement.execution {
        BoundaryExecutionRecord::AdmittedProvider(record) => {
            digest.update([1]);
            digest.update(record.provider_plan_report_identity.to_le_bytes());
            digest.update(record.provider_execution_report_identity.to_le_bytes());
            digest.update(record.provider_execution_report_fingerprint.to_le_bytes());
            digest.update(record.normalized_root_report_identity.to_le_bytes());
            digest.update(record.boundary_contract_report_fingerprint.to_le_bytes());
        }
        BoundaryExecutionRecord::CompilerBuiltin(execution) => {
            digest.update([2]);
            digest.update([compiler_builtin_execution_tag(execution)]);
        }
    }
    hash_boundary_realization(digest, &settlement.realization);
    digest.update(canonical_usize(settlement.scalar_arguments.len()));
    for argument in &settlement.scalar_arguments {
        digest.update(argument.source_value.get().to_le_bytes());
        hash_scalar_type(digest, argument.scalar_type);
        hash_integer_value(digest, argument.immediate);
        hash_machine_register(digest, argument.destination);
    }
    digest.update(canonical_usize(settlement.runtime_scalar_arguments.len()));
    for argument in &settlement.runtime_scalar_arguments {
        digest.update(argument.parameter_index.to_le_bytes());
        hash_internal_scalar_argument_source(digest, &argument.source);
        hash_value_placement(digest, &argument.placement);
        digest.update(canonical_usize(argument.code_offset));
        digest.update(canonical_usize(argument.byte_count));
    }
    digest.update(canonical_usize(settlement.arguments.len()));
    for argument in &settlement.arguments {
        hash_structural_argument(digest, argument);
    }
    digest.update(canonical_usize(settlement.byte_sequence_arguments.len()));
    for custody in &settlement.byte_sequence_arguments {
        hash_structural_argument(digest, &custody.argument);
        digest.update(custody.literal_operation.get().to_le_bytes());
        let declaration = terminal_codec::encode_structural_type_declaration(
            &custody.structural_type,
        )
        .map_err(|_| "installed D41 byte-sequence declaration cannot be encoded canonically")?;
        hash_bytes(digest, &declaration);
        hash_bytes(digest, &custody.bytes);
        digest.update(canonical_usize(custody.code_offset));
        digest.update(canonical_usize(custody.code_byte_count));
        digest.update(canonical_usize(custody.data_offset));
        digest.update(canonical_usize(custody.data_byte_count));
    }
    digest.update(canonical_usize(settlement.completion_claim_sources.len()));
    for source in &settlement.completion_claim_sources {
        hash_claim_source(digest, source);
    }
    digest.update(canonical_usize(settlement.completion_receipts.len()));
    for receipt in &settlement.completion_receipts {
        digest.update(receipt.claim.get().to_le_bytes());
        digest.update(receipt.argument_index.to_le_bytes());
    }
    digest.update(canonical_usize(
        settlement.completion_provider_custody.len(),
    ));
    for binding in &settlement.completion_provider_custody {
        hash_claim_source(digest, &binding.source);
        digest.update(binding.receipt.claim.get().to_le_bytes());
        digest.update(binding.receipt.argument_index.to_le_bytes());
        let execution = binding.provider_execution;
        digest.update(execution.provider_plan_report_identity.to_le_bytes());
        digest.update(execution.provider_execution_report_identity.to_le_bytes());
        digest.update(
            execution
                .provider_execution_report_fingerprint
                .to_le_bytes(),
        );
        digest.update(execution.normalized_root_report_identity.to_le_bytes());
        digest.update(execution.boundary_contract_report_fingerprint.to_le_bytes());
    }
    hash_boundary_result_record(digest, &settlement.native_result)?;
    digest.update(canonical_usize(settlement.operation_ordinal));
    digest.update(canonical_usize(settlement.code_offset));
    digest.update(canonical_usize(settlement.byte_count));
    Ok(())
}

const fn compiler_builtin_execution_tag(execution: CompilerBuiltinExecution) -> u8 {
    match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => 1,
        CompilerBuiltinExecution::HostedReadByte => 2,
        CompilerBuiltinExecution::HostedWriteByteI32 => 3,
    }
}

fn hash_boundary_realization(digest: &mut Sha256, realization: &BoundaryRealization) {
    match realization {
        BoundaryRealization::MetadataOnlyPort(realization) => {
            digest.update([1]);
            digest.update(realization.effect_operation.get().to_le_bytes());
            digest.update(realization.service.get().to_le_bytes());
            digest.update(realization.port.to_le_bytes());
            digest.update([realization.value]);
        }
        BoundaryRealization::DirectPortReadU8(realization) => {
            digest.update([2]);
            digest.update(realization.service.get().to_le_bytes());
            digest.update(realization.port.to_le_bytes());
        }
        BoundaryRealization::LinuxWriteLine(_) => digest.update([3]),
        BoundaryRealization::ClaimCompletionOnly(_) => digest.update([4]),
        BoundaryRealization::HostedExitProcessI32(_) => digest.update([5]),
        BoundaryRealization::HostedReadByte(_) => digest.update([6]),
        BoundaryRealization::HostedWriteByteI32(_) => digest.update([7]),
    }
}

fn hash_boundary_result_record(
    digest: &mut Sha256,
    result: &machine_code::BoundaryResultRecord,
) -> Result<(), &'static str> {
    match result {
        machine_code::BoundaryResultRecord::Unit => digest.update([0]),
        machine_code::BoundaryResultRecord::Scalar(result) => {
            digest.update([1]);
            digest.update(result.value.get().to_le_bytes());
            hash_scalar_type(digest, result.scalar_type);
            hash_value_placement(digest, &result.placement);
            digest.update(result.return_edge.get().to_le_bytes());
        }
        machine_code::BoundaryResultRecord::Structural(result) => {
            digest.update([2]);
            hash_boundary_structural_result(digest, result)?;
        }
    }
    Ok(())
}

/// The complete structural-result record body shared by the hosted read-byte
/// parent identity and the installed-settlement record hash.
fn hash_boundary_structural_result(
    digest: &mut Sha256,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<(), &'static str> {
    digest.update(result.defining_operation.get().to_le_bytes());
    digest.update(result.result.place.get().to_le_bytes());
    digest.update(result.result.structural_type.get().to_le_bytes());
    digest.update([match result.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }]);
    digest.update((result.result.qualifications.len() as u64).to_le_bytes());
    for domain in &result.result.qualifications {
        digest.update(domain.get().to_le_bytes());
    }
    digest.update((result.result.projected_qualifications.len() as u64).to_le_bytes());
    for qualification in &result.result.projected_qualifications {
        hash_structural_path(digest, &qualification.path);
        digest.update(qualification.domain.get().to_le_bytes());
    }
    digest.update((result.result.claims.len() as u64).to_le_bytes());
    for claim in &result.result.claims {
        digest.update(claim.claim.get().to_le_bytes());
        hash_structural_path(digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)
        .map_err(|_| "boundary structural result declaration cannot be encoded canonically")?;
    hash_bytes(digest, &declaration);
    hash_sum_layout(digest, &result.layout);
    digest.update(result.home_byte_offset.to_le_bytes());
    Ok(())
}

fn hash_scalar_type(digest: &mut Sha256, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => digest.update([1]),
        ScalarType::Integer(integer) => {
            digest.update([2]);
            digest.update([match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            }]);
            digest.update(integer.bits().to_le_bytes());
            digest.update([match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 1,
                semantic_vocabulary::IntegerCarrier::Address => 2,
            }]);
        }
        ScalarType::IeeeFloat(format) => {
            digest.update([3]);
            digest.update([match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
            }]);
        }
    }
}

fn hash_integer_value(digest: &mut Sha256, value: semantic_vocabulary::IntegerValue) {
    match value {
        semantic_vocabulary::IntegerValue::Signed(value) => {
            digest.update([1]);
            digest.update(value.to_le_bytes());
        }
        semantic_vocabulary::IntegerValue::Unsigned(value) => {
            digest.update([2]);
            digest.update(value.to_le_bytes());
        }
    }
}

fn hash_machine_register(digest: &mut Sha256, register: calling_conventions::MachineRegister) {
    use calling_conventions::MachineRegister;
    match register {
        MachineRegister::X86Rax => digest.update([1]),
        MachineRegister::X86Rcx => digest.update([2]),
        MachineRegister::X86Rdx => digest.update([3]),
        MachineRegister::X86Rbx => digest.update([4]),
        MachineRegister::X86Rsp => digest.update([5]),
        MachineRegister::X86Rbp => digest.update([6]),
        MachineRegister::X86Rsi => digest.update([7]),
        MachineRegister::X86Rdi => digest.update([8]),
        MachineRegister::X86R8 => digest.update([9]),
        MachineRegister::X86R9 => digest.update([10]),
        MachineRegister::X86R10 => digest.update([11]),
        MachineRegister::X86R11 => digest.update([12]),
        MachineRegister::X86R12 => digest.update([13]),
        MachineRegister::X86R13 => digest.update([14]),
        MachineRegister::X86R14 => digest.update([15]),
        MachineRegister::X86R15 => digest.update([16]),
        MachineRegister::X86Xmm(index) => {
            digest.update([17]);
            digest.update([index]);
        }
        MachineRegister::Aarch64X(index) => {
            digest.update([18]);
            digest.update([index]);
        }
        MachineRegister::Aarch64V(index) => {
            digest.update([19]);
            digest.update([index]);
        }
    }
}

fn hash_value_placement(digest: &mut Sha256, placement: &calling_conventions::ValuePlacement) {
    hash_value_shape(digest, placement.shape);
    digest.update(canonical_usize(placement.locations.len()));
    for location in &placement.locations {
        match location {
            calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                digest.update([1]);
                hash_machine_register(digest, *register);
                digest.update(value_byte_offset.to_le_bytes());
                digest.update(byte_size.to_le_bytes());
            }
            calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.update([2]);
                digest.update(stack_byte_offset.to_le_bytes());
                digest.update(value_byte_offset.to_le_bytes());
                digest.update(byte_size.to_le_bytes());
                digest.update(alignment.to_le_bytes());
            }
            calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.update([3]);
                match pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        digest.update([1]);
                        hash_machine_register(digest, *register);
                    }
                    calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        digest.update([2]);
                        digest.update(stack_byte_offset.to_le_bytes());
                        digest.update(alignment.to_le_bytes());
                    }
                }
                match copy_stack_byte_offset {
                    None => digest.update([0]),
                    Some(offset) => {
                        digest.update([1]);
                        digest.update(offset.to_le_bytes());
                    }
                }
                digest.update(byte_size.to_le_bytes());
                digest.update(alignment.to_le_bytes());
            }
        }
    }
}

fn hash_value_shape(digest: &mut Sha256, shape: calling_conventions::ValueShape) {
    match shape.class {
        calling_conventions::ValueClass::Integer => digest.update([1]),
        calling_conventions::ValueClass::Float => digest.update([2]),
        calling_conventions::ValueClass::BorrowedReference => digest.update([3]),
        calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
            digest.update([4]);
            digest.update([members]);
        }
        calling_conventions::ValueClass::SystemVAggregate { first, second } => {
            digest.update([5]);
            for class in [first, second] {
                digest.update([match class {
                    calling_conventions::SystemVEightbyteClass::Integer => 1,
                    calling_conventions::SystemVEightbyteClass::Sse => 2,
                }]);
            }
        }
    }
    digest.update(shape.byte_size.to_le_bytes());
    digest.update(shape.alignment.to_le_bytes());
}

fn hash_structural_argument(digest: &mut Sha256, argument: &terminal_psi::StructuralArgument) {
    digest.update(argument.place.get().to_le_bytes());
    digest.update([match argument.access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    }]);
    hash_structural_path(digest, &argument.path);
}

fn hash_claim_source(digest: &mut Sha256, source: &CompletionClaimSource) {
    digest.update(source.claim.get().to_le_bytes());
    match &source.entry {
        None => digest.update([0]),
        Some(entry) => {
            digest.update([1]);
            digest.update(entry.claim.get().to_le_bytes());
            digest.update(entry.input.get().to_le_bytes());
            hash_structural_path(digest, &entry.path);
        }
    }
    match &source.content {
        None => digest.update([0]),
        Some(content) => {
            digest.update([1]);
            digest.update(content.claim.get().to_le_bytes());
            digest.update([match content.input.version {
                semantic_vocabulary::ContentPlaceVersion::Entry => 1,
                semantic_vocabulary::ContentPlaceVersion::Current => 2,
            }]);
            digest.update(content.input.root.get().to_le_bytes());
            digest.update(canonical_usize(content.input.segments.len()));
            for segment in &content.input.segments {
                match segment {
                    semantic_vocabulary::ContentPlaceSegment::Case(identity) => {
                        digest.update([1]);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::Field(identity) => {
                        digest.update([2]);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::FixedIndex(index) => {
                        digest.update([3]);
                        digest.update(index.to_le_bytes());
                    }
                }
            }
            digest.update(canonical_usize(content.projections.len()));
            for projection in &content.projections {
                digest.update(projection.projection.domain.get().to_le_bytes());
                digest.update(
                    projection
                        .projection
                        .projection_report_fingerprint
                        .to_le_bytes(),
                );
                digest.update([match projection.algebra.kind {
                    semantic_vocabulary::ContentAlgebraKind::IntervalSet => 1,
                    semantic_vocabulary::ContentAlgebraKind::CountedQuantity => 2,
                }]);
                hash_bytes(digest, projection.algebra.parameter.as_bytes());
            }
        }
    }
}

fn hash_internal_scalar_argument_source(
    digest: &mut Sha256,
    source: &machine_code::InternalUnitScalarArgumentSourceRecord,
) {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            source_value,
            instruction,
            ..
        } => {
            digest.update([6]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            instruction,
            ..
        } => {
            digest.update([5]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
            source_value,
            instruction,
            scratch_byte_offset,
            ..
        } => {
            digest.update([4]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
            digest.update(scratch_byte_offset.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.update([0]);
            digest.update(parameter_index.to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.update([1]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    digest.update(value.to_le_bytes())
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    digest.update(value.to_le_bytes())
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            digest.update([3]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            digest.update([u8::from(*value)]);
            digest.update(
                u64::try_from(*definition_ordinal)
                    .expect("validated definition ordinal is u64-representable")
                    .to_le_bytes(),
            );
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.update([2]);
            digest.update(home.defining_operation.get().to_le_bytes());
            digest.update(home.source_value.get().to_le_bytes());
            digest.update(home.byte_offset.to_le_bytes());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn physical_child_identity(
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

fn span(bytes: &[u8], span: NativeByteSpan) -> Result<&[u8], &'static str> {
    let end = span
        .offset()
        .checked_add(span.byte_count())
        .ok_or("native physical child byte span overflow")?;
    bytes
        .get(span.offset()..end)
        .ok_or("native physical child byte span is out of bounds")
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_target(digest: &mut Sha256, target: NativeTarget) {
    digest.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    digest.update(canonical_usize(target.pointer_size));
    digest.update(canonical_usize(target.pointer_alignment));
}

fn hash_object_symbol(digest: &mut Sha256, symbol: object_file::ObjectSymbolHandle) {
    digest.update([u8::from(symbol.is_valid())]);
    digest.update(u64::from(symbol.arena_index()).to_le_bytes());
    digest.update(u64::from(symbol.generation()).to_le_bytes());
}

fn hash_machine_function_identity(
    digest: &mut Sha256,
    identity: function_identity::MachineFunctionIdentity,
) {
    let (tag, continuation, coordinate) = if let Some(source) = identity.source_key() {
        (1, source, 0)
    } else if let Some(continuation) = identity.program_storage_entry_continuation() {
        (2, continuation, 0)
    } else {
        (
            3,
            identity.associated_source_continuation(),
            identity
                .callback_thunk_placement_index()
                .expect("machine function identity has one closed role"),
        )
    };
    digest.update([tag]);
    for symbol in [continuation.machine, continuation.state] {
        digest.update([u8::from(symbol.is_valid())]);
        digest.update(u64::from(symbol.arena_index()).to_le_bytes());
        digest.update(u64::from(symbol.generation()).to_le_bytes());
    }
    digest.update(canonical_usize(continuation.segment_index));
    digest.update(canonical_usize(coordinate));
}

fn hash_callback_relocation(digest: &mut Sha256, relocation: NormalizedForeignCallbackRelocation) {
    hash_object_symbol(digest, relocation.object_symbol());
    hash_relocation_origin(digest, relocation.origin());
    digest.update(canonical_usize(relocation.offset()));
    digest.update(canonical_usize(relocation.byte_width()));
    digest.update(relocation.addend().to_le_bytes());
    digest.update([relocation_kind_tag(relocation.kind())]);
}

fn hash_relocation_origin(digest: &mut Sha256, origin: RelocationOrigin) {
    hash_object_symbol(digest, origin.symbol_handle());
    let (tag, coordinate) = match origin {
        RelocationOrigin::Instruction {
            selected_instruction_index,
            ..
        } => (1, u64::from(selected_instruction_index)),
        RelocationOrigin::SemanticOperation {
            operation_identity, ..
        } => (2, operation_identity),
        RelocationOrigin::SemanticEdge { edge_identity, .. } => (3, edge_identity),
        RelocationOrigin::Materialization { .. } => (4, 0),
    };
    digest.update([tag]);
    digest.update(coordinate.to_le_bytes());
}

const fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native physical evidence field fits u64")
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::{
        IntegerSign, IntegerType, NativeOptimizationProjection,
        NativeOptimizationProjectionIdentity, NativePhysicalOccurrence,
        NativeSelectedProviderPlanDigest, NativeTarget, OptimizedBoundaryOccurrenceIdentity,
        OptimizedOperatorOccurrenceIdentity, PhysicalChildCoordinate, ScalarType,
        admitted_provider_settlement_identity, boundary_occurrence_identity,
        builtin_structural_boundary_trait_settlement_identity, native_optimization_projection,
        operator_occurrence_identity, optimized_boundary_occurrence, optimized_operator_occurrence,
        validate_exact_physical_child_coordinates,
    };
    #[test]
    fn reference_projection_identity_is_distinct_from_owned_paths() {
        use terminal_psi::StructuralPathSegment;
        let identity = |path: &[StructuralPathSegment]| {
            let mut digest = sha2::Sha256::default();
            super::hash_structural_path(&mut digest, path);
            sha2::Digest::finalize(digest)
        };
        let reference = identity(&[StructuralPathSegment::Referent]);
        assert_ne!(reference, identity(&[]));
        assert_ne!(
            reference,
            identity(&[StructuralPathSegment::Field("Referent".into())])
        );
        assert_ne!(reference, identity(&[StructuralPathSegment::FixedIndex(3)]));
    }
    use terminal_psi::{SemanticFingerprint, VocabularyMarker};

    fn physical_projection() -> NativeOptimizationProjection {
        let terminal = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
        };
        let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
        let operator = optimized_operator_occurrence(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(2).expect("operator"),
            0,
            OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"operator survivor"),
        );
        let boundary = optimized_boundary_occurrence(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(3).expect("boundary operation"),
            semantic_vocabulary::BoundaryMachineId::new(4).expect("boundary"),
            1,
            OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"boundary survivor"),
        );
        native_optimization_projection(
            terminal,
            vec![operator],
            vec![boundary],
            NativeOptimizationProjectionIdentity::from_canonical_bytes(b"physical projection"),
        )
    }

    fn exact_coordinates(
        projection: &NativeOptimizationProjection,
    ) -> [PhysicalChildCoordinate; 2] {
        [
            PhysicalChildCoordinate {
                projection: projection.identity(),
                occurrence: NativePhysicalOccurrence::Operator(
                    projection.operator_occurrences()[0].identity(),
                ),
                parent_role: 1,
            },
            PhysicalChildCoordinate {
                projection: projection.identity(),
                occurrence: NativePhysicalOccurrence::Boundary(
                    projection.boundary_occurrences()[0].identity(),
                ),
                parent_role: 2,
            },
        ]
    }

    #[test]
    fn equal_boundary_requirements_at_distinct_operations_have_distinct_occurrences() {
        let terminal = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        };
        let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
        let boundary = semantic_vocabulary::BoundaryMachineId::new(2).expect("boundary");
        let first = boundary_occurrence_identity(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(3).expect("first operation"),
            boundary,
            0,
        );
        let second = boundary_occurrence_identity(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(4).expect("second operation"),
            boundary,
            1,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn operator_occurrence_identity_binds_exact_terminal_operation() {
        let terminal = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
        };
        let machine = semantic_vocabulary::MachineId::new(1).expect("machine");
        let first = operator_occurrence_identity(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(3).expect("first operation"),
            0,
        );
        let second = operator_occurrence_identity(
            terminal,
            machine,
            semantic_vocabulary::OperationId::new(4).expect("second operation"),
            1,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn physical_children_require_an_exact_survivor_bijection() {
        let projection = physical_projection();
        let [operator, boundary] = exact_coordinates(&projection);
        assert!(
            validate_exact_physical_child_coordinates(&projection, [operator, boundary]).is_ok()
        );

        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator]),
            Err("native physical evidence does not cover the exact surviving occurrence set")
        );
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator, operator, boundary]),
            Err("native physical evidence contains duplicate optimized occurrences")
        );

        let padded = PhysicalChildCoordinate {
            projection: projection.identity(),
            occurrence: NativePhysicalOccurrence::Operator(
                OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"stale occurrence"),
            ),
            parent_role: 1,
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator, boundary, padded]),
            Err("native physical child swapped or substituted its semantic parent role")
        );

        let detached = PhysicalChildCoordinate {
            projection: NativeOptimizationProjectionIdentity::from_canonical_bytes(
                b"detached projection",
            ),
            ..operator
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [detached, boundary]),
            Err("native physical child is detached from its optimized projection")
        );

        let role_swapped = PhysicalChildCoordinate {
            parent_role: 2,
            ..operator
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [role_swapped, boundary]),
            Err("native physical child swapped or substituted its semantic parent role")
        );
    }

    #[test]
    fn structural_boundary_settlement_identity_binds_the_complete_result_declaration() {
        use semantic_vocabulary::{
            BoundedIntegerType, IntegerValue, OperationId, PlaceId, StructuralCaseId,
            StructuralFieldId, StructuralTypeId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
            StructuralTypeDeclaration, StructuralTypeShape,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let original = machine_code::BoundaryStructuralResultRecord {
            defining_operation: OperationId::new(3).unwrap(),
            result: StructuralOperationResult {
                place: PlaceId::new(1).unwrap(),
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            declaration: StructuralTypeDeclaration {
                id: structural_type,
                identity: "InputResult".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![
                        StructuralCaseDeclaration {
                            id: StructuralCaseId::new(1).unwrap(),
                            identity: "End".into(),
                            fields: Vec::new(),
                        },
                        StructuralCaseDeclaration {
                            id: StructuralCaseId::new(2).unwrap(),
                            identity: "Value".into(),
                            fields: vec![StructuralFieldDeclaration {
                                id: StructuralFieldId::new(1).unwrap(),
                                identity: "value".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::BoundedInteger(
                                    BoundedIntegerType::new(
                                        integer,
                                        IntegerValue::Signed(0),
                                        IntegerValue::Signed(255),
                                    )
                                    .unwrap(),
                                ),
                            }],
                        },
                    ],
                },
            },
            layout: calling_conventions::evaluate_conventional_sum_layout(
                &[],
                &[
                    Vec::new(),
                    vec![calling_conventions::ValueShape::integer(4, 4)],
                ],
            )
            .unwrap(),
            home_byte_offset: 16,
        };
        let projection = physical_projection();
        let identity = |result: &machine_code::BoundaryStructuralResultRecord| {
            builtin_structural_boundary_trait_settlement_identity(
                &projection.boundary_occurrences()[0],
                "Input::read",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                result,
            )
            .unwrap()
        };
        let expected = identity(&original);
        for mutation in 0..7 {
            let mut changed = original.clone();
            let StructuralTypeShape::Sum { cases } = &mut changed.declaration.shape else {
                panic!("sum")
            };
            match mutation {
                0 => changed.declaration.identity.push_str("Other"),
                1 => cases[1].identity.push_str("Other"),
                2 => cases[1].id = StructuralCaseId::new(3).unwrap(),
                3 => cases[1].fields[0].identity.push_str("Other"),
                4 => cases[1].fields[0].id = StructuralFieldId::new(2).unwrap(),
                5 => {
                    cases[1].fields[0].field_type = StructuralFieldType::BoundedInteger(
                        BoundedIntegerType::new(
                            integer,
                            IntegerValue::Signed(-1),
                            IntegerValue::Signed(256),
                        )
                        .unwrap(),
                    )
                }
                _ => {
                    cases[1].fields[0].field_type =
                        StructuralFieldType::Scalar(ScalarType::Integer(integer))
                }
            }
            assert_eq!(
                changed.layout, original.layout,
                "unchanged layout cannot mask semantic drift"
            );
            assert_ne!(
                identity(&changed),
                expected,
                "declaration mutation {mutation}"
            );
        }
    }

    #[test]
    fn admitted_provider_settlement_identity_binds_the_complete_retained_row() {
        use semantic_vocabulary::{
            BoundaryMachineId, ClaimId, EdgeId, IntegerValue, OperationId, PlaceId, ServiceId,
            StructuralCaseId, StructuralTypeId, ValueId,
        };
        use target_operations::{
            BoundaryScalarArgument, ClaimCompletionOnlyRealization, MetadataOnlyPortRealization,
        };
        use terminal_psi::{
            CompletionReceipt, EntryClaim, StructuralAccess, StructuralArgument,
            StructuralCaseDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        };

        let projection = physical_projection();
        let occurrence = &projection.boundary_occurrences()[0];
        let record = machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 19).unwrap();
        let execution = super::ProviderExecutionBinding::from_execution_record(
            super::ProviderPlanReportIdentity::new(7).unwrap(),
            11,
            13,
            17,
            19,
        )
        .unwrap();
        let claim = ClaimId::new(23).unwrap();
        let argument = StructuralArgument {
            place: PlaceId::new(29).unwrap(),
            path: vec![terminal_psi::StructuralPathSegment::Field("leaf".into())],
            access: StructuralAccess::Owned,
        };
        let source = super::CompletionClaimSource {
            claim,
            entry: Some(EntryClaim {
                claim,
                input: argument.place,
                path: Vec::new(),
            }),
            content: None,
        };
        let receipt = CompletionReceipt {
            claim,
            argument_index: 0,
        };
        let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let byte_sequence = machine_code::BoundaryByteSequenceArgumentRecord {
            argument: argument.clone(),
            literal_operation: OperationId::new(47).unwrap(),
            structural_type: StructuralTypeDeclaration {
                id: StructuralTypeId::new(73).unwrap(),
                identity: "Bytes".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "Bytes".into(),
                        fields: Vec::new(),
                    }],
                },
            },
            bytes: b"payload".to_vec(),
            code_offset: 9,
            code_byte_count: 3,
            data_offset: 4,
            data_byte_count: 7,
        };
        let scalar_result =
            machine_code::BoundaryResultRecord::Scalar(machine_code::BoundaryScalarResultRecord {
                value: ValueId::new(67).unwrap(),
                scalar_type: u8_type,
                placement: calling_conventions::ValuePlacement {
                    shape: calling_conventions::ValueShape::integer(1, 1),
                    locations: vec![calling_conventions::ValueLocation::Register {
                        register: calling_conventions::MachineRegister::X86Rax,
                        value_byte_offset: 0,
                        byte_size: 1,
                    }],
                },
                return_edge: EdgeId::new(71).unwrap(),
            });
        let completion = machine_code::BoundarySettlementRecord {
            psi_operation: OperationId::new(41).unwrap(),
            boundary: BoundaryMachineId::new(43).unwrap(),
            execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(record),
            realization: super::BoundaryRealization::ClaimCompletionOnly(
                ClaimCompletionOnlyRealization,
            ),
            scalar_arguments: Vec::new(),
            runtime_scalar_arguments: Vec::new(),
            arguments: vec![argument.clone()],
            byte_sequence_arguments: vec![byte_sequence],
            completion_claim_sources: vec![source.clone()],
            completion_receipts: vec![receipt],
            completion_provider_custody: vec![machine_code::CompletionProviderCustodyBinding {
                source: source.clone(),
                receipt,
                provider_execution: record,
            }],
            native_result: machine_code::BoundaryResultRecord::Unit,
            operation_ordinal: 5,
            code_offset: 8,
            byte_count: 0,
        };
        let identity = |settlement: &machine_code::BoundarySettlementRecord,
                        port_effect: Option<&machine_code::PortEffectRecord>,
                        execution: super::ProviderExecutionBinding| {
            admitted_provider_settlement_identity(
                occurrence,
                "Extent::complete",
                NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                NativeTarget::linux_x64(),
                execution,
                settlement,
                port_effect,
            )
            .unwrap()
        };
        let expected = identity(&completion, None, execution);

        for mutation in 0..22 {
            let mut changed = completion.clone();
            match mutation {
                0 => changed.psi_operation = OperationId::new(53).unwrap(),
                1 => changed.boundary = BoundaryMachineId::new(59).unwrap(),
                2 => {
                    changed.execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                        machine_code::ProviderExecutionRecord::new(7, 11, 13, 17, 23).unwrap(),
                    )
                }
                3 => {
                    changed.execution = machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                        super::CompilerBuiltinExecution::HostedWriteByteI32,
                    )
                }
                4 => {
                    changed.realization =
                        super::BoundaryRealization::MetadataOnlyPort(MetadataOnlyPortRealization {
                            effect_operation: OperationId::new(40).unwrap(),
                            service: ServiceId::new(3).unwrap(),
                            port: 0x3f8,
                            value: 0x51,
                        })
                }
                5 => changed.scalar_arguments.push(BoundaryScalarArgument {
                    source_value: ValueId::new(31).unwrap(),
                    scalar_type: u8_type,
                    immediate: IntegerValue::Unsigned(7),
                    destination: calling_conventions::MachineRegister::X86Rdi,
                }),
                6 => changed
                    .runtime_scalar_arguments
                    .push(machine_code::ForeignCallScalarArgumentRecord {
                    parameter_index: 0,
                    source:
                        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                            source_value: ValueId::new(37).unwrap(),
                            scalar_type: ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                            ),
                            instruction: selected_instructions::SelectedInstructionId(41),
                        },
                    placement: calling_conventions::ValuePlacement {
                        shape: calling_conventions::ValueShape::integer(4, 4),
                        locations: vec![calling_conventions::ValueLocation::Register {
                            register: calling_conventions::MachineRegister::X86Rax,
                            value_byte_offset: 0,
                            byte_size: 4,
                        }],
                    },
                    code_offset: 2,
                    byte_count: 4,
                }),
                7 => changed.arguments[0].access = StructuralAccess::SharedBorrow,
                8 => changed.arguments.push(argument.clone()),
                9 => changed.byte_sequence_arguments[0].bytes = b"other".to_vec(),
                10 => changed.byte_sequence_arguments[0].data_byte_count += 1,
                11 => changed.completion_claim_sources[0].entry = None,
                12 => changed.completion_claim_sources.push(source.clone()),
                13 => changed.completion_receipts[0].argument_index = 1,
                14 => changed.completion_receipts.push(CompletionReceipt {
                    claim: ClaimId::new(61).unwrap(),
                    argument_index: 1,
                }),
                15 => {
                    changed.completion_provider_custody[0]
                        .provider_execution
                        .boundary_contract_report_fingerprint = 23
                }
                16 => {
                    changed.completion_provider_custody[0]
                        .receipt
                        .argument_index = 7
                }
                17 => changed.native_result = scalar_result.clone(),
                18 => changed.operation_ordinal += 1,
                19 => changed.code_offset += 1,
                20 => changed.byte_count += 1,
                _ => changed.completion_provider_custody.clear(),
            }
            assert_ne!(
                identity(&changed, None, execution),
                expected,
                "settlement mutation {mutation}"
            );
        }

        for mutation in 0..5 {
            let changed = match mutation {
                0 => admitted_provider_settlement_identity(
                    &optimized_boundary_occurrence(
                        occurrence.terminal(),
                        occurrence.machine(),
                        OperationId::new(9).unwrap(),
                        occurrence.boundary(),
                        occurrence.operation_ordinal(),
                        OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(
                            b"other occurrence",
                        ),
                    ),
                    "Extent::complete",
                    NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                    NativeTarget::linux_x64(),
                    execution,
                    &completion,
                    None,
                )
                .unwrap(),
                1 => admitted_provider_settlement_identity(
                    occurrence,
                    "Extent::other",
                    NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                    NativeTarget::linux_x64(),
                    execution,
                    &completion,
                    None,
                )
                .unwrap(),
                2 => admitted_provider_settlement_identity(
                    occurrence,
                    "Extent::complete",
                    NativeSelectedProviderPlanDigest::from_digest([8; 32]),
                    NativeTarget::linux_x64(),
                    execution,
                    &completion,
                    None,
                )
                .unwrap(),
                3 => admitted_provider_settlement_identity(
                    occurrence,
                    "Extent::complete",
                    NativeSelectedProviderPlanDigest::from_digest([7; 32]),
                    NativeTarget::windows_x64(),
                    execution,
                    &completion,
                    None,
                )
                .unwrap(),
                _ => identity(
                    &completion,
                    None,
                    super::ProviderExecutionBinding::from_execution_record(
                        super::ProviderPlanReportIdentity::new(7).unwrap(),
                        11,
                        13,
                        17,
                        29,
                    )
                    .unwrap(),
                ),
            };
            assert_ne!(changed, expected, "binding input mutation {mutation}");
        }

        let port_effect = machine_code::PortEffectRecord {
            psi_operation: OperationId::new(40).unwrap(),
            service: ServiceId::new(3).unwrap(),
            port: 0x3f8,
            value: 0x51,
            operation_ordinal: 4,
            code_offset: 6,
            byte_count: 2,
        };
        let port = machine_code::BoundarySettlementRecord {
            realization: super::BoundaryRealization::MetadataOnlyPort(
                MetadataOnlyPortRealization {
                    effect_operation: OperationId::new(40).unwrap(),
                    service: ServiceId::new(3).unwrap(),
                    port: 0x3f8,
                    value: 0x51,
                },
            ),
            scalar_arguments: Vec::new(),
            runtime_scalar_arguments: Vec::new(),
            arguments: Vec::new(),
            byte_sequence_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
            completion_provider_custody: Vec::new(),
            ..completion.clone()
        };
        let port_expected = identity(&port, Some(&port_effect), execution);
        assert_ne!(port_expected, identity(&port, None, execution));
        for mutation in 0..7 {
            let mut changed = port_effect.clone();
            match mutation {
                0 => changed.psi_operation = OperationId::new(43).unwrap(),
                1 => changed.service = ServiceId::new(5).unwrap(),
                2 => changed.port += 1,
                3 => changed.value += 1,
                4 => changed.operation_ordinal += 1,
                5 => changed.code_offset += 1,
                _ => changed.byte_count += 1,
            }
            assert_ne!(
                identity(&port, Some(&changed), execution),
                port_expected,
                "port-effect mutation {mutation}"
            );
        }
        for mutation in 0..4 {
            let mut changed = port.clone();
            let super::BoundaryRealization::MetadataOnlyPort(realization) =
                &mut changed.realization
            else {
                panic!("metadata-only port settlement")
            };
            match mutation {
                0 => realization.effect_operation = OperationId::new(43).unwrap(),
                1 => realization.service = ServiceId::new(5).unwrap(),
                2 => realization.port += 1,
                _ => realization.value += 1,
            }
            assert_ne!(
                identity(&changed, Some(&port_effect), execution),
                port_expected,
                "metadata-port realization mutation {mutation}"
            );
        }
    }
}
