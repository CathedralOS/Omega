//! Authenticated one-field mutation coverage for the nested evidence rows
//! retained inside each `InstalledFunction`.
//!
//! The landed `installation_function_row_rejects_every_one_field_substitution`
//! covers the row's own axes (`machine`, `attachment`, text interval, stack
//! domains, `unit_body`). The nested rosters beneath it — call stacks,
//! parameter and scalar homes, integer constants, affine-record and store
//! custody, continuation and cleanup evidence, and the scalar-transport ABI
//! fields — each join canonical record shape against the rest of the
//! installation before the row is compared byte-for-byte to the emitted
//! image. Every representable substitution therefore lands in one of two
//! places: canonical encoding rejects it, or it still encodes, recomputes a
//! distinct installation fingerprint, and independent replay against the
//! unchanged image rejects it with `ImageBindingMismatch`.

use super::{
    WriteExitProvider, continuation_unit_call_plan, edge_id, edge_owned_cleanup_plan, identity,
    machine_id, operation_id, promote_x86_cleanup_to_scalar, scalar_three_leaf_cleanup_plan,
    stored_dynamic_call_plan, structural_call_scalar_return_plan, two_function_plan,
    windows_foreign_call_plan,
};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use image_emission::{
    InstallationError, InstallationRecord, InstalledFunction, ObjectCodeAttribution,
    ObjectUnitStack, build_installation_record,
    build_installation_record_with_selected_provider_plans_and_evidence, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use machine_code::{
    MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, StructuralCallScalarReturnEvidence, UnitAffineCleanupRecord,
    UnitAffineScalarRecordEstablishmentRecord, UnitContinuationRecord, UnitIntegerConstantRecord,
    UnitParameterHomeRecord, UnitParameterRecord, UnitStackEvidence,
};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, PlaceId, ProfileDecisionId, ScalarType,
    StructuralDomainId, StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{CallSiteOwner, ScalarAbiValue, TerminalPsiProvenance};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
};

fn i32_scalar() -> ScalarType {
    ScalarType::Integer(i32_integer())
}

fn i32_integer() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("value")
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).expect("place")
}

fn structural_type(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).expect("structural type")
}

fn field_id(raw: u64) -> StructuralFieldId {
    StructuralFieldId::new(raw).expect("field")
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).expect("block")
}

fn domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).expect("domain")
}

fn empty_placement() -> ValuePlacement {
    ValuePlacement {
        shape: ValueShape::integer(0, 1),
        locations: Vec::new(),
    }
}

fn register_placement() -> ValuePlacement {
    ValuePlacement {
        shape: ValueShape::integer(4, 4),
        locations: vec![ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    }
}

/// A canonical installed scalar ABI: parameter and result placements recompute
/// the exact native call plan, and every semantic value identity is distinct.
fn canonical_scalar_abi() -> target_operations::ScalarFunctionAbi {
    let shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("scalar call plan");
    target_operations::ScalarFunctionAbi {
        parameters: vec![ScalarAbiValue {
            value: value_id(41),
            scalar_type: i32_scalar(),
            placement: call_plan.parameters[0].clone(),
        }],
        result: ScalarAbiValue {
            value: value_id(43),
            scalar_type: i32_scalar(),
            placement: call_plan.result.clone().expect("scalar result"),
        },
        call_plan,
    }
}

/// A canonical Unit-returning parameter ABI without entry spills; continuations
/// remain the only axis that could retain them.
fn canonical_parameter_abi() -> machine_code::ParameterFunctionAbiRecord {
    let shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .expect("parameter call plan");
    machine_code::ParameterFunctionAbiRecord {
        parameters: vec![ScalarAbiValue {
            value: value_id(45),
            scalar_type: i32_scalar(),
            placement: call_plan.parameters[0].clone(),
        }],
        call_plan,
        entry_register_spills: Vec::new(),
    }
}

/// A mixed scalar/structural ABI whose structural row is joined exactly to the
/// scalar-side structural parameter and home the promoted fixture retains.
fn matching_mixed_abi(
    function: &InstalledFunction,
) -> target_operations::MixedStructuralScalarFunctionAbi {
    let retained = function.scalar_structural_parameters[0];
    let retained_home = function.scalar_structural_parameter_homes[0].clone();
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![retained.shape],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("mixed call plan");
    target_operations::MixedStructuralScalarFunctionAbi {
        scalar_parameters: Vec::new(),
        structural_parameters: vec![target_operations::TargetStructuralParameter {
            place: retained.place,
            structural_type: retained.structural_type,
            multiplicity: retained.multiplicity,
            access: retained.access,
            projected_qualifications: Vec::new(),
            shape: retained.shape,
            placement: retained_home.source.clone(),
        }],
        result: ScalarAbiValue {
            value: value_id(49),
            scalar_type: i32_scalar(),
            placement: call_plan.result.clone().expect("mixed result"),
        },
        call_plan,
    }
}

/// The exact mixed ABI a promoted scalar-cleanup row can retain: one i32
/// scalar parameter on the canonical plan's leading placement, the retained
/// structural parameter joined field-for-field to its roster row and home
/// source, and one i32 result placement.
fn retained_mixed_abi(
    function: &machine_code::MachineCodeFunction,
) -> target_operations::MixedStructuralScalarFunctionAbi {
    let retained = function.scalar_structural_parameters[0];
    let retained_home = &function.scalar_structural_parameter_homes[0];
    let i32_shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![i32_shape, retained.shape],
            result: Some(i32_shape),
        },
    )
    .expect("mixed call plan");
    target_operations::MixedStructuralScalarFunctionAbi {
        scalar_parameters: vec![ScalarAbiValue {
            value: value_id(51),
            scalar_type: i32_scalar(),
            placement: call_plan.parameters[0].clone(),
        }],
        structural_parameters: vec![target_operations::TargetStructuralParameter {
            place: retained.place,
            structural_type: retained.structural_type,
            multiplicity: retained.multiplicity,
            access: retained.access,
            projected_qualifications: Vec::new(),
            shape: retained.shape,
            placement: retained_home.source.clone(),
        }],
        result: ScalarAbiValue {
            value: value_id(49),
            scalar_type: i32_scalar(),
            placement: call_plan.result.clone().expect("mixed result"),
        },
        call_plan,
    }
}

/// A mixed ABI joined to no retained structural roster: the fabricated
/// structural parameter cannot be bound to any declared parameter/home pair.
fn fabricated_mixed_abi() -> target_operations::MixedStructuralScalarFunctionAbi {
    let shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("mixed call plan");
    target_operations::MixedStructuralScalarFunctionAbi {
        scalar_parameters: Vec::new(),
        structural_parameters: vec![target_operations::TargetStructuralParameter {
            place: place_id(9),
            structural_type: structural_type(9),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            projected_qualifications: Vec::new(),
            shape,
            placement: register_placement(),
        }],
        result: ScalarAbiValue {
            value: value_id(49),
            scalar_type: i32_scalar(),
            placement: call_plan.result.clone().expect("mixed result"),
        },
        call_plan,
    }
}

fn foreign_call(text_offset: usize) -> image_emission::InstalledForeignCallStack {
    image_emission::InstalledForeignCallStack {
        owner: CallSiteOwner::Operation(operation_id(5)),
        text_offset,
        caller_live_bytes: 8,
        provider_plan_report_identity: 91,
        contribution_report_identity:
            task_plans::AdmittedStackContributionReportId::from_normalized_identity(7)
                .expect("contribution report"),
        contribution_commitment: task_plans::SameStackContributionCommitment::from_digest([3; 32]),
        contribution_bytes: 16,
        contribution_alignment: 8,
    }
}

fn integer_constant() -> UnitIntegerConstantRecord {
    UnitIntegerConstantRecord {
        defining_operation: operation_id(9),
        source_value: value_id(57),
        scalar_type: i32_integer(),
        value: IntegerValue::Signed(11),
        operation_ordinal: 4,
    }
}

fn affine_scalar_record() -> UnitAffineScalarRecordEstablishmentRecord {
    UnitAffineScalarRecordEstablishmentRecord {
        psi_operation: operation_id(9),
        result: terminal_psi::StructuralOperationResult {
            place: place_id(9),
            structural_type: structural_type(9),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        field: field_id(1),
        value: IntegerValue::Signed(3),
        shape: ValueShape::integer(4, 4),
        operation_ordinal: 4,
    }
}

fn structural_field_store() -> machine_code::UnitStructuralScalarFieldStoreRecord {
    machine_code::UnitStructuralScalarFieldStoreRecord {
        psi_operation: operation_id(9),
        destination: terminal_psi::StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: structural_type(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        path: vec![StructuralPathSegment::Field("field".to_string())],
        field: field_id(1),
        destination_placement: empty_placement(),
        field_byte_offset: 0,
        source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: operation_id(8),
            source_value: value_id(55),
            scalar_type: i32_integer(),
            value: IntegerValue::Signed(3),
        },
        parameter_home_byte_offset: 0,
        parameter_home_indirect: false,
        operation_ordinal: 4,
        code_offset: 0,
        byte_count: 1,
        bytes: vec![0x90],
    }
}

fn write_only_store() -> machine_code::UnitWriteOnlyPrimitiveStoreRecord {
    machine_code::UnitWriteOnlyPrimitiveStoreRecord {
        psi_operation: operation_id(9),
        destination: terminal_psi::StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: structural_type(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        destination_type: terminal_psi::StructuralTypeDeclaration {
            id: structural_type(1),
            identity: "store.type".to_string(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(i32_scalar()),
        },
        destination_placement: empty_placement(),
        source: machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
            defining_operation: operation_id(8),
            source_value: value_id(55),
            scalar_type: i32_integer(),
            value: IntegerValue::Signed(3),
        },
        parameter_home_byte_offset: 0,
        parameter_home_indirect: false,
        operation_ordinal: 4,
        code_offset: 0,
        byte_count: 1,
        bytes: vec![0x90],
    }
}

fn scalar_field_store() -> machine_code::ScalarStructuralScalarFieldStoreRecord {
    machine_code::ScalarStructuralScalarFieldStoreRecord {
        psi_operation: operation_id(9),
        destination: terminal_psi::StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: true,
            structural_type: structural_type(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        path: vec![StructuralPathSegment::Field("field".to_string())],
        field: field_id(1),
        destination_placement: register_placement(),
        field_byte_offset: 0,
        defining_operation: operation_id(8),
        source_value: value_id(55),
        immediate: target_operations::TargetScalarImmediate::Integer {
            scalar_type: i32_integer(),
            value: IntegerValue::Signed(3),
        },
        return_operation: operation_id(10),
        return_source_value: value_id(56),
        return_field: field_id(2),
        return_field_byte_offset: 0,
        return_scalar_type: i32_scalar(),
        operation_ordinal: 4,
        code_offset: 0,
        byte_count: 1,
        bytes: vec![0x90],
    }
}

fn structural_scalar_return() -> StructuralCallScalarReturnEvidence {
    StructuralCallScalarReturnEvidence {
        psi_edge: edge_id(3),
        psi_operation: operation_id(4),
        source_value: value_id(80),
        scalar_type: i32_scalar(),
        callee: machine_id(2),
    }
}

/// An attached Unit entry function whose two affine scalar-record
/// establishments each feed a later owned internal Unit call. Every record
/// names the parameter place its result settles into; the consuming call's
/// argument materializes the record's signed immediate into the parameter
/// register, so each record field is joined to the parameter and home
/// declarations, the semantic code attribution, the call custody, and the
/// closing edge cleanup.
fn affine_scalar_record_custody_plan() -> MachineCodePlan {
    let argument_shape = ValueShape::integer(8, 8);
    let empty_source = ValuePlacement {
        shape: argument_shape,
        locations: Vec::new(),
    };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![argument_shape],
            result: None,
        },
    )
    .expect("affine scalar call plan");
    let result = |place: u64, kind: u64| terminal_psi::StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type(kind),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let record = |operation: u64, ordinal: usize, place: u64, kind: u64, field: u64, value: i64| {
        UnitAffineScalarRecordEstablishmentRecord {
            psi_operation: operation_id(operation),
            result: result(place, kind),
            field: field_id(field),
            value: IntegerValue::Signed(i128::from(value)),
            shape: argument_shape,
            operation_ordinal: ordinal,
        }
    };
    // The argument transfer materializes the record's signed immediate into
    // the first parameter register; the record itself is the zero-byte
    // establishment its consuming argument is joined to.
    let argument = |place: u64, kind: u64, stack_offset: u32, code_offset: usize, value: i64| {
        machine_code::InternalUnitCallArgumentRecord {
            place: place_id(place),
            access: StructuralAccess::Owned,
            path: Vec::new(),
            root_structural_type: structural_type(kind),
            structural_type: structural_type(kind),
            shape: argument_shape,
            source_byte_offset: 0,
            source_location: machine_code::StructuralSourceLocation::Stack {
                byte_offset: stack_offset,
            },
            call_stack_bytes: 8,
            fixed_array_length: None,
            element_stride: None,
            source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                empty_source.clone(),
            ),
            destination: call_plan.parameters[0].clone(),
            code_offset,
            byte_count: 10,
            bytes: [0x48, 0xbf]
                .into_iter()
                .chain(value.to_le_bytes())
                .collect(),
        }
    };
    let unit_call = |operation: u64,
                     ordinal: usize,
                     code_offset: usize,
                     argument: machine_code::InternalUnitCallArgumentRecord| {
        machine_code::InternalUnitCallRecord {
            source: machine_code::InternalUnitCallSource::Authored,
            owner: CallSiteOwner::Operation(operation_id(operation)),
            target: machine_id(2),
            result: None,
            semantic_result: None,
            structural_result: None,
            scalar_arguments: Vec::new(),
            arguments: vec![argument],
            claim_transfers: Vec::new(),
            operation_ordinal: ordinal,
            code_offset,
            byte_count: 23,
        }
    };
    let relocation = |operation: u64, allocation: usize, release: usize, offset: usize| {
        machine_code::InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation_id(operation)),
            target: machine_id(2),
            unit_stack: Some(machine_code::UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: allocation,
                    allocation_byte_count: 4,
                    release_offset: release,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset,
        }
    };
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: vec![
                    record(1, 0, 1, 1, 1, 5),
                    record(2, 1, 2, 2, 2, -3),
                ],
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(1),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![
                        operation_id(1),
                        operation_id(2),
                        operation_id(3),
                        operation_id(4),
                    ],
                    edges: vec![edge_id(1)],
                },
                bytes: vec![
                    0x48, 0x83, 0xec, 0x08, // sub rsp, 8
                    0x48, 0xbf, 5, 0, 0, 0, 0, 0, 0, 0, // movabs rdi, 5
                    0xe8, 0, 0, 0, 0, // call machine 2
                    0x48, 0x83, 0xc4, 0x08, // add rsp, 8
                    0x48, 0x83, 0xec, 0x08, // sub rsp, 8
                    0x48, 0xbf, 0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, // movabs rdi, -3
                    0xe8, 0, 0, 0, 0, // call machine 2
                    0x48, 0x83, 0xc4, 0x08, // add rsp, 8
                    0xc3, // ret
                ],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: vec![
                    UnitParameterHomeRecord {
                        place: place_id(1),
                        structural_type: structural_type(1),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        shape: argument_shape,
                        source: empty_source.clone(),
                        location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                        indirect: false,
                    },
                    UnitParameterHomeRecord {
                        place: place_id(2),
                        structural_type: structural_type(2),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        shape: argument_shape,
                        source: empty_source.clone(),
                        location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                        indirect: false,
                    },
                ],
                unit_parameters: vec![
                    UnitParameterRecord {
                        place: place_id(1),
                        structural_type: structural_type(1),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        shape: argument_shape,
                    },
                    UnitParameterRecord {
                        place: place_id(2),
                        structural_type: structural_type(2),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        shape: argument_shape,
                    },
                ],
                scalar_stack: None,
                internal_calls: vec![relocation(3, 0, 19, 15), relocation(4, 23, 42, 38)],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![
                    unit_call(3, 2, 0, argument(1, 1, 0, 4, 5)),
                    unit_call(4, 3, 23, argument(2, 2, 0, 27, -3)),
                ],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(1),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 46,
                    byte_count: 1,
                }),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 4,
                        byte_count: 0,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(2)),
                        operation_ordinal: 1,
                        code_offset: 27,
                        byte_count: 0,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(3)),
                        operation_ordinal: 2,
                        code_offset: 0,
                        byte_count: 23,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(4)),
                        operation_ordinal: 3,
                        code_offset: 23,
                        byte_count: 23,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 4,
                        code_offset: 46,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(2),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(2),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 0,
                    byte_count: 1,
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(2)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

