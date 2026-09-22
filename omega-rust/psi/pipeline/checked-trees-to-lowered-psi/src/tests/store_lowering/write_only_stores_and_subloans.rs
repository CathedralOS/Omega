use crate::TerminalMachineSelection;
use crate::lower_machine;
use crate::terminal_identities::service_id;
use crate::tests::checked_write_line_literal;
use crate::unit::attached_unit::lower_root_service_reach;
use semantic_vocabulary::{IntegerValue, ScalarType, StructuralPlaceKind, ValueId};
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeShape, TerminalModule, Terminator,
};

#[test]
fn lowers_exact_raw_bytes_into_borrowed_boundary_argument() {
    let checked = checked_write_line_literal();
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower write_line literal");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine")
    };
    let literal_place = machine
        .structural_places
        .iter()
        .find_map(|place| {
            matches!(
                place.kind,
                StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    ..
                }
            )
            .then_some(place.id)
        })
        .expect("canonical byte-sequence literal place");
    let [establish, call] = machine.blocks[0].operations.as_slice() else {
        panic!("literal establishment then boundary call")
    };
    assert!(matches!(
        &establish.kind,
        OperationKind::EstablishByteSequenceLiteral { destination, bytes }
            if *destination == literal_place && bytes == &[0x80, b'A']
    ));
    assert!(matches!(
        &call.kind,
        OperationKind::BoundaryCall { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.place == literal_place && argument.path.is_empty())
    ));
    let literal_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            )
        })
        .expect("borrowed-view declaration");
    assert!(machine.structural_places.iter().any(|place| matches!(
        place.kind,
        StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. }
            if structural_type == literal_type.id
    )));
}

#[test]
fn affine_i64_record_literal_crosses_source_codec_and_verification() {
    let checked = crate::front_end::checked_program(
        r#"
        data Packet { value: i64; }

        data Sink {}
        machine Sink::accept(packet: Packet) {}

        data Root {}
        machine Root::enter() {
            let packet: Packet = Packet { value: 7 };
            Sink::accept(move packet);
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower affine scalar record");
    let module = &lowered.semantic_module;
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("caller entry machine");
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let establishments = operations
        .iter()
        .copied()
        .filter(|operation| matches!(operation.kind, OperationKind::EstablishRecord { .. }))
        .collect::<Vec<_>>();
    let [establish] = establishments.as_slice() else {
        panic!("one record establishment: {operations:#?}");
    };
    let OperationKind::EstablishRecord { fields } = &establish.kind else {
        unreachable!();
    };
    let [field] = fields.as_slice() else {
        panic!("one scalar field");
    };
    // Scalar evaluation may forward its result through private block parameters.
    // Follow every incoming binding to the actual field-value definition.
    let terminal_psi::RecordFieldValue::Scalar { value, .. } = field.value else {
        panic!("scalar field")
    };
    let mut pending = vec![value];
    let mut visited = Vec::new();
    let mut constants = Vec::new();
    while let Some(value) = pending.pop() {
        if visited.contains(&value) {
            continue;
        }
        visited.push(value);
        if let Some(operation) = operations.iter().copied().find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == value)
        }) {
            assert!(
                matches!(
                    operation.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7)
                    }
                ),
                "exact field payload: {operation:#?}"
            );
            constants.push(operation.id);
            continue;
        }
        let (block, position) = caller
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .parameters
                    .iter()
                    .position(|parameter| parameter.id == value)
                    .map(|position| (block.id, position))
            })
            .unwrap_or_else(|| panic!("field value {value:?} has no definition: {caller:#?}"));
        let mut incoming = 0;
        for predecessor in &caller.blocks {
            match &predecessor.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } if *target == block => {
                    pending.push(arguments[position]);
                    incoming += 1;
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        if successor.target == block {
                            pending.push(successor.arguments[position]);
                            incoming += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        assert!(incoming > 0, "field value must have an incoming producer");
    }
    let constant_identity = *constants
        .first()
        .expect("field traces to an exact constant");
    let calls = operations
        .iter()
        .copied()
        .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one owned consumer call");
    };
    let result = establish
        .result
        .structural()
        .expect("constructor establishes one structural result");
    assert_eq!(result.multiplicity, StructuralMultiplicity::Affine);
    assert!(result.qualifications.is_empty());
    assert!(result.projected_qualifications.is_empty());
    assert!(result.claims.is_empty());
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.place == result.place
                    && argument.path.is_empty()
                    && argument.access == StructuralAccess::Owned)
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode affine scalar record");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode affine scalar record");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify affine scalar record");

    let mut forged = decoded;
    let constant = forged
        .machines
        .iter_mut()
        .find(|machine| machine.id == forged.entry)
        .expect("same entry machine")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == constant_identity)
        .expect("same constant producer");
    let OperationKind::IntegerConstant { value } = &mut constant.kind else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(7);
    assert!(terminal_verifier::validate_module(&forged).is_err());
}

