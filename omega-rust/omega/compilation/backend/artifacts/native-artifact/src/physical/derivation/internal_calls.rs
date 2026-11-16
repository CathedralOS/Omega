/*
We derive physical evidence for selected boundary calls realized as ordinary
resolved internal calls, rejoining their call-stack custody and fragment
call-graph origins to an installed provider settlement plan.
*/

use crate::BoundaryInternalCallCustody;
use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::NativePhysicalChild;
use crate::NativePhysicalOccurrence;
use crate::NativeSelectedProviderPlan;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::boundary_physical_child_parts;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::evidence::text_relocation_overlaps;
use crate::physical::derivation::settlement_identity::installed_provider_internal_call_identity;
use crate::physical::fragment_publication::FragmentPublicationBinding;
use crate::physical::native_byte_span;
use crate::physical::operator_applications::fragment_call::validate_destination;
use machine_code::SemanticCodeSite;
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::{InvocationSite, MachineId};
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

enum InternalCallRow<'artifact> {
    Unit(&'artifact image_emission::ObjectUnitCallStack),
    Scalar(&'artifact image_emission::ObjectScalarCallStack),
}

fn rejoin_call_custody<'publication>(
    publication: &'publication FragmentPublicationBinding,
    occurrence: &OptimizedBoundaryOccurrence,
    callee: MachineId,
    field_offset: usize,
) -> Result<&'publication legalized_operations::LegalizedScalarCall, &'static str> {
    let owner = CallSiteOwner::from(occurrence.invocation_site());
    let mut functions = publication
        .physical_program()
        .functions
        .iter()
        .filter(|function| function.machine == occurrence.machine());
    let (Some(function), None) = (functions.next(), functions.next()) else {
        return Err("selected boundary internal call lacks one selected caller");
    };
    let mut records = function.calls.iter().filter(|record| record.owner == owner);
    let (Some(record), None) = (records.next(), records.next()) else {
        return Err("selected boundary internal call has no unique retained origin");
    };
    let (block, instruction) =
        image_emission::unique_block_instruction(function, record.instruction)
            .ok_or("selected boundary internal call lacks its selected instruction")?;
    let retains_owner = match occurrence.invocation_site() {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            instruction.provenance.operations.contains(&operation)
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
        | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
            instruction.provenance.edges.contains(&edge)
        }
        InvocationSite::Cleanup { .. } => {
            return Err("owned module cleanup cannot carry selected boundary call custody");
        }
    };
    if record.call.target.machine() != Some(callee)
        || instruction.kind.machine_callee() != Some(callee)
        || !retains_owner
    {
        return Err("selected boundary internal call changed its selected owner or callee");
    }
    let mut resolutions = publication
        .text_section()
        .resolved_internal_machine_calls
        .iter()
        .filter(|resolution| {
            resolution.caller == occurrence.machine() && resolution.owner == owner
        });
    let (Some(resolution), None) = (resolutions.next(), resolutions.next()) else {
        return Err("selected boundary internal call lacks one placed resolution");
    };
    if resolution.instruction != record.instruction
        || resolution.block != block.id
        || resolution.callee != callee
        || usize::try_from(resolution.field_section_offset) != Ok(field_offset)
    {
        return Err("selected boundary internal call changed its placed occurrence or callee");
    }
    Ok(&record.call)
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

