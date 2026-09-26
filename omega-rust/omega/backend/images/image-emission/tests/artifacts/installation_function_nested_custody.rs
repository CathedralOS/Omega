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

use super::installation_field_substitution_fields::{
    foreign_forwarded_parameter_call_record, installation_record_custody,
};
use super::installation_function_nested_custody_fields::{
    AffineCleanupFieldForTest, AffineCleanupScalarRecordFieldForTest,
    AffineScalarRecordsFieldForTest, ForeignCallStackRowFieldForTest, MixedAbiFieldForTest,
    NestedCallStacksFieldForTest, NestedCallStacksSelectedFieldForTest, ParameterAbiFieldForTest,
    ParameterAndHomeRowsFieldForTest, ScalarCallStacksFieldForTest,
    ScalarControlCleanupsFieldForTest, ScalarStoreRowsFieldForTest,
    ScalarStructuralRowsFieldForTest, ScalarTransportFieldForTest,
    ScalarTransportScalarRecordFieldForTest, StoreRowsFieldForTest,
    StoreRowsScalarRecordFieldForTest, StructuralCallScalarReturnFieldForTest,
    StructuralStoreRowsFieldForTest, UnitContinuationsFieldForTest, UnitScalarRowsFieldForTest,
    WriteOnlyStoreRowsFieldForTest,
};
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
    decode_installation_record, emit_direct_executable_image, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use machine_code::{
    MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, StructuralCallScalarReturnEvidence, UnitAffineCleanupRecord,
    UnitAffineScalarRecordEstablishmentRecord, UnitContinuationRecord, UnitIntegerConstantRecord,
    UnitParameterHomeRecord, UnitParameterRecord, UnitStackEvidence,
};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
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
            qualification_establishments: Vec::new(),
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
        qualification_establishments: Vec::new(),
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

/// The independent checker every nested-custody family shares: a
/// substituted record must encode canonically, round-trip through the codec,
/// recompute an installation identity distinct from the authentic record, and
/// then replay against the unchanged emitted image. An encoding canonicality
/// error surfaces before the identity and replay steps, so both leg kinds
/// reduce to the checker's exact error.
fn nested_custody_check<'a>(
    image: &'a image_emission::ExecutableImage,
    authentic_fingerprint: &'a image_emission::InstallationFingerprint,
) -> impl Fn(&InstallationRecord) -> Result<InstallationRecord, InstallationError> + 'a {
    move |record| {
        let bytes = encode_installation_record(record)?;
        let replayed = decode_installation_record(&bytes)?;
        assert_eq!(&replayed, record, "codec preserves the substituted record");
        assert_ne!(
            installation_fingerprint(&replayed)?,
            *authentic_fingerprint,
            "recomputed identity differs from the authentic record"
        );
        validate_installation_record(&replayed, image)?;
        Ok(replayed)
    }
}

/// The checker for the affine scalar-record fixture, whose consuming
/// argument's source placement carries an 8-byte shape with no locations —
/// exactly what `affine_scalar_record_source` requires — and is not
/// wire-decodable today, so the containing record cannot round-trip through
/// `decode_installation_record`. The substitution still encodes canonically,
/// recomputes a distinct installation identity, and independent replay
/// against the unchanged image judges the encoded record itself.
fn nested_custody_undecodable_check<'a>(
    image: &'a image_emission::ExecutableImage,
    authentic_fingerprint: &'a image_emission::InstallationFingerprint,
) -> impl Fn(&InstallationRecord) -> Result<InstallationRecord, InstallationError> + 'a {
    move |record| {
        encode_installation_record(record)?;
        assert_ne!(
            installation_fingerprint(record)?,
            *authentic_fingerprint,
            "recomputed identity differs from the authentic record"
        );
        validate_installation_record(record, image)?;
        Ok(record.clone())
    }
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
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
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

    let substitute = |record: &mut InstallationRecord,
                      field: NestedCallStacksFieldForTest,
                      _donor: &InstallationRecord| {
        use NestedCallStacksFieldForTest as Field;
        match field {
            Field::UnitCallStacksOwner => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }
            Field::UnitCallStacksTarget => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].target = machine_id(2);
            }
            Field::UnitCallStacksTextOffset => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].text_offset += 1;
            }
            Field::UnitCallStacksDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks.pop();
            }
            Field::UnitCallStacksInsertDistinct => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks
                    .push(image_emission::ObjectUnitCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: tail_text_offset,
                        active_frame_bytes: 32,
                        transient_bytes: 16,
                        caller_live_bytes: 48,
                    });
            }
            Field::UnitCallStacksTargetUnknownMachine => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].target = machine_id(99);
            }
            Field::UnitCallStacksActiveFrameBytes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].active_frame_bytes += 1;
            }
            Field::UnitCallStacksTransientBytes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].transient_bytes += 1;
            }
            Field::UnitCallStacksCallerLiveBytes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks[0].caller_live_bytes += 1;
            }
            Field::UnitCallStacksInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let call = row.unit_call_stacks[0];
                row.unit_call_stacks.push(call);
            }
            // A scalar call-stack row cannot be attached to a Unit-stack row:
            // scalar calls require retained scalar stack evidence.
            Field::ScalarCallStacksInsertOnUnitRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks
                    .push(image_emission::ObjectScalarCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: 4,
                        caller_live_bytes: 8,
                    });
            }
            // Foreign calls require the provider-plan report identity to appear in
            // the retained selected-provider closure; the fixture selects none.
            Field::ForeignCallStacksInsertUnselected => {
                let row = &mut record.functions_mut_for_test()[2];
                row.foreign_call_stacks.push(foreign_call(4));
            }
        }
    };
    let outcome = |field: NestedCallStacksFieldForTest| {
        use NestedCallStacksFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::UnitCallStacksOwner
            | Field::UnitCallStacksTarget
            | Field::UnitCallStacksTextOffset
            | Field::UnitCallStacksDrop
            | Field::UnitCallStacksInsertDistinct => InstallationError::ImageBindingMismatch,
            Field::UnitCallStacksTargetUnknownMachine => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitCallStacksActiveFrameBytes => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitCallStacksTransientBytes => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitCallStacksCallerLiveBytes => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitCallStacksInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarCallStacksInsertOnUnitRow => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ForeignCallStacksInsertUnselected => {
                InstallationError::ProviderSettlementClosureMismatch
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation nested call stacks / record",
        fields: NestedCallStacksFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });

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

    let substitute = |record: &mut InstallationRecord,
                      field: NestedCallStacksSelectedFieldForTest,
                      _donor: &InstallationRecord| {
        use NestedCallStacksSelectedFieldForTest as Field;
        match field {
            Field::ForeignCallStacksInsertAdmitted => {
                let row = &mut record.functions_mut_for_test()[2];
                row.foreign_call_stacks
                    .push(foreign_call(function_text_offset + 4));
            }
        }
    };
    let outcome = |field: NestedCallStacksSelectedFieldForTest| {
        use NestedCallStacksSelectedFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ForeignCallStacksInsertAdmitted => InstallationError::ImageBindingMismatch,
        })
    };
    let check = nested_custody_check(&image, &selected_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation nested call stacks / selected",
        fields: NestedCallStacksSelectedFieldForTest::INVENTORY,
        honest: &|| selected.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("foreign call PE image");
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

    let substitute = |record: &mut InstallationRecord,
                      field: ForeignCallStackRowFieldForTest,
                      _donor: &InstallationRecord| {
        use ForeignCallStackRowFieldForTest as Field;
        match field {
            Field::Owner => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }
            Field::TextOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].text_offset += 1;
            }
            Field::CallerLiveBytes => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].caller_live_bytes += 1;
            }
            Field::ProviderPlanReportIdentity => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].provider_plan_report_identity = 97;
            }
            Field::ContributionReportIdentity => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].contribution_report_identity =
                    task_plans::AdmittedStackContributionReportId::from_normalized_identity(
                        0xc011_e541,
                    )
                    .expect("contribution report");
            }
            Field::ContributionCommitment => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].contribution_commitment =
                    task_plans::SameStackContributionCommitment::from_digest([0xaa; 32]);
            }
            Field::ContributionBytes => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].contribution_bytes = 128;
            }
            Field::ContributionAlignment => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].contribution_alignment = 8;
            }
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks.pop();
            }
            Field::ProviderPlanReportIdentityUnselected => {
                let row = &mut record.functions_mut_for_test()[0];
                row.foreign_call_stacks[0].provider_plan_report_identity = 55;
            }
        }
    };
    let outcome = |field: ForeignCallStackRowFieldForTest| {
        use ForeignCallStackRowFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::Owner
            | Field::TextOffset
            | Field::CallerLiveBytes
            | Field::ProviderPlanReportIdentity
            | Field::ContributionReportIdentity
            | Field::ContributionCommitment
            | Field::ContributionBytes
            | Field::ContributionAlignment
            | Field::Drop => InstallationError::ImageBindingMismatch,
            Field::ProviderPlanReportIdentityUnselected => {
                InstallationError::ProviderSettlementClosureMismatch
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation foreign call stack row / record",
        fields: ForeignCallStackRowFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("scalar cleanup image");
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

    let substitute = |record: &mut InstallationRecord,
                      field: ScalarCallStacksFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarCallStacksFieldForTest as Field;
        match field {
            Field::ScalarCallStacksOwner => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks[0].owner = CallSiteOwner::Operation(operation_id(9));
            }
            Field::ScalarCallStacksTarget => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks[0].target = machine_id(2);
            }
            Field::ScalarCallStacksTextOffset => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks[0].text_offset += 1;
            }
            Field::ScalarCallStacksCallerLiveBytes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks[0].caller_live_bytes += 1;
            }
            Field::ScalarCallStacksDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks.pop();
            }
            Field::ScalarCallStacksInsertDistinct => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks
                    .push(image_emission::ObjectScalarCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: tail_text_offset,
                        caller_live_bytes: 8,
                    });
            }
            Field::ScalarCallStacksTargetUnknownMachine => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_call_stacks[0].target = machine_id(99);
            }
            Field::ScalarCallStacksInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let call = row.scalar_call_stacks[0];
                row.scalar_call_stacks.push(call);
            }
            // A Unit call-stack row cannot be attached to a scalar-stack row.
            Field::UnitCallStacksInsertOnScalarRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_call_stacks
                    .push(image_emission::ObjectUnitCallStack {
                        owner: CallSiteOwner::Operation(operation_id(9)),
                        target: machine_id(1),
                        text_offset: 4,
                        active_frame_bytes: 8,
                        transient_bytes: 0,
                        caller_live_bytes: 8,
                    });
            }
            // Foreign contribution custody is only canonical beneath a Unit stack.
            Field::ForeignCallStacksInsertOnScalarRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.foreign_call_stacks.push(foreign_call(4));
            }
        }
    };
    let outcome = |field: ScalarCallStacksFieldForTest| {
        use ScalarCallStacksFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarCallStacksOwner
            | Field::ScalarCallStacksTarget
            | Field::ScalarCallStacksTextOffset
            | Field::ScalarCallStacksCallerLiveBytes
            | Field::ScalarCallStacksDrop
            | Field::ScalarCallStacksInsertDistinct => InstallationError::ImageBindingMismatch,
            Field::ScalarCallStacksTargetUnknownMachine => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarCallStacksInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitCallStacksInsertOnScalarRow => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ForeignCallStacksInsertOnScalarRow => {
                InstallationError::ProviderSettlementClosureMismatch
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar call stacks / record",
        fields: ScalarCallStacksFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
}

/// The semantic parameter roster and its ABI home roster are joined to each
/// other and to the retained stored-dynamic argument custody: every leaf
/// substitution is rejected at canonical encoding except the physical home
/// location axes, which still encode and are rejected by independent replay.
#[test]
fn installation_function_parameter_and_home_rows_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.unit_parameters.len(), 1);
    assert_eq!(authentic.unit_parameter_homes.len(), 1);

    let substitute = |record: &mut InstallationRecord,
                      field: ParameterAndHomeRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use ParameterAndHomeRowsFieldForTest as Field;
        match field {
            Field::ParameterHomesLocation => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }
            Field::ParameterHomesIndirect => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].indirect = true;
            }
            Field::ParametersPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters[0].place = place_id(9);
            }
            Field::ParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters[0].structural_type = structural_type(9);
            }
            Field::ParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
            }
            Field::ParametersAccess => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            Field::ParametersShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters[0].shape = ValueShape::integer(8, 8);
            }
            Field::ParametersDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameters.pop();
            }
            Field::ParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let parameter = row.unit_parameters[0];
                row.unit_parameters.push(parameter);
            }
            // The stored-dynamic argument rejoins its source home by exact place
            // identity, so the home's place and roster membership are bound by the
            // record-level custody row before the scalar-cleanup join runs.
            Field::ParameterHomesPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].place = place_id(9);
            }
            Field::ParameterHomesDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes.pop();
            }
            Field::ParameterHomesStructuralType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }
            Field::ParameterHomesMultiplicity => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Unrestricted;
            }
            Field::ParameterHomesAccess => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }
            Field::ParameterHomesShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].shape = ValueShape::integer(8, 8);
            }
            Field::ParameterHomesSource => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_parameter_homes[0].source = register_placement();
            }
            Field::ParameterHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }
        }
    };
    let outcome = |field: ParameterAndHomeRowsFieldForTest| {
        use ParameterAndHomeRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ParameterHomesLocation | Field::ParameterHomesIndirect => {
                InstallationError::ImageBindingMismatch
            }
            Field::ParametersPlace => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersStructuralType => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParametersMultiplicity => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParametersAccess => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersShape => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersDrop => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesPlace => {
                InstallationError::InvalidStoredDynamicCall(machine_id(3))
            }
            Field::ParameterHomesDrop => InstallationError::InvalidStoredDynamicCall(machine_id(3)),
            Field::ParameterHomesStructuralType => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesMultiplicity => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesAccess => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesShape => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesSource => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation parameter and home rows / record",
        fields: ParameterAndHomeRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("scalar cleanup image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("scalar cleanup installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.scalar_structural_parameters.len(), 1);
    assert_eq!(authentic.scalar_structural_parameter_homes.len(), 1);

    let substitute = |record: &mut InstallationRecord,
                      field: ScalarStructuralRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarStructuralRowsFieldForTest as Field;
        match field {
            Field::ParameterHomesSource => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].source = register_placement();
            }
            Field::ParameterHomesLocation => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }
            Field::ParameterHomesIndirect => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].indirect = true;
            }
            Field::ParametersPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters[0].place = place_id(9);
            }
            Field::ParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters[0].structural_type = structural_type(9);
            }
            Field::ParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Unrestricted;
            }
            Field::ParametersAccess => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            Field::ParametersShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters[0].shape = ValueShape::integer(8, 8);
            }
            Field::ParametersDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameters.pop();
            }
            Field::ParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let parameter = row.scalar_structural_parameters[0];
                row.scalar_structural_parameters.push(parameter);
            }
            Field::ParameterHomesPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].place = place_id(9);
            }
            Field::ParameterHomesStructuralType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].structural_type = structural_type(9);
            }
            Field::ParameterHomesMultiplicity => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].multiplicity =
                    StructuralMultiplicity::Unrestricted;
            }
            Field::ParameterHomesAccess => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }
            Field::ParameterHomesShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes[0].shape = ValueShape::integer(8, 8);
            }
            Field::ParameterHomesDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_parameter_homes.pop();
            }
            Field::ParameterHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let home = row.scalar_structural_parameter_homes[0].clone();
                row.scalar_structural_parameter_homes.push(home);
            }
        }
    };
    let outcome = |field: ScalarStructuralRowsFieldForTest| {
        use ScalarStructuralRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ParameterHomesSource
            | Field::ParameterHomesLocation
            | Field::ParameterHomesIndirect => InstallationError::ImageBindingMismatch,
            Field::ParametersPlace => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersStructuralType => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParametersMultiplicity => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParametersAccess => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersShape => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersDrop => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParametersInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesPlace => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesStructuralType => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesMultiplicity => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesAccess => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesShape => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterHomesDrop => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ParameterHomesInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar structural rows / record",
        fields: ScalarStructuralRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
}