#[test]
fn mutable_to_write_only_access_crosses_source_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write [u8]) {}

        data Root {}
        machine Root::enter(bytes: &mut [u8]) {
            Sink::fill(&write bytes);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower write-only forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("root emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow)
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode access-bearing module");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode access-bearing module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify write-only attenuation");
}

#[test]
fn direct_write_only_primitive_store_crosses_source_codec_and_verification() {
    let checked = crate::front_end::checked_program(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32) {
                destination = 2;
            }

            data Root {}
            machine Root::enter(destination: &mut i32) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower primitive store closure");
    let module = &lowered.semantic_module;
    let [caller, callee] = module.machines.as_slice() else {
        panic!("caller and write-only callee are retained")
    };
    let [caller_parameter] = caller.structural_parameters.as_slice() else {
        panic!("caller retains one primitive referent")
    };
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        panic!("callee retains one primitive referent")
    };
    assert_eq!(caller_parameter.access, StructuralAccess::MutableBorrow);
    assert_eq!(callee_parameter.access, StructuralAccess::WriteOnlyBorrow);
    assert!(matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_parameter.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(_)))
    ));

    let [constant, store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits one constant followed by one store")
    };
    let stored_value = constant.result.expect_scalar().id;
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value, .. }
            if destination == callee_parameter.place && value == stored_value
    ));
    assert_eq!(store.result, OperationResult::Unit);

    let encoded = terminal_codec::encode_module(module).expect("encode primitive store");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode primitive store");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify primitive store");

    let mut widened = decoded.clone();
    widened.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    terminal_verifier::validate_module(&widened)
        .expect_err("a primitive store requires exact write-only access");

    let mut undefined = decoded;
    let OperationKind::WriteOnlyPrimitiveStore { value, .. } =
        &mut undefined.machines[1].blocks[0].operations[1].kind
    else {
        panic!("store operation")
    };
    *value = ValueId::new(u64::MAX).expect("nonzero undefined value");
    terminal_verifier::validate_module(&undefined)
        .expect_err("a primitive store value must be defined and dominating");
}

#[test]
fn direct_write_only_boolean_store_crosses_source_codec_and_verification() {
    let checked = crate::front_end::checked_program(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool) {
                destination = true;
            }

            data Root {}
            machine Root::enter(destination: &mut bool) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower Boolean store closure");
    let module = &lowered.semantic_module;
    let [_, callee] = module.machines.as_slice() else {
        panic!("caller and write-only callee are retained")
    };
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        panic!("callee retains one primitive referent")
    };
    assert!(matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_parameter.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean))
    ));
    let [constant, store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits one Boolean constant followed by one store")
    };
    let stored_value = constant.result.expect_scalar().id;
    assert!(matches!(
        constant.kind,
        OperationKind::BooleanConstant { value: true }
    ));
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value, .. }
            if destination == callee_parameter.place && value == stored_value
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode Boolean store");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode Boolean store");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify Boolean store");

    let mut wrong_type = decoded;
    let declaration = wrong_type
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == callee_parameter.structural_type)
        .expect("Boolean structural declaration");
    declaration.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap(),
    ));
    terminal_verifier::validate_module(&wrong_type)
        .expect_err("Boolean store value must match its exact referent type");
}

#[test]
fn primitive_literal_store_retains_unused_scalar_parameters_and_signed_literal() {
    for literal in ["17", "-17"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Sink {{}} machine Sink::fill(destination: &write i8, unused: i8) {{ destination = {literal}; }}"
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Sink::fill"))
            .expect("literal store with unused scalar input");
        let module = &lowered.semantic_module;
        let [machine] = module.machines.as_slice() else {
            panic!("one source machine");
        };
        assert_eq!(
            machine.parameters.len(),
            1,
            "unused scalar ABI input is retained"
        );
        let [constant, store] = machine.blocks[0].operations.as_slice() else {
            panic!("constant then store");
        };
        if literal == "-17" {
            assert!(matches!(
                constant.kind,
                OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Signed(-17),
                }
            ));
        }
        assert!(
            matches!(store.kind, OperationKind::WriteOnlyPrimitiveStore { destination, value, .. }
            if destination == machine.structural_parameters[0].place
                && value == constant.result.expect_scalar().id)
        );
        let encoded =
            terminal_codec::encode_module(module).expect("encode primitive literal store");
        let decoded =
            terminal_codec::decode_module(&encoded).expect("decode primitive literal store");
        assert_eq!(&decoded, module);
        terminal_verifier::validate_module(&decoded).expect("verify exact primitive literal store");
    }
}