/// The retained record custody of one attached Unit entry function holding
/// two fixed-width immediate stores into one write-only-borrowed structural
/// parameter's staged stack home. Each store joins its destination
/// declaration to the parameter and home rosters, its immediate to an earlier
/// integer-constant row, and its bounded path, field, home offsets, ordering,
/// and exact emitted bytes to the semantic attribution. The shared home
/// source uses a decodable stack placement so the retained record round-trips
/// through the installation codec.
///
/// This is a fabrication source, not an emitted plan: image emission requires
/// common-pipeline replay evidence for borrowed structural destinations, so
/// no `build_object_artifact` artifact can carry these rows into an image.
/// The test below stages the same rows on an appended `InstalledFunction`
/// and authenticates them against the canonical record shape.
fn structural_store_custody_plan() -> MachineCodePlan {
    let parameter_shape = ValueShape::integer(8, 8);
    let home_source = ValuePlacement {
        shape: parameter_shape,
        locations: vec![ValueLocation::Stack {
            stack_byte_offset: 4,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }],
    };
    let destination = || terminal_psi::StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let constant =
        |operation: u64, source: u64, value: i64, ordinal: usize| UnitIntegerConstantRecord {
            defining_operation: operation_id(operation),
            source_value: value_id(source),
            scalar_type: i32_integer(),
            value: IntegerValue::Signed(i128::from(value)),
            operation_ordinal: ordinal,
        };
    // movabs r11, <imm64>; mov dword ptr [rsp + stack_offset], r11d — the
    // exact x86-64 emission `expected_store_bytes` regenerates for a
    // non-indirect staged stack home.
    let store_bytes = |value: u64, stack_offset: u8| {
        [0x49, 0xbb]
            .into_iter()
            .chain(value.to_le_bytes())
            .chain([0x44, 0x89, 0x5c, 0x24, stack_offset])
            .collect::<Vec<u8>>()
    };
    let store = |operation: u64,
                 path: Vec<StructuralPathSegment>,
                 field: u64,
                 field_byte_offset: u32,
                 defining_operation: u64,
                 source_value: u64,
                 value: i64,
                 ordinal: usize,
                 code_offset: usize,
                 bytes: Vec<u8>| {
        machine_code::UnitStructuralScalarFieldStoreRecord {
            psi_operation: operation_id(operation),
            destination: destination(),
            path,
            field: field_id(field),
            destination_placement: home_source.clone(),
            field_byte_offset,
            source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                defining_operation: operation_id(defining_operation),
                source_value: value_id(source_value),
                scalar_type: i32_integer(),
                value: IntegerValue::Signed(i128::from(value)),
            },
            parameter_home_byte_offset: 4,
            parameter_home_indirect: false,
            operation_ordinal: ordinal,
            code_offset,
            byte_count: bytes.len(),
            bytes,
        }
    };
    let store_one = store(
        3,
        vec![StructuralPathSegment::Field("cell".to_string())],
        1,
        0,
        1,
        51,
        7,
        1,
        0,
        store_bytes(7, 4),
    );
    let store_two = store(
        4,
        vec![
            StructuralPathSegment::Field("slot".to_string()),
            StructuralPathSegment::FixedIndex(0),
        ],
        2,
        4,
        2,
        52,
        9,
        3,
        15,
        store_bytes(9, 8),
    );
    let function_bytes = store_one
        .bytes
        .iter()
        .chain(&store_two.bytes)
        .copied()
        .chain([0xc3])
        .collect::<Vec<u8>>();
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![MachineCodeFunction {
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            parameter_abi: None,
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            stored_dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: vec![constant(1, 51, 7, 0), constant(2, 52, 9, 2)],
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: vec![store_one, store_two],
            unit_write_only_primitive_stores: Vec::new(),
            scalar_structural_scalar_field_stores: Vec::new(),
            machine: machine_id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    operation_id(1),
                    operation_id(2),
                    operation_id(3),
                    operation_id(4),
                ],
                edges: vec![edge_id(1)],
            },
            bytes: function_bytes,
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: vec![UnitParameterHomeRecord {
                place: place_id(1),
                structural_type: structural_type(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                shape: parameter_shape,
                source: home_source,
                location: machine_code::StructuralSourceLocation::Stack { byte_offset: 4 },
                indirect: false,
            }],
            unit_parameters: vec![UnitParameterRecord {
                place: place_id(1),
                structural_type: structural_type(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                shape: parameter_shape,
            }],
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                structural_types: Vec::new().into(),
                psi_edge: edge_id(1),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: 30,
                byte_count: 1,
            }),
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(1)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(3)),
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: 15,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(2)),
                    operation_ordinal: 2,
                    code_offset: 15,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(4)),
                    operation_ordinal: 3,
                    code_offset: 15,
                    byte_count: 15,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(1)),
                    operation_ordinal: 4,
                    code_offset: 30,
                    byte_count: 1,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    }
}

/// The retained record custody of one attached Unit entry function holding
/// two fixed-width immediate whole-root stores into one write-only-borrowed
/// structural parameter's indirect stack home. Each store joins its
/// destination declaration and declared primitive type to the parameter, the
/// parameter home, and the cleanup's structural-type catalog, joins its
/// immediate to an earlier integer-constant row, and binds its ordering,
/// home indirection, and exact emitted bytes to the semantic attribution.
/// The home's borrowed-reference source placement is a single decodable
/// `Indirect` location so the retained record round-trips through the
/// installation codec.
///
/// This is a fabrication source, not an emitted plan: image emission
/// requires common-pipeline replay evidence for borrowed structural
/// destinations, so no `build_object_artifact` artifact can carry these rows
/// into an image. The test below stages the same rows on an appended
/// `InstalledFunction` and authenticates them against the canonical record
/// shape.
fn write_only_store_custody_plan() -> MachineCodePlan {
    let parameter_shape = ValueShape::borrowed_reference(4, 4);
    let home_source = ValuePlacement {
        shape: parameter_shape,
        locations: vec![ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Stack {
                stack_byte_offset: 4,
                alignment: 8,
            },
            copy_stack_byte_offset: None,
            byte_size: 4,
            alignment: 4,
        }],
    };
    let destination = || terminal_psi::StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let destination_type = || terminal_psi::StructuralTypeDeclaration {
        id: structural_type(1),
        identity: "store.type".to_string(),
        shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(i32_scalar()),
    };
    let constant =
        |operation: u64, source: u64, value: i64, ordinal: usize| UnitIntegerConstantRecord {
            defining_operation: operation_id(operation),
            source_value: value_id(source),
            scalar_type: i32_integer(),
            value: IntegerValue::Signed(i128::from(value)),
            operation_ordinal: ordinal,
        };
    // movabs r11, <imm64>; mov r10, [rsp + 4]; mov dword ptr [r10], r11d —
    // the exact x86-64 emission `expected_store_bytes` regenerates for an
    // indirect staged stack home.
    let store_bytes = |value: u64| {
        [0x49, 0xbb]
            .into_iter()
            .chain(value.to_le_bytes())
            .chain([0x4c, 0x8b, 0x54, 0x24, 0x04, 0x45, 0x89, 0x1a])
            .collect::<Vec<u8>>()
    };
    let store = |operation: u64,
                 defining_operation: u64,
                 source_value: u64,
                 value: i64,
                 ordinal: usize,
                 code_offset: usize,
                 bytes: Vec<u8>| {
        machine_code::UnitWriteOnlyPrimitiveStoreRecord {
            psi_operation: operation_id(operation),
            destination: destination(),
            destination_type: destination_type(),
            destination_placement: home_source.clone(),
            source: machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                defining_operation: operation_id(defining_operation),
                source_value: value_id(source_value),
                scalar_type: i32_integer(),
                value: IntegerValue::Signed(i128::from(value)),
            },
            parameter_home_byte_offset: 4,
            parameter_home_indirect: true,
            operation_ordinal: ordinal,
            code_offset,
            byte_count: bytes.len(),
            bytes,
        }
    };
    let store_one = store(3, 1, 51, 7, 1, 0, store_bytes(7));
    let store_two = store(4, 2, 52, 9, 3, 18, store_bytes(9));
    let function_bytes = store_one
        .bytes
        .iter()
        .chain(&store_two.bytes)
        .copied()
        .chain([0xc3])
        .collect::<Vec<u8>>();
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![MachineCodeFunction {
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            parameter_abi: None,
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            stored_dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: vec![constant(1, 51, 7, 0), constant(2, 52, 9, 2)],
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_write_only_primitive_stores: vec![store_one, store_two],
            scalar_structural_scalar_field_stores: Vec::new(),
            machine: machine_id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    operation_id(1),
                    operation_id(2),
                    operation_id(3),
                    operation_id(4),
                ],
                edges: vec![edge_id(1)],
            },
            bytes: function_bytes,
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: vec![UnitParameterHomeRecord {
                place: place_id(1),
                structural_type: structural_type(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                shape: parameter_shape,
                source: home_source,
                location: machine_code::StructuralSourceLocation::Stack { byte_offset: 4 },
                indirect: true,
            }],
            unit_parameters: vec![UnitParameterRecord {
                place: place_id(1),
                structural_type: structural_type(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::WriteOnlyBorrow,
                shape: parameter_shape,
            }],
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                structural_types: vec![destination_type()].into(),
                psi_edge: edge_id(1),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: 36,
                byte_count: 1,
            }),
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(1)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(3)),
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: 18,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(2)),
                    operation_ordinal: 2,
                    code_offset: 18,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(4)),
                    operation_ordinal: 3,
                    code_offset: 18,
                    byte_count: 18,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(1)),
                    operation_ordinal: 4,
                    code_offset: 36,
                    byte_count: 1,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    }
}

/// The retained record custody of one attached Unit entry function holding
/// two scalar-side structural field stores: a mutable-self store into the
/// function's own structural root with a signed immediate, plus the
/// return-field writeback identity. The roster carries no record-shape join
/// at all — the codec only bounds the roster and the path grammar — so every
/// representable field substitution still encodes, recomputes a distinct
/// installation identity, and is rejected only by independent replay against
/// the unchanged emitted image.
///
/// This is a fabrication source, not an emitted plan: image emission
/// requires common-pipeline replay evidence for scalar-side structural
/// stores, so no `build_object_artifact` artifact can carry these rows into
/// an image. The test below stages the same rows on an appended
/// `InstalledFunction` and authenticates them against the canonical record
/// shape.
fn scalar_store_custody_plan() -> MachineCodePlan {
    let store = |operation: u64, field: u64, ordinal: usize, code_offset: usize, bytes: Vec<u8>| {
        machine_code::ScalarStructuralScalarFieldStoreRecord {
            psi_operation: operation_id(operation),
            destination: terminal_psi::StructuralParameterDeclaration {
                place: place_id(1),
                position: 0,
                is_self: true,
                structural_type: structural_type(1),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            path: vec![StructuralPathSegment::Field("cell".to_string())],
            field: field_id(field),
            destination_placement: register_placement(),
            field_byte_offset: 0,
            defining_operation: operation_id(8),
            source_value: value_id(55),
            immediate: target_operations::TargetScalarImmediate::Integer {
                scalar_type: i32_integer(),
                value: IntegerValue::Signed(3),
            },
            return_operation: operation_id(10),
            return_source_value: value_id(56),
            return_field: field_id(2),
            return_field_byte_offset: 0,
            return_scalar_type: i32_scalar(),
            operation_ordinal: ordinal,
            code_offset,
            byte_count: bytes.len(),
            bytes,
        }
    };
    let store_one = store(3, 1, 0, 0, vec![0x90]);
    let store_two = store(4, 2, 1, 1, vec![0x90]);
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![MachineCodeFunction {
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            parameter_abi: None,
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            stored_dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_write_only_primitive_stores: Vec::new(),
            scalar_structural_scalar_field_stores: vec![store_one, store_two],
            machine: machine_id(1),
            attachment: Some(structural_type(1)),
            provenance: TerminalPsiProvenance {
                operations: vec![operation_id(3), operation_id(4), operation_id(10)],
                edges: vec![edge_id(1)],
            },
            bytes: vec![0x90, 0x90, 0xc3],
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                structural_types: Vec::new().into(),
                psi_edge: edge_id(1),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: 2,
                byte_count: 1,
            }),
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(3)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation_id(4)),
                    operation_ordinal: 1,
                    code_offset: 1,
                    byte_count: 1,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(1)),
                    operation_ordinal: 2,
                    code_offset: 2,
                    byte_count: 1,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    }
}

fn extra_type_catalog() -> abstract_operations::StructuralTypeCatalog {
    vec![terminal_psi::StructuralTypeDeclaration {
        id: structural_type(9),
        identity: "extra.type".to_string(),
        shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(i32_scalar()),
    }]
    .into()
}

