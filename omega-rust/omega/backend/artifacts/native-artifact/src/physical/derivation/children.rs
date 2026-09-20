//! Deriving each physical child: exit groups, byte writes and reads,
//! admitted provider settlements and normalized foreign children.

use crate::BoundaryTraitSettlementParts;
pub(crate) use crate::BoundaryTraitSettlementRole;
use crate::NativeCompilerBuiltinCatalogIdentity;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NormalizedForeignCallbackRelocations;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::{physical_child_identity, ranges_overlap, span};
use crate::physical::derivation::hashing::sha256;
use crate::physical::derivation::provider_custody::{
    direct_port_read_custody, linux_write_line_custody_is_exact, metadata_port_effect_custody,
    rejoin_admitted_provider_execution, settlement_completion_custody_is_exact,
};
use crate::physical::derivation::settlement_identity::{
    admitted_provider_boundary_trait_settlement_identity, admitted_provider_settlement_identity,
    hosted_builtin_settlement_identity,
};
use crate::physical::fragment_publication::FragmentPublicationBinding;
use crate::physical::model::native_byte_span;
use crate::physical::model::normalized_foreign_call_import_field;
pub(crate) use crate::physical::model::normalized_foreign_call_relocation;
use crate::physical::model::normalized_foreign_callback_relocation;
use crate::{
    CompilerBuiltinResult, CompilerBuiltinScalarArgument, NativeProviderExecution,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::BoundaryExecutionRecord;
use object_file::{RelocationKind, RelocationOrigin, SectionKind};
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use std::collections::BTreeSet;
use target::{Architecture, NativeTarget};
use target_operations::{
    BoundaryRealization, CompilerBuiltinExecution, NormalizedForeignCallBinding,
    ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use terminal_psi::OperationKind;

/// One declared scalar-argument form a hosted builtin settlement may retain.
/// The forms are mutually exclusive on the settlement record's two scalar
/// rosters, so at most one form binds for any installed settlement.
enum HostedScalarArgumentForm {
    /// The settlement retains no scalar argument on either roster.
    Absent,
    /// One compile-time scalar argument of the declared scalar type, carrying
    /// a signed immediate that fits that type, bound to the declared
    /// per-architecture ABI destination.
    Immediate {
        scalar: fn() -> ScalarType,
        destination: fn(Architecture) -> target_operations::MachineRegister,
    },
    /// One runtime scalar argument retained by its emitted source record;
    /// `source` restricts the record's source kind when `Some`.
    RuntimeScalar {
        source: Option<fn(&machine_code::InternalUnitScalarArgumentSourceRecord) -> bool>,
    },
}

/// One declared result-custody form for a hosted builtin settlement.
enum HostedResultForm {
    /// Unit result: the settlement retains no result record.
    Unit,
    /// One structural result bound by `bind`, which validates the record's
    /// exact custody for this builtin and reproduces the emitted bytes the
    /// settlement span must carry.
    Structural {
        bind: fn(
            &machine_code::BoundaryStructuralResultRecord,
            &OptimizedBoundaryOccurrence,
            NativeTarget,
        ) -> Result<Vec<u8>, &'static str>,
    },
}

/// One hosted builtin's declared settlement custody. The catalog is the
/// complete dispatch surface for compiler-builtin settlements: a further
/// hosted builtin is one row — its closed execution and realization pair, the
/// realization's own target applicability, the scalar-argument forms its
/// settlement may retain, and its declared result custody — not a new match
/// arm, role variant, or derivation function.
pub(crate) struct HostedBuiltinSettlement {
    /// Closed builtin execution the installed settlement must carry.
    execution: CompilerBuiltinExecution,
    /// Closed realization the installed settlement must carry. The hosted
    /// realizations carry no configurable fields, so record equality with
    /// this row is the exact variant join.
    realization: BoundaryRealization,
    /// The realization's canonical target applicability.
    supports_target: fn(NativeTarget) -> bool,
    /// Admitted scalar-argument forms; the installed settlement must bind
    /// exactly one.
    scalar_forms: &'static [HostedScalarArgumentForm],
    /// Declared result custody.
    result: HostedResultForm,
    /// Custody tags naming this builtin's execution and realization inside
    /// the HostedV1 builtin settlement identity.
    identity_tags: [u8; 2],
}

const HOSTED_BUILTIN_SETTLEMENTS: &[HostedBuiltinSettlement] = &[
    HostedBuiltinSettlement {
        execution: CompilerBuiltinExecution::HostedExitProcessI32,
        realization: BoundaryRealization::HostedExitProcessI32(
            target_operations::HostedExitProcessI32Realization,
        ),
        supports_target: target_operations::HostedExitProcessI32Realization::supports_target,
        scalar_forms: &[
            HostedScalarArgumentForm::Immediate {
                scalar: i32_scalar,
                destination: process_exit_argument_destination,
            },
            HostedScalarArgumentForm::RuntimeScalar {
                source: Some(selected_process_exit_i32_source),
            },
        ],
        result: HostedResultForm::Unit,
        identity_tags: [1, 1],
    },
    HostedBuiltinSettlement {
        execution: CompilerBuiltinExecution::HostedWriteByteI32,
        realization: BoundaryRealization::HostedWriteByteI32(
            target_operations::HostedWriteByteI32Realization,
        ),
        supports_target: target_operations::HostedWriteByteI32Realization::supports_target,
        scalar_forms: &[HostedScalarArgumentForm::RuntimeScalar { source: None }],
        result: HostedResultForm::Unit,
        identity_tags: [2, 2],
    },
    HostedBuiltinSettlement {
        execution: CompilerBuiltinExecution::HostedReadByte,
        realization: BoundaryRealization::HostedReadByte(
            target_operations::HostedReadByteRealization,
        ),
        supports_target: target_operations::HostedReadByteRealization::supports_target,
        scalar_forms: &[HostedScalarArgumentForm::Absent],
        result: HostedResultForm::Structural {
            bind: read_byte_result_bytes,
        },
        identity_tags: [3, 3],
    },
];

/// Find the hosted builtin whose closed `(execution, realization)` pair the
/// installed settlement carries. A `CompilerBuiltin` execution whose
/// realization is not the builtin's own falls through to the provider lanes
/// and remains an `UnsupportedSettlementRealization` gap, exactly as a named
/// foreign realization does.
pub(crate) fn hosted_builtin_settlement(
    settlement: &machine_code::BoundarySettlementRecord,
) -> Option<&'static HostedBuiltinSettlement> {
    let BoundaryExecutionRecord::CompilerBuiltin(execution) = settlement.execution else {
        return None;
    };
    HOSTED_BUILTIN_SETTLEMENTS.iter().find(|builtin| {
        builtin.execution == execution && builtin.realization == settlement.realization
    })
}

fn i32_scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid"))
}

