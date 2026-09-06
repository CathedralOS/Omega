use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use validation::validate_program;

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
fn unserved_assignment_destinations_keep_nested_call_realization_fence() {
    for source in [
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, input: bool) -> bool {
             self.flag = identity(identity(input));
             self.flag
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let mut saved: bool = input;
             saved = identity(identity(input));
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
fn unserved_initializers_keep_nested_call_realization_fence() {
    for source in [
        "data Container { flag: bool; }
         machine Container::identity(&self, input: bool) -> bool { input }
         machine Container::value(&self, input: bool) -> bool {
             let saved: bool = self.identity(self.identity(input));
             saved
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let mut saved: bool = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let before: bool = input;
             let saved: bool = identity(identity(before));
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::read(&self, input: bool) -> bool { input }
         machine value(container: &Container, input: bool) {
             let saved: bool = container.read(identity(input));
         }",
        "data Packet { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine packet(input: bool) -> Packet { Packet { flag: input } }
         machine value(input: bool) {
             let saved: Packet = packet(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value<machine Read>(input: bool)
         where machine Read(value: bool) -> bool;
         { let saved: bool = Read(identity(input)); }",
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
fn first_immutable_unit_result_initializers_admit_computed_scalar_operands() {
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let saved: bool = identity(identity(input));
         }",
        "data Scalar {}
         machine identity(input: bool) -> bool { input }
         machine Scalar::read(input: bool) -> bool { input }
         data Root {}
         machine Root::value(&mut self, input: bool) {
             let saved: bool = Scalar::read(identity(input));
         }",
        "pub data Host {}
         machine identity(input: bool) -> bool { input }
         boundary machine Host::read(input: bool) -> bool;
         machine value(input: bool) {
             let saved: bool = Host::read(identity(input));
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
