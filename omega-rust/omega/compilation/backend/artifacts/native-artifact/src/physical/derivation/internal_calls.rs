//! Derive selected boundary internal-call evidence from shared call custody.
//!
//! A selected checked callee inside the same artifact emits an ordinary
//! resolved internal call: no object settlement row, no normalized foreign
//! call, and no external execution binding. The emitted call's retained
//! call-stack row names the owner operation and the callee machine, and the
//! canonical module names the selected provider or target-family member.
//! This lane binds that custody — the same object rosters `fragment_call`
//! follows for resolved calls — under the actual selection parent. Providers
//! retain their settlement plan; families retain the authentic portable head
//! and exact selected member without provider installation authority.

use crate::BoundaryInternalCallCustody;
use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::NativePhysicalChild;
use crate::NativePhysicalOccurrence;
use crate::NativeSelectedProviderPlan;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::TargetFamilyInternalCall;
use crate::physical::derivation::evidence::boundary_physical_child_parts;
use crate::physical::derivation::evidence::ranges_overlap;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::settlement_identity::{
    installed_provider_internal_call_identity, target_family_internal_call_identity,
};
use crate::physical::native_byte_span;
use crate::physical::operator_applications::fragment_call::validate_destination;
use object_file::SectionKind;
use optimization_core::NativeOptimizationProjectionIdentity;
use target::{Architecture, NativeTarget, TargetProfile};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

mod call_custody;

/// One retained call-stack row a boundary call's emitted internal call can
/// rejoin; the variants mirror the caller's record families exactly.
enum InternalCallRow<'artifact> {
    Unit(&'artifact image_emission::ObjectUnitCallStack),
    Scalar(&'artifact image_emission::ObjectScalarCallStack),
}

enum InternalCallSelection<'module, 'artifact> {
    Provider {
        conformance: &'module terminal_psi::ProviderCandidateConformance,
        plan: &'artifact NativeSelectedProviderPlan,
    },
    Family {
        family: &'module terminal_psi::TargetMachineFamily,
        member: &'module terminal_psi::TargetMachineFamilyMember,
        completion_claim_sources: &'artifact [target_operations::CompletionClaimSource],
        completion_receipts: &'artifact [terminal_psi::CompletionReceipt],
    },
}

/// Native shape checks compatibility; the retained tag chooses the exact
/// deployment row. Profiles sharing a shape never acquire a tag from the host.
fn checked_family_member<'module>(
    module: &'module terminal_psi::TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    machine: semantic_vocabulary::MachineId,
    tag: &str,
    target: NativeTarget,
) -> Result<
    (
        &'module terminal_psi::TargetMachineFamily,
        &'module terminal_psi::TargetMachineFamilyMember,
    ),
    &'static str,
> {
    let mut families = module.target_machine_families.iter().filter(|family| {
        family.canonical == terminal_psi::TargetMachineCallable::Boundary(boundary)
    });
    let family = families
        .next()
        .ok_or("target-family internal call has no canonical family")?;
    if families.next().is_some() {
        return Err("target-family internal call has duplicate canonical families");
    }
    if family.head_contract == [0; 32] {
        return Err("target-family internal call has no authentic head contract");
    }
    let mut members = family.members.iter().filter(|member| member.target == tag);
    let member = members
        .next()
        .ok_or("target-family internal call has no member for its exact tag")?;
    if members.next().is_some()
        || member.callable != terminal_psi::TargetMachineCallable::Machine(machine)
        || !matches!(
            member.supply,
            terminal_psi::TargetMachineSupply::CheckedBody
                | terminal_psi::TargetMachineSupply::Boundary
        )
        || !TargetProfile::from_canonical_target_name(tag)
            .is_ok_and(|profile| profile.native_realization() == Some(target))
    {
        return Err("target-family internal call changed its member, supply, or deployment target");
    }
    Ok((family, member))
}

impl InternalCallRow<'_> {
    fn owner(&self) -> CallSiteOwner {
        match self {
            Self::Unit(call) => call.owner,
            Self::Scalar(call) => call.owner,
        }
    }

    fn target(&self) -> semantic_vocabulary::MachineId {
        match self {
            Self::Unit(call) => call.target,
            Self::Scalar(call) => call.target,
        }
    }

    fn text_offset(&self) -> usize {
        match self {
            Self::Unit(call) => call.text_offset,
            Self::Scalar(call) => call.text_offset,
        }
    }
}

/// `true` when the occurrence's operation owns a retained internal call-stack
/// row — that is, the boundary call emitted an ordinary internal call.
/// `evidence.rs` uses the predicate to reject an occurrence that somehow
/// carries this realization beside an installed settlement or a normalized
/// foreign call.
pub(crate) fn boundary_internal_call_exists(
    object: &image_emission::ObjectArtifact,
    occurrence: &OptimizedBoundaryOccurrence,
) -> bool {
    let Some(operation) = occurrence.operation() else {
        return false;
    };
    object
        .function_on_machine(occurrence.machine())
        .is_some_and(|function| {
            function
                .unit_call_stacks
                .iter()
                .map(|call| call.owner)
                .chain(function.scalar_call_stacks.iter().map(|call| call.owner))
                .any(|owner| owner == CallSiteOwner::Operation(operation))
        })
}