fn process_exit_argument_destination(
    architecture: Architecture,
) -> target_operations::MachineRegister {
    match architecture {
        Architecture::X86_64 => target_operations::MachineRegister::X86Rdi,
        Architecture::Aarch64 => target_operations::MachineRegister::Aarch64X(0),
    }
}

fn selected_process_exit_i32_source(
    source: &machine_code::InternalUnitScalarArgumentSourceRecord,
) -> bool {
    matches!(
        source,
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            scalar_type,
            ..
        } if *scalar_type == i32_scalar()
    )
}

/// Validate the read-byte result record's exact custody and reproduce the
/// emitted instruction bytes its settlement span must carry: the result is
/// defined by the occurrence's own operation, and the caller-home layout is
/// the conventional `ByteRead` sum — a zero tag at offset 0, a two-case
/// layout with the byte payload at offset 4 in the success case.
fn read_byte_result_bytes(
    result: &machine_code::BoundaryStructuralResultRecord,
    occurrence: &OptimizedBoundaryOccurrence,
    target: NativeTarget,
) -> Result<Vec<u8>, &'static str> {
    if result.defining_operation != occurrence.operation()
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
        return Err("hosted read-byte result custody is incomplete or substituted");
    }
    let payload_offset = result
        .home_byte_offset
        .checked_add(u32::from(result.layout.payload_byte_offset))
        .ok_or("hosted read-byte physical child result home overflow")?;
    match target.architecture {
        Architecture::X86_64 => {
            isa_x86_64::encode_linux_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "Linux read-byte x86-64 encoding is not reproducible")
        }
        Architecture::Aarch64 if target == NativeTarget::macos_arm64() => {
            isa_aarch64::encode_macos_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "macOS read-byte AArch64 encoding is not reproducible")
        }
        Architecture::Aarch64 => {
            isa_aarch64::encode_linux_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "Linux read-byte AArch64 encoding is not reproducible")
        }
    }
}

/// The retained scalar argument must be a signed immediate that fits its
/// declared integer type.
fn signed_immediate_fits_declared(
    immediate: semantic_vocabulary::IntegerValue,
    scalar_type: ScalarType,
) -> bool {
    let (ScalarType::Integer(integer), semantic_vocabulary::IntegerValue::Signed(value)) =
        (scalar_type, immediate)
    else {
        return false;
    };
    match integer.bits() {
        8 => i8::try_from(value).is_ok(),
        16 => i16::try_from(value).is_ok(),
        32 => i32::try_from(value).is_ok(),
        64 => i64::try_from(value).is_ok(),
        128 => true,
        _ => false,
    }
}

/// Bind the installed settlement's retained scalar custody to one of the
/// builtin's declared forms. `Some(None)` is a bound absent form; `None` means
/// no declared form matched the retained rosters.
fn bind_scalar_form(
    form: &HostedScalarArgumentForm,
    settlement: &machine_code::BoundarySettlementRecord,
    architecture: Architecture,
) -> Option<Option<CompilerBuiltinScalarArgument>> {
    match form {
        HostedScalarArgumentForm::Absent => (settlement.scalar_arguments.is_empty()
            && settlement.runtime_scalar_arguments.is_empty())
        .then_some(None),
        HostedScalarArgumentForm::Immediate {
            scalar,
            destination,
        } => match (
            settlement.scalar_arguments.as_slice(),
            settlement.runtime_scalar_arguments.as_slice(),
        ) {
            ([argument], [])
                if argument.scalar_type == scalar()
                    && signed_immediate_fits_declared(argument.immediate, argument.scalar_type)
                    && argument.destination == destination(architecture) =>
            {
                Some(Some(CompilerBuiltinScalarArgument::Immediate(*argument)))
            }
            _ => None,
        },
        HostedScalarArgumentForm::RuntimeScalar { source } => match (
            settlement.scalar_arguments.as_slice(),
            settlement.runtime_scalar_arguments.as_slice(),
        ) {
            ([], [argument]) if source.is_none_or(|source| source(&argument.source)) => Some(Some(
                CompilerBuiltinScalarArgument::RuntimeScalar(argument.clone()),
            )),
            _ => None,
        },
    }
}