/// Durable scalar homes, zero-code integer constants, and affine-record
/// establishments are authenticated per row: identity and byte-offset drift
/// still encodes and is rejected by independent replay, while scalar-type and
/// shape drift violates the canonical home-shape join.
#[test]
fn installation_function_unit_scalar_rows_reject_every_one_field_substitution() {
    let plan = stored_dynamic_call_plan();
    let artifact = build_object_artifact(&plan).expect("stored dynamic artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert_eq!(authentic.unit_scalar_homes.len(), 1);
    assert!(authentic.unit_integer_constants.is_empty());
    assert!(authentic.unit_affine_scalar_records.is_empty());

    let substitute = |record: &mut InstallationRecord,
                      field: UnitScalarRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use UnitScalarRowsFieldForTest as Field;
        match field {
            Field::ScalarHomesDefiningOperation => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes[0].defining_operation = operation_id(9);
            }
            Field::ScalarHomesSourceValue => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes[0].source_value = value_id(9);
            }
            Field::ScalarHomesByteOffset => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes[0].byte_offset = 24;
            }
            Field::ScalarHomesDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes.pop();
            }
            // The constant roster is empty in this image; inserting a canonical
            // zero-code row still encodes and independent replay rejects it.
            Field::IntegerConstantsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_integer_constants.push(integer_constant());
            }
            Field::ScalarHomesScalarType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes[0].scalar_type = ScalarType::Boolean;
            }
            Field::ScalarHomesShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_scalar_homes[0].shape = ValueShape::integer(8, 8);
            }
            Field::ScalarHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let home = row.unit_scalar_homes[0];
                row.unit_scalar_homes.push(home);
            }
            // The fabricated establishment row fails the exact affine-record join:
            // its producer, result, and field identities cannot be bound to any
            // retained custody in this image.
            Field::AffineScalarRecordsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_scalar_records.push(affine_scalar_record());
            }
        }
    };
    let outcome = |field: UnitScalarRowsFieldForTest| {
        use UnitScalarRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarHomesDefiningOperation
            | Field::ScalarHomesSourceValue
            | Field::ScalarHomesByteOffset
            | Field::ScalarHomesDrop
            | Field::IntegerConstantsInsert => InstallationError::ImageBindingMismatch,
            Field::ScalarHomesScalarType => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarHomesShape => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
            Field::ScalarHomesInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::AffineScalarRecordsInsert => InstallationError::InvalidUnitAffineScalarRecord,
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation unit scalar rows / record",
        fields: UnitScalarRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert!(authentic.unit_structural_scalar_field_stores.is_empty());
    assert!(authentic.unit_write_only_primitive_stores.is_empty());
    assert!(authentic.scalar_structural_scalar_field_stores.is_empty());

    let substitute = |record: &mut InstallationRecord,
                      field: StoreRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use StoreRowsFieldForTest as Field;
        match field {
            Field::UnitStructuralScalarField => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_structural_scalar_field_stores
                    .push(structural_field_store());
            }
            Field::UnitWriteOnlyPrimitive => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_write_only_primitive_stores
                    .push(write_only_store());
            }
            Field::ScalarStructuralScalarField => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }
        }
    };
    let outcome = |field: StoreRowsFieldForTest| {
        use StoreRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarStructuralScalarField => InstallationError::ImageBindingMismatch,
            Field::UnitStructuralScalarField => {
                InstallationError::InvalidUnitStructuralScalarFieldStore(machine_id(3))
            }
            Field::UnitWriteOnlyPrimitive => {
                InstallationError::InvalidUnitWriteOnlyPrimitiveStore(machine_id(3))
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation store rows / record",
        fields: StoreRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });

    // The scalar-side store roster also encodes on the scalar-stack row and is
    // rejected by independent replay there.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image =
        emit_direct_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");

    let substitute = |record: &mut InstallationRecord,
                      field: StoreRowsScalarRecordFieldForTest,
                      _donor: &InstallationRecord| {
        use StoreRowsScalarRecordFieldForTest as Field;
        match field {
            Field::ScalarStructuralScalarFieldStoresInsertOnScalarRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }
        }
    };
    let outcome = |field: StoreRowsScalarRecordFieldForTest| {
        use StoreRowsScalarRecordFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarStructuralScalarFieldStoresInsertOnScalarRow => {
                InstallationError::ImageBindingMismatch
            }
        })
    };
    let check = nested_custody_check(&scalar_image, &scalar_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation store rows / scalar_record",
        fields: StoreRowsScalarRecordFieldForTest::INVENTORY,
        honest: &|| scalar_record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("two-function image");
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

    let store_error = || InstallationError::InvalidUnitStructuralScalarFieldStore(machine_id(9));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(9));

    let substitute = |record: &mut InstallationRecord,
                      field: StructuralStoreRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use StructuralStoreRowsFieldForTest as Field;
        match field {
            // The projected field identity is authenticated by the emitted image
            // alone: no record-shape join reads it.
            Field::StructuralScalarFieldStoresField => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].field = field_id(9);
            }
            // Every field-only or singly-indexed carrier path is inside the
            // bounded store grammar, so its spelling is authenticated by the
            // emitted image alone.
            Field::StructuralScalarFieldStoresPathRenamed => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field("renamed".to_string())];
            }
            Field::StructuralScalarFieldStoresPathEmpty => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path = Vec::new();
            }
            Field::StructuralScalarFieldStoresPathIndexed => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::Field("cell".to_string()),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }
            // The destination declaration's qualification rosters are not part of
            // the parameter/home join; the image comparison owns them.
            Field::StructuralScalarFieldStoresDestinationQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }
            Field::StructuralScalarFieldStoresDestinationProjectedQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }
            Field::StructuralScalarFieldStoresDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores.pop();
            }
            // The producer identity must name the operation the exact-attribution
            // join is bound to.
            Field::StructuralScalarFieldStoresPsiOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].psi_operation = operation_id(9);
            }
            // Every destination-declaration axis is joined pairwise to the
            // parameter and home rosters.
            Field::StructuralScalarFieldStoresDestinationPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].destination.place = place_id(9);
            }
            Field::StructuralScalarFieldStoresDestinationPosition => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .position = 1;
            }
            Field::StructuralScalarFieldStoresDestinationIsSelf => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .is_self = true;
            }
            Field::StructuralScalarFieldStoresDestinationStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }
            Field::StructuralScalarFieldStoresDestinationMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Affine;
            }
            Field::StructuralScalarFieldStoresDestinationAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0]
                    .destination
                    .access = StructuralAccess::MutableBorrow;
            }
            // An unbounded carrier path is not representable in the store grammar.
            Field::StructuralScalarFieldStoresPathReferent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Referent];
            }
            Field::StructuralScalarFieldStoresPathEmptyField => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field(String::new())];
            }
            Field::StructuralScalarFieldStoresPathDoubleIndex => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::FixedIndex(0),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }
            Field::StructuralScalarFieldStoresDestinationPlacement => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].destination_placement =
                    register_placement();
            }
            // In-bounds or not, a changed field offset recomputes different bytes
            // or violates the parameter shape.
            Field::StructuralScalarFieldStoresFieldByteOffsetWithinBounds => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].field_byte_offset = 4;
            }
            Field::StructuralScalarFieldStoresFieldByteOffsetOutOfBounds => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].field_byte_offset = 8;
            }
            // Each immediate-source field is joined to the retained integer
            // constant row.
            Field::StructuralScalarFieldStoresSourceDefiningOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *defining_operation = operation_id(9);
            }
            Field::StructuralScalarFieldStoresSourceSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    source_value,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *source_value = value_id(9);
            }
            Field::StructuralScalarFieldStoresSourceScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    scalar_type,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }
            Field::StructuralScalarFieldStoresSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    value,
                    ..
                } = &mut row.unit_structural_scalar_field_stores[0].source
                else {
                    unreachable!()
                };
                *value = IntegerValue::Signed(8);
            }
            // Each remaining source variant is representable but cannot be joined
            // to this function's retained custody.
            Field::StructuralScalarFieldStoresSourceBoolean => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].source =
                    machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                        defining_operation: operation_id(1),
                        source_value: value_id(51),
                        value: true,
                        definition_ordinal: 0,
                    };
            }
            Field::StructuralScalarFieldStoresSourceParameter => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].source =
                    machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                        parameter_index: 0,
                        source_value: value_id(51),
                        scalar_type: i32_scalar(),
                        location: machine_code::UnitScalarParameterLocationRecord::Register(
                            calling_conventions::MachineRegister::X86Rax,
                        ),
                    };
            }
            Field::StructuralScalarFieldStoresSourceHome => {
                let row = &mut record.functions_mut_for_test()[index];
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
            }
            Field::StructuralScalarFieldStoresParameterHomeByteOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].parameter_home_byte_offset = 0;
            }
            Field::StructuralScalarFieldStoresParameterHomeIndirect => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].parameter_home_indirect = true;
            }
            Field::StructuralScalarFieldStoresOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].operation_ordinal += 1;
            }
            Field::StructuralScalarFieldStoresCodeOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].code_offset += 1;
            }
            Field::StructuralScalarFieldStoresByteCount => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].byte_count += 1;
            }
            Field::StructuralScalarFieldStoresBytesContent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].bytes[2] = 0x11;
            }
            Field::StructuralScalarFieldStoresBytesTruncate => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores[0].bytes.pop();
            }
            Field::StructuralScalarFieldStoresSwap => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores.swap(0, 1);
            }
            Field::StructuralScalarFieldStoresInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let duplicate = row.unit_structural_scalar_field_stores[0].clone();
                row.unit_structural_scalar_field_stores.push(duplicate);
            }
            Field::StructuralScalarFieldStoresInsertFabricated => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_structural_scalar_field_stores
                    .push(structural_field_store());
            }
            // The parameter and home rosters are joined pairwise; the shared
            // declaration axes surface the cleanup-facts error while the
            // home-only location, source placement, and indirection surface the
            // store join.
            Field::ParametersPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].place = place_id(9);
            }
            Field::ParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].structural_type = structural_type(9);
            }
            Field::ParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Affine;
            }
            Field::ParametersAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].access = StructuralAccess::MutableBorrow;
            }
            Field::ParametersShape => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }
            Field::ParametersDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters.pop();
            }
            Field::ParameterHomesPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].place = place_id(9);
            }
            Field::ParameterHomesStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }
            Field::ParameterHomesMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Affine;
            }
            Field::ParameterHomesAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].access = StructuralAccess::MutableBorrow;
            }
            Field::ParameterHomesShape => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }
            Field::ParameterHomesSource => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].source = register_placement();
            }
            Field::ParameterHomesLocation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }
            Field::ParameterHomesIndirect => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].indirect = true;
            }
            Field::ParameterHomesDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes.pop();
            }
            Field::ParameterHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }
            // The immediate source's retained constant row must still join every
            // field.
            Field::IntegerConstantsDefiningOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].defining_operation = operation_id(9);
            }
            Field::IntegerConstantsSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].source_value = value_id(9);
            }
            Field::IntegerConstantsScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }
            Field::IntegerConstantsValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].value = IntegerValue::Signed(8);
            }
            Field::IntegerConstantsOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].operation_ordinal = 5;
            }
            Field::IntegerConstantsDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants.remove(0);
            }
            Field::IntegerConstantsInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let duplicate = row.unit_integer_constants[0];
                row.unit_integer_constants.push(duplicate);
            }
        }
    };
    let outcome = |field: StructuralStoreRowsFieldForTest| {
        use StructuralStoreRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::StructuralScalarFieldStoresField
            | Field::StructuralScalarFieldStoresPathRenamed
            | Field::StructuralScalarFieldStoresPathEmpty
            | Field::StructuralScalarFieldStoresPathIndexed
            | Field::StructuralScalarFieldStoresDestinationQualifications
            | Field::StructuralScalarFieldStoresDestinationProjectedQualifications
            | Field::StructuralScalarFieldStoresDrop => InstallationError::ImageBindingMismatch,
            Field::StructuralScalarFieldStoresPsiOperation => store_error(),
            Field::StructuralScalarFieldStoresDestinationPlace => store_error(),
            Field::StructuralScalarFieldStoresDestinationPosition => store_error(),
            Field::StructuralScalarFieldStoresDestinationIsSelf => store_error(),
            Field::StructuralScalarFieldStoresDestinationStructuralType => store_error(),
            Field::StructuralScalarFieldStoresDestinationMultiplicity => store_error(),
            Field::StructuralScalarFieldStoresDestinationAccess => store_error(),
            Field::StructuralScalarFieldStoresPathReferent => store_error(),
            Field::StructuralScalarFieldStoresPathEmptyField => store_error(),
            Field::StructuralScalarFieldStoresPathDoubleIndex => store_error(),
            Field::StructuralScalarFieldStoresDestinationPlacement => store_error(),
            Field::StructuralScalarFieldStoresFieldByteOffsetWithinBounds => store_error(),
            Field::StructuralScalarFieldStoresFieldByteOffsetOutOfBounds => store_error(),
            Field::StructuralScalarFieldStoresSourceDefiningOperation => store_error(),
            Field::StructuralScalarFieldStoresSourceSourceValue => store_error(),
            Field::StructuralScalarFieldStoresSourceScalarType => store_error(),
            Field::StructuralScalarFieldStoresSourceValue => store_error(),
            Field::StructuralScalarFieldStoresSourceBoolean => store_error(),
            Field::StructuralScalarFieldStoresSourceParameter => store_error(),
            Field::StructuralScalarFieldStoresSourceHome => store_error(),
            Field::StructuralScalarFieldStoresParameterHomeByteOffset => store_error(),
            Field::StructuralScalarFieldStoresParameterHomeIndirect => store_error(),
            Field::StructuralScalarFieldStoresOperationOrdinal => store_error(),
            Field::StructuralScalarFieldStoresCodeOffset => store_error(),
            Field::StructuralScalarFieldStoresByteCount => store_error(),
            Field::StructuralScalarFieldStoresBytesContent => store_error(),
            Field::StructuralScalarFieldStoresBytesTruncate => store_error(),
            Field::StructuralScalarFieldStoresSwap => store_error(),
            Field::StructuralScalarFieldStoresInsertDuplicate => store_error(),
            Field::StructuralScalarFieldStoresInsertFabricated => store_error(),
            Field::ParametersPlace => cleanup_error(),
            Field::ParametersStructuralType => cleanup_error(),
            Field::ParametersMultiplicity => cleanup_error(),
            Field::ParametersAccess => cleanup_error(),
            Field::ParametersShape => cleanup_error(),
            Field::ParametersDrop => cleanup_error(),
            Field::ParameterHomesPlace => cleanup_error(),
            Field::ParameterHomesStructuralType => cleanup_error(),
            Field::ParameterHomesMultiplicity => cleanup_error(),
            Field::ParameterHomesAccess => cleanup_error(),
            Field::ParameterHomesShape => cleanup_error(),
            Field::ParameterHomesSource => store_error(),
            Field::ParameterHomesLocation => store_error(),
            Field::ParameterHomesIndirect => store_error(),
            Field::ParameterHomesDrop => cleanup_error(),
            Field::ParameterHomesInsertDuplicate => cleanup_error(),
            Field::IntegerConstantsDefiningOperation => store_error(),
            Field::IntegerConstantsSourceValue => store_error(),
            Field::IntegerConstantsScalarType => store_error(),
            Field::IntegerConstantsValue => store_error(),
            Field::IntegerConstantsOperationOrdinal => cleanup_error(),
            Field::IntegerConstantsDrop => store_error(),
            Field::IntegerConstantsInsertDuplicate => cleanup_error(),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation structural store rows / record",
        fields: StructuralStoreRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("two-function image");
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

    let store_error = || InstallationError::InvalidUnitWriteOnlyPrimitiveStore(machine_id(9));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(9));

    let substitute = |record: &mut InstallationRecord,
                      field: WriteOnlyStoreRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use WriteOnlyStoreRowsFieldForTest as Field;
        match field {
            // Dropping a retained store leaves a canonical roster: the image
            // alone authenticates that the row existed.
            Field::WriteOnlyPrimitiveStoresDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores.pop();
            }
            Field::WriteOnlyPrimitiveStoresDropAll => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores.clear();
            }
            // A distinct catalog entry keeps the joined declaration's count at
            // exactly one; only the emitted image owns the catalog's contents.
            Field::AffineCleanupStructuralTypesInsertDistinct => {
                let row = &mut record.functions_mut_for_test()[index];
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
            }
            // The producer identity must name the operation the exact-attribution
            // join is bound to.
            Field::WriteOnlyPrimitiveStoresPsiOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].psi_operation = operation_id(9);
            }
            // Every destination-declaration axis is joined pairwise to the
            // parameter and home rosters.
            Field::WriteOnlyPrimitiveStoresDestinationPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination.place = place_id(9);
            }
            Field::WriteOnlyPrimitiveStoresDestinationPosition => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination.position = 1;
            }
            Field::WriteOnlyPrimitiveStoresDestinationIsSelf => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination.is_self = true;
            }
            Field::WriteOnlyPrimitiveStoresDestinationStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }
            Field::WriteOnlyPrimitiveStoresDestinationMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Affine;
            }
            Field::WriteOnlyPrimitiveStoresDestinationAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination.access =
                    StructuralAccess::MutableBorrow;
            }
            Field::WriteOnlyPrimitiveStoresDestinationQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }
            Field::WriteOnlyPrimitiveStoresDestinationProjectedQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }
            // The declared destination type must equal the joined catalog entry:
            // its id names the destination's structural type, its identity must
            // be non-empty, and its shape must be exactly the primitive the
            // source writes.
            Field::WriteOnlyPrimitiveStoresDestinationTypeId => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination_type.id = structural_type(9);
            }
            Field::WriteOnlyPrimitiveStoresDestinationTypeIdentity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .identity = "other.type".to_string();
            }
            Field::WriteOnlyPrimitiveStoresDestinationTypeIdentityEmpty => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .identity = String::new();
            }
            Field::WriteOnlyPrimitiveStoresDestinationTypeShape => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0]
                    .destination_type
                    .shape =
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
            }
            // The staged home placement is joined field-for-field to the home's
            // retained source placement.
            Field::WriteOnlyPrimitiveStoresDestinationPlacement => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].destination_placement =
                    register_placement();
            }
            // Every axis of the retained integer-immediate source must join the
            // earlier constant row exactly.
            Field::WriteOnlyPrimitiveStoresSourceDefiningOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    defining_operation,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *defining_operation = operation_id(9);
                }
            }
            Field::WriteOnlyPrimitiveStoresSourceSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    source_value,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *source_value = value_id(9);
                }
            }
            Field::WriteOnlyPrimitiveStoresSourceScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    scalar_type,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
                }
            }
            Field::WriteOnlyPrimitiveStoresSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                if let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    value,
                    ..
                } = &mut row.unit_write_only_primitive_stores[0].source
                {
                    *value = IntegerValue::Signed(8);
                }
            }
            // Every other source carrier is rejected: the parameter variant needs
            // an installed scalar parameter ABI, the zero-code immediates need an
            // exact zero-byte definition attribution, and the home variant needs
            // a retained scalar call result.
            Field::WriteOnlyPrimitiveStoresSourceParameter => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                        parameter_index: 0,
                        source_value: value_id(51),
                        scalar_type: i32_scalar(),
                        location: machine_code::UnitScalarParameterLocationRecord::Register(
                            calling_conventions::MachineRegister::X86Rax,
                        ),
                    };
            }
            Field::WriteOnlyPrimitiveStoresSourceBoolean => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                        defining_operation: operation_id(9),
                        source_value: value_id(55),
                        value: true,
                        definition_ordinal: 0,
                    };
            }
            Field::WriteOnlyPrimitiveStoresSourceIeeeFloat => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].source =
                    machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                        defining_operation: operation_id(9),
                        source_value: value_id(55),
                        value: semantic_vocabulary::IeeeFloatValue::Binary32(0x3f80_0000),
                        definition_ordinal: 0,
                    };
            }
            Field::WriteOnlyPrimitiveStoresSourceHome => {
                let row = &mut record.functions_mut_for_test()[index];
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
            }
            Field::WriteOnlyPrimitiveStoresParameterHomeByteOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].parameter_home_byte_offset = 0;
            }
            Field::WriteOnlyPrimitiveStoresParameterHomeIndirect => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].parameter_home_indirect = false;
            }
            Field::WriteOnlyPrimitiveStoresOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].operation_ordinal += 1;
            }
            Field::WriteOnlyPrimitiveStoresCodeOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].code_offset += 1;
            }
            Field::WriteOnlyPrimitiveStoresByteCount => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].byte_count += 1;
            }
            Field::WriteOnlyPrimitiveStoresBytesContent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].bytes[2] = 0x11;
            }
            Field::WriteOnlyPrimitiveStoresBytesTruncate => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores[0].bytes.pop();
            }
            Field::WriteOnlyPrimitiveStoresSwap => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores.swap(0, 1);
            }
            Field::WriteOnlyPrimitiveStoresInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let duplicate = row.unit_write_only_primitive_stores[0].clone();
                row.unit_write_only_primitive_stores.push(duplicate);
            }
            Field::WriteOnlyPrimitiveStoresInsertFabricated => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_write_only_primitive_stores
                    .push(write_only_store());
            }
            // The parameter and home rosters are joined pairwise; the shared
            // declaration axes surface the cleanup-facts error while the
            // home-only location, source placement, and indirection surface the
            // store join.
            Field::ParametersPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].place = place_id(9);
            }
            Field::ParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].structural_type = structural_type(9);
            }
            Field::ParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Affine;
            }
            Field::ParametersAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].access = StructuralAccess::MutableBorrow;
            }
            Field::ParametersShape => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }
            Field::ParametersDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameters.pop();
            }
            Field::ParameterHomesPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].place = place_id(9);
            }
            Field::ParameterHomesStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }
            Field::ParameterHomesMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Affine;
            }
            Field::ParameterHomesAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].access = StructuralAccess::MutableBorrow;
            }
            Field::ParameterHomesShape => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }
            Field::ParameterHomesSource => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].source = register_placement();
            }
            Field::ParameterHomesLocation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }
            Field::ParameterHomesIndirect => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes[0].indirect = false;
            }
            Field::ParameterHomesDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_parameter_homes.pop();
            }
            Field::ParameterHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }
            // The immediate source's retained constant row must still join every
            // field.
            Field::IntegerConstantsDefiningOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].defining_operation = operation_id(9);
            }
            Field::IntegerConstantsSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].source_value = value_id(9);
            }
            Field::IntegerConstantsScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }
            Field::IntegerConstantsValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].value = IntegerValue::Signed(8);
            }
            Field::IntegerConstantsOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants[0].operation_ordinal = 5;
            }
            Field::IntegerConstantsDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_integer_constants.remove(0);
            }
            Field::IntegerConstantsInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let duplicate = row.unit_integer_constants[0];
                row.unit_integer_constants.push(duplicate);
            }
            // The cleanup's structural-type catalog must retain the joined
            // destination declaration exactly once: removing it or duplicating
            // it breaks the count the store join authenticates.
            Field::AffineCleanupStructuralTypesDrop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = Vec::new().into();
            }
            Field::AffineCleanupStructuralTypesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
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
            }
        }
    };
    let outcome = |field: WriteOnlyStoreRowsFieldForTest| {
        use WriteOnlyStoreRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::WriteOnlyPrimitiveStoresDrop
            | Field::WriteOnlyPrimitiveStoresDropAll
            | Field::AffineCleanupStructuralTypesInsertDistinct => {
                InstallationError::ImageBindingMismatch
            }
            Field::WriteOnlyPrimitiveStoresPsiOperation => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationPlace => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationPosition => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationIsSelf => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationStructuralType => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationMultiplicity => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationAccess => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationQualifications => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationProjectedQualifications => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationTypeId => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationTypeIdentity => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationTypeIdentityEmpty => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationTypeShape => store_error(),
            Field::WriteOnlyPrimitiveStoresDestinationPlacement => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceDefiningOperation => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceSourceValue => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceScalarType => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceValue => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceParameter => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceBoolean => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceIeeeFloat => store_error(),
            Field::WriteOnlyPrimitiveStoresSourceHome => store_error(),
            Field::WriteOnlyPrimitiveStoresParameterHomeByteOffset => store_error(),
            Field::WriteOnlyPrimitiveStoresParameterHomeIndirect => store_error(),
            Field::WriteOnlyPrimitiveStoresOperationOrdinal => store_error(),
            Field::WriteOnlyPrimitiveStoresCodeOffset => store_error(),
            Field::WriteOnlyPrimitiveStoresByteCount => store_error(),
            Field::WriteOnlyPrimitiveStoresBytesContent => store_error(),
            Field::WriteOnlyPrimitiveStoresBytesTruncate => store_error(),
            Field::WriteOnlyPrimitiveStoresSwap => store_error(),
            Field::WriteOnlyPrimitiveStoresInsertDuplicate => store_error(),
            Field::WriteOnlyPrimitiveStoresInsertFabricated => store_error(),
            Field::ParametersPlace => cleanup_error(),
            Field::ParametersStructuralType => cleanup_error(),
            Field::ParametersMultiplicity => cleanup_error(),
            Field::ParametersAccess => cleanup_error(),
            Field::ParametersShape => cleanup_error(),
            Field::ParametersDrop => cleanup_error(),
            Field::ParameterHomesPlace => cleanup_error(),
            Field::ParameterHomesStructuralType => cleanup_error(),
            Field::ParameterHomesMultiplicity => cleanup_error(),
            Field::ParameterHomesAccess => cleanup_error(),
            Field::ParameterHomesShape => cleanup_error(),
            Field::ParameterHomesSource => store_error(),
            Field::ParameterHomesLocation => store_error(),
            Field::ParameterHomesIndirect => store_error(),
            Field::ParameterHomesDrop => cleanup_error(),
            Field::ParameterHomesInsertDuplicate => cleanup_error(),
            Field::IntegerConstantsDefiningOperation => store_error(),
            Field::IntegerConstantsSourceValue => store_error(),
            Field::IntegerConstantsScalarType => store_error(),
            Field::IntegerConstantsValue => store_error(),
            Field::IntegerConstantsOperationOrdinal => cleanup_error(),
            Field::IntegerConstantsDrop => store_error(),
            Field::IntegerConstantsInsertDuplicate => cleanup_error(),
            Field::AffineCleanupStructuralTypesDrop => store_error(),
            Field::AffineCleanupStructuralTypesInsertDuplicate => store_error(),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation write only store rows / record",
        fields: WriteOnlyStoreRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("two-function image");
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

    let substitute = |record: &mut InstallationRecord,
                      field: ScalarStoreRowsFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarStoreRowsFieldForTest as Field;
        match field {
            Field::PsiOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].psi_operation = operation_id(9);
            }
            Field::DestinationPlace => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .place = place_id(9);
            }
            Field::DestinationPosition => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .position = 1;
            }
            Field::DestinationIsSelf => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .is_self = false;
            }
            Field::DestinationStructuralType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .structural_type = structural_type(9);
            }
            Field::DestinationMultiplicity => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .multiplicity = StructuralMultiplicity::Unrestricted;
            }
            Field::DestinationAccess => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .access = StructuralAccess::WriteOnlyBorrow;
            }
            Field::DestinationQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .qualifications
                    .push(domain_id(3));
            }
            Field::DestinationProjectedQualifications => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }
            Field::DestinationProjectedQualificationsReferent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0]
                    .destination
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Referent],
                        domain: domain_id(3),
                    });
            }
            Field::PathReferent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Referent];
            }
            Field::PathRenamed => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field("renamed".to_string())];
            }
            Field::PathEmpty => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].path = Vec::new();
            }
            Field::PathIndexed => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].path = vec![
                    StructuralPathSegment::Field("cell".to_string()),
                    StructuralPathSegment::FixedIndex(1),
                ];
            }
            Field::Field => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].field = field_id(9);
            }
            Field::DestinationPlacement => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].destination_placement =
                    empty_placement();
            }
            Field::FieldByteOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].field_byte_offset = 4;
            }
            Field::DefiningOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].defining_operation = operation_id(9);
            }
            Field::SourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].source_value = value_id(9);
            }
            Field::ImmediateBoolean => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Boolean(true);
            }
            Field::ImmediateScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
                        value: IntegerValue::Signed(3),
                    };
            }
            Field::ImmediateValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: i32_integer(),
                        value: IntegerValue::Signed(9),
                    };
            }
            Field::ReturnOperation => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].return_operation = operation_id(9);
            }
            Field::ReturnSourceValue => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].return_source_value = value_id(9);
            }
            Field::ReturnField => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].return_field = field_id(9);
            }
            Field::ReturnFieldByteOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].return_field_byte_offset = 4;
            }
            Field::ReturnScalarType => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].return_scalar_type =
                    ScalarType::Boolean;
            }
            Field::OperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].operation_ordinal += 1;
            }
            Field::CodeOffset => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].code_offset += 1;
            }
            Field::ByteCount => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].byte_count += 1;
            }
            Field::BytesContent => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].bytes[0] = 0x11;
            }
            Field::BytesTruncate => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].bytes.pop();
            }
            Field::Swap => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores.swap(0, 1);
            }
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores.pop();
            }
            Field::InsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[index];
                let duplicate = row.scalar_structural_scalar_field_stores[0].clone();
                row.scalar_structural_scalar_field_stores.push(duplicate);
            }
            Field::InsertFabricated => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }
            // The path grammar's remaining canonicality boundary: a field segment
            // must carry a non-empty identity.
            Field::PathEmptyField => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].path =
                    vec![StructuralPathSegment::Field(String::new())];
            }
            // The immediate's integer carrier must be fixed-width on the wire.
            Field::ImmediateScalarTypeAddress => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores[0].immediate =
                    target_operations::TargetScalarImmediate::Integer {
                        scalar_type: IntegerType::address(64).expect("address i64"),
                        value: IntegerValue::Unsigned(3),
                    };
            }
            // The retained roster is bounded at three rows.
            Field::InsertBeyondBound => {
                let row = &mut record.functions_mut_for_test()[index];
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
                row.scalar_structural_scalar_field_stores
                    .push(scalar_field_store());
            }
        }
    };
    let outcome = |field: ScalarStoreRowsFieldForTest| {
        use ScalarStoreRowsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::PsiOperation
            | Field::DestinationPlace
            | Field::DestinationPosition
            | Field::DestinationIsSelf
            | Field::DestinationStructuralType
            | Field::DestinationMultiplicity
            | Field::DestinationAccess
            | Field::DestinationQualifications
            | Field::DestinationProjectedQualifications
            | Field::DestinationProjectedQualificationsReferent
            | Field::PathReferent
            | Field::PathRenamed
            | Field::PathEmpty
            | Field::PathIndexed
            | Field::Field
            | Field::DestinationPlacement
            | Field::FieldByteOffset
            | Field::DefiningOperation
            | Field::SourceValue
            | Field::ImmediateBoolean
            | Field::ImmediateScalarType
            | Field::ImmediateValue
            | Field::ReturnOperation
            | Field::ReturnSourceValue
            | Field::ReturnField
            | Field::ReturnFieldByteOffset
            | Field::ReturnScalarType
            | Field::OperationOrdinal
            | Field::CodeOffset
            | Field::ByteCount
            | Field::BytesContent
            | Field::BytesTruncate
            | Field::Swap
            | Field::Drop
            | Field::InsertDuplicate
            | Field::InsertFabricated => InstallationError::ImageBindingMismatch,
            Field::PathEmptyField => InstallationError::InvalidSettlementArgumentField,
            Field::ImmediateScalarTypeAddress => {
                InstallationError::UnsupportedInstalledFixedIntegerType
            }
            Field::InsertBeyondBound => InstallationError::TooManyScalarStructuralScalarFieldStores,
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar store rows / record",
        fields: ScalarStoreRowsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("affine scalar image");
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

    let scalar_record_error = || InstallationError::InvalidUnitAffineScalarRecord;
    let call_error = || InstallationError::InvalidInternalUnitCall(machine_id(1));
    let cleanup_error = || InstallationError::InvalidUnitAffineCleanup(machine_id(1));

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
            qualification_establishments: Vec::new(),
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

    let substitute = |record: &mut InstallationRecord,
                      field: AffineScalarRecordsFieldForTest,
                      _donor: &InstallationRecord| {
        use AffineScalarRecordsFieldForTest as Field;
        match field {
            Field::UnitAffineScalarRecordsPsiOperation => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].psi_operation = operation_id(9);
            }
            Field::UnitAffineScalarRecordsResultPlace => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].result.place = place_id(9);
            }
            Field::UnitAffineScalarRecordsResultStructuralType => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].result.structural_type = structural_type(9);
            }
            Field::UnitAffineScalarRecordsField => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].field = field_id(9);
            }
            Field::UnitAffineScalarRecordsValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].value = IntegerValue::Signed(18);
            }
            Field::UnitAffineScalarRecordsOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].operation_ordinal += 1;
            }
            Field::UnitAffineScalarRecordsDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records.pop();
            }
            Field::UnitAffineScalarRecordsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records
                    .push(UnitAffineScalarRecordEstablishmentRecord {
                        psi_operation: operation_id(9),
                        result: terminal_psi::StructuralOperationResult {
                            qualification_establishments: Vec::new(),
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
            }
            Field::UnitAffineScalarRecordsInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let duplicate = row.unit_affine_scalar_records[0].clone();
                row.unit_affine_scalar_records.push(duplicate);
            }
            Field::UnitAffineScalarRecordsSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records.swap(0, 1);
            }
            // The parameter home's indirection flag is authenticated by the
            // emitted image alone: no record-shape join reads it.
            Field::UnitParameterHomesIndirect => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].indirect = true;
            }
            Field::UnitAffineScalarRecordsResultMultiplicity => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].result.multiplicity =
                    StructuralMultiplicity::Linear;
            }
            Field::UnitAffineScalarRecordsResultQualifications => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0]
                    .result
                    .qualifications
                    .push(domain_id(3));
            }
            Field::UnitAffineScalarRecordsResultProjectedQualifications => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0]
                    .result
                    .projected_qualifications
                    .push(StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("gate".to_string())],
                        domain: domain_id(3),
                    });
            }
            Field::UnitAffineScalarRecordsResultClaims => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].result.claims.push(
                    terminal_psi::StructuralResultClaimBinding {
                        claim: semantic_vocabulary::ClaimId::new(31).expect("claim"),
                        path: Vec::new(),
                    },
                );
            }
            Field::UnitAffineScalarRecordsValueUnsigned => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].value = IntegerValue::Unsigned(3);
            }
            Field::UnitAffineScalarRecordsValueOverflow => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].value =
                    IntegerValue::Signed(i128::from(i64::MAX) + 1);
            }
            Field::UnitAffineScalarRecordsShape => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records[0].shape = ValueShape::integer(4, 4);
            }
            Field::UnitAffineScalarRecordsInsertNoncanonical => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_affine_scalar_records.push(affine_scalar_record());
            }
            // The record's result place is joined through the parameter and home
            // rosters: declaration axes are pinned pairwise, and every roster
            // membership change upsets the cleanup's transferred-root suffix.
            Field::UnitParametersPlace => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters[0].place = place_id(9);
            }
            Field::UnitParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters[0].structural_type = structural_type(9);
            }
            Field::UnitParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters[0].multiplicity = StructuralMultiplicity::Linear;
            }
            Field::UnitParametersAccess => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters[0].access = StructuralAccess::SharedBorrow;
            }
            Field::UnitParametersShape => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters[0].shape = ValueShape::integer(4, 4);
            }
            Field::UnitParametersDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters.pop();
            }
            Field::UnitParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let parameter = row.unit_parameters[0];
                row.unit_parameters.push(parameter);
            }
            Field::UnitParametersSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameters.swap(0, 1);
            }
            Field::UnitParameterHomesPlace => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].place = place_id(9);
            }
            Field::UnitParameterHomesStructuralType => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].structural_type = structural_type(9);
            }
            Field::UnitParameterHomesMultiplicity => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Linear;
            }
            Field::UnitParameterHomesAccess => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].access = StructuralAccess::SharedBorrow;
            }
            Field::UnitParameterHomesShape => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].shape = ValueShape::integer(4, 4);
            }
            Field::UnitParameterHomesSource => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].source = register_placement();
            }
            Field::UnitParameterHomesLocation => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes[0].location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 16 };
            }
            Field::UnitParameterHomesDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes.pop();
            }
            Field::UnitParameterHomesInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let home = row.unit_parameter_homes[0].clone();
                row.unit_parameter_homes.push(home);
            }
            Field::UnitParameterHomesSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_parameter_homes.swap(0, 1);
            }
            // The argument's access and byte-transport axes are authenticated by
            // the emitted image alone: no record-shape join reads them, so each
            // substitution still encodes and replay rejects it.
            Field::InternalUnitCallsArgumentsAccess => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            Field::InternalUnitCallsArgumentsCallStackBytes => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .call_stack_bytes = 0;
            }
            Field::InternalUnitCallsArgumentsCodeOffset => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .code_offset += 1;
            }
            Field::InternalUnitCallsArgumentsBytes => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .bytes[2] ^= 0xff;
            }
            Field::InternalUnitCallsCustodyClaimTransfers => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: semantic_vocabulary::ClaimId::new(31).expect("claim"),
                        argument_index: 0,
                    });
            }
            Field::InternalUnitCallsCustodySource => {
                record.internal_unit_calls_mut_for_test()[0].custody.source =
                    provider_source.clone();
            }
            Field::InternalUnitCallsCustodyOwner => {
                record.internal_unit_calls_mut_for_test()[0].custody.owner =
                    CallSiteOwner::Operation(operation_id(9));
            }
            Field::InternalUnitCallsCustodyTarget => {
                record.internal_unit_calls_mut_for_test()[0].custody.target = machine_id(3);
            }
            Field::InternalUnitCallsCustodyResult => {
                record.internal_unit_calls_mut_for_test()[0].custody.result = Some(i32_scalar());
            }
            Field::InternalUnitCallsCustodySemanticResult => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .semantic_result = Some(abstract_operations::AbstractResult {
                    value: value_id(31),
                    scalar_type: ScalarType::Boolean,
                });
            }
            // A returned structural result joins into the function's transferred
            // affine roots, so the cleanup roster rejects it before the call.
            Field::InternalUnitCallsCustodyStructuralResult => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .structural_result = Some(structural_result.clone());
            }
            Field::InternalUnitCallsCustodyOperationOrdinal => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .operation_ordinal = 0;
            }
            Field::InternalUnitCallsCustodyCodeOffset => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .code_offset += 1;
            }
            Field::InternalUnitCallsCustodyByteCount => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .byte_count += 1;
            }
            Field::InternalUnitCallsCustodyScalarArguments => {
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
            }
            Field::InternalUnitCallsCustodyArgumentsPlace => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .place = place_id(9);
            }
            Field::InternalUnitCallsCustodyArgumentsPath => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .path = vec![StructuralPathSegment::Field("other".to_string())];
            }
            Field::InternalUnitCallsCustodyArgumentsRootStructuralType => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .root_structural_type = structural_type(9);
            }
            Field::InternalUnitCallsCustodyArgumentsStructuralType => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .structural_type = structural_type(9);
            }
            Field::InternalUnitCallsCustodyArgumentsShape => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .shape = ValueShape::integer(4, 4);
            }
            Field::InternalUnitCallsCustodyArgumentsSourcePlacement => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source = machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                    register_placement(),
                );
            }
            Field::InternalUnitCallsCustodyArgumentsSourceEstablished => {
                record.internal_unit_calls_mut_for_test()[0]
                        .custody
                        .arguments[0]
                        .source =
                        machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal {
                            psi_operation: operation_id(1),
                        };
            }
            Field::InternalUnitCallsCustodyArgumentsSourceLocation => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source_location =
                    machine_code::StructuralSourceLocation::Stack { byte_offset: 8 };
            }
            Field::InternalUnitCallsCustodyArgumentsSourceByteOffset => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .source_byte_offset = 1;
            }
            Field::InternalUnitCallsCustodyArgumentsDestination => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .destination = register_placement();
            }
            Field::InternalUnitCallsCustodyArgumentsByteCount => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .byte_count += 1;
            }
            Field::InternalUnitCallsCustodyArgumentsFixedArrayLength => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .fixed_array_length = Some(2);
            }
            Field::InternalUnitCallsCustodyArgumentsElementStride => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .element_stride = Some(8);
            }
            Field::InternalUnitCallsCustodyArgumentsDrop => {
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments
                    .pop();
            }
            Field::InternalUnitCallsCustodyArgumentsInsertDuplicate => {
                let duplicate = record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments[0]
                    .clone();
                record.internal_unit_calls_mut_for_test()[0]
                    .custody
                    .arguments
                    .push(duplicate);
            }
            // Retargeting the call to the callee's machine strips the consuming
            // call from the caller's roster, so the caller's cleanup rejects it
            // before the call's own fields are examined.
            Field::InternalUnitCallsMachine => {
                record.internal_unit_calls_mut_for_test()[0].machine = machine_id(2);
            }
            Field::InternalUnitCallsTextOffset => {
                record.internal_unit_calls_mut_for_test()[0].text_offset += 1;
            }
            Field::InternalUnitCallsDrop => {
                record.internal_unit_calls_mut_for_test().remove(0);
            }
            Field::InternalUnitCallsSwap => {
                record.internal_unit_calls_mut_for_test().swap(0, 1);
            }
            Field::InternalUnitCallsInsertDuplicate => {
                let duplicate = record.internal_unit_calls_mut_for_test()[0].clone();
                record.internal_unit_calls_mut_for_test().push(duplicate);
            }
            Field::InternalUnitCallsInsertDistinct => {
                let mut call = record.internal_unit_calls_mut_for_test()[0].clone();
                call.custody.owner = CallSiteOwner::Operation(operation_id(9));
                call.custody.operation_ordinal = 9;
                call.custody.code_offset = 40;
                call.custody.byte_count = 6;
                call.text_offset = 40;
                record.internal_unit_calls_mut_for_test().push(call);
            }
        }
    };
    let outcome = |field: AffineScalarRecordsFieldForTest| {
        use AffineScalarRecordsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::UnitAffineScalarRecordsPsiOperation
            | Field::UnitAffineScalarRecordsResultPlace
            | Field::UnitAffineScalarRecordsResultStructuralType
            | Field::UnitAffineScalarRecordsField
            | Field::UnitAffineScalarRecordsValue
            | Field::UnitAffineScalarRecordsOperationOrdinal
            | Field::UnitAffineScalarRecordsDrop
            | Field::UnitAffineScalarRecordsInsert
            | Field::UnitAffineScalarRecordsInsertDuplicate
            | Field::UnitAffineScalarRecordsSwap
            | Field::UnitParameterHomesIndirect
            | Field::InternalUnitCallsArgumentsAccess
            | Field::InternalUnitCallsArgumentsCallStackBytes
            | Field::InternalUnitCallsArgumentsCodeOffset
            | Field::InternalUnitCallsArgumentsBytes
            | Field::InternalUnitCallsCustodyClaimTransfers => {
                InstallationError::ImageBindingMismatch
            }
            Field::UnitAffineScalarRecordsResultMultiplicity => scalar_record_error(),
            Field::UnitAffineScalarRecordsResultQualifications => scalar_record_error(),
            Field::UnitAffineScalarRecordsResultProjectedQualifications => scalar_record_error(),
            Field::UnitAffineScalarRecordsResultClaims => scalar_record_error(),
            Field::UnitAffineScalarRecordsValueUnsigned => scalar_record_error(),
            Field::UnitAffineScalarRecordsValueOverflow => scalar_record_error(),
            Field::UnitAffineScalarRecordsShape => scalar_record_error(),
            Field::UnitAffineScalarRecordsInsertNoncanonical => scalar_record_error(),
            Field::UnitParametersPlace => cleanup_error(),
            Field::UnitParametersStructuralType => cleanup_error(),
            Field::UnitParametersMultiplicity => cleanup_error(),
            Field::UnitParametersAccess => cleanup_error(),
            Field::UnitParametersShape => cleanup_error(),
            Field::UnitParametersDrop => cleanup_error(),
            Field::UnitParametersInsertDuplicate => cleanup_error(),
            Field::UnitParametersSwap => cleanup_error(),
            Field::UnitParameterHomesPlace => cleanup_error(),
            Field::UnitParameterHomesStructuralType => cleanup_error(),
            Field::UnitParameterHomesMultiplicity => cleanup_error(),
            Field::UnitParameterHomesAccess => cleanup_error(),
            Field::UnitParameterHomesShape => cleanup_error(),
            Field::UnitParameterHomesSource => call_error(),
            Field::UnitParameterHomesLocation => call_error(),
            Field::UnitParameterHomesDrop => cleanup_error(),
            Field::UnitParameterHomesInsertDuplicate => cleanup_error(),
            Field::UnitParameterHomesSwap => cleanup_error(),
            Field::InternalUnitCallsCustodySource => call_error(),
            Field::InternalUnitCallsCustodyOwner => call_error(),
            Field::InternalUnitCallsCustodyTarget => call_error(),
            Field::InternalUnitCallsCustodyResult => call_error(),
            Field::InternalUnitCallsCustodySemanticResult => call_error(),
            Field::InternalUnitCallsCustodyStructuralResult => cleanup_error(),
            Field::InternalUnitCallsCustodyOperationOrdinal => call_error(),
            Field::InternalUnitCallsCustodyCodeOffset => call_error(),
            Field::InternalUnitCallsCustodyByteCount => call_error(),
            Field::InternalUnitCallsCustodyScalarArguments => call_error(),
            Field::InternalUnitCallsCustodyArgumentsPlace => cleanup_error(),
            Field::InternalUnitCallsCustodyArgumentsPath => cleanup_error(),
            Field::InternalUnitCallsCustodyArgumentsRootStructuralType => call_error(),
            Field::InternalUnitCallsCustodyArgumentsStructuralType => call_error(),
            Field::InternalUnitCallsCustodyArgumentsShape => call_error(),
            Field::InternalUnitCallsCustodyArgumentsSourcePlacement => call_error(),
            Field::InternalUnitCallsCustodyArgumentsSourceEstablished => call_error(),
            Field::InternalUnitCallsCustodyArgumentsSourceLocation => call_error(),
            Field::InternalUnitCallsCustodyArgumentsSourceByteOffset => call_error(),
            Field::InternalUnitCallsCustodyArgumentsDestination => call_error(),
            Field::InternalUnitCallsCustodyArgumentsByteCount => call_error(),
            Field::InternalUnitCallsCustodyArgumentsFixedArrayLength => call_error(),
            Field::InternalUnitCallsCustodyArgumentsElementStride => call_error(),
            Field::InternalUnitCallsCustodyArgumentsDrop => cleanup_error(),
            Field::InternalUnitCallsCustodyArgumentsInsertDuplicate => call_error(),
            Field::InternalUnitCallsMachine => cleanup_error(),
            Field::InternalUnitCallsTextOffset => call_error(),
            Field::InternalUnitCallsDrop => cleanup_error(),
            Field::InternalUnitCallsSwap => call_error(),
            Field::InternalUnitCallsInsertDuplicate => call_error(),
            Field::InternalUnitCallsInsertDistinct => call_error(),
        })
    };
    let check = nested_custody_undecodable_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation affine scalar records / record",
        fields: AffineScalarRecordsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    assert!(record.functions()[2].unit_affine_cleanup.is_some());

    let substitute = |record: &mut InstallationRecord,
                      field: AffineCleanupFieldForTest,
                      _donor: &InstallationRecord| {
        use AffineCleanupFieldForTest as Field;
        match field {
            Field::UnitAffineCleanupPsiEdge => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup.as_mut().expect("cleanup").psi_edge = edge_id(9);
            }
            Field::UnitAffineCleanupLocalsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .locals
                    .push(extra_local());
            }
            Field::UnitAffineCleanupActionsDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .pop();
            }
            Field::UnitAffineCleanupActionsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        place_id(9),
                    ));
            }
            Field::UnitAffineCleanupActionsCleanupMachine => {
                let row = &mut record.functions_mut_for_test()[2];
                let action = &mut row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0];
                let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) = action
                else {
                    panic!("fixture cleanup action");
                };
                nominal.cleanup_machine = machine_id(2);
            }
            Field::UnitAffineCleanupActionsPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                let action = &mut row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0];
                let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal) = action
                else {
                    panic!("fixture cleanup action");
                };
                nominal.place = place_id(9);
            }
            Field::UnitAffineCleanupActionsVariant => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup.as_mut().expect("cleanup").actions[0] =
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9));
            }
            Field::UnitAffineCleanupCodeOffset => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .code_offset += 1;
            }
            Field::UnitAffineCleanupByteCount => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .byte_count -= 1;
            }
            // Dropping the cleanup leaves `unit_body` asserted without its
            // retained evidence; the projection join rejects it.
            Field::UnitAffineCleanupDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup = None;
            }
            // A scalar affine cleanup cannot be forged onto a Unit-stack row.
            Field::ScalarAffineCleanupInsertOnUnitRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup = row.unit_affine_cleanup.clone();
            }
            // Continuations need an exact block/attribution custody that this
            // fixture does not retain; the fabricated boundary row is rejected by
            // the unit-cleanup joins on either row.
            Field::UnitContinuationsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_continuations.push(UnitContinuationRecord {
                    operation_ordinal: 3,
                    successor_operation_ordinal: 4,
                    source_block: block_id(1),
                    target_block: block_id(2),
                    bindings: Vec::new(),
                    cleanup: row.unit_affine_cleanup.clone().expect("cleanup"),
                });
            }
            Field::UnitAffineCleanupStructuralTypes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = extra_type_catalog();
            }
            Field::UnitContinuationsInsertOnUnitCaller => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations.push(UnitContinuationRecord {
                    operation_ordinal: 1,
                    successor_operation_ordinal: 2,
                    source_block: block_id(1),
                    target_block: block_id(2),
                    bindings: Vec::new(),
                    cleanup: row.unit_affine_cleanup.clone().expect("cleanup"),
                });
            }
        }
    };
    let outcome = |field: AffineCleanupFieldForTest| {
        use AffineCleanupFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::UnitAffineCleanupStructuralTypes => InstallationError::ImageBindingMismatch,
            Field::UnitAffineCleanupPsiEdge => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupLocalsInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupActionsDrop => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupActionsInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupActionsCleanupMachine => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupActionsPlace => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupActionsVariant => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupCodeOffset => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupByteCount => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupDrop => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupInsertOnUnitRow => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitContinuationsInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitContinuationsInsertOnUnitCaller => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation affine cleanup / record",
        fields: AffineCleanupFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });

    // The scalar-side cleanup projection joins the same axes on the promoted
    // scalar row; its verifier-owned type catalog still encodes and replay
    // rejects the substitution.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image =
        emit_direct_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");
    assert!(scalar_record.functions()[2].scalar_affine_cleanup.is_some());

    let substitute = |record: &mut InstallationRecord,
                      field: AffineCleanupScalarRecordFieldForTest,
                      _donor: &InstallationRecord| {
        use AffineCleanupScalarRecordFieldForTest as Field;
        match field {
            Field::ScalarAffineCleanupPsiEdge => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .psi_edge = edge_id(9);
            }
            Field::ScalarAffineCleanupLocalsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .locals
                    .push(extra_local());
            }
            Field::ScalarAffineCleanupActionsDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .actions
                    .pop();
            }
            Field::ScalarAffineCleanupCodeOffset => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .code_offset += 1;
            }
            Field::ScalarAffineCleanupByteCount => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .byte_count -= 1;
            }
            Field::ScalarAffineCleanupDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup = None;
            }
            // A Unit cleanup cannot be forged beneath scalar stack evidence.
            Field::UnitAffineCleanupInsertOnScalarRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.unit_affine_cleanup = row.scalar_affine_cleanup.clone();
            }
            Field::ScalarAffineCleanupStructuralTypes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_affine_cleanup
                    .as_mut()
                    .expect("cleanup")
                    .structural_types = extra_type_catalog();
            }
        }
    };
    let outcome = |field: AffineCleanupScalarRecordFieldForTest| {
        use AffineCleanupScalarRecordFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarAffineCleanupStructuralTypes => InstallationError::ImageBindingMismatch,
            Field::ScalarAffineCleanupPsiEdge => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupLocalsInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupActionsDrop => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupCodeOffset => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupByteCount => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ScalarAffineCleanupDrop => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::UnitAffineCleanupInsertOnScalarRow => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
        })
    };
    let check = nested_custody_check(&scalar_image, &scalar_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation affine cleanup / scalar_record",
        fields: AffineCleanupScalarRecordFieldForTest::INVENTORY,
        honest: &|| scalar_record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
}

