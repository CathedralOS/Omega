//! Authenticated one-field mutation coverage for installed attached-Unit
//! scalar-call custody rows.
//!
//! Each `InstalledInternalUnitScalarCall` joins one retained
//! `internal_unit_scalar_calls` custody record to its caller's text offset.
//! Most fields are canonical projections: the record shape rejoins the owner,
//! target, call plan, result home and source, argument destinations and
//! sources, and the call interval against the caller's retained homes,
//! constants, and stack evidence plus the callee's scalar ABI, so a one-field
//! substitution is rejected at canonical encoding. Bounded slack remains in
//! `operation_ordinal`, the call `byte_count`, the nested argument and result
//! `code_offset`/`byte_count` intervals, and a `Home` argument source naming
//! an earlier producer; those substitutions still encode. Each must recompute
//! a distinct installation fingerprint and be rejected by independent replay
//! against the unchanged image.

use calling_conventions::{
    CallSignature, CallbackMaterialization, CallingPolicy, EntryControl, MachineRegister,
    NativeParameterId, NativePlace, RegisterSet, StaticMachineBinderId, ValueLocation,
    ValuePlacement, ValueShape, evaluate_call_plan,
};
use image_emission::{
    InstallationError, InstalledFunction, InstalledInternalUnitScalarCall,
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, installation_fingerprint,
    validate_installation_record,
};
use machine_code::{
    InternalCallRelocation, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallArgumentRecord, InternalUnitScalarCallRecord,
    InternalUnitScalarCallResultRecord, MachineCodeFunction, MachineCodePlan,
    ScalarControlFlowEvidence, ScalarStackEvidence, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, UnitAffineCleanupRecord, UnitCallStackEvidence, UnitIntegerConstantRecord,
    UnitScalarHomeRecord, UnitStackEvidence,
};
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ProfileDecisionId,
    ScalarType, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{CallSiteOwner, ScalarAbiValue, ScalarFunctionAbi, TerminalPsiProvenance};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[test]
