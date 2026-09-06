//! Exact ABI fragment widths determine the bytes stored in a result home.

use super::*;

#[test]
fn projected_result_home_retains_narrow_direct_abi_fragments() {
    for count in 9_u16..=16 {
        check(count, false);
        if matches!(count - 8, 1 | 2 | 4 | 8) {
            check(count, true);
        }
    }
}

fn check(count: u16, full: bool) {
    let remaining_calls = if full {
        (0..count - 1)
            .rev()
            .map(|index| format!("take(result[{index}]);"))
            .collect::<String>()
    } else {
        String::new()
    };
    let source = format!(
        "data Token {{ value: u8; }}
            machine forward(value: [Token; {count}]) -> [Token; {count}] {{ value }}
            machine take(value: Token) {{}}
            machine enter(value: [Token; {count}]) {{
                let result: [Token; {count}] = forward(value);
                take(result[{}]); {remaining_calls}
            }}",
        count - 1
    );
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "enter").unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let entry = terminal.semantic_module.entry;
    drop(terminal);
    drop(checked);
    for case in target_cases() {
        if case.policy == CallingPolicy::MicrosoftX64 || !matches!(count - 8, 1 | 2 | 4 | 8) {
            assert!(lower_to_target_operations(&plan, case.target).is_err());
            continue;
        }
        let target = lower_to_target_operations(&plan, case.target).unwrap();
        let assigned = assign_registers(&target).unwrap();
        let assigned_index = assigned
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        for offset in [u32::from(count), 24] {
            if offset == 16 {
                continue;
            }
            let mut changed = assigned.clone();
            let AssignedOperation::UnitBody(body) =
                &mut changed.functions[assigned_index].operation
            else {
                panic!("Unit caller");
            };
            let assigned_target_operations::AssignedUnitOperation::StructuralResultCall {
                result_home: Some(home),
                ..
            } = &mut body.operations[0]
            else {
                panic!("stored result");
            };
            home.byte_offset = offset;
            assert!(
                emit_machine_code(&changed).is_err(),
                "noncanonical home {offset}"
            );
        }
        let emitted = emit_machine_code(&assigned).unwrap();
        let caller_index = emitted
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        let function = &emitted.functions[caller_index];
        let producer = &function.internal_unit_calls[0];
        let result = producer.structural_result.as_ref().unwrap();
        let home = result.result_home.as_ref().unwrap();
        assert_eq!(
            home.requirement.layout.shape(),
            ValueShape::integer(count, 1)
        );
        assert_eq!(home.home_byte_offset, 16);
        assert_eq!(
            home.code_offset + home.byte_count,
            producer.code_offset + producer.byte_count
        );
        let widths = result
            .caller_result_placement
            .locations
            .iter()
            .map(|location| {
                let ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                } = location
                else {
                    panic!("direct result");
                };
                (*value_byte_offset, *byte_size)
            })
            .collect::<Vec<_>>();
        assert_eq!(widths, [(0, 8), (8, count - 8)]);
        let cleanup = function.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(
            cleanup.actions.len(),
            if full { 0 } else { usize::from(count - 1) }
        );
        for (action, index) in cleanup.actions.iter().zip((0..count - 1).rev()) {
            let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                panic!("exact residual index");
            };
            assert_eq!(discard.place, result.operation_result.place);
            assert_eq!(
                discard.path,
                [terminal_psi::StructuralPathSegment::FixedIndex(u64::from(
                    index
                ))]
            );
        }
        let consumer = &function.internal_unit_calls[1];
        assert_eq!(
            consumer.arguments[0].source_location.stack_byte_offset(),
            Some(16)
        );
        assert_eq!(
            consumer.arguments[0].source_byte_offset,
            u32::from(count - 1)
        );
        for mutation in 0..3 {
            let mut changed = emitted.clone();
            let function = &mut changed.functions[caller_index];
            let result = function.internal_unit_calls[0]
                .structural_result
                .as_mut()
                .unwrap();
            match mutation {
                0 => result.result_home.as_mut().unwrap().home_byte_offset += 8,
                1 => {
                    let home = result.result_home.as_mut().unwrap();
                    home.bytes[0] ^= 1;
                    function.bytes[home.code_offset] ^= 1;
                }
                2 => {
                    for placement in [
                        &mut result.caller_result_placement,
                        &mut result.callee_result_placement,
                    ] {
                        let ValueLocation::Register { byte_size, .. } = &mut placement.locations[1]
                        else {
                            unreachable!();
                        };
                        *byte_size = if count == 16 { 4 } else { 8 };
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                build_object_artifact(&changed).is_err(),
                "fragment mutation {mutation}"
            );
        }
        let object = build_object_artifact(&emitted).unwrap();
        assert_eq!(emit_object_container(&object).psi, plan.psi);
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        assert_eq!(
            decode_installation_record(&encode_installation_record(&installation).unwrap())
                .unwrap(),
            installation
        );
        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            super::affine_call_result_host::execute_byte_array(
                &image,
                object.entry_function().text_offset,
                count,
            );
        }
    }
}