fn extra_local() -> (
    semantic_vocabulary::OperationId,
    terminal_psi::StructuralPlaceDeclaration,
    terminal_psi::StructuralTypeDeclaration,
) {
    (
        operation_id(9),
        terminal_psi::StructuralPlaceDeclaration {
            id: place_id(9),
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
        terminal_psi::StructuralTypeDeclaration {
            id: structural_type(9),
            identity: "local.type".to_string(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(i32_scalar()),
        },
    )
}

/// Replay-side assertion for a substitution that still encodes: the codec must
/// preserve the mutated record exactly, the recomputed installation identity
/// must differ from the authentic fingerprint, and independent replay against
/// the unchanged emitted image must reject the row.
fn assert_substitution_rejected_by_replay(
    field: &str,
    record: &InstallationRecord,
    image: &image_emission::ExecutableImage,
    authentic_fingerprint: &image_emission::InstallationFingerprint,
    index: usize,
    mutate: impl Fn(&mut InstalledFunction),
) {
    let mut changed = record.clone();
    mutate(&mut changed.functions_mut_for_test()[index]);
    assert_ne!(changed, *record, "{field}: substitution changes the row");
    let bytes = encode_installation_record(&changed)
        .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
    let replayed = decode_installation_record(&bytes)
        .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
    assert_eq!(
        replayed, changed,
        "{field}: codec preserves the substituted row"
    );
    assert_ne!(
        installation_fingerprint(&replayed)
            .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
        *authentic_fingerprint,
        "{field}: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&replayed, image),
        Err(InstallationError::ImageBindingMismatch),
        "{field}: independent replay rejects the substituted row"
    );
}

/// Encode-side assertion for a substitution that is not independently
/// representable: a canonical record-shape join rejects it before any identity
/// or replay could accept it.
fn assert_substitution_rejected_at_encoding(
    field: &str,
    record: &InstallationRecord,
    index: usize,
    mutate: impl Fn(&mut InstalledFunction),
    expected: InstallationError,
) {
    let mut changed = record.clone();
    mutate(&mut changed.functions_mut_for_test()[index]);
    assert_ne!(changed, *record, "{field}: substitution changes the row");
    assert_eq!(
        encode_installation_record(&changed),
        Err(expected),
        "{field}: canonical encoding rejects the substitution"
    );
}

/// Encode-side analogue for a record-roster substitution that is not
/// independently representable: a canonical record-shape join rejects it
/// before any identity or replay could accept it.
fn assert_record_substitution_rejected_at_encoding(
    field: &str,
    record: &InstallationRecord,
    mutate: impl Fn(&mut InstallationRecord),
    expected: InstallationError,
) {
    let mut changed = record.clone();
    mutate(&mut changed);
    assert_ne!(changed, *record, "{field}: substitution changes the record");
    assert_eq!(
        encode_installation_record(&changed),
        Err(expected),
        "{field}: canonical encoding rejects the substitution"
    );
}

/// Replay-side assertion for the affine scalar-record fixture: the consuming
/// argument's source placement carries an 8-byte shape with no locations —
/// exactly what `affine_scalar_record_source` requires — and that placement
/// is not wire-decodable today, so the containing record cannot round-trip
/// through `decode_installation_record`. The substitution still encodes
/// canonically, recomputes a distinct installation identity, and independent
/// replay against the unchanged image rejects it.
fn assert_undecodable_row_substitution_rejected_by_replay(
    field: &str,
    record: &InstallationRecord,
    image: &image_emission::ExecutableImage,
    authentic_fingerprint: &image_emission::InstallationFingerprint,
    index: usize,
    mutate: impl Fn(&mut InstalledFunction),
) {
    let mut changed = record.clone();
    mutate(&mut changed.functions_mut_for_test()[index]);
    assert_ne!(changed, *record, "{field}: substitution changes the row");
    encode_installation_record(&changed)
        .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
    assert_ne!(
        installation_fingerprint(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
        *authentic_fingerprint,
        "{field}: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&changed, image),
        Err(InstallationError::ImageBindingMismatch),
        "{field}: independent replay rejects the substituted row"
    );
}

/// Record-roster analogue of
/// [`assert_undecodable_row_substitution_rejected_by_replay`].
fn assert_undecodable_record_substitution_rejected_by_replay(
    field: &str,
    record: &InstallationRecord,
    image: &image_emission::ExecutableImage,
    authentic_fingerprint: &image_emission::InstallationFingerprint,
    mutate: impl Fn(&mut InstallationRecord),
) {
    let mut changed = record.clone();
    mutate(&mut changed);
    assert_ne!(changed, *record, "{field}: substitution changes the record");
    encode_installation_record(&changed)
        .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
    assert_ne!(
        installation_fingerprint(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
        *authentic_fingerprint,
        "{field}: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&changed, image),
        Err(InstallationError::ImageBindingMismatch),
        "{field}: independent replay rejects the substituted record"
    );
}

/// Every representable leaf of an installed Unit call-stack row, its canonical
/// ordering, and the scalar/foreign call-stack projections are authenticated:
/// in-plan retargets and offset drift still encode and are rejected by
/// independent replay, while stack arithmetic, unknown targets, and ordering
/// violations are rejected at canonical encoding.
#[test]
fn installation_function_nested_call_stacks_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.unit_call_stacks.len(), 1);
    assert_eq!(
        authentic.unit_call_stacks[0].owner,
        CallSiteOwner::CleanupAction {
            edge: edge_id(3),
            action_ordinal: 0
        }
    );

    let tail_text_offset = authentic.text_offset + authentic.byte_count - 4;
    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "unit_call_stacks[0].owner",
            Box::new(|row| {
                row.unit_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }),
        ),
        (
            "unit_call_stacks[0].target",
            Box::new(|row| {
                row.unit_call_stacks[0].target = machine_id(2);
            }),
        ),
        (
            "unit_call_stacks[0].text_offset",
            Box::new(|row| {
                row.unit_call_stacks[0].text_offset += 1;
            }),
        ),
        (
            "unit_call_stacks::drop",
            Box::new(|row| {
                row.unit_call_stacks.pop();
            }),
        ),
        (
            "unit_call_stacks::insert-distinct",
            Box::new(move |row| {
                row.unit_call_stacks
                    .push(image_emission::ObjectUnitCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: tail_text_offset,
                        active_frame_bytes: 32,
                        transient_bytes: 16,
                        caller_live_bytes: 48,
                    });
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_call_stacks[0].target::unknown-machine",
            Box::new(|row| {
                row.unit_call_stacks[0].target = machine_id(99);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_call_stacks[0].active_frame_bytes",
            Box::new(|row| {
                row.unit_call_stacks[0].active_frame_bytes += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_call_stacks[0].transient_bytes",
            Box::new(|row| {
                row.unit_call_stacks[0].transient_bytes += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_call_stacks[0].caller_live_bytes",
            Box::new(|row| {
                row.unit_call_stacks[0].caller_live_bytes += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_call_stacks::insert-duplicate",
            Box::new(|row| {
                let call = row.unit_call_stacks[0];
                row.unit_call_stacks.push(call);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // A scalar call-stack row cannot be attached to a Unit-stack row:
        // scalar calls require retained scalar stack evidence.
        (
            "scalar_call_stacks::insert-on-unit-row",
            Box::new(|row| {
                row.scalar_call_stacks
                    .push(image_emission::ObjectScalarCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: 4,
                        caller_live_bytes: 8,
                    });
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // Foreign calls require the provider-plan report identity to appear in
        // the retained selected-provider closure; the fixture selects none.
        (
            "foreign_call_stacks::insert-unselected",
            Box::new(|row| {
                row.foreign_call_stacks.push(foreign_call(4));
            }),
            InstallationError::ProviderSettlementClosureMismatch,
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }

    // With the report identity admitted by `selected_provider_plans`, the same
    // fabricated foreign row clears the closure join, encodes, and independent
    // replay still rejects it against the foreign-call-free image.
    let selected = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        ProfileDecisionId::new(41).expect("profile"),
        [91],
        std::iter::empty::<&dyn installation_evidence::ProviderExecutionEvidence>(),
        None,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
    .expect("selected provider installation");
    validate_installation_record(&selected, &image).expect("selected image binding");
    let selected_fingerprint = installation_fingerprint(&selected).expect("fingerprint");
    let function_text_offset = selected.functions()[2].text_offset;
    assert_substitution_rejected_by_replay(
        "foreign_call_stacks::insert-admitted",
        &selected,
        &image,
        &selected_fingerprint,
        2,
        move |row| {
            row.foreign_call_stacks
                .push(foreign_call(function_text_offset + 4));
        },
    );
}

/// Every retained field of a genuinely emitted foreign-call stack row is
/// authenticated: the object pipeline validates the call's exact site bytes,
/// evaluated plans, MXCSR custody, and same-stack contribution before the PE
/// image resolves its import relocation, so the installed row exists only
/// because the emitted image retains it. Each of the eight fields — owner,
/// text offset, caller-live bytes, provider-plan report identity,
/// contribution report identity, contribution commitment, contribution bytes,
/// and contribution alignment — is independently representable, recomputes a
/// distinct installation identity, and is rejected by independent replay
/// against the unchanged image. An unselected provider identity is rejected
/// at canonical encoding and a dropped row is rejected by replay.
#[test]
fn installation_foreign_call_stack_row_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(91);
    let plan = windows_foreign_call_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("foreign call object");
    let image = emit_executable_image(&artifact, 3).expect("foreign call PE image");
    assert_eq!(image.foreign_calls().len(), 1, "one emitted foreign call");
    // Select one unexecuted plan beside the executed provider's so the
    // report-identity field stays representable inside the closure.
    let record = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        ProfileDecisionId::new(41).expect("profile"),
        [91, 97],
        [&provider],
        None,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
    .expect("foreign call installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[0].clone();
    let [call] = authentic.foreign_call_stacks.as_slice() else {
        panic!("one installed foreign-call stack row")
    };
    assert_eq!(call.owner, CallSiteOwner::Operation(operation_id(61)));
    assert_eq!(call.provider_plan_report_identity, 91);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "foreign_call_stacks[0].owner",
            Box::new(|row| {
                row.foreign_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }),
        ),
        (
            "foreign_call_stacks[0].text_offset",
            Box::new(|row| {
                row.foreign_call_stacks[0].text_offset += 1;
            }),
        ),
        (
            "foreign_call_stacks[0].caller_live_bytes",
            Box::new(|row| {
                row.foreign_call_stacks[0].caller_live_bytes += 1;
            }),
        ),
        (
            "foreign_call_stacks[0].provider_plan_report_identity",
            Box::new(|row| {
                row.foreign_call_stacks[0].provider_plan_report_identity = 97;
            }),
        ),
        (
            "foreign_call_stacks[0].contribution_report_identity",
            Box::new(|row| {
                row.foreign_call_stacks[0].contribution_report_identity =
                    task_plans::AdmittedStackContributionReportId::from_normalized_identity(
                        0xc011_e541,
                    )
                    .expect("contribution report");
            }),
        ),
        (
            "foreign_call_stacks[0].contribution_commitment",
            Box::new(|row| {
                row.foreign_call_stacks[0].contribution_commitment =
                    task_plans::SameStackContributionCommitment::from_digest([0xaa; 32]);
            }),
        ),
        (
            "foreign_call_stacks[0].contribution_bytes",
            Box::new(|row| {
                row.foreign_call_stacks[0].contribution_bytes = 128;
            }),
        ),
        (
            "foreign_call_stacks[0].contribution_alignment",
            Box::new(|row| {
                row.foreign_call_stacks[0].contribution_alignment = 8;
            }),
        ),
        (
            "foreign_call_stacks::drop",
            Box::new(|row| {
                row.foreign_call_stacks.pop();
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            0,
            mutate,
        );
    }

    assert_substitution_rejected_at_encoding(
        "foreign_call_stacks[0].provider_plan_report_identity::unselected",
        &record,
        0,
        |row| {
            row.foreign_call_stacks[0].provider_plan_report_identity = 55;
        },
        InstallationError::ProviderSettlementClosureMismatch,
    );
}

/// Every representable leaf of an installed scalar call-stack row is
/// authenticated: owner, target, and text offset substitutions still encode
/// and independent replay rejects them, while ordering violations and scalar
/// calls on a Unit-stack row are rejected at canonical encoding.
#[test]
fn installation_function_scalar_call_stacks_reject_every_one_field_substitution() {
    let mut plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut plan.functions[2]);
    let artifact = build_object_artifact(&plan).expect("scalar cleanup artifact");
    let image = emit_executable_image(&artifact, 3).expect("scalar cleanup image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("scalar cleanup installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.scalar_call_stacks.len(), 1);
    assert_eq!(
        authentic.scalar_call_stacks[0].owner,
        CallSiteOwner::CleanupAction {
            edge: edge_id(3),
            action_ordinal: 0
        }
    );

    let tail_text_offset = authentic.text_offset + authentic.byte_count - 4;
    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "scalar_call_stacks[0].owner",
            Box::new(|row| {
                row.scalar_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }),
        ),
        (
            "scalar_call_stacks[0].target",
            Box::new(|row| {
                row.scalar_call_stacks[0].target = machine_id(2);
            }),
        ),
        (
            "scalar_call_stacks[0].text_offset",
            Box::new(|row| {
                row.scalar_call_stacks[0].text_offset += 1;
            }),
        ),
        (
            "scalar_call_stacks[0].caller_live_bytes",
            Box::new(|row| {
                row.scalar_call_stacks[0].caller_live_bytes += 1;
            }),
        ),
        (
            "scalar_call_stacks::drop",
            Box::new(|row| {
                row.scalar_call_stacks.pop();
            }),
        ),
        (
            "scalar_call_stacks::insert-distinct",
            Box::new(move |row| {
                row.scalar_call_stacks
                    .push(image_emission::ObjectScalarCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: tail_text_offset,
                        caller_live_bytes: 8,
                    });
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "scalar_call_stacks[0].target::unknown-machine",
            Box::new(|row| {
                row.scalar_call_stacks[0].target = machine_id(99);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_call_stacks::insert-duplicate",
            Box::new(|row| {
                let call = row.scalar_call_stacks[0];
                row.scalar_call_stacks.push(call);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // A Unit call-stack row cannot be attached to a scalar-stack row.
        (
            "unit_call_stacks::insert-on-scalar-row",
            Box::new(|row| {
                row.unit_call_stacks
                    .push(image_emission::ObjectUnitCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: 4,
                        active_frame_bytes: 8,
                        transient_bytes: 0,
                        caller_live_bytes: 8,
                    });
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // Foreign contribution custody is only canonical beneath a Unit stack.
        (
            "foreign_call_stacks::insert-on-scalar-row",
            Box::new(|row| {
                row.foreign_call_stacks.push(foreign_call(4));
            }),
            InstallationError::ProviderSettlementClosureMismatch,
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
}

/// The semantic parameter roster and its ABI home roster are joined to each
/// other and to the retained stored-dynamic argument custody: every leaf
/// substitution is rejected at canonical encoding except the physical home
/// location axes, which still encode and are rejected by independent replay.
#[test]
fn installation_function_parameter_and_home_rows_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.unit_parameters.len(), 1);
    assert_eq!(authentic.unit_parameter_homes.len(), 1);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "unit_parameter_homes[0].location",
            Box::new(|row| {
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }),
        ),
        (
            "unit_parameter_homes[0].indirect",
            Box::new(|row| {
                row.unit_parameter_homes[0].indirect = true;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_parameters[0].place",
            Box::new(|row| {
                row.unit_parameters[0].place = place_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters[0].structural_type",
            Box::new(|row| {
                row.unit_parameters[0].structural_type = structural_type(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters[0].multiplicity",
            Box::new(|row| {
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters[0].access",
            Box::new(|row| {
                row.unit_parameters[0].access = StructuralAccess::SharedBorrow;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters[0].shape",
            Box::new(|row| {
                row.unit_parameters[0].shape = ValueShape::integer(8, 8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters::drop",
            Box::new(|row| {
                row.unit_parameters.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameters::insert-duplicate",
            Box::new(|row| {
                let parameter = row.unit_parameters[0];
                row.unit_parameters.push(parameter);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // The stored-dynamic argument rejoins its source home by exact place
        // identity, so the home's place and roster membership are bound by the
        // record-level custody row before the scalar-cleanup join runs.
        (
            "unit_parameter_homes[0].place",
            Box::new(|row| {
                row.unit_parameter_homes[0].place = place_id(9);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "unit_parameter_homes::drop",
            Box::new(|row| {
                row.unit_parameter_homes.pop();
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "unit_parameter_homes[0].structural_type",
            Box::new(|row| {
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameter_homes[0].multiplicity",
            Box::new(|row| {
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Unrestricted;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameter_homes[0].access",
            Box::new(|row| {
                row.unit_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameter_homes[0].shape",
            Box::new(|row| {
                row.unit_parameter_homes[0].shape = ValueShape::integer(8, 8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameter_homes[0].source",
            Box::new(|row| {
                row.unit_parameter_homes[0].source = register_placement();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_parameter_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
}

/// The scalar-side structural parameter and home rosters follow the same join
/// beneath scalar custody: declaration axes are pinned to the homes, while the
/// physical home source, location, and indirection axes still encode and are
/// rejected by independent replay.
#[test]
fn installation_function_scalar_structural_rows_reject_every_one_field_substitution() {
    let mut plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut plan.functions[2]);
    let artifact = build_object_artifact(&plan).expect("scalar cleanup artifact");
    let image = emit_executable_image(&artifact, 3).expect("scalar cleanup image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("scalar cleanup installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.scalar_structural_parameters.len(), 1);
    assert_eq!(authentic.scalar_structural_parameter_homes.len(), 1);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "scalar_structural_parameter_homes[0].source",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].source = register_placement();
            }),
        ),
        (
            "scalar_structural_parameter_homes[0].location",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }),
        ),
        (
            "scalar_structural_parameter_homes[0].indirect",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].indirect = true;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "scalar_structural_parameters[0].place",
            Box::new(|row| {
                row.scalar_structural_parameters[0].place = place_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters[0].structural_type",
            Box::new(|row| {
                row.scalar_structural_parameters[0].structural_type = structural_type(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters[0].multiplicity",
            Box::new(|row| {
                row.scalar_structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Unrestricted;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters[0].access",
            Box::new(|row| {
                row.scalar_structural_parameters[0].access = StructuralAccess::SharedBorrow;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters[0].shape",
            Box::new(|row| {
                row.scalar_structural_parameters[0].shape = ValueShape::integer(8, 8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters::drop",
            Box::new(|row| {
                row.scalar_structural_parameters.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameters::insert-duplicate",
            Box::new(|row| {
                let parameter = row.scalar_structural_parameters[0];
                row.scalar_structural_parameters.push(parameter);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes[0].place",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].place = place_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes[0].structural_type",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].structural_type = structural_type(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes[0].multiplicity",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].multiplicity =
                    StructuralMultiplicity::Unrestricted;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes[0].access",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes[0].shape",
            Box::new(|row| {
                row.scalar_structural_parameter_homes[0].shape = ValueShape::integer(8, 8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes::drop",
            Box::new(|row| {
                row.scalar_structural_parameter_homes.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_structural_parameter_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.scalar_structural_parameter_homes[0].clone();
                row.scalar_structural_parameter_homes.push(home);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
}

/// Durable scalar homes, zero-code integer constants, and affine-record
/// establishments are authenticated per row: identity and byte-offset drift
/// still encodes and is rejected by independent replay, while scalar-type and
/// shape drift violates the canonical home-shape join.
#[test]
fn installation_function_unit_scalar_rows_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.unit_scalar_homes.len(), 1);
    assert!(authentic.unit_integer_constants.is_empty());
    assert!(authentic.unit_affine_scalar_records.is_empty());

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "unit_scalar_homes[0].defining_operation",
            Box::new(|row| {
                row.unit_scalar_homes[0].defining_operation = operation_id(9);
            }),
        ),
        (
            "unit_scalar_homes[0].source_value",
            Box::new(|row| {
                row.unit_scalar_homes[0].source_value = value_id(9);
            }),
        ),
        (
            "unit_scalar_homes[0].byte_offset",
            Box::new(|row| {
                row.unit_scalar_homes[0].byte_offset = 24;
            }),
        ),
        (
            "unit_scalar_homes::drop",
            Box::new(|row| {
                row.unit_scalar_homes.pop();
            }),
        ),
        // The constant roster is empty in this image; inserting a canonical
        // zero-code row still encodes and independent replay rejects it.
        (
            "unit_integer_constants::insert",
            Box::new(|row| {
                row.unit_integer_constants.push(integer_constant());
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_scalar_homes[0].scalar_type",
            Box::new(|row| {
                row.unit_scalar_homes[0].scalar_type = ScalarType::Boolean;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_scalar_homes[0].shape",
            Box::new(|row| {
                row.unit_scalar_homes[0].shape = ValueShape::integer(8, 8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_scalar_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.unit_scalar_homes[0];
                row.unit_scalar_homes.push(home);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // The fabricated establishment row fails the exact affine-record join:
        // its producer, result, and field identities cannot be bound to any
        // retained custody in this image.
        (
            "unit_affine_scalar_records::insert",
            Box::new(|row| {
                row.unit_affine_scalar_records.push(affine_scalar_record());
            }),
            InstallationError::InvalidUnitAffineScalarRecord,
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
}

/// Store custody rows are joined to the parameter declarations, homes, and
/// scalar transport of the owning function. The unit-side store rosters reject
/// fabricated rows at canonical encoding; the scalar-side store roster accepts
/// a representable row at encoding and independent replay still rejects it
/// because the image retains no such store.
#[test]
fn installation_function_store_rows_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert!(authentic.unit_structural_scalar_field_stores.is_empty());
    assert!(authentic.unit_write_only_primitive_stores.is_empty());
    assert!(authentic.scalar_structural_scalar_field_stores.is_empty());

    assert_substitution_rejected_by_replay(
        "scalar_structural_scalar_field_stores::insert",
        &record,
        &image,
        &authentic_fingerprint,
        2,
        |row| {
            row.scalar_structural_scalar_field_stores
                .push(scalar_field_store());
        },
    );

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_structural_scalar_field_stores::insert",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores
                    .push(structural_field_store());
            }),
            InstallationError::InvalidUnitStructuralScalarFieldStore(machine_id(3)),
        ),
        (
            "unit_write_only_primitive_stores::insert",
            Box::new(|row| {
                row.unit_write_only_primitive_stores
                    .push(write_only_store());
            }),
            InstallationError::InvalidUnitWriteOnlyPrimitiveStore(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }

    // The scalar-side store roster also encodes on the scalar-stack row and is
    // rejected by independent replay there.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image = emit_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");
    assert_substitution_rejected_by_replay(
        "scalar_structural_scalar_field_stores::insert-on-scalar-row",
        &scalar_record,
        &scalar_image,
        &scalar_fingerprint,
        2,
        |row| {
            row.scalar_structural_scalar_field_stores
                .push(scalar_field_store());
        },
    );
}

/// The retained structural-store roster authenticates every field against the
/// canonical record shape: each fixed-width immediate store joins its
/// destination declaration to the parameter and home rosters, its immediate
/// source to an earlier integer-constant row, and its home offsets, interval,
/// and exact emitted bytes to the semantic attribution.
///
/// No `build_object_artifact` image can carry these rows: emission requires
/// retained common-pipeline replay evidence for borrowed structural
/// destinations, which only `build_function_fragment_object_artifact` admits.
/// The test therefore stages the retained custody on an appended
/// `InstalledFunction` — record-shape canonicalization applies the same joins
/// the emitted custody would face — and authenticates it against an emitted
/// image: the fields no record-shape join reads (the projected field
/// identity, the bounded carrier path's spelling, the destination
/// qualifications, and roster membership) still encode, recompute a distinct
/// installation identity, and are rejected by independent replay of the
/// mutated record. Every join into the parameter, home, constant, or
/// attribution rosters and every non-canonical path or source shape is
/// rejected at canonical encoding.
#[test]
fn installation_function_structural_store_rows_reject_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("two-function artifact");
    let image = emit_executable_image(&artifact, 3).expect("two-function image");
    let mut record =
        build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
            .expect("two-function installation");
    validate_installation_record(&record, &image).expect("exact image binding");

    // Stage the retained custody on an appended function row: the record is
    // canonical on its own, and independent replay rejects it because the
    // unchanged image carries no such function.
    let mut staged = structural_store_custody_plan().functions;
    let source = staged.remove(0);
    let index = record.functions().len();
    let text_offset = record.image_sections().text_byte_count;
    let byte_count = source.bytes.len();
    record.functions_mut_for_test().push(InstalledFunction {
        machine: machine_id(9),
        attachment: source.attachment,
        scalar_abi: source.scalar_abi,
        mixed_structural_scalar_abi: source.mixed_structural_scalar_abi,
        parameter_abi: source.parameter_abi,
        structural_call_scalar_return: source.structural_call_scalar_return,
        text_offset,
        byte_count,
        unit_stack: source.unit_stack.map(|stack| ObjectUnitStack {
            frame_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            local_peak_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            stack_alignment: stack.stack_alignment,
        }),
        scalar_stack: None,
        unit_call_stacks: Vec::new(),
        scalar_call_stacks: Vec::new(),
        foreign_call_stacks: Vec::new(),
        unit_body: true,
        unit_parameters: source.unit_parameters,
        unit_parameter_homes: source.unit_parameter_homes,
        unit_scalar_homes: source.unit_scalar_homes,
        unit_integer_constants: source.unit_integer_constants,
        unit_affine_scalar_records: source.unit_affine_scalar_records,
        unit_structural_scalar_field_stores: source.unit_structural_scalar_field_stores,
        unit_write_only_primitive_stores: source.unit_write_only_primitive_stores,
        scalar_structural_scalar_field_stores: source.scalar_structural_scalar_field_stores,
        unit_continuations: source.unit_continuations,
        unit_affine_cleanup: source.unit_affine_cleanup,
        scalar_affine_cleanup: source.scalar_affine_cleanup,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: source.scalar_structural_parameters,
        scalar_structural_parameter_homes: source.scalar_structural_parameter_homes,
    });
    record.semantic_code_attribution_mut_for_test().extend(
        source
            .semantic_code_attribution
            .into_iter()
            .map(|attribution| ObjectCodeAttribution {
                machine: machine_id(9),
                text_offset: text_offset + attribution.code_offset,
                attribution,
            }),
    );
    record.image_sections_mut_for_test().text_byte_count += byte_count;
    record.image_sections_mut_for_test().final_text_byte_count += byte_count;

    // The staged roster is fully wire-decodable, so the staged record
    // round-trips and recomputes its own identity before any substitution
    // below; independent replay rejects it because the emitted image carries
    // none of these rows.
    let canonical = encode_installation_record(&record).expect("canonical encoding");
    assert_eq!(
        decode_installation_record(&canonical).expect("canonical decoding"),
        record
    );
    assert_eq!(
        validate_installation_record(&record, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the staged store custody"
    );
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[index].clone();
    assert_eq!(authentic.unit_structural_scalar_field_stores.len(), 2);
    assert_eq!(authentic.unit_integer_constants.len(), 2);
    assert_eq!(authentic.unit_parameters.len(), 1);
    assert_eq!(authentic.unit_parameter_homes.len(), 1);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        // The projected field identity is authenticated by the emitted image
        // alone: no record-shape join reads it.
        (
            "unit_structural_scalar_field_stores[0].field",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].field = field_id(9);
            }),
        ),
        // Every field-only or singly-indexed carrier path is inside the
        // bounded store grammar, so its spelling is authenticated by the
        // emitted image alone.
        (
            "unit_structural_scalar_field_stores[0].path::renamed",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field("renamed".to_string())];
            }),
        ),
        (
            "unit_structural_scalar_field_stores[0].path::empty",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path = Vec::new();
            }),
        ),
        (
            "unit_structural_scalar_field_stores[0].path::indexed",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::Field("cell".to_string()),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }),
        ),
        // The destination declaration's qualification rosters are not part of
        // the parameter/home join; the image comparison owns them.
        (
            "unit_structural_scalar_field_stores[0].destination.qualifications",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.projected_qualifications",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }),
        ),
        (
            "unit_structural_scalar_field_stores::drop",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores.pop();
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            index,
            mutate,
        );
    }

    let store_error = || InstallationError::InvalidUnitStructuralScalarFieldStore(machine_id(9));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(9));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        // The producer identity must name the operation the exact-attribution
        // join is bound to.
        (
            "unit_structural_scalar_field_stores[0].psi_operation",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].psi_operation = operation_id(9);
            }),
            store_error(),
        ),
        // Every destination-declaration axis is joined pairwise to the
        // parameter and home rosters.
        (
            "unit_structural_scalar_field_stores[0].destination.place",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].destination.place = place_id(9);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.position",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .position = 1;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.is_self",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .is_self = true;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.structural_type",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.multiplicity",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Affine;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination.access",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .access = StructuralAccess::MutableBorrow;
            }),
            store_error(),
        ),
        // An unbounded carrier path is not representable in the store grammar.
        (
            "unit_structural_scalar_field_stores[0].path::referent",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Referent];
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].path::empty-field",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field(String::new())];
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].path::double-index",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::FixedIndex(0),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].destination_placement",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].destination_placement =
                    register_placement();
            }),
            store_error(),
        ),
        // In-bounds or not, a changed field offset recomputes different bytes
        // or violates the parameter shape.
        (
            "unit_structural_scalar_field_stores[0].field_byte_offset::within-bounds",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].field_byte_offset = 4;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].field_byte_offset::out-of-bounds",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].field_byte_offset = 8;
            }),
            store_error(),
        ),
        // Each immediate-source field is joined to the retained integer
        // constant row.
        (
            "unit_structural_scalar_field_stores[0].source.defining_operation",
            Box::new(|row| {
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *defining_operation = operation_id(9);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].source.source_value",
            Box::new(|row| {
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    source_value,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *source_value = value_id(9);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].source.scalar_type",
            Box::new(|row| {
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    scalar_type,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].source.value",
            Box::new(|row| {
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    value,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *value = IntegerValue::Signed(8);
            }),
            store_error(),
        ),
        // Each remaining source variant is representable but cannot be joined
        // to this function's retained custody.
        (
            "unit_structural_scalar_field_stores[0].source::boolean",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].source =
                    machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                        defining_operation: operation_id(1),
                        source_value: value_id(51),
                        value: true,
                        definition_ordinal: 0,
                    };
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].source::parameter",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].source =
                    machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                        parameter_index: 0,
                        source_value: value_id(51),
                        scalar_type: i32_scalar(),
                        location: machine_code::UnitScalarParameterLocationRecord::Register(
                            calling_conventions::MachineRegister::X86Rax,
                        ),
                    };
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].source::home",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].source =
                    machine_code::InternalUnitScalarArgumentSourceRecord::Home(
                        machine_code::UnitScalarHomeRecord {
                            defining_operation: operation_id(1),
                            source_value: value_id(51),
                            scalar_type: i32_scalar(),
                            shape: ValueShape::integer(4, 4),
                            byte_offset: 0,
                        },
                    );
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].parameter_home_byte_offset",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].parameter_home_byte_offset = 0;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].parameter_home_indirect",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].parameter_home_indirect = true;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].operation_ordinal",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].operation_ordinal += 1;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].code_offset",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].code_offset += 1;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].byte_count",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].byte_count += 1;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].bytes::content",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].bytes[2] = 0x11;
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores[0].bytes::truncate",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores[0].bytes.pop();
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores::swap",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores.swap(0, 1);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.unit_structural_scalar_field_stores[0].clone();
                row.unit_structural_scalar_field_stores.push(duplicate);
            }),
            store_error(),
        ),
        (
            "unit_structural_scalar_field_stores::insert-fabricated",
            Box::new(|row| {
                row.unit_structural_scalar_field_stores
                    .push(structural_field_store());
            }),
            store_error(),
        ),
        // The parameter and home rosters are joined pairwise; the shared
        // declaration axes surface the cleanup-facts error while the
        // home-only location, source placement, and indirection surface the
        // store join.
        (
            "unit_parameters[0].place",
            Box::new(|row| {
                row.unit_parameters[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].structural_type",
            Box::new(|row| {
                row.unit_parameters[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].multiplicity",
            Box::new(|row| {
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Affine;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].access",
            Box::new(|row| {
                row.unit_parameters[0].access = StructuralAccess::MutableBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].shape",
            Box::new(|row| {
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters::drop",
            Box::new(|row| {
                row.unit_parameters.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].place",
            Box::new(|row| {
                row.unit_parameter_homes[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].structural_type",
            Box::new(|row| {
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].multiplicity",
            Box::new(|row| {
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Affine;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].access",
            Box::new(|row| {
                row.unit_parameter_homes[0].access = StructuralAccess::MutableBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].shape",
            Box::new(|row| {
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].source",
            Box::new(|row| {
                row.unit_parameter_homes[0].source = register_placement();
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes[0].location",
            Box::new(|row| {
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes[0].indirect",
            Box::new(|row| {
                row.unit_parameter_homes[0].indirect = true;
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes::drop",
            Box::new(|row| {
                row.unit_parameter_homes.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }),
            cleanup_error(),
        ),
        // The immediate source's retained constant row must still join every
        // field.
        (
            "unit_integer_constants[0].defining_operation",
            Box::new(|row| {
                row.unit_integer_constants[0].defining_operation = operation_id(9);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].source_value",
            Box::new(|row| {
                row.unit_integer_constants[0].source_value = value_id(9);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].scalar_type",
            Box::new(|row| {
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].value",
            Box::new(|row| {
                row.unit_integer_constants[0].value = IntegerValue::Signed(8);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].operation_ordinal",
            Box::new(|row| {
                row.unit_integer_constants[0].operation_ordinal = 5;
            }),
            cleanup_error(),
        ),
        (
            "unit_integer_constants::drop",
            Box::new(|row| {
                row.unit_integer_constants.remove(0);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.unit_integer_constants[0];
                row.unit_integer_constants.push(duplicate);
            }),
            cleanup_error(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, index, mutate, expected);
    }
}

/// The retained write-only primitive-store roster authenticates every field
/// against canonical record shape: each whole-root store joins its producer
/// identity, destination declaration, declared primitive type, source, home
/// offset and indirection, ordering, and exact emitted bytes to the parameter
/// and home rosters, the cleanup's structural-type catalog, the earlier
/// integer-constant rows, and the semantic attribution. Every field-level
/// substitution is therefore rejected at canonical encoding — nothing in the
/// row escapes the record-shape join — while dropping retained rows or
/// inserting a distinct catalog declaration still encodes, recomputes a
/// distinct installation identity, and is rejected only by independent replay
/// against the unchanged emitted image.
#[test]
fn installation_function_write_only_store_rows_reject_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("two-function artifact");
    let image = emit_executable_image(&artifact, 3).expect("two-function image");
    let mut record =
        build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
            .expect("two-function installation");
    validate_installation_record(&record, &image).expect("exact image binding");

    // Stage the retained custody on an appended function row: the record is
    // canonical on its own, and independent replay rejects it because the
    // unchanged image carries no such function.
    let mut staged = write_only_store_custody_plan().functions;
    let source = staged.remove(0);
    let index = record.functions().len();
    let text_offset = record.image_sections().text_byte_count;
    let byte_count = source.bytes.len();
    record.functions_mut_for_test().push(InstalledFunction {
        machine: machine_id(9),
        attachment: source.attachment,
        scalar_abi: source.scalar_abi,
        mixed_structural_scalar_abi: source.mixed_structural_scalar_abi,
        parameter_abi: source.parameter_abi,
        structural_call_scalar_return: source.structural_call_scalar_return,
        text_offset,
        byte_count,
        unit_stack: source.unit_stack.map(|stack| ObjectUnitStack {
            frame_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            local_peak_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            stack_alignment: stack.stack_alignment,
        }),
        scalar_stack: None,
        unit_call_stacks: Vec::new(),
        scalar_call_stacks: Vec::new(),
        foreign_call_stacks: Vec::new(),
        unit_body: true,
        unit_parameters: source.unit_parameters,
        unit_parameter_homes: source.unit_parameter_homes,
        unit_scalar_homes: source.unit_scalar_homes,
        unit_integer_constants: source.unit_integer_constants,
        unit_affine_scalar_records: source.unit_affine_scalar_records,
        unit_structural_scalar_field_stores: source.unit_structural_scalar_field_stores,
        unit_write_only_primitive_stores: source.unit_write_only_primitive_stores,
        scalar_structural_scalar_field_stores: source.scalar_structural_scalar_field_stores,
        unit_continuations: source.unit_continuations,
        unit_affine_cleanup: source.unit_affine_cleanup,
        scalar_affine_cleanup: source.scalar_affine_cleanup,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: source.scalar_structural_parameters,
        scalar_structural_parameter_homes: source.scalar_structural_parameter_homes,
    });
    record.semantic_code_attribution_mut_for_test().extend(
        source
            .semantic_code_attribution
            .into_iter()
            .map(|attribution| ObjectCodeAttribution {
                machine: machine_id(9),
                text_offset: text_offset + attribution.code_offset,
                attribution,
            }),
    );
    record.image_sections_mut_for_test().text_byte_count += byte_count;
    record.image_sections_mut_for_test().final_text_byte_count += byte_count;

    // The staged roster is fully wire-decodable, so the staged record
    // round-trips and recomputes its own identity before any substitution
    // below; independent replay rejects it because the emitted image carries
    // none of these rows.
    let canonical = encode_installation_record(&record).expect("canonical encoding");
    assert_eq!(
        decode_installation_record(&canonical).expect("canonical decoding"),
        record
    );
    assert_eq!(
        validate_installation_record(&record, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the staged store custody"
    );
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[index].clone();
    assert_eq!(authentic.unit_write_only_primitive_stores.len(), 2);
    assert_eq!(authentic.unit_integer_constants.len(), 2);
    assert_eq!(authentic.unit_parameters.len(), 1);
    assert_eq!(authentic.unit_parameter_homes.len(), 1);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        // Dropping a retained store leaves a canonical roster: the image
        // alone authenticates that the row existed.
        (
            "unit_write_only_primitive_stores::drop",
            Box::new(|row| {
                row.unit_write_only_primitive_stores.pop();
            }),
        ),
        (
            "unit_write_only_primitive_stores::drop-all",
            Box::new(|row| {
                row.unit_write_only_primitive_stores.clear();
            }),
        ),
        // A distinct catalog entry keeps the joined declaration's count at
        // exactly one; only the emitted image owns the catalog's contents.
        (
            "unit_affine_cleanup.structural_types::insert-distinct",
            Box::new(|row| {
                let mut catalog: Vec<_> = row
                    .unit_affine_cleanup
                    .as_ref()
                    .expect("cleanup")
                    .structural_types
                    .iter()
                    .cloned()
                    .collect();
                catalog.push(terminal_psi::StructuralTypeDeclaration {
                    id: structural_type(9),
                    identity: "extra.type".to_string(),
                    shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(i32_scalar()),
                });
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = catalog.into();
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            index,
            mutate,
        );
    }

    let store_error = || InstallationError::InvalidUnitWriteOnlyPrimitiveStore(machine_id(9));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(9));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        // The producer identity must name the operation the exact-attribution
        // join is bound to.
        (
            "unit_write_only_primitive_stores[0].psi_operation",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].psi_operation = operation_id(9);
            }),
            store_error(),
        ),
        // Every destination-declaration axis is joined pairwise to the
        // parameter and home rosters.
        (
            "unit_write_only_primitive_stores[0].destination.place",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination.place = place_id(9);
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.position",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination.position = 1;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.is_self",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination.is_self = true;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.structural_type",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.multiplicity",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Affine;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.access",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination.access =
                    StructuralAccess::MutableBorrow;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.qualifications",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination.projected_qualifications",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }),
            store_error(),
        ),
        // The declared destination type must equal the joined catalog entry:
        // its id names the destination's structural type, its identity must
        // be non-empty, and its shape must be exactly the primitive the
        // source writes.
        (
            "unit_write_only_primitive_stores[0].destination_type.id",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination_type.id = structural_type(9);
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination_type.identity",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .identity = "other.type".to_string();
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination_type.identity::empty",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .identity = String::new();
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].destination_type.shape",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .shape =
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
            }),
            store_error(),
        ),
        // The staged home placement is joined field-for-field to the home's
        // retained source placement.
        (
            "unit_write_only_primitive_stores[0].destination_placement",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].destination_placement =
                    register_placement();
            }),
            store_error(),
        ),
        // Every axis of the retained integer-immediate source must join the
        // earlier constant row exactly.
        (
            "unit_write_only_primitive_stores[0].source.defining_operation",
            Box::new(|row| {
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    defining_operation,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *defining_operation = operation_id(9);
                }
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source.source_value",
            Box::new(|row| {
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    source_value,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *source_value = value_id(9);
                }
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source.scalar_type",
            Box::new(|row| {
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    scalar_type,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
                }
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source.value",
            Box::new(|row| {
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    value,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *value = IntegerValue::Signed(8);
                }
            }),
            store_error(),
        ),
        // Every other source carrier is rejected: the parameter variant needs
        // an installed scalar parameter ABI, the zero-code immediates need an
        // exact zero-byte definition attribution, and the home variant needs
        // a retained scalar call result.
        (
            "unit_write_only_primitive_stores[0].source::parameter",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                        parameter_index: 0,
                        source_value: value_id(51),
                        scalar_type: i32_scalar(),
                        location: machine_code::UnitScalarParameterLocationRecord::Register(
                            calling_conventions::MachineRegister::X86Rax,
                        ),
                    };
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source::boolean",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                        defining_operation: operation_id(9),
                        source_value: value_id(55),
                        value: true,
                        definition_ordinal: 0,
                    };
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source::ieee-float",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                        defining_operation: operation_id(9),
                        source_value: value_id(55),
                        value: semantic_vocabulary::IeeeFloatValue::Binary32(0x3f80_0000),
                        definition_ordinal: 0,
                    };
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].source::home",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Home(
                        machine_code::UnitScalarHomeRecord {
                            defining_operation: operation_id(1),
                            source_value: value_id(51),
                            scalar_type: i32_scalar(),
                            shape: ValueShape::integer(4, 4),
                            byte_offset: 0,
                        },
                    );
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].parameter_home_byte_offset",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].parameter_home_byte_offset = 0;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].parameter_home_indirect",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].parameter_home_indirect = false;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].operation_ordinal",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].operation_ordinal += 1;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].code_offset",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].code_offset += 1;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].byte_count",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].byte_count += 1;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].bytes::content",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].bytes[2] = 0x11;
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores[0].bytes::truncate",
            Box::new(|row| {
                row.unit_write_only_primitive_stores[0].bytes.pop();
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores::swap",
            Box::new(|row| {
                row.unit_write_only_primitive_stores.swap(0, 1);
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.unit_write_only_primitive_stores[0].clone();
                row.unit_write_only_primitive_stores.push(duplicate);
            }),
            store_error(),
        ),
        (
            "unit_write_only_primitive_stores::insert-fabricated",
            Box::new(|row| {
                row.unit_write_only_primitive_stores
                    .push(write_only_store());
            }),
            store_error(),
        ),
        // The parameter and home rosters are joined pairwise; the shared
        // declaration axes surface the cleanup-facts error while the
        // home-only location, source placement, and indirection surface the
        // store join.
        (
            "unit_parameters[0].place",
            Box::new(|row| {
                row.unit_parameters[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].structural_type",
            Box::new(|row| {
                row.unit_parameters[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].multiplicity",
            Box::new(|row| {
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Affine;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].access",
            Box::new(|row| {
                row.unit_parameters[0].access = StructuralAccess::MutableBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].shape",
            Box::new(|row| {
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters::drop",
            Box::new(|row| {
                row.unit_parameters.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].place",
            Box::new(|row| {
                row.unit_parameter_homes[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].structural_type",
            Box::new(|row| {
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].multiplicity",
            Box::new(|row| {
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Affine;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].access",
            Box::new(|row| {
                row.unit_parameter_homes[0].access = StructuralAccess::MutableBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].shape",
            Box::new(|row| {
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].source",
            Box::new(|row| {
                row.unit_parameter_homes[0].source = register_placement();
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes[0].location",
            Box::new(|row| {
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes[0].indirect",
            Box::new(|row| {
                row.unit_parameter_homes[0].indirect = false;
            }),
            store_error(),
        ),
        (
            "unit_parameter_homes::drop",
            Box::new(|row| {
                row.unit_parameter_homes.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }),
            cleanup_error(),
        ),
        // The immediate source's retained constant row must still join every
        // field.
        (
            "unit_integer_constants[0].defining_operation",
            Box::new(|row| {
                row.unit_integer_constants[0].defining_operation = operation_id(9);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].source_value",
            Box::new(|row| {
                row.unit_integer_constants[0].source_value = value_id(9);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].scalar_type",
            Box::new(|row| {
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].value",
            Box::new(|row| {
                row.unit_integer_constants[0].value = IntegerValue::Signed(8);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants[0].operation_ordinal",
            Box::new(|row| {
                row.unit_integer_constants[0].operation_ordinal = 5;
            }),
            cleanup_error(),
        ),
        (
            "unit_integer_constants::drop",
            Box::new(|row| {
                row.unit_integer_constants.remove(0);
            }),
            store_error(),
        ),
        (
            "unit_integer_constants::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.unit_integer_constants[0];
                row.unit_integer_constants.push(duplicate);
            }),
            cleanup_error(),
        ),
        // The cleanup's structural-type catalog must retain the joined
        // destination declaration exactly once: removing it or duplicating
        // it breaks the count the store join authenticates.
        (
            "unit_affine_cleanup.structural_types::drop",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = Vec::new().into();
            }),
            store_error(),
        ),
        (
            "unit_affine_cleanup.structural_types::insert-duplicate",
            Box::new(|row| {
                let mut catalog: Vec<_> = row
                    .unit_affine_cleanup
                    .as_ref()
                    .expect("cleanup")
                    .structural_types
                    .iter()
                    .cloned()
                    .collect();
                catalog.push(catalog[0].clone());
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = catalog.into();
            }),
            store_error(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, index, mutate, expected);
    }
}

/// The retained scalar-side structural field-store roster carries no
/// record-shape join at all: the codec bounds only the roster count and the
/// path grammar, so every representable field substitution still encodes,
/// recomputes a distinct installation identity, and is rejected only by
/// independent replay against the unchanged emitted image. The codec-side
/// rejections — an empty field identity, an address-carrier immediate type,
/// and a roster past the retained bound — are pinned separately so
/// encode-time canonicality stays distinct from replay-time rejection.
#[test]
fn installation_function_scalar_store_rows_reject_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("two-function artifact");
    let image = emit_executable_image(&artifact, 3).expect("two-function image");
    let mut record =
        build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
            .expect("two-function installation");
    validate_installation_record(&record, &image).expect("exact image binding");

    // Stage the retained custody on an appended function row: the record is
    // canonical on its own, and independent replay rejects it because the
    // unchanged image carries no such function.
    let mut staged = scalar_store_custody_plan().functions;
    let source = staged.remove(0);
    let index = record.functions().len();
    let text_offset = record.image_sections().text_byte_count;
    let byte_count = source.bytes.len();
    record.functions_mut_for_test().push(InstalledFunction {
        machine: machine_id(9),
        attachment: source.attachment,
        scalar_abi: source.scalar_abi,
        mixed_structural_scalar_abi: source.mixed_structural_scalar_abi,
        parameter_abi: source.parameter_abi,
        structural_call_scalar_return: source.structural_call_scalar_return,
        text_offset,
        byte_count,
        unit_stack: source.unit_stack.map(|stack| ObjectUnitStack {
            frame_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            local_peak_bytes: stack.frame.map_or(0, |frame| frame.byte_size),
            stack_alignment: stack.stack_alignment,
        }),
        scalar_stack: None,
        unit_call_stacks: Vec::new(),
        scalar_call_stacks: Vec::new(),
        foreign_call_stacks: Vec::new(),
        unit_body: true,
        unit_parameters: source.unit_parameters,
        unit_parameter_homes: source.unit_parameter_homes,
        unit_scalar_homes: source.unit_scalar_homes,
        unit_integer_constants: source.unit_integer_constants,
        unit_affine_scalar_records: source.unit_affine_scalar_records,
        unit_structural_scalar_field_stores: source.unit_structural_scalar_field_stores,
        unit_write_only_primitive_stores: source.unit_write_only_primitive_stores,
        scalar_structural_scalar_field_stores: source.scalar_structural_scalar_field_stores,
        unit_continuations: source.unit_continuations,
        unit_affine_cleanup: source.unit_affine_cleanup,
        scalar_affine_cleanup: source.scalar_affine_cleanup,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: source.scalar_structural_parameters,
        scalar_structural_parameter_homes: source.scalar_structural_parameter_homes,
    });
    record.semantic_code_attribution_mut_for_test().extend(
        source
            .semantic_code_attribution
            .into_iter()
            .map(|attribution| ObjectCodeAttribution {
                machine: machine_id(9),
                text_offset: text_offset + attribution.code_offset,
                attribution,
            }),
    );
    record.image_sections_mut_for_test().text_byte_count += byte_count;
    record.image_sections_mut_for_test().final_text_byte_count += byte_count;

    // The staged roster is fully wire-decodable, so the staged record
    // round-trips and recomputes its own identity before any substitution
    // below; independent replay rejects it because the emitted image carries
    // none of these rows.
    let canonical = encode_installation_record(&record).expect("canonical encoding");
    assert_eq!(
        decode_installation_record(&canonical).expect("canonical decoding"),
        record
    );
    assert_eq!(
        validate_installation_record(&record, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the staged store custody"
    );
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[index].clone();
    assert_eq!(authentic.scalar_structural_scalar_field_stores.len(), 2);

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "scalar_structural_scalar_field_stores[0].psi_operation",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].psi_operation = operation_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.place",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .place = place_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.position",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .position = 1;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.is_self",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .is_self = false;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.structural_type",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.multiplicity",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Unrestricted;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.access",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .access = StructuralAccess::WriteOnlyBorrow;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.qualifications",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.projected_qualifications",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination.projected_qualifications::referent",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Referent],
                        domain: domain_id(3),
                    });
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].path::referent",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Referent];
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].path::renamed",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field("renamed".to_string())];
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].path::empty",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].path = Vec::new();
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].path::indexed",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::Field("cell".to_string()),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].field",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].field = field_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].destination_placement",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].destination_placement =
                    empty_placement();
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].field_byte_offset",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].field_byte_offset = 4;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].defining_operation",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].defining_operation = operation_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].source_value",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].source_value = value_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].immediate::boolean",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Boolean(true);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].immediate.scalar_type",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
                        value: IntegerValue::Signed(3),
                    };
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].immediate.value",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: i32_integer(),
                        value: IntegerValue::Signed(9),
                    };
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].return_operation",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].return_operation = operation_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].return_source_value",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].return_source_value = value_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].return_field",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].return_field = field_id(9);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].return_field_byte_offset",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].return_field_byte_offset = 4;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].return_scalar_type",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].return_scalar_type =
                    ScalarType::Boolean;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].operation_ordinal",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].operation_ordinal += 1;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].code_offset",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].code_offset += 1;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].byte_count",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].byte_count += 1;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].bytes::content",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].bytes[0] = 0x11;
            }),
        ),
        (
            "scalar_structural_scalar_field_stores[0].bytes::truncate",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].bytes.pop();
            }),
        ),
        (
            "scalar_structural_scalar_field_stores::swap",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores.swap(0, 1);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores::drop",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores.pop();
            }),
        ),
        (
            "scalar_structural_scalar_field_stores::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.scalar_structural_scalar_field_stores[0].clone();
                row.scalar_structural_scalar_field_stores.push(duplicate);
            }),
        ),
        (
            "scalar_structural_scalar_field_stores::insert-fabricated",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            index,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        // The path grammar's remaining canonicality boundary: a field segment
        // must carry a non-empty identity.
        (
            "scalar_structural_scalar_field_stores[0].path::empty-field",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field(String::new())];
            }),
            InstallationError::InvalidSettlementArgumentField,
        ),
        // The immediate's integer carrier must be fixed-width on the wire.
        (
            "scalar_structural_scalar_field_stores[0].immediate.scalar_type::address",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: IntegerType::address(64).expect("address i64"),
                        value: IntegerValue::Unsigned(3),
                    };
            }),
            InstallationError::UnsupportedInstalledFixedIntegerType,
        ),
        // The retained roster is bounded at three rows.
        (
            "scalar_structural_scalar_field_stores::insert-beyond-bound",
            Box::new(|row| {
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }),
            InstallationError::TooManyScalarStructuralScalarFieldStores,
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, index, mutate, expected);
    }
}

/// The affine scalar-record roster authenticates every field against an
/// authentic emitted image: each establishment's zero-byte semantic span, its
/// declared result place and field, and its signed immediate are joined to the
/// parameter homes, and the single later owned internal Unit call consuming it
/// must materialize exactly the record's bytes into the argument register.
/// The consuming argument's source placement — an empty-location placement
/// carrying the record's 8-byte shape — is not wire-decodable today, so the
/// containing record encodes but does not round-trip; that gap is pinned in
/// the test body. The identities no record-shape join reads — producer, result
/// place and type, field, value, ordinal — still encode, recompute a distinct
/// installation identity, and are rejected by independent replay of the
/// mutated record. Every canonical-shape violation and every join into the
/// parameter roster, the consuming call, or the roster itself is rejected at
/// canonical encoding.
#[test]
fn installation_function_affine_scalar_records_reject_every_one_field_substitution() {
    let plan = affine_scalar_record_custody_plan();
    let artifact = build_object_artifact(&plan).expect("affine scalar artifact");
    let image = emit_executable_image(&artifact, 3).expect("affine scalar image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("affine scalar installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[0].clone();
    assert_eq!(authentic.unit_affine_scalar_records.len(), 2);
    assert_eq!(record.internal_unit_calls().len(), 2);

    // The authentic record encodes canonically, but the consuming argument's
    // source placement — an empty-location placement carrying an 8-byte shape,
    // the exact form `affine_scalar_record_source` requires — is not decodable
    // through `decode_direct_placement` today. The wire gap is pinned here so
    // each substitution below is authenticated by canonical encoding,
    // identity recompute, and independent replay of the mutated record.
    let canonical = encode_installation_record(&record).expect("canonical encoding");
    assert_eq!(
        decode_installation_record(&canonical),
        Err(InstallationError::UnsupportedInternalUnitCallPlacement)
    );

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "unit_affine_scalar_records[0].psi_operation",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].psi_operation = operation_id(9);
            }),
        ),
        (
            "unit_affine_scalar_records[0].result.place",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].result.place = place_id(9);
            }),
        ),
        (
            "unit_affine_scalar_records[0].result.structural_type",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].result.structural_type = structural_type(9);
            }),
        ),
        (
            "unit_affine_scalar_records[0].field",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].field = field_id(9);
            }),
        ),
        (
            "unit_affine_scalar_records[0].value",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].value = IntegerValue::Signed(18);
            }),
        ),
        (
            "unit_affine_scalar_records[0].operation_ordinal",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].operation_ordinal += 1;
            }),
        ),
        (
            "unit_affine_scalar_records::drop",
            Box::new(|row| {
                row.unit_affine_scalar_records.pop();
            }),
        ),
        (
            "unit_affine_scalar_records::insert",
            Box::new(|row| {
                row.unit_affine_scalar_records
                    .push(UnitAffineScalarRecordEstablishmentRecord {
                        psi_operation: operation_id(9),
                        result: terminal_psi::StructuralOperationResult {
                            place: place_id(9),
                            structural_type: structural_type(9),
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        },
                        field: field_id(9),
                        value: IntegerValue::Signed(7),
                        shape: ValueShape::integer(8, 8),
                        operation_ordinal: 5,
                    });
            }),
        ),
        (
            "unit_affine_scalar_records::insert-duplicate",
            Box::new(|row| {
                let duplicate = row.unit_affine_scalar_records[0].clone();
                row.unit_affine_scalar_records.push(duplicate);
            }),
        ),
        (
            "unit_affine_scalar_records::swap",
            Box::new(|row| {
                row.unit_affine_scalar_records.swap(0, 1);
            }),
        ),
        // The parameter home's indirection flag is authenticated by the
        // emitted image alone: no record-shape join reads it.
        (
            "unit_parameter_homes[0].indirect",
            Box::new(|row| {
                row.unit_parameter_homes[0].indirect = true;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_undecodable_row_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            0,
            mutate,
        );
    }

    let scalar_record_error = || InstallationError::InvalidUnitAffineScalarRecord;
    let call_error = || InstallationError::InvalidInternalUnitCall(machine_id(1));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(1));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_affine_scalar_records[0].result.multiplicity",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].result.multiplicity =
                    StructuralMultiplicity::Linear;
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].result.qualifications",
            Box::new(|row| {
                row.unit_affine_scalar_records[0]
                    .result
                    .qualifications
                    .push(domain_id(3));
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].result.projected_qualifications",
            Box::new(|row| {
                row.unit_affine_scalar_records[0]
                    .result
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].result.claims",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].result.claims.push(
                    terminal_psi::StructuralResultClaimBinding {
                        claim: semantic_vocabulary::ClaimId::new(31).expect("claim"),
                        path: Vec::new(),
                    },
                );
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].value::unsigned",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].value = IntegerValue::Unsigned(3);
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].value::overflow",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].value =
                    IntegerValue::Signed(i128::from(i64::MAX) + 1);
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records[0].shape",
            Box::new(|row| {
                row.unit_affine_scalar_records[0].shape = ValueShape::integer(4, 4);
            }),
            scalar_record_error(),
        ),
        (
            "unit_affine_scalar_records::insert-noncanonical",
            Box::new(|row| {
                row.unit_affine_scalar_records.push(affine_scalar_record());
            }),
            scalar_record_error(),
        ),
        // The record's result place is joined through the parameter and home
        // rosters: declaration axes are pinned pairwise, and every roster
        // membership change upsets the cleanup's transferred-root suffix.
        (
            "unit_parameters[0].place",
            Box::new(|row| {
                row.unit_parameters[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].structural_type",
            Box::new(|row| {
                row.unit_parameters[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].multiplicity",
            Box::new(|row| {
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Linear;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].access",
            Box::new(|row| {
                row.unit_parameters[0].access = StructuralAccess::SharedBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters[0].shape",
            Box::new(|row| {
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters::drop",
            Box::new(|row| {
                row.unit_parameters.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters::insert-duplicate",
            Box::new(|row| {
                let parameter = row.unit_parameters[0];
                row.unit_parameters.push(parameter);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameters::swap",
            Box::new(|row| {
                row.unit_parameters.swap(0, 1);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].place",
            Box::new(|row| {
                row.unit_parameter_homes[0].place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].structural_type",
            Box::new(|row| {
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].multiplicity",
            Box::new(|row| {
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Linear;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].access",
            Box::new(|row| {
                row.unit_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].shape",
            Box::new(|row| {
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes[0].source",
            Box::new(|row| {
                row.unit_parameter_homes[0].source = register_placement();
            }),
            call_error(),
        ),
        (
            "unit_parameter_homes[0].location",
            Box::new(|row| {
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 16 };
            }),
            call_error(),
        ),
        (
            "unit_parameter_homes::drop",
            Box::new(|row| {
                row.unit_parameter_homes.pop();
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes::insert-duplicate",
            Box::new(|row| {
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }),
            cleanup_error(),
        ),
        (
            "unit_parameter_homes::swap",
            Box::new(|row| {
                row.unit_parameter_homes.swap(0, 1);
            }),
            cleanup_error(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 0, mutate, expected);
    }

    // The record-level call roster carries the consuming custody: each
    // argument leaf is joined to the record's place, home, immediate bytes
    // and span, while the roster's physical ordering is canonical.
    let provider_source = machine_code::InternalUnitCallSource::InstalledProvider {
        boundary: semantic_vocabulary::BoundaryMachineId::new(7).expect("boundary"),
        provider: Box::new(terminal_psi::ProviderCandidateConformance {
            boundary: semantic_vocabulary::BoundaryMachineId::new(7).expect("boundary"),
            requirement_identity: "requirement".into(),
            provider_identity: "provider".into(),
            candidate_identity: "candidate".into(),
            candidate: machine_id(2),
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let structural_result = machine_code::InternalStructuralCallResult {
        operation_result: terminal_psi::StructuralOperationResult {
            place: place_id(31),
            structural_type: structural_type(31),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        result_home: None,
        function_result: terminal_psi::StructuralResultDeclaration {
            place: place_id(31),
            structural_type: structural_type(31),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        },
        returned_claim_transfers: Vec::new(),
        returned_claims: Vec::new(),
        caller_result_placement: empty_placement(),
        callee_result_placement: empty_placement(),
    };
    let still_encodes_calls: Vec<(&'static str, Box<dyn Fn(&mut InstallationRecord)>)> = vec![
        // The argument's access and byte-transport axes are authenticated by
        // the emitted image alone: no record-shape join reads them, so each
        // substitution still encodes and replay rejects it.
        (
            "internal_unit_calls[0].arguments[0].access",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .access = StructuralAccess::SharedBorrow;
            }),
        ),
        (
            "internal_unit_calls[0].arguments[0].call_stack_bytes",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .call_stack_bytes = 0;
            }),
        ),
        (
            "internal_unit_calls[0].arguments[0].code_offset",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .code_offset += 1;
            }),
        ),
        (
            "internal_unit_calls[0].arguments[0].bytes",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .bytes[2] ^= 0xff;
            }),
        ),
        (
            "internal_unit_calls[0].custody.claim_transfers",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: semantic_vocabulary::ClaimId::new(31).expect("claim"),
                        argument_index: 0,
                    });
            }),
        ),
    ];
    for (field, mutate) in still_encodes_calls {
        assert_undecodable_record_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            mutate,
        );
    }

    let rejected_calls: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstallationRecord)>,
        InstallationError,
    )> = vec![
        (
            "internal_unit_calls[0].custody.source",
            Box::new(move |record| {
                record.internal_unit_calls_mut_for_test()[0].custody.source =
                    provider_source.clone();
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.owner",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0].custody.owner =
                    CallSiteOwner::Operation(operation_id(9));
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.target",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0].custody.target = machine_id(3);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.result",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0].custody.result = Some(i32_scalar());
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.semantic_result",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .semantic_result = Some(abstract_operations::AbstractResult {
                    value: value_id(31),
                    scalar_type: ScalarType::Boolean,
                });
            }),
            call_error(),
        ),
        // A returned structural result joins into the function's transferred
        // affine roots, so the cleanup roster rejects it before the call.
        (
            "internal_unit_calls[0].custody.structural_result",
            Box::new(move |record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .structural_result = Some(structural_result.clone());
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls[0].custody.operation_ordinal",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .operation_ordinal = 0;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.code_offset",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .code_offset += 1;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.byte_count",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .byte_count += 1;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.scalar_arguments",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .scalar_arguments
                    .push(machine_code::InternalUnitScalarCallArgumentRecord {
                        parameter_index: 0,
                        source:
                            machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                                defining_operation: operation_id(8),
                                source_value: value_id(55),
                                scalar_type: i32_integer(),
                                value: IntegerValue::Signed(3),
                            },
                        destination: register_placement(),
                        code_offset: 0,
                        byte_count: 5,
                    });
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].place",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .place = place_id(9);
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].path",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .path = vec![StructuralPathSegment::Field("other".to_string())];
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].root_structural_type",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .root_structural_type = structural_type(9);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].structural_type",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .structural_type = structural_type(9);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].shape",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .shape = ValueShape::integer(4, 4);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].source::placement",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source = machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                    register_placement(),
                );
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].source::established",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source =
                    machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal {
                        psi_operation: operation_id(1),
                    };
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].source_location",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source_location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].source_byte_offset",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source_byte_offset = 1;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].destination",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .destination = register_placement();
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].byte_count",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .byte_count += 1;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].fixed_array_length",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .fixed_array_length = Some(2);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments[0].element_stride",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .element_stride = Some(8);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments::drop",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments
                    .pop();
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls[0].custody.arguments::insert-duplicate",
            Box::new(|record| {
                let duplicate = record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .clone();
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments
                    .push(duplicate);
            }),
            call_error(),
        ),
        // Retargeting the call to the callee's machine strips the consuming
        // call from the caller's roster, so the caller's cleanup rejects it
        // before the call's own fields are examined.
        (
            "internal_unit_calls[0].machine",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0].machine = machine_id(2);
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls[0].text_offset",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test()[0].text_offset += 1;
            }),
            call_error(),
        ),
        (
            "internal_unit_calls::drop",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test().remove(0);
            }),
            cleanup_error(),
        ),
        (
            "internal_unit_calls::swap",
            Box::new(|record| {
                record.internal_unit_calls_mut_for_test().swap(0, 1);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls::insert-duplicate",
            Box::new(|record| {
                let duplicate = record.internal_unit_calls_mut_for_test()[0].clone();
                record.internal_unit_calls_mut_for_test().push(duplicate);
            }),
            call_error(),
        ),
        (
            "internal_unit_calls::insert-distinct",
            Box::new(|record| {
                let mut call = record.internal_unit_calls_mut_for_test()[0].clone();
                call.custody.owner = CallSiteOwner::Operation(operation_id(9));
                call.custody.operation_ordinal = 9;
                call.custody.code_offset = 40;
                call.custody.byte_count = 6;
                call.text_offset = 40;
                record.internal_unit_calls_mut_for_test().push(call);
            }),
            call_error(),
        ),
    ];
    for (field, mutate, expected) in rejected_calls {
        assert_record_substitution_rejected_at_encoding(field, &record, mutate, expected);
    }
}