fn installation_internal_unit_scalar_call_row_rejects_every_one_field_substitution() {
    let plan = attached_unit_scalar_call_plan();
    let artifact = build_object_artifact(&plan).expect("scalar-call artifact");
    let image = emit_executable_image(&artifact, 3).expect("scalar-call image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("scalar-call installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");

    assert_eq!(record.internal_unit_scalar_calls().len(), 2);
    let authentic = &record.internal_unit_scalar_calls()[0];
    assert_eq!(authentic.machine, machine_id(1));
    assert_eq!(authentic.text_offset, 9);
    assert_eq!(
        authentic.custody.owner,
        CallSiteOwner::Operation(operation_id(2))
    );
    assert_eq!(authentic.custody.target, machine_id(2));
    assert_eq!(authentic.custody.operation_ordinal, 1);
    assert_eq!(authentic.custody.code_offset, 9);
    assert_eq!(authentic.custody.byte_count, 31);
    assert_eq!(authentic.custody.result.code_offset, 32);
    assert_eq!(authentic.custody.result.byte_count, 8);
    assert_eq!(authentic.custody.arguments.len(), 1);
    assert_eq!(authentic.custody.arguments[0].parameter_index, 0);
    assert_eq!(authentic.custody.arguments[0].code_offset, 13);
    assert_eq!(authentic.custody.arguments[0].byte_count, 10);
    let second = &record.internal_unit_scalar_calls()[1];
    assert_eq!(second.text_offset, 40);
    assert_eq!(
        second.custody.owner,
        CallSiteOwner::Operation(operation_id(3))
    );
    assert_eq!(second.custody.operation_ordinal, 2);
    assert_eq!(second.custody.code_offset, 40);
    assert_eq!(second.custody.result.code_offset, 63);
    assert_eq!(second.custody.arguments[0].code_offset, 44);
    let produced_home = authentic.custody.result.home;
    let second_home = second.custody.result.home;
    assert_eq!(produced_home.byte_offset, 0);
    assert_eq!(second_home.byte_offset, 8);

    let i32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let i32_shape = ValueShape::integer(4, 4);
    let wrong_placement = ValuePlacement {
        shape: i32_shape,
        locations: vec![ValueLocation::Register {
            register: MachineRegister::X86Rsi,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    };
    let wrong_immediate = InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        defining_operation: operation_id(1),
        source_value: value_id(1),
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
        value: IntegerValue::Signed(-3),
    };
    let parameter_source = InternalUnitScalarArgumentSourceRecord::Parameter {
        parameter_index: 0,
        source_value: value_id(11),
        scalar_type: i32_scalar,
        location: machine_code::UnitScalarParameterLocationRecord::Register(
            MachineRegister::X86Rdi,
        ),
    };
    let boolean_source = InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
        defining_operation: operation_id(1),
        source_value: value_id(1),
        value: true,
        definition_ordinal: 0,
    };
    let wrong_result_source = wrong_placement.clone();
    let wrong_argument_destination = wrong_placement.clone();

    // Interval and producer slack that survives the canonical shape joins: the
    // call span may extend past its nested result interval, argument and result
    // intervals may move inside the call span, the operation ordinal may grow
    // while roster order and constant precedence survive, and a `Home` source
    // may name any earlier producer in the same caller. Each substitution
    // encodes, decodes to the same record, recomputes a distinct installation
    // identity, and is rejected by independent replay against the unchanged
    // image.
    type SlackMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut InstalledInternalUnitScalarCall)>,
    );
    let slack_mutations: Vec<SlackMutation> = vec![
        (
            "operation_ordinal",
            0,
            Box::new(|row| {
                row.custody.operation_ordinal = 2;
            }),
        ),
        (
            "operation_ordinal::second_call",
            1,
            Box::new(|row| {
                row.custody.operation_ordinal = 3;
            }),
        ),
        (
            "byte_count",
            0,
            Box::new(|row| {
                row.custody.byte_count = 32;
            }),
        ),
        (
            "byte_count::second_call",
            1,
            Box::new(|row| {
                row.custody.byte_count = 36;
            }),
        ),
        (
            "result.code_offset",
            0,
            Box::new(|row| {
                row.custody.result.code_offset = 31;
            }),
        ),
        (
            "result.byte_count",
            0,
            Box::new(|row| {
                row.custody.result.byte_count = 7;
            }),
        ),
        (
            "argument.code_offset",
            0,
            Box::new(|row| {
                row.custody.arguments[0].code_offset = 14;
            }),
        ),
        (
            "argument.byte_count",
            0,
            Box::new(|row| {
                row.custody.arguments[0].byte_count = 9;
            }),
        ),
        (
            "argument.source::earlier_home",
            1,
            Box::new(move |row| {
                row.custody.arguments[0].source =
                    InternalUnitScalarArgumentSourceRecord::Home(produced_home);
            }),
        ),
        (
            "result.code_offset::second_call",
            1,
            Box::new(|row| {
                row.custody.result.code_offset = 62;
            }),
        ),
        (
            "result.byte_count::second_call",
            1,
            Box::new(|row| {
                row.custody.result.byte_count = 7;
            }),
        ),
        (
            "argument.code_offset::second_call",
            1,
            Box::new(|row| {
                row.custody.arguments[0].code_offset = 45;
            }),
        ),
        (
            "argument.byte_count::second_call",
            1,
            Box::new(|row| {
                row.custody.arguments[0].byte_count = 11;
            }),
        ),
    ];
    for (field, row_index, mutate) in slack_mutations {
        let mut changed = record.clone();
        mutate(&mut changed.internal_unit_scalar_calls_mut_for_test()[row_index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: slack substitution encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: slack substitution decodes: {error:?}"));
        assert_eq!(replayed, changed, "{field}: codec round trip is exact");
        assert_ne!(
            installation_fingerprint(&replayed).expect("substituted fingerprint"),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // Dropped rows keep the surviving roster canonical: each removed call is a
    // complete row substitution that still encodes, recomputes a distinct
    // installation identity, and is rejected by independent replay.
    for dropped in [0_usize, 1_usize] {
        let mut changed = record.clone();
        changed
            .internal_unit_scalar_calls_mut_for_test()
            .remove(dropped);
        let bytes = encode_installation_record(&changed).expect("dropped row encodes");
        let replayed = decode_installation_record(&bytes).expect("dropped row decodes");
        assert_eq!(replayed, changed);
        assert_ne!(
            installation_fingerprint(&replayed).expect("substituted fingerprint"),
            authentic_fingerprint,
            "dropped row {dropped}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "dropped row {dropped}: independent replay rejects the substitution"
        );
    }

    // Every remaining field is a canonical projection bound by the caller's
    // retained homes, constants, and stack evidence or by the callee's scalar
    // ABI, so a one-field substitution is rejected at canonical encoding
    // before any identity or replay could accept it.
    let invalid = InstallationError::InvalidInternalUnitScalarCall(machine_id(1));
    type CallMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut InstalledInternalUnitScalarCall)>,
        InstallationError,
    );
    let mutations: Vec<CallMutation> = vec![
        (
            "machine::other_function",
            0,
            Box::new(|row| {
                row.machine = machine_id(2);
            }),
            InstallationError::InvalidInternalUnitScalarCall(machine_id(2)),
        ),
        (
            "machine::unknown_function",
            0,
            Box::new(|row| {
                row.machine = machine_id(4);
            }),
            InstallationError::InvalidInternalUnitScalarCall(machine_id(4)),
        ),
        (
            "text_offset",
            0,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "text_offset::second_call",
            1,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "owner::unknown_operation",
            0,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::Operation(operation_id(7));
            }),
            invalid.clone(),
        ),
        (
            "owner::cleanup_action",
            0,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::CleanupAction {
                    edge: edge_id(1),
                    action_ordinal: 0,
                };
            }),
            invalid.clone(),
        ),
        (
            "target::caller_without_scalar_abi",
            0,
            Box::new(|row| {
                row.custody.target = machine_id(1);
            }),
            invalid.clone(),
        ),
        (
            "target::unknown_function",
            0,
            Box::new(|row| {
                row.custody.target = machine_id(4);
            }),
            invalid.clone(),
        ),
        (
            "call_plan::parameters",
            0,
            Box::new(|row| {
                row.custody.call_plan.parameters.clear();
            }),
            invalid.clone(),
        ),
        (
            "call_plan::result",
            0,
            Box::new(|row| {
                row.custody.call_plan.result = None;
            }),
            invalid.clone(),
        ),
        (
            "call_plan::stack_alignment",
            0,
            Box::new(|row| {
                row.custody.call_plan.stack_alignment = 8;
            }),
            invalid.clone(),
        ),
        (
            "result.home::defining_operation",
            0,
            Box::new(|row| {
                row.custody.result.home.defining_operation = operation_id(3);
            }),
            invalid.clone(),
        ),
        (
            "result.home::source_value",
            0,
            Box::new(|row| {
                row.custody.result.home.source_value = value_id(9);
            }),
            invalid.clone(),
        ),
        (
            "result.home::scalar_type",
            0,
            Box::new(|row| {
                row.custody.result.home.scalar_type = ScalarType::Boolean;
            }),
            invalid.clone(),
        ),
        (
            "result.home::shape",
            0,
            Box::new(|row| {
                row.custody.result.home.shape = ValueShape::integer(8, 8);
            }),
            invalid.clone(),
        ),
        (
            "result.home::byte_offset",
            0,
            Box::new(|row| {
                row.custody.result.home.byte_offset = 8;
            }),
            invalid.clone(),
        ),
        (
            "result.home::other_call_home",
            0,
            Box::new(move |row| {
                row.custody.result.home = second_home;
            }),
            invalid.clone(),
        ),
        (
            "result.source",
            0,
            Box::new(move |row| {
                row.custody.result.source = wrong_result_source.clone();
            }),
            invalid.clone(),
        ),
        (
            "result.code_offset::before_call",
            0,
            Box::new(|row| {
                row.custody.result.code_offset = 8;
            }),
            invalid.clone(),
        ),
        (
            "result.byte_count::zero",
            0,
            Box::new(|row| {
                row.custody.result.byte_count = 0;
            }),
            invalid.clone(),
        ),
        (
            "result.byte_count::past_call_end",
            0,
            Box::new(|row| {
                row.custody.result.byte_count = 9;
            }),
            invalid.clone(),
        ),
        (
            "code_offset",
            0,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "code_offset::second_call",
            1,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            invalid.clone(),
        ),
        (
            "byte_count::zero",
            0,
            Box::new(|row| {
                row.custody.byte_count = 0;
            }),
            invalid.clone(),
        ),
        (
            "byte_count::inside_result_end",
            0,
            Box::new(|row| {
                row.custody.byte_count = 30;
            }),
            invalid.clone(),
        ),
        (
            "byte_count::past_function_end",
            0,
            Box::new(|row| {
                row.custody.byte_count = 68;
            }),
            invalid.clone(),
        ),
        (
            "operation_ordinal::constant_ordinal",
            0,
            Box::new(|row| {
                row.custody.operation_ordinal = 0;
            }),
            invalid.clone(),
        ),
        (
            "operation_ordinal::out_of_order",
            0,
            Box::new(|row| {
                row.custody.operation_ordinal = 3;
            }),
            invalid.clone(),
        ),
        (
            "argument.parameter_index",
            0,
            Box::new(|row| {
                row.custody.arguments[0].parameter_index = 1;
            }),
            invalid.clone(),
        ),
        (
            "argument.destination",
            0,
            Box::new(move |row| {
                row.custody.arguments[0].destination = wrong_argument_destination.clone();
            }),
            invalid.clone(),
        ),
        (
            "argument.source::unknown_immediate",
            0,
            Box::new(move |row| {
                row.custody.arguments[0].source = wrong_immediate;
            }),
            invalid.clone(),
        ),
        (
            "argument.source::unknown_constant_operation",
            0,
            Box::new(|row| {
                row.custody.arguments[0].source =
                    InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                        defining_operation: operation_id(9),
                        source_value: value_id(1),
                        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
                        value: IntegerValue::Signed(-17),
                    };
            }),
            invalid.clone(),
        ),
        (
            "argument.source::parameter",
            0,
            Box::new(move |row| {
                row.custody.arguments[0].source = parameter_source;
            }),
            invalid.clone(),
        ),
        (
            "argument.source::boolean_immediate",
            0,
            Box::new(move |row| {
                row.custody.arguments[0].source = boolean_source;
            }),
            invalid.clone(),
        ),
        (
            "argument.source::self_home",
            0,
            Box::new(move |row| {
                row.custody.arguments[0].source =
                    InternalUnitScalarArgumentSourceRecord::Home(produced_home);
            }),
            invalid.clone(),
        ),
        (
            "argument.source::own_result_home",
            1,
            Box::new(move |row| {
                row.custody.arguments[0].source =
                    InternalUnitScalarArgumentSourceRecord::Home(second_home);
            }),
            invalid.clone(),
        ),
        (
            "argument.code_offset::before_call",
            0,
            Box::new(|row| {
                row.custody.arguments[0].code_offset = 4;
            }),
            invalid.clone(),
        ),
        (
            "argument.byte_count::zero",
            0,
            Box::new(|row| {
                row.custody.arguments[0].byte_count = 0;
            }),
            invalid.clone(),
        ),
        (
            "argument.byte_count::past_call_end",
            0,
            Box::new(|row| {
                row.custody.arguments[0].byte_count = 28;
            }),
            invalid.clone(),
        ),
        (
            "arguments::extra_argument",
            0,
            Box::new(|row| {
                let extra = row.custody.arguments[0].clone();
                row.custody.arguments.push(extra);
            }),
            invalid.clone(),
        ),
        (
            "arguments::dropped_argument",
            0,
            Box::new(|row| {
                row.custody.arguments.clear();
            }),
            invalid.clone(),
        ),
        (
            "duplicated_row",
            usize::MAX,
            Box::new(|_| {}),
            invalid.clone(),
        ),
        (
            "swapped_rows",
            usize::MAX - 1,
            Box::new(|_| {}),
            invalid.clone(),
        ),
    ];
    for (field, row_index, mutate, expected) in mutations {
        let mut changed = record.clone();
        match row_index {
            usize::MAX => {
                let duplicated = changed.internal_unit_scalar_calls()[0].clone();
                changed
                    .internal_unit_scalar_calls_mut_for_test()
                    .insert(1, duplicated);
            }
            sentinel if sentinel == usize::MAX - 1 => {
                changed.internal_unit_scalar_calls_mut_for_test().swap(0, 1);
            }
            row_index => {
                mutate(&mut changed.internal_unit_scalar_calls_mut_for_test()[row_index]);
            }
        }
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted row is rejected at canonical encoding"
        );
    }
}