/// The projected scalar-control cleanup roster retains only the cleanup axis
/// of each DFS leaf: every projected leaf is a canonical projection of the
/// emitted image's scalar-control custody, so every substitution is rejected
/// at canonical encoding.
#[test]
fn installation_function_scalar_control_cleanups_reject_every_one_field_substitution() {
    let plan = scalar_three_leaf_cleanup_plan();
    let artifact = build_object_artifact(&plan).expect("three-leaf artifact");
    let image = emit_direct_executable_image(&artifact, 1).expect("three-leaf image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("three-leaf installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    assert_eq!(
        record.functions()[0].scalar_control_affine_cleanups.len(),
        3
    );

    let record_authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let substitute = |record: &mut InstallationRecord,
                      field: ScalarControlCleanupsFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarControlCleanupsFieldForTest as Field;
        match field {
            Field::PsiEdge => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].psi_edge = edge_id(9);
            }
            Field::StructuralTypes => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].structural_types = extra_type_catalog();
            }
            Field::LocalsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0]
                    .locals
                    .push(extra_local());
            }
            Field::ActionsDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].actions.pop();
            }
            Field::ActionsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9)),
                );
            }
            Field::CodeOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].code_offset += 1;
            }
            Field::ByteCount => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups[0].byte_count -= 1;
            }
            Field::Reorder => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups.swap(0, 1);
            }
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.scalar_control_affine_cleanups.pop();
            }
            Field::InsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let cleanup = row.scalar_control_affine_cleanups[0].clone();
                row.scalar_control_affine_cleanups.push(cleanup);
            }
        }
    };
    let outcome = |field: ScalarControlCleanupsFieldForTest| {
        use ScalarControlCleanupsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::PsiEdge => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::StructuralTypes => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::LocalsInsert => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::ActionsDrop => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::ActionsInsert => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::CodeOffset => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::ByteCount => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::Reorder => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::Drop => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::InsertDuplicate => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        })
    };
    let check = nested_custody_check(&image, &record_authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar control cleanups / record",
        fields: ScalarControlCleanupsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[2].clone();
    assert!(authentic.scalar_abi.is_none());
    assert!(authentic.parameter_abi.is_none());
    assert!(authentic.structural_call_scalar_return.is_none());

    let substitute = |record: &mut InstallationRecord,
                      field: ScalarTransportFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarTransportFieldForTest as Field;
        match field {
            Field::ScalarAbiInsertCanonical => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_abi = Some(canonical_scalar_abi());
            }
            Field::ScalarAbiInsertCanonicalAltValue => {
                let row = &mut record.functions_mut_for_test()[2];
                let mut abi = canonical_scalar_abi();
                abi.result.value = value_id(99);
                row.scalar_abi = Some(abi);
            }
            Field::ParameterAbiInsertCanonical => {
                let row = &mut record.functions_mut_for_test()[2];
                row.parameter_abi = Some(canonical_parameter_abi());
            }
            // A canonical scalar ABI requires placements the native call plan
            // recomputes exactly; a substituted placement fails the join.
            Field::ScalarAbiInsertNoncanonical => {
                let row = &mut record.functions_mut_for_test()[2];
                let mut abi = canonical_scalar_abi();
                abi.parameters[0].placement = empty_placement();
                row.scalar_abi = Some(abi);
            }
            // The mixed ABI requires scalar stack evidence and an exact structural
            // roster join; neither exists on the Unit-stack row.
            Field::MixedStructuralScalarAbiInsertOnUnitRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi = Some(fabricated_mixed_abi());
            }
            // Entry spills are retained only alongside continuation custody.
            Field::ParameterAbiInsertWithSpills => {
                let row = &mut record.functions_mut_for_test()[2];
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
            }
            // The structural-call/scalar-return evidence is exact only for the
            // bounded carrier: one Operation-owned scalar-result call joined to
            // matching attribution and cleanup rows.
            Field::StructuralCallScalarReturnInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.structural_call_scalar_return = Some(structural_scalar_return());
            }
            Field::StructuralCallScalarReturnInsertOnUnitCaller => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return = Some(structural_scalar_return());
            }
            Field::ParameterAbiInsertOnCallee => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi = Some(canonical_parameter_abi());
            }
        }
    };
    let outcome = |field: ScalarTransportFieldForTest| {
        use ScalarTransportFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarAbiInsertCanonical
            | Field::ScalarAbiInsertCanonicalAltValue
            | Field::ParameterAbiInsertCanonical => InstallationError::ImageBindingMismatch,
            Field::ScalarAbiInsertNoncanonical => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::MixedStructuralScalarAbiInsertOnUnitRow => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::ParameterAbiInsertWithSpills => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::StructuralCallScalarReturnInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(3))
            }
            Field::StructuralCallScalarReturnInsertOnUnitCaller => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::ParameterAbiInsertOnCallee => {
                InstallationError::InvalidInternalUnitCall(machine_id(3))
            }
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar transport / record",
        fields: ScalarTransportFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });

    // On the scalar-stack row the canonical scalar ABI and the exactly
    // matching mixed ABI still encode; independent replay rejects both
    // against the image that retains neither.
    let mut scalar_plan = edge_owned_cleanup_plan();
    promote_x86_cleanup_to_scalar(&mut scalar_plan.functions[2]);
    let scalar_artifact = build_object_artifact(&scalar_plan).expect("scalar cleanup artifact");
    let scalar_image =
        emit_direct_executable_image(&scalar_artifact, 3).expect("scalar cleanup image");
    let scalar_record =
        build_installation_record(&scalar_image, ProfileDecisionId::new(41).expect("profile"))
            .expect("scalar cleanup installation");
    validate_installation_record(&scalar_record, &scalar_image).expect("exact image binding");
    let scalar_fingerprint = installation_fingerprint(&scalar_record).expect("fingerprint");

    let substitute = |record: &mut InstallationRecord,
                      field: ScalarTransportScalarRecordFieldForTest,
                      _donor: &InstallationRecord| {
        use ScalarTransportScalarRecordFieldForTest as Field;
        match field {
            Field::ScalarAbiInsertCanonicalOnScalarRow => {
                let row = &mut record.functions_mut_for_test()[2];
                row.scalar_abi = Some(canonical_scalar_abi());
            }
            Field::MixedStructuralScalarAbiInsertMatching => {
                let row = &mut record.functions_mut_for_test()[2];
                let abi = matching_mixed_abi(row);
                row.mixed_structural_scalar_abi = Some(abi);
            }
        }
    };
    let outcome = |field: ScalarTransportScalarRecordFieldForTest| {
        use ScalarTransportScalarRecordFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::ScalarAbiInsertCanonicalOnScalarRow
            | Field::MixedStructuralScalarAbiInsertMatching => {
                InstallationError::ImageBindingMismatch
            }
        })
    };
    let check = nested_custody_check(&scalar_image, &scalar_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation scalar transport / scalar_record",
        fields: ScalarTransportScalarRecordFieldForTest::INVENTORY,
        honest: &|| scalar_record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("continuation image");
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

    let substitute = |record: &mut InstallationRecord,
                      field: UnitContinuationsFieldForTest,
                      _donor: &InstallationRecord| {
        use UnitContinuationsFieldForTest as Field;
        match field {
            Field::SourceBlock => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].source_block = block_id(9);
            }
            Field::TargetBlock => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].target_block = block_id(9);
            }
            Field::BindingsParameter => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings[0].parameter = value_id(52);
            }
            // The other live integer parameter is a representable argument.
            Field::BindingsArgument => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings[0].argument = value_id(46);
            }
            Field::BindingsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0]
                    .bindings
                    .push(abstract_operations::ValueBinding {
                        parameter: value_id(51),
                        argument: value_id(46),
                        scalar_type: i32_scalar(),
                    });
            }
            Field::BindingsDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings.pop();
            }
            // The call owns ordinal 0; the continuation must chain at ordinal 1.
            Field::OperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].operation_ordinal = 0;
            }
            Field::SuccessorOperationOrdinal => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].successor_operation_ordinal = 4;
            }
            // A continuation may not loop back to a block already on the chain.
            Field::TargetBlockRevisit => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].target_block = row.unit_continuations[0].source_block;
            }
            // The nested cleanup edge must carry its own zero-byte attribution.
            Field::CleanupPsiEdge => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.psi_edge = edge_id(8);
            }
            // Nor may it reuse the final return edge.
            Field::CleanupPsiEdgeReturnedEdge => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.psi_edge =
                    row.unit_affine_cleanup.as_ref().expect("cleanup").psi_edge;
            }
            Field::CleanupCodeOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.code_offset += 1;
            }
            Field::CleanupByteCount => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.byte_count = 1;
            }
            Field::CleanupLocalsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.locals.push(extra_local());
            }
            // The continuation cleanup retains exactly the returned cleanup's
            // structural type roster.
            Field::CleanupStructuralTypes => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.structural_types = extra_type_catalog();
            }
            // Residual discards name a partially consumed root; this caller moves
            // nothing across the boundary.
            Field::CleanupActionsInsertResidual => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place: place_id(1),
                            path: Vec::new(),
                            structural_type: structural_type(1),
                        },
                    ),
                );
            }
            Field::CleanupActionsInsertRoot => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].cleanup.actions.push(
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place_id(9)),
                );
            }
            // A binding may not rename a value already live across the boundary.
            Field::BindingsParameterLiveValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings[0].parameter = value_id(46);
            }
            Field::BindingsArgumentUnknownValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings[0].argument = value_id(99);
            }
            Field::BindingsScalarType => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations[0].bindings[0].scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
            }
            Field::BindingsInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let binding = row.unit_continuations[0].bindings[0];
                row.unit_continuations[0].bindings.push(binding);
            }
            // Dropping the row strands the zero-byte edge attribution the record
            // still carries; duplicating it breaks the strict ordinal chain.
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.unit_continuations.pop();
            }
            Field::InsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let continuation = row.unit_continuations[0].clone();
                row.unit_continuations.push(continuation);
            }
        }
    };
    let outcome = |field: UnitContinuationsFieldForTest| {
        use UnitContinuationsFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::SourceBlock
            | Field::TargetBlock
            | Field::BindingsParameter
            | Field::BindingsArgument
            | Field::BindingsInsert
            | Field::BindingsDrop => InstallationError::ImageBindingMismatch,
            Field::OperationOrdinal => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::SuccessorOperationOrdinal => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::TargetBlockRevisit => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::CleanupPsiEdge => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::CleanupPsiEdgeReturnedEdge => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::CleanupCodeOffset => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::CleanupByteCount => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::CleanupLocalsInsert => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::CleanupStructuralTypes => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::CleanupActionsInsertResidual => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::CleanupActionsInsertRoot => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::BindingsParameterLiveValue => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::BindingsArgumentUnknownValue => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::BindingsScalarType => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::BindingsInsertDuplicate => {
                InstallationError::InvalidUnitAffineCleanup(machine_id(1))
            }
            Field::Drop => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
            Field::InsertDuplicate => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation unit continuations / record",
        fields: UnitContinuationsFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("continuation image");
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

    // Every other leaf is a canonical projection: the caller plan is
    // recomputed from the parameter scalar types, each placement and the
    // distinct value identities rejoin the plan and the spill roster, each
    // spill rejoins its parameter, register, frame offset and code interval,
    // and the continuation bindings rejoin the bound parameter's type.
    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(1));

    let substitute = |record: &mut InstallationRecord,
                      field: ParameterAbiFieldForTest,
                      _donor: &InstallationRecord| {
        use ParameterAbiFieldForTest as Field;
        match field {
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi = None;
            }
            Field::CallPlanPolicy => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .policy = CallingPolicy::MicrosoftX64;
            }
            Field::CallPlanParametersDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .clear();
            }
            Field::CallPlanParametersInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .push(register_placement());
            }
            Field::CallPlanParameters => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters[0] = register_placement();
            }
            Field::CallPlanParameters1 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters[1] = register_placement();
            }
            Field::CallPlanParametersSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .parameters
                    .swap(0, 1);
            }
            Field::CallPlanParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let placement = abi.call_plan.parameters[0].clone();
                abi.call_plan.parameters.push(placement);
            }
            Field::CallPlanResult => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .result = Some(register_placement());
            }
            Field::CallPlanCallbackMaterializationsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
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
            }
            Field::CallPlanOrdinaryClobbers => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .ordinary_clobbers = calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::X86Rbx,
                ]);
            }
            Field::CallPlanStackAlignment => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .stack_alignment = 8;
            }
            Field::CallPlanShadowBytes => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .shadow_bytes = 32;
            }
            Field::CallPlanEntryControl => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .call_plan
                    .entry_control = calling_conventions::EntryControl::InterruptReturn;
            }
            Field::ParametersValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .value = value_id(47);
            }
            Field::ParametersValueOtherParameter => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .value = value_id(46);
            }
            Field::ParametersScalarTypeU32 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = u32_scalar;
            }
            Field::ParametersScalarTypeI64 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = i64_scalar;
            }
            Field::ParametersScalarTypeBoolean => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .scalar_type = ScalarType::Boolean;
            }
            Field::ParametersPlacement => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[0]
                    .placement = register_placement();
            }
            Field::Parameters1Value => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .value = value_id(47);
            }
            Field::Parameters1ValueOtherParameter => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .value = value_id(45);
            }
            Field::Parameters1ScalarTypeI64 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .scalar_type = i64_scalar;
            }
            Field::Parameters1ScalarTypeBoolean => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .scalar_type = ScalarType::Boolean;
            }
            Field::Parameters1Placement => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .placement = register_placement();
            }
            Field::ParametersInsert => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .push(ScalarAbiValue {
                        value: value_id(47),
                        scalar_type: i32_scalar(),
                        placement: register_placement(),
                    });
            }
            Field::ParametersDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .pop();
            }
            Field::ParametersSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters
                    .swap(0, 1);
            }
            Field::ParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let parameter = abi.parameters[0].clone();
                abi.parameters.push(parameter);
            }
            Field::EntryRegisterSpillsSourceValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .source_value = value_id(47);
            }
            Field::EntryRegisterSpillsParameterIndex => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .parameter_index = 1;
            }
            Field::EntryRegisterSpillsRegister => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .register = calling_conventions::MachineRegister::X86Rdx;
            }
            Field::EntryRegisterSpillsByteOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .byte_offset = 8;
            }
            Field::EntryRegisterSpillsCodeOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .code_offset += 1;
            }
            Field::EntryRegisterSpillsByteCount => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[0]
                    .byte_count = 4;
            }
            Field::EntryRegisterSpills1SourceValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .source_value = value_id(45);
            }
            Field::EntryRegisterSpills1ParameterIndex => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .parameter_index = 0;
            }
            Field::EntryRegisterSpills1Register => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .register = calling_conventions::MachineRegister::X86Rdx;
            }
            Field::EntryRegisterSpills1ByteOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .byte_offset = 0;
            }
            Field::EntryRegisterSpills1CodeOffset => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .code_offset += 1;
            }
            Field::EntryRegisterSpills1ByteCount => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills[1]
                    .byte_count = 6;
            }
            Field::EntryRegisterSpillsInsert => {
                let row = &mut record.functions_mut_for_test()[0];
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
            }
            Field::EntryRegisterSpillsDrop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills
                    .pop();
            }
            Field::EntryRegisterSpillsSwap => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .entry_register_spills
                    .swap(0, 1);
            }
            Field::EntryRegisterSpillsInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[0];
                let abi = row.parameter_abi.as_mut().expect("parameter ABI");
                let spill = abi.entry_register_spills[0];
                abi.entry_register_spills.push(spill);
            }
            // The second parameter is not bound by any continuation: a same-shape
            // scalar type still satisfies the recomputed caller plan and every join,
            // so the substitution encodes, decodes exactly, recomputes a distinct
            // identity, and is rejected by independent replay against the unchanged
            // image.
            Field::Parameters1ScalarTypeU32 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.parameter_abi
                    .as_mut()
                    .expect("parameter ABI")
                    .parameters[1]
                    .scalar_type = u32_scalar;
            }
        }
    };
    let outcome = |field: ParameterAbiFieldForTest| {
        use ParameterAbiFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::Parameters1ScalarTypeU32 => InstallationError::ImageBindingMismatch,
            Field::Drop => invalid.clone(),
            Field::CallPlanPolicy => invalid.clone(),
            Field::CallPlanParametersDrop => invalid.clone(),
            Field::CallPlanParametersInsert => invalid.clone(),
            Field::CallPlanParameters => invalid.clone(),
            Field::CallPlanParameters1 => invalid.clone(),
            Field::CallPlanParametersSwap => invalid.clone(),
            Field::CallPlanParametersInsertDuplicate => invalid.clone(),
            Field::CallPlanResult => invalid.clone(),
            Field::CallPlanCallbackMaterializationsInsert => invalid.clone(),
            Field::CallPlanOrdinaryClobbers => invalid.clone(),
            Field::CallPlanStackAlignment => invalid.clone(),
            Field::CallPlanShadowBytes => invalid.clone(),
            Field::CallPlanEntryControl => invalid.clone(),
            Field::ParametersValue => invalid.clone(),
            Field::ParametersValueOtherParameter => invalid.clone(),
            Field::ParametersScalarTypeU32 => invalid.clone(),
            Field::ParametersScalarTypeI64 => invalid.clone(),
            Field::ParametersScalarTypeBoolean => invalid.clone(),
            Field::ParametersPlacement => invalid.clone(),
            Field::Parameters1Value => invalid.clone(),
            Field::Parameters1ValueOtherParameter => invalid.clone(),
            Field::Parameters1ScalarTypeI64 => invalid.clone(),
            Field::Parameters1ScalarTypeBoolean => invalid.clone(),
            Field::Parameters1Placement => invalid.clone(),
            Field::ParametersInsert => invalid.clone(),
            Field::ParametersDrop => invalid.clone(),
            Field::ParametersSwap => invalid.clone(),
            Field::ParametersInsertDuplicate => invalid.clone(),
            Field::EntryRegisterSpillsSourceValue => invalid.clone(),
            Field::EntryRegisterSpillsParameterIndex => invalid.clone(),
            Field::EntryRegisterSpillsRegister => invalid.clone(),
            Field::EntryRegisterSpillsByteOffset => invalid.clone(),
            Field::EntryRegisterSpillsCodeOffset => invalid.clone(),
            Field::EntryRegisterSpillsByteCount => invalid.clone(),
            Field::EntryRegisterSpills1SourceValue => invalid.clone(),
            Field::EntryRegisterSpills1ParameterIndex => invalid.clone(),
            Field::EntryRegisterSpills1Register => invalid.clone(),
            Field::EntryRegisterSpills1ByteOffset => invalid.clone(),
            Field::EntryRegisterSpills1CodeOffset => invalid.clone(),
            Field::EntryRegisterSpills1ByteCount => invalid.clone(),
            Field::EntryRegisterSpillsInsert => invalid.clone(),
            Field::EntryRegisterSpillsDrop => invalid.clone(),
            Field::EntryRegisterSpillsSwap => invalid.clone(),
            Field::EntryRegisterSpillsInsertDuplicate => invalid.clone(),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation parameter abi / record",
        fields: ParameterAbiFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("mixed-ABI image");
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

    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(3));

    let substitute = |record: &mut InstallationRecord,
                      field: MixedAbiFieldForTest,
                      _donor: &InstallationRecord| {
        use MixedAbiFieldForTest as Field;
        match field {
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi = None;
            }
            Field::ScalarParametersValue => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .value = value_id(59);
            }
            Field::ScalarParametersScalarTypeU32 => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = u32_scalar;
            }
            Field::StructuralParametersProjectedQualificationsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .projected_qualifications
                    .push(terminal_psi::StructuralPathQualification {
                        path: vec![StructuralPathSegment::Field("projected".to_string())],
                        domain: semantic_vocabulary::StructuralDomainId::new(1).expect("domain"),
                    });
            }
            Field::ResultValue => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .value = value_id(59);
            }
            Field::ResultScalarTypeU32 => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = u32_scalar;
            }
            Field::CallPlanPolicy => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .policy = CallingPolicy::MicrosoftX64;
            }
            Field::CallPlanParametersDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .clear();
            }
            Field::CallPlanParametersInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .push(register_placement());
            }
            Field::CallPlanParameters => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters[0] = register_placement();
            }
            Field::CallPlanParameters1 => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters[1] = register_placement();
            }
            Field::CallPlanParametersSwap => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .parameters
                    .swap(0, 1);
            }
            Field::CallPlanParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let placement = abi.call_plan.parameters[0].clone();
                abi.call_plan.parameters.push(placement);
            }
            Field::CallPlanResult => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .result = None;
            }
            Field::CallPlanCallbackMaterializationsInsert => {
                let row = &mut record.functions_mut_for_test()[2];
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
            }
            Field::CallPlanOrdinaryClobbers => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .ordinary_clobbers = calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::X86Rbx,
                ]);
            }
            Field::CallPlanStackAlignment => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .stack_alignment = 8;
            }
            Field::CallPlanShadowBytes => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .shadow_bytes = 32;
            }
            Field::CallPlanEntryControl => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .call_plan
                    .entry_control = calling_conventions::EntryControl::InterruptReturn;
            }
            Field::ScalarParametersValueResultCollision => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .value = value_id(49);
            }
            Field::ScalarParametersScalarTypeI64 => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = i64_scalar;
            }
            Field::ScalarParametersScalarTypeBoolean => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .scalar_type = ScalarType::Boolean;
            }
            Field::ScalarParametersPlacement => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters[0]
                    .placement = register_placement();
            }
            Field::ScalarParametersInsert => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters
                    .push(ScalarAbiValue {
                        value: value_id(59),
                        scalar_type: i32_scalar(),
                        placement: register_placement(),
                    });
            }
            Field::ScalarParametersDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .scalar_parameters
                    .clear();
            }
            Field::ScalarParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let parameter = abi.scalar_parameters[0].clone();
                abi.scalar_parameters.push(parameter);
            }
            Field::StructuralParametersPlace => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .place = place_id(9);
            }
            Field::StructuralParametersStructuralType => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .structural_type = structural_type(9);
            }
            Field::StructuralParametersMultiplicity => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .multiplicity = StructuralMultiplicity::Unrestricted;
            }
            Field::StructuralParametersAccess => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .access = StructuralAccess::SharedBorrow;
            }
            Field::StructuralParametersShape => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .shape = ValueShape::integer(8, 8);
            }
            Field::StructuralParametersPlacement => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters[0]
                    .placement = register_placement();
            }
            Field::StructuralParametersInsert => {
                let row = &mut record.functions_mut_for_test()[2];
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
            }
            Field::StructuralParametersDrop => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .structural_parameters
                    .clear();
            }
            Field::StructuralParametersInsertDuplicate => {
                let row = &mut record.functions_mut_for_test()[2];
                let abi = row.mixed_structural_scalar_abi.as_mut().expect("mixed ABI");
                let parameter = abi.structural_parameters[0].clone();
                abi.structural_parameters.push(parameter);
            }
            Field::ResultScalarTypeI64 => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = i64_scalar;
            }
            Field::ResultScalarTypeBoolean => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .scalar_type = ScalarType::Boolean;
            }
            Field::ResultPlacement => {
                let row = &mut record.functions_mut_for_test()[2];
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
            }
            Field::ResultValueParameterCollision => {
                let row = &mut record.functions_mut_for_test()[2];
                row.mixed_structural_scalar_abi
                    .as_mut()
                    .expect("mixed ABI")
                    .result
                    .value = value_id(51);
            }
        }
    };
    let outcome = |field: MixedAbiFieldForTest| {
        use MixedAbiFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::Drop
            | Field::ScalarParametersValue
            | Field::ScalarParametersScalarTypeU32
            | Field::StructuralParametersProjectedQualificationsInsert
            | Field::ResultValue
            | Field::ResultScalarTypeU32 => InstallationError::ImageBindingMismatch,
            Field::CallPlanPolicy => invalid.clone(),
            Field::CallPlanParametersDrop => invalid.clone(),
            Field::CallPlanParametersInsert => invalid.clone(),
            Field::CallPlanParameters => invalid.clone(),
            Field::CallPlanParameters1 => invalid.clone(),
            Field::CallPlanParametersSwap => invalid.clone(),
            Field::CallPlanParametersInsertDuplicate => invalid.clone(),
            Field::CallPlanResult => invalid.clone(),
            Field::CallPlanCallbackMaterializationsInsert => invalid.clone(),
            Field::CallPlanOrdinaryClobbers => invalid.clone(),
            Field::CallPlanStackAlignment => invalid.clone(),
            Field::CallPlanShadowBytes => invalid.clone(),
            Field::CallPlanEntryControl => invalid.clone(),
            Field::ScalarParametersValueResultCollision => invalid.clone(),
            Field::ScalarParametersScalarTypeI64 => invalid.clone(),
            Field::ScalarParametersScalarTypeBoolean => invalid.clone(),
            Field::ScalarParametersPlacement => invalid.clone(),
            Field::ScalarParametersInsert => invalid.clone(),
            Field::ScalarParametersDrop => invalid.clone(),
            Field::ScalarParametersInsertDuplicate => invalid.clone(),
            Field::StructuralParametersPlace => invalid.clone(),
            Field::StructuralParametersStructuralType => invalid.clone(),
            Field::StructuralParametersMultiplicity => invalid.clone(),
            Field::StructuralParametersAccess => invalid.clone(),
            Field::StructuralParametersShape => invalid.clone(),
            Field::StructuralParametersPlacement => invalid.clone(),
            Field::StructuralParametersInsert => invalid.clone(),
            Field::StructuralParametersDrop => invalid.clone(),
            Field::StructuralParametersInsertDuplicate => invalid.clone(),
            Field::ResultScalarTypeI64 => invalid.clone(),
            Field::ResultScalarTypeBoolean => invalid.clone(),
            Field::ResultPlacement => invalid.clone(),
            Field::ResultValueParameterCollision => invalid.clone(),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation mixed abi / record",
        fields: MixedAbiFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
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
    let image = emit_direct_executable_image(&artifact, 3).expect("structural-call image");
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

    let invalid = InstallationError::InvalidUnitAffineCleanup(machine_id(1));

    let substitute = |record: &mut InstallationRecord,
                      field: StructuralCallScalarReturnFieldForTest,
                      _donor: &InstallationRecord| {
        use StructuralCallScalarReturnFieldForTest as Field;
        match field {
            Field::PsiEdge => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .psi_edge = edge_id(8);
            }
            Field::PsiOperation => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .psi_operation = operation_id(9);
            }
            Field::SourceValue => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .source_value = value_id(9);
            }
            Field::ScalarTypeU32 => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
            }
            Field::ScalarTypeBoolean => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .scalar_type = ScalarType::Boolean;
            }
            Field::CalleeUnknown => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .callee = machine_id(9);
            }
            Field::CalleeCaller => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return
                    .as_mut()
                    .expect("scalar return")
                    .callee = machine_id(1);
            }
            Field::Drop => {
                let row = &mut record.functions_mut_for_test()[0];
                row.structural_call_scalar_return = None;
            }
        }
    };
    let outcome = |field: StructuralCallScalarReturnFieldForTest| {
        use StructuralCallScalarReturnFieldForTest as Field;
        MutationOutcome::ExactError(match field {
            Field::Drop => InstallationError::ImageBindingMismatch,
            Field::PsiEdge => invalid.clone(),
            Field::PsiOperation => invalid.clone(),
            Field::SourceValue => invalid.clone(),
            Field::ScalarTypeU32 => invalid.clone(),
            Field::ScalarTypeBoolean => invalid.clone(),
            Field::CalleeUnknown => invalid.clone(),
            Field::CalleeCaller => invalid.clone(),
        })
    };
    let check = nested_custody_check(&image, &authentic_fingerprint);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation structural call scalar return / record",
        fields: StructuralCallScalarReturnFieldForTest::INVENTORY,
        honest: &|| record.clone(),
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });
}
