//! Derive installed-provider internal-call evidence by replaying its retained
//! call custody.
//!
//! An installed provider realized inside the same artifact emits an ordinary
//! resolved internal call: no object settlement row, no normalized foreign
//! call, and no external execution binding. The emitted call's retained
//! call-stack row names the owner operation and the callee machine, and the
//! module's selected provider conformance names the exact provider and
//! candidate identities that callee realizes. This lane binds that custody —
//! the same object rosters `fragment_call` follows for resolved calls — under
//! a boundary-trait settlement parent so the child's identity still commits
//! the occurrence, requirement, plan, and target.

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::InstalledProviderInternalCallCustody;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NativeSelectedProviderPlan;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::physical_child_identity;
use crate::physical::derivation::evidence::ranges_overlap;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::hashing::sha256;
use crate::physical::derivation::settlement_identity::installed_provider_internal_call_identity;
use crate::physical::native_byte_span;
use crate::physical::operator_applications::fragment_call::validate_destination;
use object_file::SectionKind;
use optimization_core::NativeOptimizationProjectionIdentity;
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

/// One retained call-stack row a boundary call's emitted internal call can
/// rejoin; the variants mirror the caller's record families exactly.
enum InternalCallRow<'artifact> {
    Unit(&'artifact image_emission::ObjectUnitCallStack),
    Scalar(&'artifact image_emission::ObjectScalarCallStack),
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
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .is_some_and(|function| {
            function
                .unit_call_stacks
                .iter()
                .map(|call| call.owner)
                .chain(function.scalar_call_stacks.iter().map(|call| call.owner))
                .any(|owner| owner == CallSiteOwner::Operation(operation))
        })
}

/// Derive the D41 physical child for one installed provider boundary call
/// realized as an ordinary resolved internal call.
///
/// `Ok(None)` leaves custody this lane does not cover — a boundary call that
/// emitted no internal call, or one whose structural-argument or
/// completion-receipt shapes no retained call row carries — without a claimed
/// child; the occurrence then blocks as `UnrealizedBoundaryOccurrence`.
/// Custody drift that would contradict a validated object or its Terminal
/// operation is an error, matching the settlement derivations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_installed_provider_internal_call_child(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    target: NativeTarget,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let source_operation = occurrence
        .operation()
        .ok_or("returning provider call requires an operation invocation")?;
    // The emitted call must still rejoin exactly one Terminal boundary call
    // on the occurrence's machine. This lane binds the plain internal-call
    // custody only: structural-argument and completion-receipt shapes retain
    // no call row, so those boundary calls decline here.
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == source_operation)
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("installed provider internal call does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        return Err("installed provider internal call owner is not a Terminal boundary call");
    };
    if *boundary != occurrence.boundary() {
        return Err("installed provider internal call changed its boundary");
    }
    if !structural_arguments.is_empty() || !completion_receipts.is_empty() {
        return Ok(None);
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("installed provider internal call names an absent object function")?;
    if !function.provenance.operations.contains(&source_operation) {
        return Err("installed provider internal call left its function provenance");
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
            Err("installed provider internal call rejoins duplicate call custody")
        };
    };
    // The emitted callee must be the checked-machine candidate one exact
    // selected conformance row names for this boundary's requirement. The
    // candidate catalog can hold several providers for one boundary, so the
    // retained call's target is what selects among them.
    let conformances = module
        .provider_candidates
        .iter()
        .filter(|conformance| {
            conformance.boundary == *boundary && conformance.candidate == call.target()
        })
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return Err("installed provider internal call cannot rejoin one provider conformance");
    };
    if conformance.requirement_identity != requirement_identity {
        return Err("installed provider internal call changed its requirement");
    }
    let callee = object
        .functions()
        .iter()
        .find(|candidate| candidate.machine == call.target())
        .ok_or("installed provider internal call names an absent object callee")?;
    let call_field_offset = call.text_offset();
    let opcode = match target.architecture {
        Architecture::X86_64 => call_field_offset.checked_sub(1),
        Architecture::Aarch64 => Some(call_field_offset),
    }
    .ok_or("installed provider internal call opcode offset underflow")?;
    let end = call_field_offset
        .checked_add(4)
        .ok_or("installed provider internal call extent overflow")?;
    let bytes = object
        .text_bytes()
        .get(opcode..end)
        .ok_or("installed provider internal call is outside object text")?;
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
        .ok_or("installed provider internal call lacks its exact semantic interval")?;
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
        return Err("installed provider internal call has ambiguous attribution or a relocation");
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
        return Err("installed provider internal call bytes changed across physical custody");
    }

    let custody = match call {
        InternalCallRow::Unit(call) => InstalledProviderInternalCallCustody::Unit {
            owner: call.owner,
            target: call.target,
            call_field_offset: call.text_offset,
            active_frame_bytes: call.active_frame_bytes,
            transient_bytes: call.transient_bytes,
            caller_live_bytes: call.caller_live_bytes,
        },
        InternalCallRow::Scalar(call) => InstalledProviderInternalCallCustody::Scalar {
            owner: call.owner,
            target: call.target,
            call_field_offset: call.text_offset,
            caller_live_bytes: call.caller_live_bytes,
        },
    };
    let parent_identity = installed_provider_internal_call_identity(
        occurrence,
        requirement_identity,
        selected_plan.plan_digest(),
        target,
        *boundary,
        &conformance.provider_identity,
        &conformance.candidate_identity,
        &custody,
    );
    let role = BoundaryTraitSettlementRole::InstalledProviderInternalCall {
        boundary: *boundary,
        provider_identity: conformance.provider_identity.clone(),
        candidate_identity: conformance.candidate_identity.clone(),
        call: custody,
    };
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
    let relocation = PhysicalRelocationDisposition::ResolvedInternalCall;
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
        &relocation,
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