/// Derive the D41 physical child for one installed hosted builtin settlement.
/// The caller matched the builtin's closed execution and realization pair;
/// here the realization's own target applicability binds, the settlement's
/// retained scalar and result custody must join the catalog row's declared
/// shapes exactly, and the emitted span must attach to the object and the
/// final image byte-identically with no relocation inside it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_hosted_builtin_child(
    builtin: &HostedBuiltinSettlement,
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
    if !(builtin.supports_target)(target) {
        return Err("hosted builtin physical child requires a canonical supported target");
    }
    let Some(scalar_argument) = builtin
        .scalar_forms
        .iter()
        .find_map(|form| bind_scalar_form(form, settlement, target.architecture))
    else {
        return Err("hosted builtin physical child does not carry its declared scalar custody");
    };
    if !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
    {
        return Err("hosted builtin settlement custody is incomplete or substituted");
    }
    let (result, expected) = match builtin.result {
        HostedResultForm::Unit if settlement.native_result.is_unit() => {
            (CompilerBuiltinResult::Unit, None)
        }
        HostedResultForm::Unit => {
            return Err("hosted builtin settlement changed its declared unit result");
        }
        HostedResultForm::Structural { bind } => {
            let Some(result) = settlement.native_result.structural() else {
                return Err("hosted builtin settlement lacks its structural result custody");
            };
            (
                CompilerBuiltinResult::Structural(result.clone()),
                Some(bind(result, occurrence, target)?),
            )
        }
    };
    if settlement.byte_count == 0 {
        return Err("hosted builtin physical child requires a nonempty emitted span");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("hosted builtin physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("hosted builtin physical child object span overflow")?;
    if installed.text_offset != expected_object_offset
        || expected
            .as_ref()
            .is_some_and(|expected| expected.len() != settlement.byte_count)
    {
        return Err("hosted builtin physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes
        || object_bytes != final_image_bytes
        || expected.is_some_and(|expected| machine_bytes != expected.as_slice())
    {
        return Err("hosted builtin physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("hosted builtin physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("hosted builtin physical child unexpectedly contains a relocation");
    }

    let parent_identity = hosted_builtin_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        builtin.identity_tags,
        scalar_argument.as_ref(),
        &result,
    )?;
    let role = BoundaryTraitSettlementRole::CompilerBuiltin {
        catalog: NativeCompilerBuiltinCatalogIdentity::HostedV1,
        execution: builtin.execution,
        realization: builtin.realization,
        scalar_argument,
        result,
    };
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
pub(crate) fn derive_admitted_provider_settlement_child(
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_normalized_foreign_child(
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
    publication: Option<&FragmentPublicationBinding>,
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
        || !completion_receipts.is_empty()
    {
        return Ok(None);
    }

    if foreign.operation_ordinal != occurrence.operation_ordinal()
        || !declaration.has_valid_parameter_order()
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
    let pointer_shape = calling_conventions::ValueShape::integer(
        u16::try_from(target.pointer_size)
            .map_err(|_| "normalized foreign D41 pointer size does not fit its ABI")?,
        u16::try_from(target.pointer_alignment)
            .map_err(|_| "normalized foreign D41 pointer alignment does not fit its ABI")?,
    );
    // The normalized foreign lane admits structural custody only as the exact
    // source-rooted borrowed flat-record lane the lowering owns: each expected
    // Terminal argument row must rejoin its declared formal and the observed
    // evaluated plan must place each formal's referent pointer as one
    // pointer-width word. Any other valid signature shape retains no complete
    // physical evidence rather than claiming a custody join it cannot prove.
    let structural_parameter_shapes =
        if !structural_arguments.is_empty() || !declaration.structural_parameters.is_empty() {
            let caller = module
                .machines
                .iter()
                .find(|machine| machine.id == occurrence.machine())
                .ok_or("normalized foreign D41 child names an absent caller machine")?;
            let Some(shapes) = normalized_foreign_structural_parameter_shapes(
                structural_arguments,
                &declaration.structural_parameters,
                &caller.structural_parameters,
                &declaration.parameter_order,
                foreign.callback_address.is_some()
                    || !foreign
                        .boundary_entry_plan
                        .call
                        .callback_materializations
                        .is_empty(),
                &foreign.boundary_entry_plan.call.parameters,
                pointer_shape,
            )?
            else {
                return Ok(None);
            };
            Some(shapes)
        } else {
            None
        };
    // Fragment publication retains the call's custody as relocation-free
    // plan rows instead of object relocations; rejoin the unresolved import
    // field, its declared import symbol, and the selected roster row before
    // the custody checks below split on which route produced the call.
    let fragment_rejoin = publication
        .map(|publication| {
            fragment_normalized_foreign_rejoin(
                publication,
                occurrence,
                foreign,
                structural_arguments,
            )
        })
        .transpose()?;
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
        (
            terminal_psi::OperationResult::Scalar(value),
            terminal_psi::BoundaryMachineResult::Scalar(declared),
            None,
        ) => {
            // Fragment custody rows keep `scalar_result` empty; the retained
            // roster row's result-home requirement carries the same custody
            // the object route proves through its emitted durable-home record.
            let Some(home) =
                fragment_rejoin.and_then(|rejoin| rejoin.record.call.result_home.as_ref())
            else {
                return Err("normalized foreign D41 child changed its scalar result custody");
            };
            if value.scalar_type != *declared
                || home.defining_operation != occurrence.operation()
                || home.source_value != value.id
                || home.scalar_type != *declared
            {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            let Some(shape) = fixed_integer_shape(*declared) else {
                return Ok(None);
            };
            if home.shape != shape {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            Some(shape)
        }
        _ => return Err("normalized foreign D41 child changed its scalar result custody"),
    };
    if let Some(rejoin) = fragment_rejoin
        && rejoin.record.call.result_home.is_some() != result_shape.is_some()
    {
        return Err("normalized foreign D41 child changed its scalar result custody");
    }
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
        // Fragment custody projects the entry plan wholesale but cannot yet
        // carry callback materialization custody rows; the call retains no
        // complete physical evidence rather than claiming a custody join it
        // cannot prove.
        (None, materials) if fragment_rejoin.is_some() && !materials.is_empty() => {
            return Ok(None);
        }
        _ => return Err("normalized foreign D41 child changed its callback-plan custody"),
    };
    let callback_ordinal = callback
        .map(|callback| usize::try_from(callback.target.application.native_ordinal))
        .transpose()
        .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
    if callback_ordinal.is_some_and(|ordinal| ordinal > declaration.parameter_order.len()) {
        return Err("normalized foreign D41 callback ordinal is outside its native signature");
    }
    for (physical, semantic_position) in foreign
        .scalar_arguments
        .iter()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
    {
        let native_position = semantic_position
            + usize::from(callback_ordinal.is_some_and(|ordinal| ordinal <= semantic_position));
        if usize::try_from(physical.parameter_index).ok() != Some(native_position)
            || foreign
                .boundary_entry_plan
                .call
                .parameters
                .get(native_position)
                != Some(&physical.placement)
        {
            return Err("normalized foreign D41 child changed a scalar argument position");
        }
    }
    // Terminal retains authored order independently of the physical proposal.
    // Reconstruct that interleave instead of letting either lane claim the
    // other lane's ABI positions, even when their word shapes happen to match.
    let mut scalar_shapes = parameter_shapes.into_iter();
    let mut structural_shapes = structural_parameter_shapes.into_iter().flatten();
    let mut native_parameter_shapes = declaration
        .parameter_order
        .iter()
        .map(|kind| {
            match kind {
                terminal_psi::BoundaryParameterKind::Scalar => scalar_shapes.next(),
                terminal_psi::BoundaryParameterKind::Structural => structural_shapes.next(),
            }
            .ok_or("normalized foreign D41 child changed its parameter order")
        })
        .collect::<Result<Vec<_>, _>>()?;
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

    // The image's foreign-call roster is the object's own roster plus any
    // fragment-publication custody rows; the call must rejoin exactly the
    // projected row the emitted image now retains.
    let retained_calls: &[image_emission::ObjectForeignCall] = match publication {
        Some(binding) => binding.foreign_call_custody(),
        None => object.foreign_calls(),
    };
    let matching_image_calls = retained_calls
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
    // Fragment publication binds the same call custody through the
    // relocation-free object plan — the unresolved import field plus its
    // declared import symbol — where the object route binds it through
    // relocation records and an object-plan import row.
    let (mutable_intervals, relocation) = if let Some(rejoin) = fragment_rejoin {
        let plan = publication
            .expect("fragment rejoin requires the publication binding")
            .relocation_free_object();
        let field = &rejoin.field.resolution;
        let field_offset = usize::try_from(field.field_section_offset)
            .map_err(|_| "normalized foreign D41 import field offset does not fit this target")?;
        let field_byte_width = usize::from(field.field_byte_width);
        let field_kind = match field.kind {
            machine_code::NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                RelocationKind::X86_64Relative32
            }
            machine_code::NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                RelocationKind::Aarch64Branch26
            }
        };
        if field_offset != foreign.text_offset
            || field_byte_width != 4
            || field.addend != 0
            || field_kind != expected_kind
        {
            return Err(
                "normalized foreign D41 child changed import owner, symbol, addend, kind, or span",
            );
        }
        let overlapping_fields = plan
            .unresolved_normalized_foreign_calls
            .iter()
            .filter(|candidate| {
                usize::try_from(candidate.resolution.field_section_offset).is_ok_and(|offset| {
                    ranges_overlap(
                        object_offset,
                        object_end,
                        offset,
                        offset.saturating_add(usize::from(candidate.resolution.field_byte_width)),
                    )
                })
            })
            .collect::<Vec<_>>();
        let [sole_field] = overlapping_fields.as_slice() else {
            return Err(
                "normalized foreign D41 child contains an unowned or out-of-span relocation",
            );
        };
        if !std::ptr::eq(*sole_field, rejoin.field)
            || field_offset < object_offset
            || field_offset
                .checked_add(field_byte_width)
                .is_none_or(|end| end > object_end)
        {
            return Err(
                "normalized foreign D41 child contains an unowned or out-of-span relocation",
            );
        }
        let field_start = field_offset
            .checked_sub(object_offset)
            .expect("import field verified inside the operation interval");
        (
            vec![(field_start, field_start + field_byte_width)],
            PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(
                normalized_foreign_call_import_field(
                    foreign.locator.identity_digest().as_bytes(),
                    boundary_plan_identity,
                    occurrence.machine(),
                    occurrence.operation(),
                    field.boundary,
                    field.ordinal,
                    rejoin.import.symbol,
                    foreign.text_offset,
                    field_byte_width,
                    field.addend,
                    field_kind,
                    final_image_symbol_digest,
                ),
            ),
        )
    } else {
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
                let exact_callback_relocation = |offset: usize,
                                                 kind: RelocationKind|
                 -> Result<_, &'static str> {
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
                    machine_code::CallbackAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => NormalizedForeignCallbackRelocations::X86_64Relative32 {
                        callback_function: callback.target.callback_function,
                        relocation: exact_callback_relocation(
                            relocation_offset,
                            RelocationKind::X86_64Relative32,
                        )?,
                    },
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
            return Err(
                "normalized foreign D41 child contains an unowned or out-of-span relocation",
            );
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
        (
            mutable_intervals,
            PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
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
            ),
        )
    };

    let machine_span = native_byte_span(code_offset, byte_count);
    let object_span = native_byte_span(object_offset, byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes {
        return Err("normalized foreign D41 child changed before object custody");
    }
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
        structural_arguments,
        &declaration.structural_parameters,
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

/// The fragment-publication rejoin of one normalized-foreign D41 child: the
/// relocation-free object plan's unresolved import field, the declared import
/// symbol it names, and the selected-plan roster row the projected custody
/// was bound from. The three must agree on `{caller, operation, boundary}`
/// and the roster coordinate `{boundary, ordinal}` before physical evidence
/// may claim the projected custody row.
#[derive(Clone, Copy)]
struct FragmentForeignCallRejoin<'a> {
    field: &'a object_file::RelocationFreeObjectUnresolvedForeignCall,
    import: &'a object_file::RelocationFreeObjectNormalizedImport,
    record: &'a selected_instructions::SelectedNormalizedForeignCall,
}

/// Independently replays the custody projection's join: the unique unresolved
/// import field owned by this call site, its declared import symbol, and the
/// selected roster row retaining the call's evaluated binding. A substituted
/// plan row, placement, or provider binding fails here rather than realizing
/// silently.
fn fragment_normalized_foreign_rejoin<'a>(
    publication: &'a FragmentPublicationBinding,
    occurrence: &OptimizedBoundaryOccurrence,
    foreign: &image_emission::ObjectForeignCall,
    structural_arguments: &[terminal_psi::StructuralArgument],
) -> Result<FragmentForeignCallRejoin<'a>, &'static str> {
    let plan = publication.relocation_free_object();
    let selected = publication.selected_plan();
    let mut fields = plan
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|candidate| {
            candidate.resolution.caller == occurrence.machine()
                && candidate.resolution.operation == occurrence.operation()
                && candidate.resolution.boundary == occurrence.boundary()
                && candidate.resolution.state
                    == machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
        })
        .collect::<Vec<_>>();
    if fields.len() != 1 {
        return Err("normalized foreign D41 child cannot rejoin one unresolved import field");
    }
    let field = fields.remove(0);
    if usize::try_from(field.resolution.field_section_offset) != Ok(foreign.text_offset) {
        return Err("normalized foreign D41 import field does not match its call offset");
    }
    let mut imports = plan
        .normalized_imports
        .iter()
        .filter(|import| {
            import.symbol == field.symbol
                && import.boundary == field.resolution.boundary
                && import.ordinal == field.resolution.ordinal
        })
        .collect::<Vec<_>>();
    if imports.len() != 1 {
        return Err("normalized foreign D41 child cannot rejoin one declared import symbol");
    }
    let import = imports.remove(0);
    if import.name
        != object_file::canonical_normalized_foreign_import_symbol_name(
            import.boundary,
            import.ordinal,
        )
    {
        return Err("normalized foreign D41 import symbol drifted from its canonical name");
    }
    let mut functions = selected
        .functions
        .iter()
        .filter(|function| function.machine == occurrence.machine())
        .collect::<Vec<_>>();
    if functions.len() != 1 {
        return Err("normalized foreign D41 child names an absent selected function");
    }
    let function = functions.remove(0);
    let index = usize::try_from(field.resolution.ordinal)
        .map_err(|_| "normalized foreign D41 roster ordinal does not fit this target")?;
    let record = function
        .normalized_foreign_calls
        .get(index)
        .ok_or("normalized foreign D41 roster row is absent for its ordinal")?;
    if record.instruction != field.resolution.instruction
        || record.operation != field.resolution.operation
        || record.call.boundary != field.resolution.boundary
    {
        return Err("normalized foreign D41 roster row lost its call target");
    }
    let Some(block) = function
        .blocks
        .iter()
        .find(|block| block.id == field.resolution.block)
    else {
        return Err("normalized foreign D41 roster row names an absent instruction");
    };
    // Instruction identities are function-global, not block-local indexes:
    // find the roster instruction by id inside its recorded block.
    let Some(instruction) = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == field.resolution.instruction)
    else {
        return Err("normalized foreign D41 roster row names an absent instruction");
    };
    if !matches!(
        instruction.kind,
        selected_instructions::SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal }
            if boundary == field.resolution.boundary && ordinal == field.resolution.ordinal
    ) {
        return Err("normalized foreign D41 instruction kind does not match its roster row");
    }
    if record.call.binding.locator != foreign.locator
        || record.call.binding.boundary_entry_plan != foreign.boundary_entry_plan
        || record.call.binding.same_stack_contribution != foreign.same_stack_contribution
        || machine_code::ProviderExecutionRecord::from(record.call.provider_execution)
            != foreign.provider_execution
        || record.call.structural_arguments.len() != structural_arguments.len()
        || !fragment_scalar_argument_custody_rejoin(
            publication,
            function,
            block,
            record,
            instruction,
            foreign,
        )
    {
        return Err("normalized foreign D41 roster row drifted from its projected custody");
    }
    Ok(FragmentForeignCallRejoin {
        field,
        import,
        record,
    })
}