/// Derive the D41 physical child for one selected boundary call
/// realized as an ordinary resolved internal call.
///
/// `Ok(None)` leaves custody this lane does not cover — a boundary call that
/// emitted no internal call — without a claimed
/// child; the occurrence then blocks as `UnrealizedBoundaryOccurrence`.
/// Custody drift that would contradict a validated object or its Terminal
/// operation is an error, matching the settlement derivations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_boundary_internal_call_child(
    module: &terminal_psi::TerminalModule,
    terminal: terminal_psi::TerminalPsiIdentity,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: Option<&NativeSelectedProviderPlan>,
    target: NativeTarget,
    fragment_publication: Option<
        &crate::physical::fragment_publication::FragmentPublicationBinding,
    >,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let source_operation = occurrence
        .operation()
        .ok_or("selected boundary internal call requires an operation invocation")?;
    // The emitted call must still rejoin exactly one Terminal boundary call
    // on the occurrence's machine. This lane binds the plain internal-call
    // custody. Image publication already joined its complete operand and
    // result transport; structural operands do not make the call foreign.
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| machine.operations())
        .filter(|operation| operation.id == source_operation)
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("selected boundary internal call does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        return Err("selected boundary internal call owner is not a Terminal boundary call");
    };
    if *boundary != occurrence.boundary() {
        return Err("selected boundary internal call changed its boundary");
    }
    let function = object
        .function_on_machine(occurrence.machine())
        .ok_or("selected boundary internal call names an absent object function")?;
    if !function.provenance.operations.contains(&source_operation) {
        return Err("selected boundary internal call left its function provenance");
    }
    let calls = function
        .unit_call_stacks
        .iter()
        .map(InternalCallRow::Unit)
        .chain(
            function
                .scalar_call_stacks
                .iter()
                .map(InternalCallRow::Scalar),
        )
        .filter(|call| call.owner() == CallSiteOwner::Operation(source_operation))
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        return if calls.is_empty() {
            Ok(None)
        } else {
            Err("selected boundary internal call rejoins duplicate call custody")
        };
    };
    let record = call_custody::rejoin(
        fragment_publication.ok_or("selected boundary internal call lacks fragment custody")?,
        occurrence,
        call.target(),
        call.text_offset(),
    )?;
    if record.claim_transfers
        != completion_receipts
            .iter()
            .map(|receipt| terminal_psi::ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect::<Vec<_>>()
    {
        return Err("selected boundary internal call changed its completion transfers");
    }
    let selection = if let Some(plan) = selected_plan {
        // A provider and a target family carry distinct selection custody.
        // The shared physical call transport grants neither kind of authority.
        // Code sharing is not choosing-declaration equality. Rejoin the exact
        // installed origin retained by ordinary call lowering, not a search
        // for whichever catalog conformance happens to name this body.
        let legalized_operations::NativeCallOrigin::InstalledProvider {
            boundary: selected_boundary,
            provider,
            assumption,
            completion_receipts: retained_receipts,
            ..
        } = &record.source
        else {
            return Err("installed provider internal call lost its exact selected origin");
        };
        if assumption
            .as_ref()
            .is_some_and(|row| !plan.matches_assumption(module, requirement_identity, row))
            || selected_boundary != boundary
            || assumption
                .as_ref()
                .map_or(provider.boundary != *boundary, |row| {
                    !row.matches(*boundary, provider)
                })
            || provider.candidate != call.target()
            || retained_receipts != completion_receipts
        {
            return Err(
                "installed provider internal call changed its selection or completion custody",
            );
        }
        let conformances = module
            .provider_candidates
            .iter()
            .filter(|conformance| *conformance == provider)
            .collect::<Vec<_>>();
        let [conformance] = conformances.as_slice() else {
            return Err("installed provider internal call cannot rejoin one provider conformance");
        };
        if module.identities.text(conformance.requirement_identity)
            != Some(plan.selected_requirement_identity(requirement_identity))
        {
            return Err("installed provider internal call changed its requirement");
        }
        InternalCallSelection::Provider { conformance, plan }
    } else {
        let legalized_operations::NativeCallOrigin::TargetFamily {
            boundary: selected_boundary,
            member: selected_member,
            target: tag,
            completion_claim_sources,
            completion_receipts: retained_receipts,
        } = &record.source
        else {
            return Err("target-family internal call lost its selected-family origin");
        };
        if selected_boundary != boundary
            || *selected_member != call.target()
            || retained_receipts != completion_receipts
        {
            return Err("target-family internal call changed its head or completion custody");
        }
        let (family, member) =
            checked_family_member(module, *boundary, *selected_member, tag, target)?;
        InternalCallSelection::Family {
            family,
            member,
            completion_claim_sources,
            completion_receipts: retained_receipts,
        }
    };
    let callee = object
        .functions()
        .iter()
        .find(|candidate| candidate.machine == call.target())
        .ok_or("selected boundary internal call names an absent object callee")?;
    let call_field_offset = call.text_offset();
    let opcode = match target.architecture {
        Architecture::X86_64 => call_field_offset.checked_sub(1),
        Architecture::Aarch64 => Some(call_field_offset),
    }
    .ok_or("selected boundary internal call opcode offset underflow")?;
    let end = call_field_offset
        .checked_add(4)
        .ok_or("selected boundary internal call extent overflow")?;
    let bytes = object
        .text_bytes()
        .get(opcode..end)
        .ok_or("selected boundary internal call is outside object text")?;
    validate_destination(target.architecture, opcode, bytes, callee.text_offset)?;
    // The operation's complete emitted interval is its exact semantic-code
    // attribution row; the resolved call must sit inside it, and an internal
    // call carries no relocation of its own.
    let mut attribution = object.semantic_code_attribution().iter().filter(|row| {
        row.machine == function.machine
            && row.attribution.site == machine_code::SemanticCodeSite::Operation(source_operation)
            && row.attribution.operation_ordinal == occurrence.operation_ordinal()
            && row.text_offset <= opcode
            && row
                .text_offset
                .checked_add(row.attribution.byte_count)
                .is_some_and(|limit| end <= limit)
    });
    let interval = attribution
        .next()
        .ok_or("selected boundary internal call lacks its exact semantic interval")?;
    if attribution.next().is_some()
        || function
            .text_offset
            .checked_add(interval.attribution.code_offset)
            != Some(interval.text_offset)
        || object.relocations().records().any(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && ranges_overlap(
                    opcode,
                    end,
                    relocation.offset,
                    relocation.offset.saturating_add(relocation.byte_width),
                )
        })
    {
        return Err("selected boundary internal call has ambiguous attribution or a relocation");
    }
    let machine_span = native_byte_span(
        interval.attribution.code_offset,
        interval.attribution.byte_count,
    );
    let object_span = native_byte_span(interval.text_offset, interval.attribution.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("selected boundary internal call bytes changed across physical custody");
    }

    let custody = match call {
        InternalCallRow::Unit(call) => BoundaryInternalCallCustody::Unit {
            owner: call.owner,
            target: call.target,
            call_field_offset: call.text_offset,
            active_frame_bytes: call.active_frame_bytes,
            transient_bytes: call.transient_bytes,
            caller_live_bytes: call.caller_live_bytes,
        },
        InternalCallRow::Scalar(call) => BoundaryInternalCallCustody::Scalar {
            owner: call.owner,
            target: call.target,
            call_field_offset: call.text_offset,
            caller_live_bytes: call.caller_live_bytes,
        },
    };
    let parent = match selection {
        InternalCallSelection::Provider { conformance, plan } => {
            let provider_declaration = module
                .identities
                .text(conformance.provider_declaration.identity())
                .ok_or("installed provider physical identity lost its choosing declaration")?;
            let parent_identity = installed_provider_internal_call_identity(
                occurrence,
                requirement_identity,
                plan.plan_digest(),
                target,
                *boundary,
                provider_declaration,
                module
                    .identities
                    .text(conformance.provider_identity)
                    .unwrap_or_default(),
                module
                    .identities
                    .text(conformance.candidate_identity)
                    .unwrap_or_default(),
                &custody,
                plan.assumption_closure(),
            );
            let role = BoundaryTraitSettlementRole::InstalledProviderInternalCall {
                boundary: *boundary,
                provider_declaration: provider_declaration.to_owned(),
                provider_identity: module
                    .identities
                    .text(conformance.provider_identity)
                    .unwrap_or_default()
                    .to_string(),
                candidate_identity: module
                    .identities
                    .text(conformance.candidate_identity)
                    .unwrap_or_default()
                    .to_string(),
                call: custody,
            };
            PhysicalChildParent::BoundaryTraitSettlement(
                BoundaryTraitSettlementParts {
                    occurrence: *occurrence,
                    requirement_identity: requirement_identity.to_owned(),
                    selected_plan_digest: plan.plan_digest(),
                    target,
                    role,
                    identity: parent_identity,
                }
                .into(),
            )
        }
        InternalCallSelection::Family {
            family,
            member,
            completion_claim_sources,
            completion_receipts,
        } => {
            let identity = target_family_internal_call_identity(
                terminal,
                occurrence,
                requirement_identity,
                &family.head_contract,
                member,
                target,
                completion_claim_sources,
                completion_receipts,
                &custody,
            )?;
            PhysicalChildParent::TargetFamilyInternalCall(TargetFamilyInternalCall {
                terminal,
                occurrence: *occurrence,
                head_identity: requirement_identity.to_owned(),
                head_contract: family.head_contract,
                member: member.clone(),
                target,
                completion_claim_sources: completion_claim_sources.to_vec(),
                completion_receipts: completion_receipts.to_vec(),
                call: custody,
                identity,
            })
        }
    };
    Ok(Some(
        boundary_physical_child_parts(
            parent,
            projection,
            NativePhysicalOccurrence::Boundary(occurrence.identity()),
            machine_span,
            object_span,
            final_image_span,
            machine_bytes,
            object_bytes,
            final_image_bytes,
            PhysicalRelocationDisposition::ResolvedInternalCall,
        )
        .into(),
    ))
}