/// Unit-body cleanup evidence is pinned leaf-for-leaf by the record-shape
/// joins: the semantic edge, structural type closure, locals, action roster,
/// and the nested nominal-action custody all reject at canonical encoding
/// except the verifier-owned structural type catalog, which still encodes and
/// is rejected by independent replay. The same holds for the scalar affine
/// cleanup beneath a scalar-stack row.
#[test]
fn installation_function_affine_cleanup_rejects_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    assert!(record.functions()[2].unit_affine_cleanup.is_some());

    assert_substitution_rejected_by_replay(
        "unit_affine_cleanup.structural_types",
        &record,
        &image,
        &authentic_fingerprint,
        2,
        |row| {
            row.unit_affine_cleanup
                .as_mut()
                .expect("cleanup")
                .structural_types = extra_type_catalog();
        },
    );

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "unit_affine_cleanup.psi_edge",
            Box::new(|row| {
                row.unit_affine_cleanup.as_mut().expect("cleanup").psi_edge = edge_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.locals::insert",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .locals
                    .push(extra_local());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.actions::drop",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.actions::insert",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        place_id(9),
                    ));
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.actions[0].cleanup_machine",
            Box::new(|row| {
                let action = &mut row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0];
                let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) = action
                else {
                    panic!("fixture cleanup action");
                };
                nominal.cleanup_machine = machine_id(2);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.actions[0].place",
            Box::new(|row| {
                let action = &mut row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0];
                let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) = action
                else {
                    panic!("fixture cleanup action");
                };
                nominal.place = place_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.actions[0]::variant",
            Box::new(|row| {
                row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0] =
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9));
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.code_offset",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .code_offset += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "unit_affine_cleanup.byte_count",
            Box::new(|row| {
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .byte_count -= 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // Dropping the cleanup leaves `unit_body` asserted without its
        // retained evidence; the projection join rejects it.
        (
            "unit_affine_cleanup::drop",
            Box::new(|row| {
                row.unit_affine_cleanup = None;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // A scalar affine cleanup cannot be forged onto a Unit-stack row.
        (
            "scalar_affine_cleanup::insert-on-unit-row",
            Box::new(|row| {
                row.scalar_affine_cleanup = row.unit_affine_cleanup.clone();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // Continuations need an exact block/attribution custody that this
        // fixture does not retain; the fabricated boundary row is rejected by
        // the unit-cleanup joins on either row.
        (
            "unit_continuations::insert",
            Box::new(|row| {
                row.unit_continuations.push(UnitContinuationRecord {
                    operation_ordinal: 3,
                    successor_operation_ordinal: 4,
                    source_block: block_id(1),
                    target_block: block_id(2),
                    bindings: Vec::new(),
                    cleanup: row.unit_affine_cleanup.clone().expect("cleanup"),
                });
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
    assert_substitution_rejected_at_encoding(
        "unit_continuations::insert-on-unit-caller",
        &record,
        0,
        |row| {
            row.unit_continuations.push(UnitContinuationRecord {
                operation_ordinal: 1,
                successor_operation_ordinal: 2,
                source_block: block_id(1),
                target_block: block_id(2),
                bindings: Vec::new(),
                cleanup: row.unit_affine_cleanup.clone().expect("cleanup"),
            });
        },
        InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
    );

    // The scalar-side cleanup projection joins the same axes on the promoted
    // scalar row; its verifier-owned type catalog still encodes and replay
    // rejects the substitution.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image = emit_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");
    assert!(scalar_record.functions()[2].scalar_affine_cleanup.is_some());

    assert_substitution_rejected_by_replay(
        "scalar_affine_cleanup.structural_types",
        &scalar_record,
        &scalar_image,
        &scalar_fingerprint,
        2,
        |row| {
            row.scalar_affine_cleanup
                .as_mut()
                .expect("cleanup")
                .structural_types = extra_type_catalog();
        },
    );

    let scalar_rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "scalar_affine_cleanup.psi_edge",
            Box::new(|row| {
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .psi_edge = edge_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_affine_cleanup.locals::insert",
            Box::new(|row| {
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .locals
                    .push(extra_local());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_affine_cleanup.actions::drop",
            Box::new(|row| {
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_affine_cleanup.code_offset",
            Box::new(|row| {
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .code_offset += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_affine_cleanup.byte_count",
            Box::new(|row| {
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .byte_count -= 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        (
            "scalar_affine_cleanup::drop",
            Box::new(|row| {
                row.scalar_affine_cleanup = None;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // A Unit cleanup cannot be forged beneath scalar stack evidence.
        (
            "unit_affine_cleanup::insert-on-scalar-row",
            Box::new(|row| {
                row.unit_affine_cleanup = row.scalar_affine_cleanup.clone();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in scalar_rejected {
        assert_substitution_rejected_at_encoding(field, &scalar_record, 2, mutate, expected);
    }
}

/// The projected scalar-control cleanup roster retains only the cleanup axis
/// of each DFS leaf: every projected leaf is a canonical projection of the
/// emitted image's scalar-control custody, so every substitution is rejected
/// at canonical encoding.
#[test]
fn installation_function_scalar_control_cleanups_reject_every_one_field_substitution() {
    let plan = scalar_three_leaf_cleanup_plan();
    let artifact = build_object_artifact(&plan).expect("three-leaf artifact");
    let image = emit_executable_image(&artifact, 1).expect("three-leaf image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("three-leaf installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    assert_eq!(
        record.functions()[0].scalar_control_affine_cleanups.len(),
        3
    );

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "scalar_control_affine_cleanups[0].psi_edge",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].psi_edge = edge_id(9);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].structural_types",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].structural_types = extra_type_catalog();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].locals::insert",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0]
                    .locals
                    .push(extra_local());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].actions::drop",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].actions.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].actions::insert",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9)),
                );
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].code_offset",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].code_offset += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups[0].byte_count",
            Box::new(|row| {
                row.scalar_control_affine_cleanups[0].byte_count -= 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups::reorder",
            Box::new(|row| {
                row.scalar_control_affine_cleanups.swap(0, 1);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups::drop",
            Box::new(|row| {
                row.scalar_control_affine_cleanups.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_control_affine_cleanups::insert-duplicate",
            Box::new(|row| {
                let cleanup = row.scalar_control_affine_cleanups[0].clone();
                row.scalar_control_affine_cleanups.push(cleanup);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 0, mutate, expected);
    }
}

/// The scalar-transport axes — the installed scalar ABI, the mixed
/// structural/scalar ABI, the Unit-returning parameter ABI, and the
/// structural-call/scalar-return evidence — are each independently
/// authenticated. Canonical substitutions still encode, recompute a distinct
/// installation identity, and are rejected by independent replay; rows that
/// violate the transport joins are rejected at canonical encoding.
#[test]
fn installation_function_scalar_transport_rejects_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert!(authentic.scalar_abi.is_none());
    assert!(authentic.parameter_abi.is_none());
    assert!(authentic.structural_call_scalar_return.is_none());

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "scalar_abi::insert-canonical",
            Box::new(|row| {
                row.scalar_abi = Some(canonical_scalar_abi());
            }),
        ),
        (
            "scalar_abi::insert-canonical-alt-value",
            Box::new(|row| {
                let mut abi = canonical_scalar_abi();
                abi.result.value = value_id(99);
                row.scalar_abi = Some(abi);
            }),
        ),
        (
            "parameter_abi::insert-canonical",
            Box::new(|row| {
                row.parameter_abi = Some(canonical_parameter_abi());
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        // A canonical scalar ABI requires placements the native call plan
        // recomputes exactly; a substituted placement fails the join.
        (
            "scalar_abi::insert-noncanonical",
            Box::new(|row| {
                let mut abi = canonical_scalar_abi();
                abi.parameters[0].placement = empty_placement();
                row.scalar_abi = Some(abi);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // The mixed ABI requires scalar stack evidence and an exact structural
        // roster join; neither exists on the Unit-stack row.
        (
            "mixed_structural_scalar_abi::insert-on-unit-row",
            Box::new(|row| {
                row.mixed_structural_scalar_abi = Some(fabricated_mixed_abi());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // Entry spills are retained only alongside continuation custody.
        (
            "parameter_abi::insert-with-spills",
            Box::new(|row| {
                let mut abi = canonical_parameter_abi();
                abi.entry_register_spills = vec![machine_code::UnitEntryRegisterSpillRecord {
                    source_value: value_id(45),
                    parameter_index: 0,
                    register: calling_conventions::MachineRegister::X86Rdi,
                    byte_offset: 0,
                    code_offset: 0,
                    byte_count: 5,
                }];
                row.parameter_abi = Some(abi);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
        // The structural-call/scalar-return evidence is exact only for the
        // bounded carrier: one Operation-owned scalar-result call joined to
        // matching attribution and cleanup rows.
        (
            "structural_call_scalar_return::insert",
            Box::new(|row| {
                row.structural_call_scalar_return = Some(structural_scalar_return());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
    assert_substitution_rejected_at_encoding(
        "structural_call_scalar_return::insert-on-unit-caller",
        &record,
        0,
        |row| {
            row.structural_call_scalar_return = Some(structural_scalar_return());
        },
        InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
    );
    assert_substitution_rejected_at_encoding(
        "parameter_abi::insert-on-callee",
        &record,
        0,
        |row| {
            row.parameter_abi = Some(canonical_parameter_abi());
        },
        InstallationError::InvalidInternalUnitCall(machine_id(3)),
    );

    // On the scalar-stack row the canonical scalar ABI and the exactly
    // matching mixed ABI still encode; independent replay rejects both
    // against the image that retains neither.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image = emit_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");

    assert_substitution_rejected_by_replay(
        "scalar_abi::insert-canonical-on-scalar-row",
        &scalar_record,
        &scalar_image,
        &scalar_fingerprint,
        2,
        |row| {
            row.scalar_abi = Some(canonical_scalar_abi());
        },
    );
    assert_substitution_rejected_by_replay(
        "mixed_structural_scalar_abi::insert-matching",
        &scalar_record,
        &scalar_image,
        &scalar_fingerprint,
        2,
        |row| {
            let abi = matching_mixed_abi(row);
            row.mixed_structural_scalar_abi = Some(abi);
        },
    );
}

/// Every representable leaf of an installed Unit continuation row is
/// authenticated: the source and target blocks, each binding's parameter and
/// argument identities, and inserted or dropped bindings still encode and are
/// rejected by independent replay, while the operation ordinals, a revisited
/// target block, the nested zero-byte cleanup's edge, interval, locals,
/// structural types and actions, bindings naming live or unknown values or
/// mismatched types, and dropped or duplicated continuation rows are rejected
/// at canonical encoding.
#[test]
fn installation_function_unit_continuations_reject_every_one_field_substitution() {
    let plan = continuation_unit_call_plan();
    let artifact = build_object_artifact(&plan).expect("continuation artifact");
    let image = emit_executable_image(&artifact, 3).expect("continuation image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("continuation installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[0].clone();
    assert_eq!(authentic.unit_continuations.len(), 1);
    assert_eq!(authentic.unit_continuations[0].operation_ordinal, 1);
    assert_eq!(
        authentic.unit_continuations[0].successor_operation_ordinal,
        2
    );
    assert_eq!(authentic.unit_continuations[0].bindings.len(), 1);
    assert_eq!(authentic.unit_continuations[0].cleanup.byte_count, 0);
    assert!(authentic.unit_affine_cleanup.is_some());

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "unit_continuations[0].source_block",
            Box::new(|row| {
                row.unit_continuations[0].source_block = block_id(9);
            }),
        ),
        (
            "unit_continuations[0].target_block",
            Box::new(|row| {
                row.unit_continuations[0].target_block = block_id(9);
            }),
        ),
        (
            "unit_continuations[0].bindings[0].parameter",
            Box::new(|row| {
                row.unit_continuations[0].bindings[0].parameter = value_id(52);
            }),
        ),
        // The other live integer parameter is a representable argument.
        (
            "unit_continuations[0].bindings[0].argument",
            Box::new(|row| {
                row.unit_continuations[0].bindings[0].argument = value_id(46);
            }),
        ),
        (
            "unit_continuations[0].bindings::insert",
            Box::new(|row| {
                row.unit_continuations[0]
                    .bindings
                    .push(abstract_operations::ValueBinding {
                        parameter: value_id(51),
                        argument: value_id(46),
                        scalar_type: i32_scalar(),
                    });
            }),
        ),
        (
            "unit_continuations[0].bindings::drop",
            Box::new(|row| {
                row.unit_continuations[0].bindings.pop();
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            0,
            mutate,
        );
    }

    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        // The call owns ordinal 0; the continuation must chain at ordinal 1.
        (
            "unit_continuations[0].operation_ordinal",
            Box::new(|row| {
                row.unit_continuations[0].operation_ordinal = 0;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].successor_operation_ordinal",
            Box::new(|row| {
                row.unit_continuations[0].successor_operation_ordinal = 4;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // A continuation may not loop back to a block already on the chain.
        (
            "unit_continuations[0].target_block::revisit",
            Box::new(|row| {
                row.unit_continuations[0].target_block = row.unit_continuations[0].source_block;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // The nested cleanup edge must carry its own zero-byte attribution.
        (
            "unit_continuations[0].cleanup.psi_edge",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.psi_edge = edge_id(8);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // Nor may it reuse the final return edge.
        (
            "unit_continuations[0].cleanup.psi_edge::returned-edge",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.psi_edge =
                    row.unit_affine_cleanup.as_ref().expect("cleanup").psi_edge;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].cleanup.code_offset",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.code_offset += 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].cleanup.byte_count",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.byte_count = 1;
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].cleanup.locals::insert",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.locals.push(extra_local());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // The continuation cleanup retains exactly the returned cleanup's
        // structural type roster.
        (
            "unit_continuations[0].cleanup.structural_types",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.structural_types = extra_type_catalog();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // Residual discards name a partially consumed root; this caller moves
        // nothing across the boundary.
        (
            "unit_continuations[0].cleanup.actions::insert-residual",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place: place_id(1),
                            path: Vec::new(),
                            structural_type: structural_type(1),
                        },
                    ),
                );
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].cleanup.actions::insert-root",
            Box::new(|row| {
                row.unit_continuations[0].cleanup.actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9)),
                );
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // A binding may not rename a value already live across the boundary.
        (
            "unit_continuations[0].bindings[0].parameter::live-value",
            Box::new(|row| {
                row.unit_continuations[0].bindings[0].parameter = value_id(46);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].bindings[0].argument::unknown-value",
            Box::new(|row| {
                row.unit_continuations[0].bindings[0].argument = value_id(99);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].bindings[0].scalar_type",
            Box::new(|row| {
                row.unit_continuations[0].bindings[0].scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations[0].bindings::insert-duplicate",
            Box::new(|row| {
                let binding = row.unit_continuations[0].bindings[0];
                row.unit_continuations[0].bindings.push(binding);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        // Dropping the row strands the zero-byte edge attribution the record
        // still carries; duplicating it breaks the strict ordinal chain.
        (
            "unit_continuations::drop",
            Box::new(|row| {
                row.unit_continuations.pop();
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "unit_continuations::insert-duplicate",
            Box::new(|row| {
                let continuation = row.unit_continuations[0].clone();
                row.unit_continuations.push(continuation);
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 0, mutate, expected);
    }
}

/// Authenticated one-field mutation coverage for the continuation caller's
/// retained `parameter_abi`: the call plan, both scalar parameters, and both
/// entry register spills are canonical projections the record shape recomputes
/// from the parameter scalar types and independently rejoins to the frame
/// prologue and the continuation bindings. The unbound second parameter's
/// same-shape scalar type is the bounded slack — it still encodes, recomputes
/// a distinct installation fingerprint, and is rejected by independent replay;
/// every other substitution is rejected at canonical encoding.
#[test]
fn installation_function_parameter_abi_rejects_every_one_field_substitution() {
    let plan = continuation_unit_call_plan();
    let artifact = build_object_artifact(&plan).expect("continuation artifact");
    let image = emit_executable_image(&artifact, 3).expect("continuation image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("continuation installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[0].clone();
    let abi = authentic
        .parameter_abi
        .as_ref()
        .expect("retained parameter ABI");
    assert_eq!(abi.parameters.len(), 2);
    assert_eq!(abi.parameters[0].value, value_id(45));
    assert_eq!(abi.parameters[1].value, value_id(46));
    assert_eq!(abi.entry_register_spills.len(), 2);
    assert_eq!(abi.entry_register_spills[0].byte_offset, 0);
    assert_eq!(abi.entry_register_spills[1].byte_offset, 8);

    let u32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).expect("i64"));

    // The second parameter is not bound by any continuation: a same-shape
    // scalar type still satisfies the recomputed caller plan and every join,
    // so the substitution encodes, decodes exactly, recomputes a distinct
    // identity, and is rejected by independent replay against the unchanged
    // image.
    assert_substitution_rejected_by_replay(
        "parameter_abi.parameters[1].scalar_type::u32",
        &record,
        &image,
        &authentic_fingerprint,
        0,
        |row| {
            row.parameter_abi
                .as_mut()
                .expect("parameter ABI")
                .parameters[1]
                .scalar_type = u32_scalar;
        },
    );

    // Every other leaf is a canonical projection: the caller plan is
    // recomputed from the parameter scalar types, each placement and the
    // distinct value identities rejoin the plan and the spill roster, each
    // spill rejoins its parameter, register, frame offset and code interval,
    // and the continuation bindings rejoin the bound parameter's type.
    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(1));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "parameter_abi::drop",
            Box::new(|row| {
                row.parameter_abi = None;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.policy",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .policy = CallingPolicy::MicrosoftX64;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters::drop",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .clear();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters::insert",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .push(register_placement());
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters[0]",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters[0] = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters[1]",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters[1] = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters::swap",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .swap(0, 1);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let placement = abi.call_plan.parameters[0].clone();
                abi.call_plan.parameters.push(placement);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.result",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .result = Some(register_placement());
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.callback_materializations::insert",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .callback_materializations
                    .push(calling_conventions::CallbackMaterialization {
                        binder: calling_conventions::StaticMachineBinderId::new(1).expect("binder"),
                        destination: calling_conventions::NativePlace::Parameter(
                            calling_conventions::NativeParameterId::new(1).expect("parameter"),
                        ),
                    });
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.ordinary_clobbers",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .ordinary_clobbers = calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::X86Rbx,
                ]);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.stack_alignment",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .stack_alignment = 8;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.shadow_bytes",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .shadow_bytes = 32;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.call_plan.entry_control",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .entry_control = calling_conventions::EntryControl::InterruptReturn;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].value",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .value = value_id(47);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].value::other_parameter",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .value = value_id(46);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].scalar_type::u32",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = u32_scalar;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].scalar_type::i64",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = i64_scalar;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].scalar_type::boolean",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[0].placement",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .placement = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[1].value",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .value = value_id(47);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[1].value::other_parameter",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .value = value_id(45);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[1].scalar_type::i64",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .scalar_type = i64_scalar;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[1].scalar_type::boolean",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters[1].placement",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .placement = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters::insert",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .push(ScalarAbiValue {
                        value: value_id(47),
                        scalar_type: i32_scalar(),
                        placement: register_placement(),
                    });
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters::drop",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .pop();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters::swap",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .swap(0, 1);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let parameter = abi.parameters[0].clone();
                abi.parameters.push(parameter);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].source_value",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .source_value = value_id(47);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].parameter_index",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .parameter_index = 1;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].register",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .register = calling_conventions::MachineRegister::X86Rdx;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].byte_offset",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .byte_offset = 8;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].code_offset",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .code_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[0].byte_count",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .byte_count = 4;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].source_value",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .source_value = value_id(45);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].parameter_index",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .parameter_index = 0;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].register",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .register = calling_conventions::MachineRegister::X86Rdx;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].byte_offset",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .byte_offset = 0;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].code_offset",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .code_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills[1].byte_count",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .byte_count = 6;
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills::insert",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills
                    .push(machine_code::UnitEntryRegisterSpillRecord {
                        source_value: value_id(47),
                        parameter_index: 2,
                        register: calling_conventions::MachineRegister::X86Rdx,
                        byte_offset: 16,
                        code_offset: 14,
                        byte_count: 5,
                    });
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills::drop",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills
                    .pop();
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills::swap",
            Box::new(|row| {
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills
                    .swap(0, 1);
            }),
            invalid.clone(),
        ),
        (
            "parameter_abi.entry_register_spills::insert-duplicate",
            Box::new(|row| {
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let spill = abi.entry_register_spills[0];
                abi.entry_register_spills.push(spill);
            }),
            invalid.clone(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 0, mutate, expected);
    }
}

/// Authenticated one-field mutation coverage for a scalar-cleanup row's
/// retained `mixed_structural_scalar_abi`: the call plan is recomputed from
/// the scalar and structural shapes, every scalar parameter and the result
/// rejoin their plan placements and carry distinct value identities, and the
/// structural parameter rejoins the retained scalar-structural roster row and
/// home source. Semantic scalar value identities, the unbound same-shape
/// scalar types, the structural parameter's projected qualifications, and a
/// dropped ABI are the bounded slack; every other substitution is rejected at
/// canonical encoding.
#[test]
fn installation_function_mixed_abi_rejects_every_one_field_substitution() {
    let mut plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut plan.functions[2]);
    plan.functions[2].mixed_structural_scalar_abi = Some(retained_mixed_abi(&plan.functions[2]));
    let artifact = build_object_artifact(&plan).expect("mixed-ABI artifact");
    let image = emit_executable_image(&artifact, 3).expect("mixed-ABI image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("mixed-ABI installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    let abi = authentic
        .mixed_structural_scalar_abi
        .as_ref()
        .expect("retained mixed ABI");
    assert_eq!(abi.scalar_parameters.len(), 1);
    assert_eq!(abi.scalar_parameters[0].value, value_id(51));
    assert_eq!(abi.structural_parameters.len(), 1);
    assert_eq!(abi.result.value, value_id(49));

    let u32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).expect("i64"));

    let still_encodes: Vec<(&'static str, Box<dyn Fn(&mut InstalledFunction)>)> = vec![
        (
            "mixed_structural_scalar_abi::drop",
            Box::new(|row| {
                row.mixed_structural_scalar_abi = None;
            }),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].value",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .value = value_id(59);
            }),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].scalar_type::u32",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = u32_scalar;
            }),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].projected_qualifications::insert",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .projected_qualifications
                    .push(terminal_psi::StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("projected".to_string())],
                        domain: semantic_vocabulary::StructuralDomainId::new(1).expect("domain"),
                    });
            }),
        ),
        (
            "mixed_structural_scalar_abi.result.value",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .value = value_id(59);
            }),
        ),
        (
            "mixed_structural_scalar_abi.result.scalar_type::u32",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = u32_scalar;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        assert_substitution_rejected_by_replay(
            field,
            &record,
            &image,
            &authentic_fingerprint,
            2,
            mutate,
        );
    }

    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(3));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "mixed_structural_scalar_abi.call_plan.policy",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .policy = CallingPolicy::MicrosoftX64;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters::drop",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .clear();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters::insert",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .push(register_placement());
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters[0]",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters[0] = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters[1]",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters[1] = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters::swap",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .swap(0, 1);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let placement = abi.call_plan.parameters[0].clone();
                abi.call_plan.parameters.push(placement);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.result",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .result = None;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.callback_materializations::insert",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .callback_materializations
                    .push(calling_conventions::CallbackMaterialization {
                        binder: calling_conventions::StaticMachineBinderId::new(1).expect("binder"),
                        destination: calling_conventions::NativePlace::Parameter(
                            calling_conventions::NativeParameterId::new(1).expect("parameter"),
                        ),
                    });
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.ordinary_clobbers",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .ordinary_clobbers = calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::X86Rbx,
                ]);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.stack_alignment",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .stack_alignment = 8;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.shadow_bytes",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .shadow_bytes = 32;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.call_plan.entry_control",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .entry_control = calling_conventions::EntryControl::InterruptReturn;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].value::result_collision",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .value = value_id(49);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].scalar_type::i64",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = i64_scalar;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].scalar_type::boolean",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters[0].placement",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .placement = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters::insert",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters
                    .push(ScalarAbiValue {
                        value: value_id(59),
                        scalar_type: i32_scalar(),
                        placement: register_placement(),
                    });
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters::drop",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters
                    .clear();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.scalar_parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let parameter = abi.scalar_parameters[0].clone();
                abi.scalar_parameters.push(parameter);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].place",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .place = place_id(9);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].structural_type",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .structural_type = structural_type(9);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].multiplicity",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .multiplicity = StructuralMultiplicity::Unrestricted;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].access",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].shape",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .shape = ValueShape::integer(8, 8);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters[0].placement",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .placement = register_placement();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters::insert",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters
                    .push(target_operations::TargetStructuralParameter {
                        place: place_id(9),
                        structural_type: structural_type(9),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        projected_qualifications: Vec::new(),
                        shape: ValueShape::integer(0, 1),
                        placement: empty_placement(),
                    });
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters::drop",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters
                    .clear();
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.structural_parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let parameter = abi.structural_parameters[0].clone();
                abi.structural_parameters.push(parameter);
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.result.scalar_type::i64",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = i64_scalar;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.result.scalar_type::boolean",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.result.placement",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .placement = ValuePlacement {
                    shape: ValueShape::integer(4, 4),
                    locations: vec![ValueLocation::Register {
                        register: calling_conventions::MachineRegister::X86Rsi,
                        value_byte_offset: 0,
                        byte_size: 4,
                    }],
                };
            }),
            invalid.clone(),
        ),
        (
            "mixed_structural_scalar_abi.result.value::parameter_collision",
            Box::new(|row| {
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .value = value_id(51);
            }),
            invalid.clone(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 2, mutate, expected);
    }
}

/// Authenticated one-field mutation coverage for the caller's retained
/// `structural_call_scalar_return`: every leaf rejoins the one
/// Operation-owned scalar-result call, its attribution, and the empty
/// return-edge cleanup, so a one-field substitution is rejected at canonical
/// encoding; dropping the row still encodes, recomputes a distinct
/// installation fingerprint, and is rejected by independent replay against
/// the unchanged image.
#[test]
fn installation_function_structural_call_scalar_return_rejects_every_one_field_substitution() {
    let plan = structural_call_scalar_return_plan();
    let artifact = build_object_artifact(&plan).expect("structural-call artifact");
    let image = emit_executable_image(&artifact, 3).expect("structural-call image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("structural-call installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[0].clone();
    let returned = authentic
        .structural_call_scalar_return
        .as_ref()
        .expect("retained structural-call scalar return");
    assert_eq!(returned.psi_edge, edge_id(1));
    assert_eq!(returned.psi_operation, operation_id(1));
    assert_eq!(returned.source_value, value_id(80));
    assert_eq!(returned.callee, machine_id(2));

    assert_substitution_rejected_by_replay(
        "structural_call_scalar_return::drop",
        &record,
        &image,
        &authentic_fingerprint,
        0,
        |row| {
            row.structural_call_scalar_return = None;
        },
    );

    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(1));
    let rejected: Vec<(
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    )> = vec![
        (
            "structural_call_scalar_return.psi_edge",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .psi_edge = edge_id(8);
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.psi_operation",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .psi_operation = operation_id(9);
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.source_value",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .source_value = value_id(9);
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.scalar_type::u32",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.scalar_type::boolean",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.callee::unknown",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .callee = machine_id(9);
            }),
            invalid.clone(),
        ),
        (
            "structural_call_scalar_return.callee::caller",
            Box::new(|row| {
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .callee = machine_id(1);
            }),
            invalid.clone(),
        ),
    ];
    for (field, mutate, expected) in rejected {
        assert_substitution_rejected_at_encoding(field, &record, 0, mutate, expected);
    }
}