/// Rejoins one roster row's scalar-argument lane against its projected
/// custody records. Each projected record must keep the roster argument's
/// authored plan position and placement verbatim and name the exact emitted
/// custody the projection derived — the register copy or contiguous
/// outgoing-slot address-plus-store pair for an integer immediate or a
/// scalar-call `Home`, and the exact durable-home slot and normalization span
/// for a scalar result — re-derived here from the retained text section
/// rather than trusted.
fn fragment_scalar_argument_custody_rejoin(
    publication: &FragmentPublicationBinding,
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    foreign: &image_emission::ObjectForeignCall,
) -> bool {
    let plan = &record.call.binding.boundary_entry_plan.call;
    if !plan.callback_materializations.is_empty()
        || foreign.scalar_arguments.len() != record.call.scalar_arguments.len()
        || plan.parameters.len()
            != record.call.scalar_arguments.len() + record.call.structural_arguments.len()
    {
        return false;
    }
    let mut fragments = publication
        .text_section()
        .functions
        .iter()
        .filter(|fragment| fragment.machine == foreign.machine);
    let (Some(fragment), None) = (fragments.next(), fragments.next()) else {
        return false;
    };
    let home_offsets = fragment_durable_home_offsets(function);
    if !fragment_scalar_result_custody_rejoin(
        function,
        block,
        fragment,
        record,
        instruction,
        &home_offsets,
        foreign.scalar_result.as_ref(),
    ) {
        return false;
    }
    // The call's operand roster is every register-resident argument word in
    // authored plan order — scalar arguments and structural referent pointers
    // alike — then the optional scalar result definition.
    let register_arguments = record
        .call
        .scalar_arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.placement.locations.as_slice(),
                [calling_conventions::ValueLocation::Register { .. }]
            )
        })
        .count()
        + record
            .call
            .structural_arguments
            .iter()
            .filter(|argument| {
                matches!(
                    argument.destination.locations.as_slice(),
                    [calling_conventions::ValueLocation::Register { .. }]
                )
            })
            .count();
    if instruction.operands.len()
        != register_arguments + usize::from(record.call.result_home.is_some())
    {
        return false;
    }
    for (scalar_ordinal, (argument, physical)) in record
        .call
        .scalar_arguments
        .iter()
        .zip(&foreign.scalar_arguments)
        .enumerate()
    {
        let Ok(native_position) = usize::try_from(argument.parameter_index) else {
            return false;
        };
        let ScalarType::Integer(integer) = argument.source.scalar_type() else {
            return false;
        };
        if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || !matches!(integer.bits(), 8 | 16 | 32 | 64)
            || scalar_ordinal.checked_sub(1).is_some_and(|previous| {
                record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
            })
            || physical.parameter_index != argument.parameter_index
            || physical.placement != argument.placement
            || plan.parameters.get(native_position) != Some(&argument.placement)
            || argument.placement.shape
                != calling_conventions::ValueShape::integer(integer.bits() / 8, integer.bits() / 8)
        {
            return false;
        }
        let register_operand = plan
            .parameters
            .iter()
            .take(native_position)
            .filter(|placement| {
                matches!(
                    placement.locations.as_slice(),
                    [calling_conventions::ValueLocation::Register { .. }]
                )
            })
            .count();
        let span_rejoins = match (argument.source, physical.source) {
            (
                target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation: projected_operation,
                    source_value: projected_value,
                    scalar_type: projected_type,
                    value: projected_integer,
                },
            ) => {
                (defining_operation, source_value, scalar_type, value)
                    == (
                        projected_operation,
                        projected_value,
                        projected_type,
                        projected_integer,
                    )
                    && semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_ok()
                    && fragment_immediate_argument_span_rejoin(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        physical,
                    )
            }
            (
                target_operations::TargetUnitScalarArgumentSource::Home(home),
                machine_code::InternalUnitScalarArgumentSourceRecord::Home(projected),
            ) => {
                let allocated = home_offsets
                    .iter()
                    .find(|(operation, _)| *operation == home.defining_operation)
                    .map(|(_, offset)| *offset);
                home.shape == argument.placement.shape
                    && projected
                        == machine_code::UnitScalarHomeRecord {
                            defining_operation: home.defining_operation,
                            source_value: home.source_value,
                            scalar_type: home.scalar_type,
                            shape: home.shape,
                            byte_offset: allocated.unwrap_or(u32::MAX),
                        }
                    && allocated.is_some()
                    && fragment_immediate_argument_span_rejoin(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        physical,
                    )
                    && fragment_scalar_call_result_producer(function, &home)
            }
            _ => false,
        };
        if !span_rejoins {
            return false;
        }
    }
    true
}