/// Authenticated one-field mutation coverage for the callee's retained
/// `scalar_abi`: every `ScalarFunctionAbi` leaf is a canonical projection the
/// record shape recomputes from the parameter and result scalar types or
/// rejoins against the caller's scalar calls. Semantic value identities are
/// the bounded slack — each still encodes, recomputes a distinct installation
/// fingerprint, and is rejected by independent replay against the unchanged
/// image; every other substitution is rejected at canonical encoding, either
/// by the callee row's own canonical shape or by the caller's call join.
#[test]
fn installation_function_scalar_abi_row_rejects_every_one_field_substitution() {
    let plan = attached_unit_scalar_call_plan();
    let artifact = build_object_artifact(&plan).expect("scalar-call artifact");
    let image = emit_executable_image(&artifact, 3).expect("scalar-call image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("scalar-call installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = record.functions()[1].clone();
    let abi = authentic.scalar_abi.as_ref().expect("retained scalar ABI");
    assert_eq!(abi.parameters.len(), 1);
    assert_eq!(abi.parameters[0].value, value_id(11));
    assert_eq!(abi.result.value, value_id(12));

    let i32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let i32_shape = ValueShape::integer(4, 4);
    let wrong_placement = move || ValuePlacement {
        shape: i32_shape,
        locations: vec![ValueLocation::Register {
            register: MachineRegister::X86Rsi,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    };
    let u32_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).expect("i64"));
    let callback_row = CallbackMaterialization {
        binder: StaticMachineBinderId::new(1).expect("binder"),
        destination: NativePlace::Parameter(NativeParameterId::new(1).expect("parameter")),
    };

    // Semantic value identities are not part of the recomputed native plan:
    // a fresh parameter or result value still encodes, decodes to the same
    // record, recomputes a distinct installation identity, and is rejected by
    // independent replay against the unchanged image.
    type SlackMutation = (&'static str, Box<dyn Fn(&mut InstalledFunction)>);
    let slack_mutations: Vec<SlackMutation> = vec![
        (
            "scalar_abi.parameters[0].value",
            Box::new(|row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].value = value_id(41);
            }),
        ),
        (
            "scalar_abi.result.value",
            Box::new(|row| {
                row.scalar_abi.as_mut().expect("scalar ABI").result.value = value_id(41);
            }),
        ),
    ];
    for (field, mutate) in slack_mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[1]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: slack substitution encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: slack substitution decodes: {error:?}"));
        assert_eq!(replayed, changed, "{field}: codec round trip is exact");
        assert_ne!(
            installation_fingerprint(&replayed).expect("substituted fingerprint"),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // The callee row's own canonical shape recomputes the native call plan
    // from the parameter and result scalar types and requires exact
    // placements, distinct value identities, and a roster matching the plan.
    let abi_shape = InstallationError::InvalidUnitAffineCleanup(machine_id(2));
    // Substitutions the ABI's own shape admits are still rejected by the
    // caller's scalar-call join: the argument's source type and the produced
    // result home rejoin the retained parameter and result scalar types.
    let call_join = InstallationError::InvalidInternalUnitScalarCall(machine_id(1));
    type AbiMutation = (
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    );
    let mutations: Vec<AbiMutation> = vec![
        (
            "scalar_abi::drop",
            Box::new(|row| {
                row.scalar_abi = None;
            }),
            call_join.clone(),
        ),
        (
            "scalar_abi.call_plan.policy",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .policy = CallingPolicy::MicrosoftX64;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.parameters::drop",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .parameters
                    .clear();
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.parameters::insert",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .parameters
                    .push(wrong_placement());
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.parameters[0]",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .parameters[0] = wrong_placement();
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.result",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .result = None;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.callback_materializations::insert",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .callback_materializations
                    .push(callback_row.clone());
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.ordinary_clobbers",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .ordinary_clobbers = RegisterSet::new([MachineRegister::X86Rbx]);
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.stack_alignment",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .stack_alignment = 8;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.shadow_bytes",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .shadow_bytes = 32;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.call_plan.entry_control",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .call_plan
                    .entry_control = EntryControl::InterruptReturn;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters[0].scalar_type::u32",
            Box::new(move |row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].scalar_type = u32_scalar;
            }),
            call_join.clone(),
        ),
        (
            "scalar_abi.parameters[0].scalar_type::i64",
            Box::new(move |row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].scalar_type = i64_scalar;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters[0].scalar_type::boolean",
            Box::new(|row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].scalar_type =
                    ScalarType::Boolean;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters[0].placement",
            Box::new(move |row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].placement =
                    wrong_placement();
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters[0].value::result_collision",
            Box::new(|row| {
                row.scalar_abi.as_mut().expect("scalar ABI").parameters[0].value = value_id(12);
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters::insert",
            Box::new(move |row| {
                let abi = row.scalar_abi.as_mut().expect("scalar ABI");
                abi.parameters.push(ScalarAbiValue {
                    value: value_id(13),
                    scalar_type: i32_scalar,
                    placement: wrong_placement(),
                });
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters::drop",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .parameters
                    .clear();
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.parameters::insert-duplicate",
            Box::new(|row| {
                let abi = row.scalar_abi.as_mut().expect("scalar ABI");
                let parameter = abi.parameters[0].clone();
                abi.parameters.push(parameter);
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.result.scalar_type::u32",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .result
                    .scalar_type = u32_scalar;
            }),
            call_join.clone(),
        ),
        (
            "scalar_abi.result.scalar_type::i64",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .result
                    .scalar_type = i64_scalar;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.result.scalar_type::boolean",
            Box::new(|row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .result
                    .scalar_type = ScalarType::Boolean;
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.result.placement",
            Box::new(move |row| {
                row.scalar_abi
                    .as_mut()
                    .expect("scalar ABI")
                    .result
                    .placement = wrong_placement();
            }),
            abi_shape.clone(),
        ),
        (
            "scalar_abi.result.value::parameter_collision",
            Box::new(|row| {
                row.scalar_abi.as_mut().expect("scalar ABI").result.value = value_id(11);
            }),
            abi_shape.clone(),
        ),
    ];
    for (field, mutate, expected) in mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[1]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted row is rejected at canonical encoding"
        );
    }
}

/// Authenticated one-field mutation coverage for the caller's retained
/// `unit_integer_constants` roster.
///
/// The constant both scalar calls name through `IntegerImmediate` argument
/// sources is bound by the canonical argument-source join: every one-field
/// substitution on it is rejected at encoding. A second retained constant no
/// call references is bound only by canonical roster shape — each of its
/// representable fields still encodes, recomputes a distinct installation
/// identity, and is rejected by independent replay against the unchanged
/// image, while non-canonical type, value, identity-collision, ordering,
/// swap, and duplication substitutions are rejected at canonical encoding.
#[test]
fn installation_function_integer_constant_rows_reject_every_one_field_substitution() {
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let mut plan = attached_unit_scalar_call_plan();
    // A second, call-unreferenced constant: one fresh provenance operation
    // attributed to zero bytes after the return-edge row.
    plan.functions[0]
        .provenance
        .operations
        .push(operation_id(4));
    plan.functions[0]
        .semantic_code_attribution
        .push(SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation_id(4)),
            operation_ordinal: 4,
            code_offset: 75,
            byte_count: 0,
        });
    plan.functions[0]
        .unit_integer_constants
        .push(UnitIntegerConstantRecord {
            defining_operation: operation_id(4),
            source_value: value_id(4),
            scalar_type: i32_type,
            value: IntegerValue::Signed(41),
            operation_ordinal: 4,
        });
    let artifact = build_object_artifact(&plan).expect("two-constant artifact");
    let image = emit_executable_image(&artifact, 3).expect("two-constant image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("two-constant installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic = &record.functions()[0];
    assert_eq!(authentic.unit_integer_constants.len(), 2);
    assert_eq!(
        authentic.unit_integer_constants[0],
        UnitIntegerConstantRecord {
            defining_operation: operation_id(1),
            source_value: value_id(1),
            scalar_type: i32_type,
            value: IntegerValue::Signed(-17),
            operation_ordinal: 0,
        }
    );
    assert_eq!(
        authentic.unit_integer_constants[1],
        UnitIntegerConstantRecord {
            defining_operation: operation_id(4),
            source_value: value_id(4),
            scalar_type: i32_type,
            value: IntegerValue::Signed(41),
            operation_ordinal: 4,
        }
    );

    // The unreferenced row and whole-roster substitutions remain canonical:
    // each still encodes, decodes to the same record, recomputes a distinct
    // installation identity, and is rejected by independent replay against
    // the unchanged image.
    type SlackMutation = (&'static str, Box<dyn Fn(&mut InstalledFunction)>);
    let slack_mutations: Vec<SlackMutation> = vec![
        (
            "unreferenced.defining_operation",
            Box::new(|row| {
                row.unit_integer_constants[1].defining_operation = operation_id(7);
            }),
        ),
        (
            "unreferenced.source_value",
            Box::new(|row| {
                row.unit_integer_constants[1].source_value = value_id(7);
            }),
        ),
        (
            "unreferenced.scalar_type",
            Box::new(|row| {
                row.unit_integer_constants[1].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }),
        ),
        (
            "unreferenced.value",
            Box::new(|row| {
                row.unit_integer_constants[1].value = IntegerValue::Signed(42);
            }),
        ),
        (
            "unreferenced.operation_ordinal",
            Box::new(|row| {
                row.unit_integer_constants[1].operation_ordinal = 9;
            }),
        ),
        (
            "drop_unreferenced",
            Box::new(|row| {
                row.unit_integer_constants.remove(1);
            }),
        ),
        (
            "insert_distinct",
            Box::new(|row| {
                row.unit_integer_constants.push(UnitIntegerConstantRecord {
                    defining_operation: operation_id(8),
                    source_value: value_id(8),
                    scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
                    value: IntegerValue::Signed(7),
                    operation_ordinal: 10,
                });
            }),
        ),
    ];
    for (field, mutate) in slack_mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: slack substitution encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: slack substitution decodes: {error:?}"));
        assert_eq!(replayed, changed, "{field}: codec round trip is exact");
        assert_ne!(
            installation_fingerprint(&replayed).expect("substituted fingerprint"),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // Every field of the call-referenced row is a canonical projection the
    // scalar-call argument sources rejoin: a one-field substitution is
    // rejected at canonical encoding. Non-canonical shape, admission,
    // identity-collision, and ordering substitutions on either row are
    // rejected the same way before any identity or replay could accept them.
    let call_join = InstallationError::InvalidInternalUnitScalarCall(machine_id(1));
    let roster_shape = InstallationError::InvalidUnitAffineCleanup(machine_id(1));
    type ConstantMutation = (
        &'static str,
        Box<dyn Fn(&mut InstalledFunction)>,
        InstallationError,
    );
    let mutations: Vec<ConstantMutation> = vec![
        (
            "referenced.defining_operation",
            Box::new(|row| {
                row.unit_integer_constants[0].defining_operation = operation_id(9);
            }),
            call_join.clone(),
        ),
        (
            "referenced.source_value",
            Box::new(|row| {
                row.unit_integer_constants[0].source_value = value_id(9);
            }),
            call_join.clone(),
        ),
        (
            "referenced.scalar_type::other_fixed_integer",
            Box::new(|row| {
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 64).expect("i64");
            }),
            call_join.clone(),
        ),
        (
            "referenced.value",
            Box::new(|row| {
                row.unit_integer_constants[0].value = IntegerValue::Signed(-16);
            }),
            call_join.clone(),
        ),
        (
            "referenced.operation_ordinal",
            Box::new(|row| {
                row.unit_integer_constants[0].operation_ordinal = 1;
            }),
            call_join.clone(),
        ),
        (
            "drop_referenced",
            Box::new(|row| {
                row.unit_integer_constants.remove(0);
            }),
            call_join.clone(),
        ),
        (
            "referenced.scalar_type::address_carrier",
            Box::new(|row| {
                row.unit_integer_constants[0].scalar_type =
                    IntegerType::address(64).expect("address");
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.scalar_type::wide",
            Box::new(|row| {
                row.unit_integer_constants[1].scalar_type =
                    IntegerType::new(IntegerSign::Signed, 128).expect("i128");
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.value::unadmitted",
            Box::new(|row| {
                row.unit_integer_constants[1].value = IntegerValue::Signed(1_i128 << 31);
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.value::sign_mismatch",
            Box::new(|row| {
                row.unit_integer_constants[1].value = IntegerValue::Unsigned(41);
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.operation_ordinal::not_increasing",
            Box::new(|row| {
                row.unit_integer_constants[1].operation_ordinal = 0;
            }),
            roster_shape.clone(),
        ),
        (
            "referenced.operation_ordinal::past_successor",
            Box::new(|row| {
                row.unit_integer_constants[0].operation_ordinal = 5;
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.defining_operation::collision",
            Box::new(|row| {
                row.unit_integer_constants[1].defining_operation = operation_id(1);
            }),
            roster_shape.clone(),
        ),
        (
            "unreferenced.source_value::collision",
            Box::new(|row| {
                row.unit_integer_constants[1].source_value = value_id(1);
            }),
            roster_shape.clone(),
        ),
        (
            "swap_rows",
            Box::new(|row| {
                row.unit_integer_constants.swap(0, 1);
            }),
            roster_shape.clone(),
        ),
        (
            "insert_duplicate",
            Box::new(|row| {
                let constant = row.unit_integer_constants[1];
                row.unit_integer_constants.push(constant);
            }),
            roster_shape.clone(),
        ),
    ];
    for (field, mutate, expected) in mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted row is rejected at canonical encoding"
        );
    }
}

/// One attached Unit caller holding one integer constant and two sequential
/// scalar calls into one scalar-ABI callee. The caller keeps a 16-byte frame
/// with one durable 8-byte scalar home per call result; each call owns its
/// outbound call area, the `movabs rdi, imm64` argument materialization, the
/// call instruction, the outbound release, and the result normalization plus
/// home store.
fn attached_unit_scalar_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let i32_scalar = ScalarType::Integer(i32_type);
    let i32_shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![i32_shape],
            result: Some(i32_shape),
        },
    )
    .expect("scalar call plan");
    let parameter_destination = call_plan.parameters[0].clone();
    let result_source = call_plan
        .result
        .clone()
        .expect("scalar call plan carries a result placement");

    let home = |defining_operation, source_value, byte_offset| UnitScalarHomeRecord {
        defining_operation,
        source_value,
        scalar_type: i32_scalar,
        shape: i32_shape,
        byte_offset,
    };
    let first_home = home(operation_id(2), value_id(2), 0);
    let second_home = home(operation_id(3), value_id(3), 8);
    let constant = UnitIntegerConstantRecord {
        defining_operation: operation_id(1),
        source_value: value_id(1),
        scalar_type: i32_type,
        value: IntegerValue::Signed(-17),
        operation_ordinal: 0,
    };
    let scalar_call = |owner: OperationId,
                       produced: UnitScalarHomeRecord,
                       operation_ordinal,
                       code_offset,
                       argument_offset,
                       result_offset| {
        InternalUnitScalarCallRecord {
            owner: CallSiteOwner::Operation(owner),
            target: machine_id(2),
            call_plan: call_plan.clone(),
            result: InternalUnitScalarCallResultRecord {
                home: produced,
                source: result_source.clone(),
                code_offset: result_offset,
                byte_count: 8,
            },
            arguments: vec![InternalUnitScalarCallArgumentRecord {
                parameter_index: 0,
                source: InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation: operation_id(1),
                    source_value: value_id(1),
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(-17),
                },
                destination: parameter_destination.clone(),
                code_offset: argument_offset,
                byte_count: 10,
            }],
            operation_ordinal,
            code_offset,
            byte_count: 31,
        }
    };
    let call_relocation =
        |owner: OperationId, allocation_offset, release_offset, offset| InternalCallRelocation {
            owner: CallSiteOwner::Operation(owner),
            target: machine_id(2),
            unit_stack: Some(UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset,
                    allocation_byte_count: 4,
                    release_offset,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset,
        };

    MachineCodePlan {
        psi: identity(),
        target,
        entry: machine_id(1),
        functions: vec![
            MachineCodeFunction {
                machine: machine_id(1),
                attachment: Some(StructuralTypeId::new(1).expect("attachment type")),
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1), operation_id(2), operation_id(3)],
                    edges: vec![edge_id(1)],
                },
                bytes: vec![
                    0x48, 0x83, 0xec, 0x10, // 0: sub rsp, 16 — frame allocation
                    0xb8, 0xef, 0xff, 0xff, 0xff, // 4: mov eax, -17 — constant
                    0x48, 0x83, 0xec, 0x08, // 9: sub rsp, 8 — first call area
                    0x48, 0xbf, 0xef, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, // 13: movabs rdi, -17 — argument
                    0xe8, 0, 0, 0, 0, // 23: call rel32 — relocation at 24
                    0x48, 0x83, 0xc4, 0x08, // 28: add rsp, 8 — call area release
                    0x48, 0x63, 0xc0, // 32: movsxd rax, eax — result normalize
                    0x48, 0x89, 0x44, 0x24, 0x00, // 35: mov [rsp], rax — home store
                    0x48, 0x83, 0xec, 0x08, // 40: sub rsp, 8 — second call area
                    0x48, 0xbf, 0xef, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, // 44: movabs rdi, -17 — argument
                    0xe8, 0, 0, 0, 0, // 54: call rel32 — relocation at 55
                    0x48, 0x83, 0xc4, 0x08, // 59: add rsp, 8 — call area release
                    0x48, 0x63, 0xc0, // 63: movsxd rax, eax — result normalize
                    0x48, 0x89, 0x44, 0x24, 0x08, // 66: mov [rsp+8], rax — home store
                    0x48, 0x83, 0xc4, 0x10, // 71: add rsp, 16 — frame release
                    0xc3, // 75: ret
                ],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: Some(StackAdjustmentPair {
                        byte_size: 16,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 71,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![
                    call_relocation(operation_id(2), 9, 28, 24),
                    call_relocation(operation_id(3), 40, 59, 55),
                ],
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                internal_unit_scalar_calls: vec![
                    scalar_call(operation_id(2), first_home, 1, 9, 13, 32),
                    scalar_call(operation_id(3), second_home, 2, 40, 44, 63),
                ],
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: vec![first_home, second_home],
                unit_integer_constants: vec![constant],
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    psi_edge: edge_id(1),
                    structural_types: Vec::new().into(),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 75,
                    byte_count: 1,
                }),
                unit_continuations: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
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
                        code_offset: 9,
                        byte_count: 31,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(3)),
                        operation_ordinal: 2,
                        code_offset: 40,
                        byte_count: 31,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 3,
                        code_offset: 75,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                machine: machine_id(2),
                attachment: None,
                scalar_abi: Some(ScalarFunctionAbi {
                    call_plan: call_plan.clone(),
                    parameters: vec![ScalarAbiValue {
                        value: value_id(11),
                        scalar_type: i32_scalar,
                        placement: parameter_destination.clone(),
                    }],
                    result: ScalarAbiValue {
                        value: value_id(12),
                        scalar_type: i32_scalar,
                        placement: result_source.clone(),
                    },
                }),
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(9)],
                    edges: vec![edge_id(9)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: Some(ScalarStackEvidence {
                    mutations: Vec::new(),
                    control_flow: ScalarControlFlowEvidence::Linear,
                    stack_alignment: 16,
                    cleanup_preservation: None,
                }),
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
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
                unit_affine_cleanup: None,
                unit_continuations: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("value")
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