#[test]
fn direct_write_only_ieee_float_store_crosses_source_codec_and_verification() {
    let checked = crate::front_end::checked_program(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write f32) {
                destination = 1.25f32;
            }

            data Root {}
            machine Root::enter(destination: &mut f32) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower IEEE float store closure");
    let module = &lowered.semantic_module;
    let [_, callee] = module.machines.as_slice() else {
        panic!("caller and write-only callee are retained")
    };
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        panic!("callee retains one primitive referent")
    };
    assert!(matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_parameter.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32
        )))
    ));
    let [constant, store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits one IEEE float constant followed by one store")
    };
    let stored_value = constant.result.expect_scalar().id;
    assert!(matches!(
        constant.kind,
        OperationKind::IeeeFloatConstant {
            value: semantic_vocabulary::IeeeFloatValue::Binary32(0x3fa0_0000)
        }
    ));
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value, .. }
            if destination == callee_parameter.place && value == stored_value
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode IEEE float store");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode IEEE float store");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify IEEE float store");
}

#[test]
fn direct_write_only_fixed_integer_parameter_store_crosses_source_codec_and_verification() {
    let checked = crate::front_end::checked_program(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement;
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Sink::fill"))
        .expect("lower fixed-integer parameter store closure");
    let module = &lowered.semantic_module;
    let [callee] = module.machines.as_slice() else {
        panic!("one write-only callee is retained")
    };
    let [replacement] = callee.parameters.as_slice() else {
        panic!("callee retains one fixed-integer scalar parameter")
    };
    assert!(
        matches!(replacement.scalar_type, ScalarType::Integer(integer)
        if integer == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32).unwrap())
    );
    let [store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits only the parameter-sourced store")
    };
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value, .. }
            if destination == callee.structural_parameters[0].place
                && value == replacement.id
    ));

    let encoded =
        terminal_codec::encode_module(module).expect("encode fixed-integer parameter store");
    let decoded =
        terminal_codec::decode_module(&encoded).expect("decode fixed-integer parameter store");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify fixed-integer parameter store");

    let mut undefined = decoded;
    let OperationKind::WriteOnlyPrimitiveStore { value, .. } =
        &mut undefined.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = ValueId::new(u64::MAX).expect("nonzero undefined value");
    terminal_verifier::validate_module(&undefined)
        .expect_err("runtime store source must be the exact declared scalar parameter");
}

#[test]
fn write_only_common_field_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Leaf [copy] { value: u16; }
        data Inner [copy] { leaf: Leaf; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write Leaf) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.leaf);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower projected forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("projected caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path.len() == 2
                    && argument.path.iter().all(|segment| matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                    )))
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode projected module");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode projected module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify projected write-only subloan");

    let mut path_drifted = decoded.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut path_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].path[1] = structural_arguments[0].path[0].clone();
    terminal_verifier::validate_module(&path_drifted)
        .expect_err("a redirected common-field identity must reject");

    let mut target_type_drifted = decoded.clone();
    target_type_drifted.machines[1].structural_parameters[0].structural_type =
        target_type_drifted.machines[0].structural_parameters[0].structural_type;
    terminal_verifier::validate_module(&target_type_drifted)
        .expect_err("the projected leaf must match the callee's exact structural type");

    let mut target_access_drifted = decoded.clone();
    target_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::MutableBorrow;
    terminal_verifier::validate_module(&target_access_drifted)
        .expect_err("the projected leaf must match the callee's exact access");

    let mut access_drifted = decoded;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut access_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    terminal_verifier::validate_module(&access_drifted)
        .expect_err("a projected write-only argument cannot widen to shared access");
}