/// Re-derives the emitted interval one integer-immediate argument occupies
/// and compares it to the projected record: the register copy whose result
/// feeds the call operand, or the contiguous outgoing-slot
/// `FrameAddress`/`Store` pair this call's argument transport owns.
#[allow(clippy::too_many_arguments)]
fn fragment_immediate_argument_span_rejoin(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    argument: &target_operations::TargetUnitScalarCallArgument,
    register_operand: usize,
    physical: &machine_code::ForeignCallScalarArgumentRecord,
) -> bool {
    let span = match argument.placement.locations.as_slice() {
        [
            calling_conventions::ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == argument.placement.shape.byte_size => {
            let Some(operand) = instruction.operands.get(register_operand) else {
                return false;
            };
            if usize::from(operand.operand) != register_operand {
                return false;
            }
            let Some(register_value) = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
            else {
                return false;
            };
            let selected_instructions::VirtualRegisterOrigin::InstructionResult {
                instruction: materialization,
                source_value,
            } = register_value.origin
            else {
                return false;
            };
            if source_value != argument.source.source_value() {
                return false;
            }
            fragment_instruction_span(fragment, materialization)
        }
        [
            calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                alignment,
            },
        ] if *byte_size == argument.placement.shape.byte_size => {
            let slot = selected_instructions::OutgoingArgumentSlotId {
                operation: record.operation,
                argument_index: argument.parameter_index,
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            };
            let mut outgoing = function
                .outgoing_arguments
                .iter()
                .filter(|outgoing| outgoing.id == slot);
            let (Some(outgoing), None) = (outgoing.next(), outgoing.next()) else {
                return false;
            };
            if outgoing.abi_stack_byte_offset != *stack_byte_offset
                || outgoing.byte_size != u32::from(*byte_size)
                || outgoing.alignment != *alignment
            {
                return false;
            }
            let source_value = argument.source.source_value();
            let Ok(stored) = u8::try_from(*byte_size) else {
                return false;
            };
            let Some(address_index) = block.instructions.iter().position(|candidate| {
                matches!(
                    candidate.kind,
                    selected_instructions::SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Outgoing(candidate),
                        byte_offset: 0,
                    } if candidate == slot
                ) && candidate.provenance.operations.as_slice() == [record.operation]
                    && candidate.provenance.values.as_slice() == [source_value]
            }) else {
                return false;
            };
            let address = &block.instructions[address_index];
            let Some(store) = block.instructions.get(address_index + 1) else {
                return false;
            };
            let selected_instructions::SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: stored_bytes,
            } = store.kind
            else {
                return false;
            };
            if stored_bytes != stored
                || store.provenance.operations.as_slice() != [record.operation]
                || store.provenance.values.as_slice() != [source_value]
            {
                return false;
            }
            let Some(address_operand) = store.operands.first() else {
                return false;
            };
            let Some(address_value) = function
                .virtual_registers
                .get(address_operand.virtual_register.0 as usize)
            else {
                return false;
            };
            if address_value.origin
                != (selected_instructions::VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction: address.id,
                    source_value,
                })
            {
                return false;
            }
            let Some((address_offset, address_bytes)) =
                fragment_instruction_span(fragment, address.id)
            else {
                return false;
            };
            let Some((store_offset, store_bytes)) = fragment_instruction_span(fragment, store.id)
            else {
                return false;
            };
            if store_offset != address_offset + address_bytes {
                return false;
            }
            Some((address_offset, address_bytes + store_bytes))
        }
        _ => None,
    };
    span.is_some_and(|(offset, bytes)| {
        usize::try_from(offset) == Ok(physical.code_offset)
            && usize::try_from(bytes) == Ok(physical.byte_count)
    })
}

