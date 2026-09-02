use std::collections::{BTreeMap, BTreeSet};

use omega_boundary_applications::TerminalBoundaryApplicationCoverage;
use omega_machine_code::BoundaryExecutionRecord;
use omega_object_file::SectionKind;
use omega_optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution,
};
use psi_core::{IntegerSign, IntegerType, ScalarType};
use psi_terminal::OperationKind;
use sha2::{Digest, Sha256};

use super::{model::*, operator_applications::derive_operator_physical_span};
use crate::{
    NativePhysicalEvidenceScope, NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
    boundary_applications::boundary_application_coverage_identity,
};

pub(crate) fn derive_physical_evidence(
    scope: &NativePhysicalEvidenceScope,
    terminal_artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
    selected_provider_plans: &[NativeSelectedProviderPlan],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Result<Option<NativePhysicalEvidence>, &'static str> {
    if matches!(scope, NativePhysicalEvidenceScope::Unavailable) {
        return Ok(None);
    }
    let module = psi_terminal_codec::decode_module(terminal_artifact.semantic_bytes())
        .map_err(|_| "native physical evidence cannot decode Terminal semantics")?;
    if module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
        || !object.port_effects().is_empty()
        || !object.object().layout.normalized_imports.is_empty()
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

    let boundary_identities = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut children = Vec::new();
    for occurrence in projection.boundary_occurrences() {
        let key = (
            occurrence.machine(),
            occurrence.operation(),
            occurrence.boundary(),
            occurrence.operation_ordinal(),
        );
        let Some(installed) = settlements.get(&key) else {
            // A valid artifact may realize this Terminal occurrence through
            // a normalized foreign/callback role outside the first D32 lane.
            return Ok(None);
        };
        if !matches!(
            (
                installed.settlement.execution,
                &installed.settlement.realization,
            ),
            (
                BoundaryExecutionRecord::CompilerBuiltin(
                    CompilerBuiltinExecution::LinuxExitGroupI32
                ),
                BoundaryRealization::LinuxExitGroupI32(_),
            )
        ) {
            // The artifact remains usable, but this first D32 lane must not
            // claim complete physical coverage for roles it cannot replay.
            return Ok(None);
        }
        if installed.settlement.byte_count == 0 {
            return Err("Linux exit-group physical child requires a nonempty emitted span");
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
    terminal: psi_terminal::TerminalPsiIdentity,
    module: &psi_terminal::TerminalModule,
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
    terminal: psi_terminal::TerminalPsiIdentity,
    machine: psi_core::MachineId,
    operation: psi_core::OperationId,
    operation_ordinal: usize,
) -> OptimizedOperatorOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn boundary_occurrence_identity(
    terminal: psi_terminal::TerminalPsiIdentity,
    machine: psi_core::MachineId,
    operation: psi_core::OperationId,
    boundary: psi_core::BoundaryMachineId,
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
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
    installed: &omega_image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let [scalar_argument] = settlement.scalar_arguments.as_slice() else {
        return Err("Linux exit-group physical child requires one scalar argument");
    };
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    let expected_destination = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_target_operations::MachineRegister::X86Rdi
        }
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_target_operations::MachineRegister::Aarch64X(0)
        }
        _ => return Err("Linux exit-group physical child requires a Linux ELF target"),
    };
    if scalar_argument.scalar_type != ScalarType::Integer(i32_type)
        || !matches!(scalar_argument.immediate, psi_core::IntegerValue::Signed(value) if i32::try_from(value).is_ok())
        || scalar_argument.destination != expected_destination
        || !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || settlement.native_result.is_some()
    {
        return Err("Linux exit-group D41 settlement custody is incomplete or substituted");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("Linux exit-group physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("Linux exit-group physical child object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("Linux exit-group physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.output().final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("Linux exit-group physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("Linux exit-group physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("Linux exit-group physical child unexpectedly contains a relocation");
    }

    let catalog = NativeCompilerBuiltinCatalogIdentity::LinuxElfV1;
    let execution =
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::LinuxExitGroupI32);
    let realization = BoundaryRealization::LinuxExitGroupI32(Default::default());
    let parent_identity = boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        catalog,
        scalar_argument,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
            target,
            catalog,
            execution,
            realization,
            scalar_argument: *scalar_argument,
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

fn boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    catalog: NativeCompilerBuiltinCatalogIdentity,
    scalar_argument: &omega_target_operations::BoundaryScalarArgument,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([match catalog {
        NativeCompilerBuiltinCatalogIdentity::LinuxElfV1 => 1,
    }]);
    digest.update([1, 1]); // CompilerBuiltin::LinuxExitGroupI32 + realization.
    digest.update(scalar_argument.source_value.get().to_le_bytes());
    digest.update([1]); // exact signed i32 scalar schema
    let psi_core::IntegerValue::Signed(value) = scalar_argument.immediate else {
        unreachable!("D41 settlement shape was checked")
    };
    digest.update(i32::try_from(value).expect("checked i32").to_le_bytes());
    digest.finalize().into()
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
    }]);
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

fn terminal_identity_bytes(terminal: psi_terminal::TerminalPsiIdentity) -> Vec<u8> {
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
    use super::*;
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    fn physical_projection() -> NativeOptimizationProjection {
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let operator = optimized_operator_occurrence(
            terminal,
            machine,
            psi_core::OperationId::new(2).expect("operator"),
            0,
            OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"operator survivor"),
        );
        let boundary = optimized_boundary_occurrence(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("boundary operation"),
            psi_core::BoundaryMachineId::new(4).expect("boundary"),
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
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let boundary = psi_core::BoundaryMachineId::new(2).expect("boundary");
        let first = boundary_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("first operation"),
            boundary,
            0,
        );
        let second = boundary_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(4).expect("second operation"),
            boundary,
            1,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn operator_occurrence_identity_binds_exact_terminal_operation() {
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let first = operator_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("first operation"),
            0,
        );
        let second = operator_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(4).expect("second operation"),
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
}
