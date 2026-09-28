//! Derive hosted builtin physical evidence from the closed settlement catalog.

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::CompilerBuiltinResult;
use crate::CompilerBuiltinScalarArgument;
use crate::NativeCompilerBuiltinCatalogIdentity;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NativeSelectedProviderPlanDigest;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::physical_child_identity;
use crate::physical::derivation::evidence::ranges_overlap;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::hashing::sha256;
use crate::physical::derivation::settlement_identity::hosted_builtin_settlement_identity;
use crate::physical::native_byte_span;
use machine_code::BoundaryExecutionRecord;
use object_file::SectionKind;
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::ScalarType;
use target::Architecture;
use target::NativeTarget;
use target_operations::BoundaryRealization;
use target_operations::CompilerBuiltinExecution;

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
        &relocation,
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