/// Re-derives a roster row's scalar-result custody — the durable-home record,
/// its plan placement, and the normalization instruction span — and requires
/// the projected record to equal the re-derivation exactly.
fn fragment_scalar_result_custody_rejoin(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    physical: Option<&machine_code::ForeignCallScalarResultRecord>,
) -> bool {
    let plan = &record.call.binding.boundary_entry_plan.call;
    let (Some(home), Some(placement)) = (&record.call.result_home, &plan.result) else {
        return physical.is_none() && record.call.result_home.is_none() && plan.result.is_none();
    };
    let Some(physical) = physical else {
        return false;
    };
    let ScalarType::Integer(integer) = home.scalar_type else {
        return false;
    };
    if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || !matches!(integer.bits(), 8 | 16 | 32 | 64)
        || home.defining_operation != record.operation
        || home.shape
            != calling_conventions::ValueShape::integer(integer.bits() / 8, integer.bits() / 8)
        || placement.shape != home.shape
        || !matches!(
            placement.locations.as_slice(),
            [
                calling_conventions::ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == home.shape.byte_size
        )
    {
        return false;
    }
    let Some(position) = block
        .instructions
        .iter()
        .position(|candidate| candidate.id == record.instruction)
    else {
        return false;
    };
    let Some(normalization) = block.instructions.get(position + 1) else {
        return false;
    };
    let [input, output] = normalization.operands.as_slice() else {
        return false;
    };
    let Some(call_result) = instruction.operands.last() else {
        return false;
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    let Some(expected_kind) = fragment_scalar_result_normalization_kind(home.scalar_type) else {
        return false;
    };
    if normalization.kind != expected_kind
        || input.virtual_register != call_result.virtual_register
        || normalization.provenance.values.as_slice() != [home.source_value]
        || output_origin
            != Some(
                selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: normalization.id,
                    source_value: home.source_value,
                },
            )
    {
        return false;
    }
    let Some((offset, byte_count)) = fragment_instruction_span(fragment, normalization.id) else {
        return false;
    };
    let Some(byte_offset) = home_offsets
        .iter()
        .find(|(operation, _)| *operation == home.defining_operation)
        .map(|(_, offset)| *offset)
    else {
        return false;
    };
    *physical
        == machine_code::ForeignCallScalarResultRecord {
            home: machine_code::UnitScalarHomeRecord {
                defining_operation: home.defining_operation,
                source_value: home.source_value,
                scalar_type: home.scalar_type,
                shape: home.shape,
                byte_offset,
            },
            source: placement.clone(),
            code_offset: usize::try_from(offset).unwrap_or(usize::MAX),
            byte_count: usize::try_from(byte_count).unwrap_or(usize::MAX),
        }
}

/// The normalization kind one scalar result's durable definition emits —
/// the rejoin-side copy of the projection's fixed-integer lane mapping.
fn fragment_scalar_result_normalization_kind(
    scalar_type: ScalarType,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
    match scalar_type {
        ScalarType::Boolean => Some(Kind::ZeroExtendU8),
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (semantic_vocabulary::IntegerSign::Unsigned, 8) => Some(Kind::ZeroExtendU8),
            (semantic_vocabulary::IntegerSign::Unsigned, 16) => Some(Kind::ZeroExtendU16),
            (semantic_vocabulary::IntegerSign::Unsigned, 32) => Some(Kind::ZeroExtendU32),
            (semantic_vocabulary::IntegerSign::Unsigned, 64)
            | (semantic_vocabulary::IntegerSign::Signed, 64) => Some(Kind::CopyI64),
            (semantic_vocabulary::IntegerSign::Signed, 8) => Some(Kind::SignExtendI8),
            (semantic_vocabulary::IntegerSign::Signed, 16) => Some(Kind::SignExtendI16),
            (semantic_vocabulary::IntegerSign::Signed, 32) => Some(Kind::SignExtendI32),
            _ => None,
        },
        _ => None,
    }
}

