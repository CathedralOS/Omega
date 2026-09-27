use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::{CheckedTrees, ExpressionHandle};
use typed_trees::data::DataMember;
use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

fn source(declaration: &str, target: &str, callee_blocks: bool) -> String {
    let acknowledgement = if callee_blocks { "block " } else { "" };
    format!(
        "pub data Decoy {{ case Eof; case Byte(value: i32 [0..=255]); }}
        pub data ByteRead {{ case Eof; case Byte(value: i32 [0..=255]); }}
        pub boundary trait Console {{ machine read_byte() -> ByteRead reaches Console blocks; crashes Trap; }}
        pub data ConsoleNativeProvider {{}}
        pub data OtherProvider {{}}
        {declaration}
        machine decoy() -> Decoy {{ Decoy::Eof }}
        machine wide(value: i64) {{}}
        machine main() reaches Console {{
            let observed: ByteRead = {acknowledgement}{target}();
            transition observed {{
                ByteRead::Eof -> done()
                ByteRead::Byte {{ value }} -> consumed(value)
            }}
            state done() {{}}
            state consumed(value: i32) {{}}
        }}"
    )
}

fn canonical() -> CheckedTrees {
    crate::front_end::checked_program(&source(
        "machine ConsoleNativeProvider::read_byte() -> ByteRead
            satisfies Console::read_byte via ForeignBinding::CompilerIntrinsic
            crashes Trap blocks;",
        "ConsoleNativeProvider::read_byte",
        true,
    ))
}

#[test]
fn byte_input_spelling_does_not_reclassify_a_scalar_to_unit_intrinsic() {
    let program = crate::front_end::checked_program(
        "pub boundary trait Console { machine read_byte(value: i32) reaches Console; }
        pub data ConsoleNativeProvider {}
        machine ConsoleNativeProvider::read_byte(value: i32)
            satisfies Console::read_byte via ForeignBinding::CompilerIntrinsic;
        machine main() reaches Console { ConsoleNativeProvider::read_byte(7); }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::read_byte")
        .unwrap();
    let mut evaluator = Evaluator::new_checked(&program, &[255]);
    assert_eq!(
        evaluator.exact_console_intrinsic_host_method(program.machine_states(machine)[0].symbol),
        None
    );
    let _ = evaluator.run_entry("main");
    assert_eq!(evaluator.stdin_cursor, 0);
    assert!(!evaluator.host_boundary_touched);
    assert!(!evaluator.non_fs_host_boundary_touched);
}

#[test]
fn byte_input_rejects_changed_result_schema_before_consuming_input() {
    for mutation in 0..8 {
        let mut program = canonical();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::read_byte")
            .unwrap()
            .clone();
        let state = program.machine_states(&machine)[0].clone();
        let result_symbol = program.type_reference_symbol(state.return_type);
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == result_symbol)
            .unwrap()
            .clone();
        let DataMember::Variant(byte) = &program.data_members(&data)[1] else {
            panic!("canonical Byte payload");
        };
        let payload = byte.payload;
        let field_type = program.data_payload_fields(byte)[0].type_reference;
        match mutation {
            0 => program
                .typed
                .data_members
                .span_mut_or_empty(data.members)
                .swap(0, 1),
            1 => {
                let members = program.typed.data_members.span_mut_or_empty(data.members);
                members[1] = members[0].clone();
            }
            2 => {
                program.typed.data_payload_fields.span_mut_or_empty(payload)[0].relevance =
                    language_core::BindingRelevance::Erased;
            }
            3 => {
                let wide = program
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "wide")
                    .unwrap();
                let wide_type =
                    program.state_parameters(&program.machine_states(wide)[0])[0].type_reference;
                program.typed.data_payload_fields.span_mut_or_empty(payload)[0].type_reference =
                    wide_type;
            }
            7 => {
                let DataMember::Variant(byte) =
                    &mut program.typed.data_members.span_mut_or_empty(data.members)[1]
                else {
                    panic!("canonical Byte variant");
                };
                byte.payload = arena::HandleSpan::empty();
            }
            4..=6 => {
                let TypeReferenceNode::Constrained { constraints, .. } =
                    program.type_reference_table.type_reference(field_type)
                else {
                    panic!("canonical byte range");
                };
                let constraints = *constraints;
                let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = &mut program
                    .typed
                    .type_reference_table
                    .constraints_mut(constraints)[0]
                else {
                    panic!("canonical byte interval");
                };
                match mutation {
                    4 => *maximum = *minimum,
                    5 => *minimum = *maximum,
                    _ => *maximum = ExpressionHandle::invalid(),
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validation::exact_byte_read_result_type(&program, state.return_type),
            None,
            "schema mutation {mutation}"
        );
        let mut evaluator = Evaluator::new_checked(&program, &[255]);
        assert!(
            evaluator.read_stdin_byte_value(state.symbol).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(evaluator.stdin_cursor, 0, "mutation {mutation}");
    }
}

#[test]
fn concrete_byte_input_lookalikes_do_not_gain_host_authority() {
    for (label, declaration, target, callee_blocks) in [
        (
            "other provider",
            "machine OtherProvider::read_byte() -> ByteRead satisfies Console::read_byte via ForeignBinding::CompilerIntrinsic crashes Trap blocks;",
            "OtherProvider::read_byte",
            true,
        ),
        (
            "foreign binding",
            "machine ConsoleNativeProvider::read_byte() -> ByteRead satisfies Console::read_byte via ForeignBinding::Syscall(60) crashes Trap blocks;",
            "ConsoleNativeProvider::read_byte",
            true,
        ),
        (
            "authored body",
            "machine ConsoleNativeProvider::read_byte() -> ByteRead satisfies Console::read_byte { ByteRead::Eof }",
            "ConsoleNativeProvider::read_byte",
            false,
        ),
    ] {
        let program =
            crate::front_end::checked_program(&source(declaration, target, callee_blocks));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == target)
            .unwrap();
        let target_symbol = program.machine_states(machine)[0].symbol;
        let mut evaluator = Evaluator::new_checked(&program, &[37]);
        assert_eq!(
            evaluator.exact_console_intrinsic_host_method(target_symbol),
            None,
            "{label}"
        );
        let outcome = evaluator.run_entry("main");
        if label == "authored body" {
            assert!(outcome.is_ok(), "ordinary body must execute");
        }
        assert_eq!(evaluator.stdin_cursor, 0, "{label}");
        assert!(!evaluator.host_boundary_touched, "{label}");
        assert!(!evaluator.non_fs_host_boundary_touched, "{label}");
    }
}