pub(crate) fn boundary_internal_call_exists(
    object: &image_emission::ObjectArtifact,
    occurrence: &OptimizedBoundaryOccurrence,
) -> bool {
    if matches!(occurrence.invocation_site(), InvocationSite::Cleanup { .. }) {
        return false;
    }
    let invocation_owner = CallSiteOwner::from(occurrence.invocation_site());
    object
        .function_on_machine(occurrence.machine())
        .is_some_and(|function| {
            function
                .unit_call_stacks
                .iter()
                .map(|call| call.owner)
                .chain(function.scalar_call_stacks.iter().map(|call| call.owner))
                .any(|owner| owner == invocation_owner)
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_boundary_internal_call_child(
    pass: crate::physical::PhysicalEvidencePass,
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    plan: &NativeSelectedProviderPlan,
    target: NativeTarget,
    fragment_publication: Option<&FragmentPublicationBinding>,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let mut machines = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine());
    let (Some(machine), None) = (machines.next(), machines.next()) else {
        return Err("selected boundary internal call does not rejoin one Terminal machine");
    };
    let (boundary, completion_receipts, semantic_site) = match occurrence.invocation_site() {
        InvocationSite::InvokeNoNormal(_) => {
            return Err(
                "ordinary no-normal invocation cannot acquire boundary selection authority",
            );
        }
        InvocationSite::Cleanup { .. } => {
            return Err("owned module cleanup cannot acquire boundary selection authority");
        }
        InvocationSite::Operation(source_operation) => {
            let mut operations = machine
                .operations()
                .filter(|operation| operation.id == source_operation);
            let (Some(operation), None) = (operations.next(), operations.next()) else {
                return Err(
                    "selected boundary internal call does not rejoin one Terminal operation",
                );
            };
            let OperationKind::BoundaryCall {
                boundary,
                completion_receipts,
                ..
            } = &operation.kind
            else {
                return Err(
                    "selected boundary internal call owner is not a Terminal boundary call",
                );
            };
            (
                boundary,
                completion_receipts.as_slice(),
                SemanticCodeSite::Operation(source_operation),
            )
        }
        InvocationSite::ExternalCompletion(source_edge) => {
            let mut completions =
                machine
                    .blocks
                    .iter()
                    .filter_map(|block| match &block.terminator {
                        terminal_psi::Terminator::ExternalCompletion { edge, boundary, .. }
                            if *edge == source_edge =>
                        {
                            Some(boundary)
                        }
                        _ => None,
                    });
            let (Some(boundary), None) = (completions.next(), completions.next()) else {
                return Err(
                    "selected boundary internal call does not rejoin one Terminal completion edge",
                );
            };
            (boundary, &[][..], SemanticCodeSite::Edge(source_edge))
        }
    };
    if *boundary != occurrence.boundary() {
        return Err("selected boundary internal call changed its boundary");
    }
    let function = object
        .function_on_machine(occurrence.machine())
        .ok_or("selected boundary internal call names an absent object function")?;
    let retains_owner = match occurrence.invocation_site() {
        InvocationSite::Operation(operation) => function.provenance.operations.contains(&operation),
        InvocationSite::ExternalCompletion(edge) | InvocationSite::InvokeNoNormal(edge) => {
            function.provenance.edges.contains(&edge)
        }
        InvocationSite::Cleanup { .. } => false,
    };
    if !retains_owner {
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
        .filter(|call| call.owner() == CallSiteOwner::from(occurrence.invocation_site()))
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        return if calls.is_empty() {
            Ok(None)
        } else {
            Err("selected boundary internal call rejoins duplicate call custody")
        };
    };
    let record = rejoin_call_custody(
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
        return Err("installed provider internal call changed its selection or completion custody");
    }
    let conformances = module
        .provider_candidates
        .iter()
        .filter(|conformance| *conformance == provider)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return Err("installed provider internal call cannot rejoin one provider conformance");
    };
    let supplied_requirement = if let Some(assumption) = assumption {
        module
            .boundary_machine(assumption.selected_requirement)
            .and_then(|row| module.identities.text(row.identity))
            .filter(|identity| !identity.is_empty())
            .ok_or("installed provider internal call lost its supplied requirement")?
    } else {
        requirement_identity
    };
    if module.identities.text(conformance.requirement_identity) != Some(supplied_requirement) {
        return Err("installed provider internal call changed its requirement");
    }
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
    pass.verify(|| validate_destination(target.architecture, opcode, bytes, callee.text_offset))?;
    let mut attribution = object.semantic_code_attribution().iter().filter(|row| {
        row.machine == function.machine
            && row.attribution.site == semantic_site
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
        || text_relocation_overlaps(object, (opcode, end))
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
    let final_image_bytes = span(image, final_image_span)?;
    if pass.is_receiver() && (machine_bytes != object_bytes || object_bytes != final_image_bytes) {
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
    let parent = {
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
                selected_supply: plan.plan_digest().into(),
                target,
                role,
                identity: parent_identity,
            }
            .into(),
        )
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