/// The caller's durable scalar-home area, re-derived in emitted instruction
/// order: every scalar-producing selected call row is one producer whose slot
/// is 8-aligned and sized by its home shape. Projection and this rejoin share
/// the rule so `Home` sources and `scalar_result` records name the same slot.
fn fragment_durable_home_offsets(
    function: &selected_instructions::SelectedFunction,
) -> Vec<(semantic_vocabulary::OperationId, u32)> {
    let mut producers = Vec::new();
    for contract in &function.calls {
        if let Some(placement) = &contract.call.result_placement {
            producers.push((
                contract.instruction,
                contract.operation,
                placement.shape.byte_size,
            ));
        }
    }
    for record in &function.normalized_foreign_calls {
        if let Some(home) = &record.call.result_home {
            producers.push((
                record.instruction,
                home.defining_operation,
                home.shape.byte_size,
            ));
        }
        for argument in &record.call.scalar_arguments {
            if let target_operations::TargetUnitScalarArgumentSource::Home(requirement) =
                argument.source
            {
                let Some(instruction) = function
                    .calls
                    .iter()
                    .map(|contract| (contract.operation, contract.instruction))
                    .chain(
                        function
                            .normalized_foreign_calls
                            .iter()
                            .map(|record| (record.operation, record.instruction)),
                    )
                    .find(|(operation, _)| *operation == requirement.defining_operation)
                    .map(|(_, instruction)| instruction)
                else {
                    continue;
                };
                producers.push((
                    instruction,
                    requirement.defining_operation,
                    requirement.shape.byte_size,
                ));
            }
        }
    }
    producers.sort_by_key(|(instruction, _, _)| instruction.0);
    producers.dedup_by_key(|(_, operation, _)| *operation);
    let mut cursor = 0u32;
    let mut offsets = Vec::new();
    for (_, operation, byte_size) in producers {
        let Some(aligned) = cursor.checked_add(7).map(|cursor| cursor & !7) else {
            return Vec::new();
        };
        cursor = aligned;
        offsets.push((operation, cursor));
        let Some(next) = cursor.checked_add(u32::from(byte_size)) else {
            return Vec::new();
        };
        cursor = next;
    }
    offsets
}

