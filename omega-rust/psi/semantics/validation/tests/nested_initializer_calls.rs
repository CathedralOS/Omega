use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use validation::validate_program;

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn free_scalar_initializers_admit_nested_call_evaluation() {
    for mutability in ["", "mut "] {
        let source = format!(
            "machine identity(input: bool) -> bool {{ input }}
             machine value(input: bool) -> bool {{
                 let {mutability}saved: bool = identity(identity(input));
                 saved
             }}"
        );
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{mutability}: {diagnostics:?}");
    }
}

#[test]
fn mixed_boundary_return_initializers_admit_nested_calls_with_immutable_scalar_formals() {
    for boundary_crash in ["", "crashes Abort"] {
        let source = format!(
            r#"
            machine abort() -> u16 crashes Abort {{ crash Abort; }}
            boundary trait PortIo {{}}
            pub data Receipt [linear] {{ value: u64; }}
            boundary machine Receipt::settle(self, first: u16, second: u16) -> u16
            reaches PortIo {boundary_crash} ensures true;
            data Wrapper {{}}
            machine Wrapper::measure(receipt: Receipt, value: u16) -> u16
            reaches PortIo crashes Abort
            {{ let accepted: u16 = receipt.settle(abort(), value); accepted }}
        "#
        );
        let messages = diagnostics(&source);
        assert!(messages.is_empty(), "{boundary_crash}: {messages:?}");
        let messages = diagnostics(&source.replace(
            "measure(receipt: Receipt, value: u16)",
            "measure(receipt: Receipt, mut value: u16)",
        ));
        assert!(
            messages
                .iter()
                .any(|message| message
                    .contains("value-call argument cannot itself be a machine call")),
            "mutable scalar formal remains outside this source family: {messages:?}"
        );
    }
}

#[test]
fn free_scalar_local_assignments_admit_nested_call_evaluation() {
    let diagnostics = diagnostics(
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: bool = input;
             saved = identity(identity(input));
             saved
         }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn unit_scalar_store_assignments_admit_nested_call_evaluation() {
    // The unit statement sequence owns these assignments: the nested operand is
    // a checked `AssignmentValue` computation argument evaluated exactly once
    // before the outer call and the store.
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let mut saved: bool = input;
             saved = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(saved: &mut bool, input: bool) {
             saved = identity(identity(input));
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, input: bool) {
             let mut saved: bool = input;
             saved = identity(identity(input));
             self.flag = saved;
         }",
        "machine identity(input: bool) -> bool { input }
         machine choose(left: bool, right: bool) -> bool { left }
         machine value(input: bool) {
             let mut saved: bool = input;
             saved = choose(identity(input), identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn unserved_assignment_destinations_keep_nested_call_realization_fence() {
    for source in [
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, input: bool) -> bool {
             self.flag = identity(identity(input));
             self.flag
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: [bool; 1] = [input];
             saved[0] = identity(identity(input));
             input
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: Container = Container { flag: input };
             saved.flag = identity(identity(input));
             input
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &mut Container, input: bool) {
             param.flag = identity(identity(input));
         }",
        "machine identity(input: i32) -> i32 { input }
         machine value(input: i32) {
             let mut saved: i32 in Wrapping = 0;
             saved = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let saved: bool = input;
             saved = identity(identity(input));
             saved
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(
            diagnostics
                .iter()
                .any(|message| message
                    .contains("value-call argument cannot itself be a machine call")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn nested_call_assignments_with_unrealized_calls_keep_the_fence() {
    for source in [
        // A boundary requirement call has no checked-body callee for the
        // assignment's scalar computation.
        "boundary trait Host { machine read(input: bool) -> bool reaches Host; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) reaches Host {
             let mut saved: bool = input;
             saved = Host::read(identity(input));
         }",
        // A receiver call keeps the initializer-only receiver admission.
        "data Scalar {}
         machine identity(input: bool) -> bool { input }
         machine Scalar::read(input: bool) -> bool { input }
         machine value(scalar: Scalar, input: bool) {
             let mut saved: bool = input;
             saved = scalar.read(identity(input));
         }",
        // A callee whose result is structural has no primitive store value.
        "data Container { flag: bool; }
         machine rebuild(flag: bool) -> Container { Container { flag: flag } }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let mut saved: Container = Container { flag: input };
             saved = rebuild(identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(
            diagnostics
                .iter()
                .any(|message| message
                    .contains("value-call argument cannot itself be a machine call")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn immutable_unit_result_initializers_admit_computed_scalar_operands() {
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let saved: bool = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let before: bool = input;
             let saved: bool = identity(identity(before));
         }",
        "data Scalar {}
         machine identity(input: bool) -> bool { input }
         machine Scalar::read(input: bool) -> bool { input }
         data Root {}
         machine Root::value(&mut self, input: bool) {
             let saved: bool = Scalar::read(identity(input));
         }",
        "boundary trait Host { machine read(input: bool) -> bool reaches Host; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) reaches Host {
             let saved: bool = Host::read(identity(input));
         }",
        "pub data Packet { flag: bool; }
         boundary trait Host { machine read(input: bool) -> Packet reaches Host; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) reaches Host {
             let saved: Packet = Host::read(identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn plain_structural_result_source_family_defers_producer_support_to_lowering() {
    // Validation accepts the source family; it does not assert that an
    // executable structural producer exists. The lowerer pins that boundary.
    let messages = diagnostics(
        "data Packet { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine packet(input: bool) -> Packet { Packet { flag: input } }
         machine value(input: bool) {
             let saved: Packet = packet(identity(input));
         }",
    );
    assert!(messages.is_empty(), "{messages:?}");
}

#[test]
fn nominal_boundary_parameter_result_initializers_keep_exact_requirement_eligibility() {
    for (declaration, result) in [("", "bool"), ("pub data Packet { flag: bool; }", "Packet")] {
        let source = format!(
            "{declaration}
             boundary trait Host {{ machine read(input: bool) -> {result} reaches Host; }}
             machine identity(input: bool) -> bool {{ input }}
             machine value<machine Read>(input: bool)
             where machine Read satisfies Host::read;
             reaches Host {{
                 let saved: {result} = Read(identity(input));
             }}"
        );
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn unit_result_initializer_operands_still_validate_arity_and_types() {
    for (argument, expected) in [
        ("identity()", "expects 1 argument"),
        ("identity(7)", "bool"),
    ] {
        let source = format!(
            "machine identity(input: bool) -> bool {{ input }}
             machine value(input: bool) {{ let saved: bool = identity({argument}); }}"
        );
        let diagnostics = diagnostics(&source);
        assert!(
            diagnostics.iter().any(|message| message.contains(expected)),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn nested_scalar_binding_arguments_still_validate_arity_and_types() {
    for (argument, expected) in [
        ("identity()", "expects 1 argument"),
        ("identity(7)", "bool"),
    ] {
        for body in [
            format!("let saved: bool = identity({argument}); saved"),
            format!("let mut saved: bool = input; saved = identity({argument}); saved"),
        ] {
            let source = format!(
                "machine identity(input: bool) -> bool {{ input }}
                 machine value(input: bool) -> bool {{
                     {body}
                 }}"
            );
            let diagnostics = diagnostics(&source);
            assert!(
                diagnostics.iter().any(|message| message.contains(expected)),
                "{body}: {diagnostics:?}"
            );
        }
    }
}
