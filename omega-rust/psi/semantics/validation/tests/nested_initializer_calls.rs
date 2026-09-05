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
fn attached_and_unit_initializers_keep_nested_call_realization_fence() {
    for source in [
        "data Container { flag: bool; }
         machine Container::identity(&self, input: bool) -> bool { input }
         machine Container::value(&self, input: bool) -> bool {
             let saved: bool = self.identity(self.identity(input));
             saved
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let saved: bool = identity(identity(input));
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
