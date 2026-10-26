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
use crate::physical::derivation::foreign_calls::derive_normalized_foreign_child;
use crate::physical::derivation::hashing::{
    canonical_usize, hash_callback_relocation, hash_machine_function_identity, hash_object_symbol,
    hash_relocation_origin, physical_evidence_gap_identity, sha256,
};
use crate::physical::derivation::hosted_builtins::{
    derive_hosted_builtin_child, hosted_builtin_settlement,
};
use crate::physical::derivation::internal_calls::{
    boundary_internal_call_exists, derive_boundary_internal_call_child,
};
use crate::physical::derivation::provider_settlements::derive_admitted_provider_settlement_child;
use crate::physical::native_optimization_projection;
use crate::physical::native_physical_evidence;
use crate::physical::native_physical_evidence_gap;
use crate::physical::operator_applications::derive_operator_physical_span;
use crate::physical::optimized_boundary_occurrence;
use crate::physical::optimized_operator_occurrence;
use crate::physical::{NativePhysicalEvidenceGap, NativePhysicalEvidenceGapSubject};
use crate::{NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use object_file::relocation_kind_tag;
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::CallSiteOwner;
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
    let module = terminal_artifact.semantic_module();
    let boundary_application_coverage = boundary_application_coverage
        .ok_or("native physical evidence requires exact boundary-application coverage custody")?;

    let projection = match scope {
        NativePhysicalEvidenceScope::Unavailable => unreachable!("unavailable returned above"),
        NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence => {
            derive_identity_projection(
                terminal_artifact.manifest().semantic(),
                module,
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
            installed.settlement.invocation_site,
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
                occurrence.invocation_site(),
                occurrence.boundary(),
                occurrence.operation_ordinal(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    if settlements.keys().any(|key| !occurrence_keys.contains(key)) {
        return Err("native physical evidence found a stale boundary settlement");
    }

    // Fragment publication independently reconstructs the object's foreign
    // calls. This is evidence for the same roster, not additional calls.
    let fragment_publication = match scope {
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) => {
            optimized.fragment_publication()
        }
        _ => None,
    };
    let retained_custody = match scope {
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) => {
            optimized.foreign_call_custody()
        }
        _ => &[],
    };
    let mut foreign_calls = BTreeMap::new();
    if fragment_publication.is_some() && retained_custody != object.foreign_calls() {
        return Err("native physical evidence foreign calls differ from fragment projection");
    }
    for foreign in object.foreign_calls() {
        let invocation_site = match foreign.owner {
            CallSiteOwner::Operation(operation) => {
                semantic_vocabulary::InvocationSite::Operation(operation)
            }
            CallSiteOwner::ExternalCompletion(edge) => {
                semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
            }
            CallSiteOwner::CleanupAction { .. } => {
                return Ok(blocked(
                    NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner {
                        machine: foreign.machine,
                        owner: foreign.owner,
                    },
                ));
            }
        };
        let key = (foreign.machine, invocation_site);
        if foreign_calls.insert(key, foreign).is_some() {
            return Err("native physical evidence found duplicate normalized foreign calls");
        }
    }
    let foreign_occurrence_keys = projection
        .boundary_occurrences()
        .iter()
        .map(|occurrence| (occurrence.machine(), occurrence.invocation_site()))
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
        .map(|boundary| {
            (
                boundary.id,
                module
                    .identities
                    .text(boundary.identity)
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut children = Vec::new();
    let mut consumed_port_effects = BTreeSet::new();
    for occurrence in projection.boundary_occurrences() {
        let key = (
            occurrence.machine(),
            occurrence.invocation_site(),
            occurrence.boundary(),
            occurrence.operation_ordinal(),
        );
        let installed = settlements.get(&key);
        let foreign = foreign_calls.get(&(occurrence.machine(), occurrence.invocation_site()));
        let internal_call = boundary_internal_call_exists(object, occurrence);
        if [installed.is_some(), foreign.is_some(), internal_call]
            .into_iter()
            .filter(|present| *present)
            .count()
            > 1
        {
            return Err("native physical evidence found two realizations for one boundary call");
        }
        let requirement = boundary_identities
            .get(&occurrence.boundary())
            .copied()
            .ok_or("native physical evidence names an absent boundary requirement")?;
        // A family is selected by its exact deployment row, not by a provider
        // plan. Its unchanged boundary occurrence still owns this emitted call.
        let family = module.target_machine_families.iter().any(|family| {
            family.canonical == terminal_psi::TargetMachineCallable::Boundary(occurrence.boundary())
                && !family
                    .members
                    .iter()
                    .any(|member| member.callable == family.canonical)
        });
        if family && installed.is_none() && foreign.is_none() {
            if let Some(child) = derive_boundary_internal_call_child(
                module,
                terminal_artifact.manifest().semantic(),
                object,
                image,
                occurrence,
                projection.identity(),
                requirement,
                None,
                target,
                fragment_publication,
            )? {
                children.push(child);
                continue;
            }
            return Ok(blocked(
                NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence {
                    occurrence: *occurrence,
                },
            ));
        }
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
        super::provider_custody::validate_family_provider_custody(
            terminal_artifact.manifest().semantic(),
            module,
            target,
            occurrence.boundary(),
            requirement,
            selected_plan,
        )?;
        match (installed, foreign) {
            (Some(installed), None) => {
                if let Some(builtin) = hosted_builtin_settlement(&installed.settlement) {
                    children.push(derive_hosted_builtin_child(
                        builtin,
                        occurrence,
                        projection.identity(),
                        requirement,
                        selected_plan.plan_digest(),
                        target,
                        object,
                        image,
                        installed,
                    )?);
                } else if let Some(child) = derive_admitted_provider_settlement_child(
                    module,
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
                )? {
                    children.push(child);
                } else {
                    return Ok(blocked(
                        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization {
                            occurrence: *occurrence,
                        },
                    ));
                }
            }
            (None, Some(foreign)) => {
                let Some(child) = derive_normalized_foreign_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan,
                    provider_executions,
                    target,
                    module,
                    object,
                    image,
                    final_image_symbol_digest,
                    foreign,
                    fragment_publication,
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
            (None, None) => {
                // An installed provider realized inside this same artifact
                // retains neither a settlement nor a normalized foreign call:
                // it emitted an ordinary resolved internal call, and that
                // retained custody binds the occurrence here.
                if let Some(child) = derive_boundary_internal_call_child(
                    module,
                    terminal_artifact.manifest().semantic(),
                    object,
                    image,
                    occurrence,
                    projection.identity(),
                    requirement,
                    Some(selected_plan),
                    target,
                    fragment_publication,
                )? {
                    children.push(child);
                } else {
                    return Ok(blocked(
                        NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence {
                            occurrence: *occurrence,
                        },
                    ));
                }
            }
            (Some(_), Some(_)) => {
                unreachable!("dual boundary realizations rejected above")
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
            module,
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
            &[span.machine],
            &[span.object],
            &[span.final_image],
            span.machine_bytes_digest,
            span.object_bytes_digest,
            span.final_image_bytes_digest,
            &span.relocation,
        );
        children.push(
            NativePhysicalChildParts {
                parent,
                projection: projection.identity(),
                occurrence: physical_occurrence,
                machine_spans: vec![span.machine],
                object_spans: vec![span.object],
                final_image_spans: vec![span.final_image],
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
    for child in children {
        validate_physical_child_spans(child)?;
        if !child
            .parent()
            .owns_occurrence(projection.terminal(), child.occurrence())
        {
            return Err("native physical child is detached from its semantic parent occurrence");
        }
    }
    validate_exact_physical_child_coordinates(
        projection,
        children.iter().map(|child| PhysicalChildCoordinate {
            projection: child.projection(),
            occurrence: child.occurrence(),
            parent_role: child.parent().occurrence_role_tag(),
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
            if let terminal_psi::Terminator::ExternalCompletion { edge, boundary, .. } =
                &block.terminator
            {
                let site = semantic_vocabulary::InvocationSite::ExternalCompletion(*edge);
                let identity = boundary_occurrence_identity(
                    terminal,
                    machine.id,
                    site,
                    *boundary,
                    operation_ordinal,
                );
                boundary_occurrences.push(optimized_boundary_occurrence(
                    terminal,
                    machine.id,
                    site,
                    *boundary,
                    operation_ordinal,
                    identity,
                ));
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
    crate::physical::identity_bytes::append_operator_occurrence_identity(
        &mut canonical,
        machine,
        operation,
        operation_ordinal,
    );
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

pub(crate) fn boundary_occurrence_identity(
    terminal: terminal_psi::TerminalPsiIdentity,
    machine: semantic_vocabulary::MachineId,
    site: impl Into<semantic_vocabulary::InvocationSite>,
    boundary: semantic_vocabulary::BoundaryMachineId,
    operation_ordinal: usize,
) -> OptimizedBoundaryOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    crate::physical::identity_bytes::append_boundary_occurrence_identity(
        &mut canonical,
        machine,
        site,
        boundary,
        operation_ordinal,
    );
    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(&canonical)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn physical_child_identity(
    parent: &PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    machine_spans: &[NativeByteSpan],
    object_spans: &[NativeByteSpan],
    final_image_spans: &[NativeByteSpan],
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: &PhysicalRelocationDisposition,
) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.native-physical-child.sha256.v3\0");
    digest.byte(parent.role_tag());
    digest.raw(&parent.identity());
    digest.raw(&projection.bytes());
    digest.byte(occurrence.role_tag());
    digest.raw(&occurrence.identity());
    for spans in [machine_spans, object_spans, final_image_spans] {
        digest.raw(&canonical_usize(spans.len()));
        for span in spans {
            digest.raw(&canonical_usize(span.offset()));
            digest.raw(&canonical_usize(span.byte_count()));
        }
    }
    digest.raw(&machine_bytes_digest);
    digest.raw(&object_bytes_digest);
    digest.raw(&final_image_bytes_digest);
    digest.byte(match *relocation {
        PhysicalRelocationDisposition::DirectInstructionBytes => 1,
        PhysicalRelocationDisposition::ResolvedInternalCall => 2,
        PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(_) => 3,
        PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(_) => 4,
    });
    if let PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(relocation) = relocation {
        digest.raw(relocation.locator_identity());
        digest.raw(relocation.boundary_plan_identity());
        hash_object_symbol(&mut digest, relocation.object_symbol());
        hash_relocation_origin(&mut digest, relocation.origin());
        digest.raw(&canonical_usize(relocation.offset()));
        digest.raw(&canonical_usize(relocation.byte_width()));
        digest.raw(&relocation.addend().to_le_bytes());
        digest.byte(relocation_kind_tag(relocation.kind()));
        digest.raw(&canonical_usize(relocation.callbacks().len()));
        for callback in relocation.callbacks() {
            match callback {
                NormalizedForeignCallbackRelocations::X86_64Relative32 {
                    callback_function,
                    relocation,
                } => {
                    digest.byte(1);
                    hash_machine_function_identity(&mut digest, *callback_function);
                    hash_callback_relocation(&mut digest, *relocation);
                }
                NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                    callback_function,
                    page,
                    page_offset,
                } => {
                    digest.byte(2);
                    hash_machine_function_identity(&mut digest, *callback_function);
                    hash_callback_relocation(&mut digest, *page);
                    hash_callback_relocation(&mut digest, *page_offset);
                }
            }
        }
        digest.raw(relocation.final_image_symbol_identity());
    }
    if let PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(field) =
        relocation
    {
        digest.raw(field.locator_identity());
        digest.raw(field.boundary_plan_identity());
        digest.raw(&field.caller().get().to_le_bytes());
        match field.invocation_site() {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                digest.byte(0);
                digest.raw(&operation.get().to_le_bytes());
            }
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                digest.byte(1);
                digest.raw(&edge.get().to_le_bytes());
            }
        }
        digest.raw(&field.boundary().get().to_le_bytes());
        digest.raw(&field.ordinal().to_le_bytes());
        digest.raw(&field.object_symbol().get().to_le_bytes());
        digest.raw(&canonical_usize(field.offset()));
        digest.raw(&canonical_usize(field.byte_width()));
        digest.raw(&field.addend().to_le_bytes());
        digest.byte(relocation_kind_tag(field.kind()));
        digest.raw(field.final_image_symbol_identity());
    }
    digest.finish()
}

fn physical_evidence_identity(
    projection: NativeOptimizationProjectionIdentity,
    children: &[NativePhysicalChild],
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-physical-evidence.sha256.v2\0",
    );
    digest.raw(&projection.bytes());
    digest.raw(&canonical_usize(children.len()));
    for child in children {
        digest.raw(child.identity());
    }
    digest.finish()
}

fn terminal_identity_bytes(terminal: terminal_psi::TerminalPsiIdentity) -> Vec<u8> {
    crate::physical::identity_bytes::terminal_identity_bytes(terminal)
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

/// Each child retains a canonical covered union in each coordinate domain.
/// Its intervals have equal lengths across custody boundaries; no hull is
/// permitted to claim compiler-only bytes between authored intervals.
fn validate_physical_child_spans(child: &NativePhysicalChild) -> Result<(), &'static str> {
    let domains = [
        child.machine_spans(),
        child.object_spans(),
        child.final_image_spans(),
    ];
    if domains[0].is_empty() || domains.iter().any(|spans| spans.len() != domains[0].len()) {
        return Err("native physical child has inconsistent covered span inventories");
    }
    for spans in domains {
        let mut previous_end = None;
        for span in spans {
            let end = span
                .offset()
                .checked_add(span.byte_count())
                .ok_or("native physical child span end overflow")?;
            if span.byte_count() == 0
                || previous_end.is_some_and(|previous| previous >= span.offset())
            {
                return Err(
                    "native physical child spans are empty, unordered, overlapping or nonmaximal",
                );
            }
            previous_end = Some(end);
        }
    }
    if domains[0]
        .iter()
        .zip(domains[1])
        .zip(domains[2])
        .any(|((machine, object), image)| {
            machine.byte_count() != object.byte_count()
                || machine.byte_count() != image.byte_count()
        })
    {
        return Err("native physical child span lengths differ across custody domains");
    }
    Ok(())
}

/// The shared boundary-child tail both provider derivations close with:
/// per-layer byte digests, the child identity, and the parts record. The
/// call sites differ only in the relocation disposition they mint.
#[expect(clippy::too_many_arguments)]
pub(crate) fn boundary_physical_child_parts(
    parent: PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
    machine_bytes: &[u8],
    object_bytes: &[u8],
    final_image_bytes: &[u8],
    relocation: PhysicalRelocationDisposition,
) -> NativePhysicalChildParts {
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let identity = physical_child_identity(
        &parent,
        projection,
        occurrence,
        &[machine_span],
        &[object_span],
        &[final_image_span],
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        &relocation,
    );
    NativePhysicalChildParts {
        parent,
        projection,
        occurrence,
        machine_spans: vec![machine_span],
        object_spans: vec![object_span],
        final_image_spans: vec![final_image_span],
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
        identity,
    }
}
