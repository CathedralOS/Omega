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
    WriteExitProvider, continuation_unit_call_plan, edge_id, edge_owned_cleanup_plan, machine_id,
    operation_id, promote_x86_cleanup_to_scalar, scalar_three_leaf_cleanup_plan,
    stored_dynamic_call_plan, structural_call_scalar_return_plan, windows_foreign_call_plan,
};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use image_emission::{
    InstallationError, InstallationRecord, InstalledFunction, build_installation_record,
    build_installation_record_with_selected_provider_plans_and_evidence, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use machine_code::{
    StructuralCallScalarReturnEvidence, UnitAffineScalarRecordEstablishmentRecord,
    UnitContinuationRecord, UnitIntegerConstantRecord,
};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, PlaceId, ProfileDecisionId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{CallSiteOwner, ScalarAbiValue};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

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
                let binding = row.unit_continuations[0].bindings[0].clone();
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
