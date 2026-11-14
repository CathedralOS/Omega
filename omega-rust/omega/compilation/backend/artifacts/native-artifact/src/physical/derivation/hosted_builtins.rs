/*
We derive hosted builtin physical children from our closed settlement catalog,
verifying that each installed settlement binds its declared scalar argument and
result custody shapes and reproduces identical instruction bytes across machine,
object, and final-image custody.
*/

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::CompilerBuiltinResult;
use crate::CompilerBuiltinScalarArgument;
use crate::NativeCompilerBuiltinCatalogIdentity;
use crate::NativePhysicalChild;
use crate::NativePhysicalOccurrence;
use crate::NativeSelectedProviderPlanDigest;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::boundary_physical_child_parts;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::evidence::text_relocation_overlaps;
use crate::physical::derivation::settlement_identity::hosted_builtin_settlement_identity;
use crate::physical::native_byte_span;
use machine_code::BoundaryExecutionRecord;
use machine_code::InternalUnitScalarArgumentSourceRecord;
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::ScalarType;
use target::Architecture;
use target::NativeTarget;
use target_operations::BoundaryRealization;
use target_operations::CompilerBuiltinExecution;
use target_operations::HostedExitProcessI32Realization;
use target_operations::HostedReadByteRealization;
use target_operations::HostedWriteByteI32Realization;
use target_operations::MachineRegister;

enum HostedScalarArgumentForm {
    Absent,
    Immediate {
        scalar: fn() -> ScalarType,
        destination: fn(Architecture) -> MachineRegister,
    },
    RuntimeScalar {
        source: Option<fn(&InternalUnitScalarArgumentSourceRecord) -> bool>,
    },
}

enum HostedResultForm {
    Unit,
    Structural {
        bind: fn(
            crate::physical::PhysicalEvidencePass,
            &machine_code::BoundaryStructuralResultRecord,
            &OptimizedBoundaryOccurrence,
            NativeTarget,
        ) -> Result<Option<Vec<u8>>, &'static str>,
    },
}

pub(crate) struct HostedBuiltinSettlement {
    execution: CompilerBuiltinExecution,
    realization: BoundaryRealization,
    supports_target: fn(NativeTarget) -> bool,
    scalar_forms: &'static [HostedScalarArgumentForm],
    result: HostedResultForm,
    identity_tags: [u8; 2],
}

const HOSTED_BUILTIN_SETTLEMENTS: &[HostedBuiltinSettlement] = &[
    HostedBuiltinSettlement {
        execution: CompilerBuiltinExecution::HostedExitProcessI32,
        realization: BoundaryRealization::HostedExitProcessI32(HostedExitProcessI32Realization),
        supports_target: HostedExitProcessI32Realization::supports_target,
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
        realization: BoundaryRealization::HostedWriteByteI32(HostedWriteByteI32Realization),
        supports_target: HostedWriteByteI32Realization::supports_target,
        scalar_forms: &[HostedScalarArgumentForm::RuntimeScalar { source: None }],
        result: HostedResultForm::Unit,
        identity_tags: [2, 2],
    },
    HostedBuiltinSettlement {
        execution: CompilerBuiltinExecution::HostedReadByte,
        realization: BoundaryRealization::HostedReadByte(HostedReadByteRealization),
        supports_target: HostedReadByteRealization::supports_target,
        scalar_forms: &[HostedScalarArgumentForm::Absent],
        result: HostedResultForm::Structural {
            bind: read_byte_result_bytes,
        },
        identity_tags: [3, 3],
    },
];

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

fn process_exit_argument_destination(architecture: Architecture) -> MachineRegister {
    match architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    }
}

fn selected_process_exit_i32_source(source: &InternalUnitScalarArgumentSourceRecord) -> bool {
    matches!(
        source,
        InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            scalar_type,
            ..
        } if *scalar_type == i32_scalar()
    )
}

fn read_byte_result_bytes(
    pass: crate::physical::PhysicalEvidencePass,
    result: &machine_code::BoundaryStructuralResultRecord,
    occurrence: &OptimizedBoundaryOccurrence,
    target: NativeTarget,
) -> Result<Option<Vec<u8>>, &'static str> {
    if Some(result.defining_operation) != occurrence.operation()
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
        .checked_add(result.layout.payload_byte_offset)
        .ok_or("hosted read-byte physical child result home overflow")?;
    if !pass.is_receiver() {
        return Ok(None);
    }
    match target.architecture {
        Architecture::X86_64 if target == NativeTarget::macos_x64() => {
            isa_x86_64::encode_darwin_read_byte_to_stack(result.home_byte_offset, payload_offset)
                .map_err(|_| "macOS read-byte x86-64 encoding is not reproducible")
        }
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
    .map(Some)
}

fn signed_immediate_fits_declared(immediate: IntegerValue, scalar_type: ScalarType) -> bool {
    let (ScalarType::Integer(integer), IntegerValue::Signed(value)) = (scalar_type, immediate)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_hosted_builtin_child(
    pass: crate::physical::PhysicalEvidencePass,
    builtin: &HostedBuiltinSettlement,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
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
                bind(pass, result, occurrence, target)?,
            )
        }
    };
    if settlement.byte_count == 0 {
        return Err("hosted builtin physical child requires a nonempty emitted span");
    }
    let function = object
        .function_on_machine(occurrence.machine())
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
    let final_image_bytes = span(image, final_image_span)?;
    if pass.is_receiver()
        && (machine_bytes != object_bytes
            || object_bytes != final_image_bytes
            || expected.is_some_and(|expected| machine_bytes != expected.as_slice()))
    {
        return Err("hosted builtin physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("hosted builtin physical child relocation span overflow")?;
    if text_relocation_overlaps(object, (installed.text_offset, object_end)) {
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
    Ok(boundary_physical_child_parts(
        parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes,
        object_bytes,
        final_image_bytes,
        PhysicalRelocationDisposition::DirectInstructionBytes,
    )
    .into())
}