/// The byte span one selected instruction occupies in the placed fragment,
/// rebased to absolute object `.text`, or none.
fn fragment_instruction_span(
    fragment: &machine_code::PlacedFunctionFragment,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(u64, u64)> {
    fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction)
        .map(|span| (span.section_offset, span.byte_count))
}

/// The producing call one scalar-call-result `Home` names: exactly one
/// internal or foreign call roster row claims the home's defining operation,
/// a foreign producer must record exactly this home as its result, and the
/// producer instruction defines the home's source value as its own
/// instruction result.
fn fragment_scalar_call_result_producer(
    function: &selected_instructions::SelectedFunction,
    home: &target_operations::TargetUnitScalarHomeRequirement,
) -> bool {
    let mut internal = function
        .calls
        .iter()
        .filter(|contract| contract.operation == home.defining_operation);
    let mut foreign = function
        .normalized_foreign_calls
        .iter()
        .filter(|record| record.operation == home.defining_operation);
    let (producer, result_home) = match (internal.next(), foreign.next()) {
        (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
        (None, Some(record)) if foreign.next().is_none() => {
            (record.instruction, Some(record.call.result_home.as_ref()))
        }
        _ => return false,
    };
    if let Some(recorded) = result_home
        && recorded != Some(home)
    {
        return false;
    }
    let Some(producer) = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.id == producer)
    else {
        return false;
    };
    producer.operands.iter().any(|operand| {
        function
            .virtual_registers
            .get(operand.virtual_register.0 as usize)
            .is_some_and(|register| {
                register.origin
                    == (selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: producer.id,
                        source_value: home.source_value,
                    })
            })
    })
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

/// Rejoin the expected structural-argument custody of one normalized foreign
/// Terminal boundary call against the observed evaluated-plan parameter
/// placements.
///
/// Returns `Ok(Some(_))` — one pointer-width signature shape per declared
/// structural formal — only for the exact source-rooted borrowed flat-record
/// lane the lowering owns: every argument carries a nonempty field-only path
/// rooted at a caller structural parameter, matches its declared formal's
/// position and access, that formal is borrowed, unrestricted, and
/// unqualified, and the observed plan places each formal's referent pointer
/// as exactly one pointer-width word. `Ok(None)` names a valid signature
/// shape this lane cannot prove — a callback-bearing signature, an owned or
/// qualified formal, or an argument
/// that is not such a projection — so the artifact retains no complete
/// physical evidence rather than claiming an unprovable custody join.
/// `Err` names contradictory retained custody: an argument count, declared
/// formal position, or access that changed between declaration and call, or
/// an observed plan whose parameter rows no longer realize each formal's
/// pointer word.
#[allow(clippy::too_many_arguments)]
pub(crate) fn normalized_foreign_structural_parameter_shapes(
    structural_arguments: &[terminal_psi::StructuralArgument],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    caller_structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    parameter_order: &[terminal_psi::BoundaryParameterKind],
    callback_present: bool,
    plan_parameters: &[calling_conventions::ValuePlacement],
    pointer_shape: calling_conventions::ValueShape,
) -> Result<Option<Vec<calling_conventions::ValueShape>>, &'static str> {
    if structural_arguments.len() != structural_parameters.len() {
        return Err("normalized foreign D41 child changed its structural call occurrence");
    }
    if structural_arguments.is_empty() {
        return Ok(Some(Vec::new()));
    }
    // Callback address custody is a separate native-only slot, not a
    // structural formal. That combination is not yet covered by this lane.
    if callback_present {
        return Ok(None);
    }
    let structural_positions = parameter_order
        .iter()
        .enumerate()
        .filter_map(|(position, kind)| {
            (*kind == terminal_psi::BoundaryParameterKind::Structural).then_some(position)
        });
    if plan_parameters.len() != parameter_order.len()
        || structural_positions.clone().count() != structural_arguments.len()
    {
        return Err("normalized foreign D41 child changed its structural call custody");
    }
    let mut shapes = Vec::with_capacity(structural_arguments.len());
    for (lane_position, ((argument, parameter), native_position)) in structural_arguments
        .iter()
        .zip(structural_parameters)
        .zip(structural_positions)
        .enumerate()
    {
        let placement = &plan_parameters[native_position];
        if usize::try_from(parameter.position).ok() != Some(lane_position)
            || argument.access != parameter.access
        {
            return Err("normalized foreign D41 child changed its structural call occurrence");
        }
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
            || !caller_structural_parameters
                .iter()
                .any(|source| source.place == argument.place)
        {
            return Ok(None);
        }
        let placed_pointer_word = match placement.locations.as_slice() {
            [
                calling_conventions::ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ]
            | [
                calling_conventions::ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] => *byte_size,
            _ => {
                return Err("normalized foreign D41 child changed its structural call custody");
            }
        };
        if placement.shape != pointer_shape || placed_pointer_word != pointer_shape.byte_size {
            return Err("normalized foreign D41 child changed its structural call custody");
        }
        shapes.push(pointer_shape);
    }
    Ok(Some(shapes))
}