#[test]
fn direct_root_literal_indexed_write_only_subloan_crosses_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [u16; 2]) {
            Sink::fill(&write values[1]);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower direct indexed forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("direct indexed caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path == [StructuralPathSegment::FixedIndex(1)])
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode indexed module");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode indexed module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify direct indexed write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("direct indexed caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    let mut out_of_bounds = decoded.clone();
    mutate_path(&mut out_of_bounds, &|path| {
        path[0] = StructuralPathSegment::FixedIndex(2);
    });
    terminal_verifier::validate_module(&out_of_bounds)
        .expect_err("an out-of-bounds direct subloan index must reject");

    let mut missing_index = decoded.clone();
    mutate_path(&mut missing_index, &|path| path.clear());
    terminal_verifier::validate_module(&missing_index)
        .expect_err("omitting the direct subloan coordinate must reject");

    let mut duplicated_index = decoded.clone();
    mutate_path(&mut duplicated_index, &|path| {
        path.push(StructuralPathSegment::FixedIndex(0));
    });
    terminal_verifier::validate_module(&duplicated_index)
        .expect_err("duplicating the direct subloan coordinate must reject");

    let mut source_access_drifted = decoded.clone();
    source_access_drifted.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    terminal_verifier::validate_module(&source_access_drifted)
        .expect_err("a direct indexed subloan requires an exact write-only source");

    let mut target_access_drifted = decoded.clone();
    target_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::MutableBorrow;
    terminal_verifier::validate_module(&target_access_drifted)
        .expect_err("a direct indexed subloan cannot widen its target access");

    let mut target_type_drifted = decoded.clone();
    target_type_drifted.machines[1].structural_parameters[0].structural_type =
        target_type_drifted.machines[0].structural_parameters[0].structural_type;
    terminal_verifier::validate_module(&target_type_drifted)
        .expect_err("a direct indexed subloan must retain the exact element type");

    let mut target_multiplicity_drifted = decoded;
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("a direct indexed subloan cannot become linear");
}

#[test]
fn finite_literal_index_suffix_crosses_source_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]) {
            Sink::fill(&write values[1][2][3][4][5][6]);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower finite literal-index forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("finite literal-index caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path == [
                        StructuralPathSegment::FixedIndex(1),
                        StructuralPathSegment::FixedIndex(2),
                        StructuralPathSegment::FixedIndex(3),
                        StructuralPathSegment::FixedIndex(4),
                        StructuralPathSegment::FixedIndex(5),
                        StructuralPathSegment::FixedIndex(6),
                    ])
    ));

    let encoded =
        terminal_codec::encode_module(module).expect("encode finite literal-index module");
    let decoded =
        terminal_codec::decode_module(&encoded).expect("decode finite literal-index module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded)
        .expect("verify finite literal-index write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("finite literal-index caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    for (position, invalid_index) in [(0, 2), (1, 3), (2, 4), (3, 5), (4, 6), (5, 7)] {
        let mut out_of_bounds = decoded.clone();
        mutate_path(&mut out_of_bounds, &|path| {
            path[position] = StructuralPathSegment::FixedIndex(invalid_index);
        });
        terminal_verifier::validate_module(&out_of_bounds)
            .expect_err("every literal-index array bound must replay independently");
    }

    let mut missing_final_index = decoded.clone();
    mutate_path(&mut missing_final_index, &|path| {
        path.pop();
    });
    terminal_verifier::validate_module(&missing_final_index)
        .expect_err("omitting the final coordinate must reject exact target rejoin");

    let mut index_beyond_leaf = decoded.clone();
    mutate_path(&mut index_beyond_leaf, &|path| {
        path.push(StructuralPathSegment::FixedIndex(0));
    });
    terminal_verifier::validate_module(&index_beyond_leaf)
        .expect_err("an index beyond the selected primitive leaf must reject");

    let mut source_access_drifted = decoded.clone();
    source_access_drifted.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    terminal_verifier::validate_module(&source_access_drifted)
        .expect_err("a literal-index subloan requires exact write-only source access");

    let mut shared_access_drifted = decoded.clone();
    shared_access_drifted.machines[0].structural_parameters[0].access =
        StructuralAccess::SharedBorrow;
    shared_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::SharedBorrow;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut shared_access_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("finite literal-index shared-access mutation call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    // A shared subloan of the same static literal-index path from a shared
    // root is an ordinary unrestricted reborrow (the verifier's
    // `is_unrestricted_shared_subloan`), so re-accessing every side as shared
    // is a different valid module rather than drift; only the asymmetric
    // drifts above and below reject.
    terminal_verifier::validate_module(&shared_access_drifted)
        .expect("a consistently shared literal-index subloan verifies");

    let mut target_multiplicity_drifted = decoded;
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("a literal-index subloan cannot become linear");
}

#[test]
fn field_prefixed_finite_literal_index_suffix_crosses_terminal() {
    let source = r#"
        data Outer [copy] { values: [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2][3][4][5][6]);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower field-prefixed finite literal-index forwarding");
    let module = &lowered.semantic_module;

    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("field-prefixed finite literal-index forwarding call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [
            StructuralPathSegment::Field(_),
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::FixedIndex(2),
            StructuralPathSegment::FixedIndex(3),
            StructuralPathSegment::FixedIndex(4),
            StructuralPathSegment::FixedIndex(5),
            StructuralPathSegment::FixedIndex(6),
        ]
    ));
    let encoded = terminal_codec::encode_module(module).expect("encode field-prefixed module");
    let mut decoded =
        terminal_codec::decode_module(&encoded).expect("decode field-prefixed module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded)
        .expect("verify field-prefixed finite literal-index write-only subloan");

    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut decoded.machines[0].blocks[0].operations[0].kind
    else {
        panic!("field-prefixed finite literal-index decoded call")
    };
    structural_arguments[0].path.swap(0, 1);
    terminal_verifier::validate_module(&decoded)
        .expect_err("reordering a field and fixed-index segment must reject");
}

#[test]
fn literal_indexed_write_only_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Inner [copy] { values: [u16; 2]; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.values[1]);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower literal-indexed forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("literal-indexed caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && matches!(argument.path.as_slice(), [
                        StructuralPathSegment::Field(_),
                        StructuralPathSegment::Field(_),
                        StructuralPathSegment::FixedIndex(1),
                    ]))
    ));

    let encoded = terminal_codec::encode_module(module).expect("encode indexed module");
    let decoded = terminal_codec::decode_module(&encoded).expect("decode indexed module");
    assert_eq!(&decoded, module);
    terminal_verifier::validate_module(&decoded).expect("verify indexed write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("literal-indexed caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    let mut out_of_bounds = decoded.clone();
    mutate_path(&mut out_of_bounds, &|path| {
        *path.last_mut().expect("index") = StructuralPathSegment::FixedIndex(2);
    });
    terminal_verifier::validate_module(&out_of_bounds)
        .expect_err("an out-of-bounds literal subloan index must reject");

    let mut missing_index = decoded.clone();
    mutate_path(&mut missing_index, &|path| {
        path.pop();
    });
    terminal_verifier::validate_module(&missing_index)
        .expect_err("omitting the indexed subloan coordinate must reject");

    let mut duplicated_index = decoded.clone();
    mutate_path(&mut duplicated_index, &|path| {
        path.push(StructuralPathSegment::FixedIndex(1));
    });
    terminal_verifier::validate_module(&duplicated_index)
        .expect_err("duplicating the indexed subloan coordinate must reject");

    let mut reordered_index = decoded.clone();
    mutate_path(&mut reordered_index, &|path| {
        path.rotate_right(1);
    });
    terminal_verifier::validate_module(&reordered_index)
        .expect_err("moving the index before its field path must reject");

    // Field-prefixed `Owned` sources are a valid subloan authority now, so
    // drifting the source access to `Owned` no longer describes a real fault.
    // Retarget the callee's expected leaf to a record declaration instead:
    // the supplied `u16` leaf then mismatches the referent the subloan names.
    let mut leaf_type_drifted = decoded.clone();
    let record_type = leaf_type_drifted
        .structural_types
        .iter()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::Record { .. }))
        .expect("a record type declaration")
        .id;
    leaf_type_drifted.machines[1].structural_parameters[0].structural_type = record_type;
    terminal_verifier::validate_module(&leaf_type_drifted)
        .expect_err("an indexed subloan's leaf type must match the callee referent");

    let mut target_multiplicity_drifted = decoded.clone();
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("an indexed subloan cannot become a linear projected call");

    let mut field_drifted = decoded;
    mutate_path(&mut field_drifted, &|path| {
        path[1] = path[0].clone();
    });
    terminal_verifier::validate_module(&field_drifted)
        .expect_err("a redirected indexed-subloan field identity must reject");
}

#[test]
fn rejects_tampered_owned_carrier_for_source_literal() {
    let mut checked = checked_write_line_literal();
    let literal_type = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter_mut()
        .find(|plan| {
            matches!(
                plan.shape,
                checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(_)
            )
        })
        .expect("literal type");
    literal_type.shape = checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 2 },
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect_err("an owned carrier must not establish a borrowed source literal");
    assert!(
        error.to_string().contains("requires a borrowed-view type"),
        "{error}"
    );
}

#[test]
fn bounded_installation_reach_lowers_source_free_terminal_dependency() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete()
            reaches <= MachineControl + PortIo;
        }

        machine pic_complete()
        satisfies InterruptCompletion::complete
        reaches PortIo
        { }

        machine invoke<machine Completion>()
        where machine Completion satisfies InterruptCompletion::complete;
        { Completion(); }

    "#;
    let checked = crate::front_end::checked_program(source);
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("generic invocation machine");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, root.symbol, &service_ids)
        .expect("lower root service reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("terminal root must retain one installation reach dependency");
    };
    assert!(
        dependency
            .requirement_identity
            .contains("InterruptCompletion::complete")
    );
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}
