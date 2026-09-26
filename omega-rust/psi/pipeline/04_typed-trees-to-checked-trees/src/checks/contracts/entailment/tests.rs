use crate::checks::contracts::entailment::integral_parameter_reflexivity;
use crate::tests::front_end::typed_program;

#[test]
fn integral_reflexivity_requires_the_same_exact_parameter_and_builtin_type() {
    for (source_type, right, expected) in [
        ("u64", "left", true),
        ("u64", "right", false),
        ("f64", "left", false),
    ] {
        let program = typed_program(&format!(
            "machine theorem(left: {source_type}, right: {source_type}) ensures left == {right} {{}}"
        ));
        let (source, expression) = program
            .proof_facts
            .iter()
            .find_map(|(source, fact)| {
                let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    return None;
                };
                Some((source, *expression))
            })
            .expect("Boolean ensure");
        let fact = facts::Fact {
            payload: facts::FactPayload::ContractBooleanExpression {
                kind: facts::ContractFactKind::Ensures,
                fact: source,
                expression,
                instantiated: arena::Handle::invalid(),
            },
            ..Default::default()
        };
        assert_eq!(
            integral_parameter_reflexivity(&program, &fact),
            expected,
            "{source_type} / {right}"
        );
    }
}

const PROJECTION_SOURCE: &str = r#"
    data Pair [copy] { left: u64; right: u64; }
    machine make_pair(left: u64, right: u64) -> Pair terminates; {
        transition { _ -> (Pair { left: left, right: right }) }
    }
    data Input { left: u64; right: u64; }
    proposition projected_equal(left: u64, right: u64) = make_pair(left, right).left == right;
"#;

#[test]
fn projection_entailment_cannot_reuse_a_mutated_entry_premise() {
    for postcondition in [
        "make_pair(self.left, self.right).left == self.right",
        "projected_equal(self.left, self.right)",
    ] {
        let program = typed_program(&format!(
            r#"
            {PROJECTION_SOURCE}
            machine Input::break_equality(&mut self)
            requires make_pair(self.left, self.right).left == self.right
            ensures {postcondition}
            {{ self.left = 0; self.right = 1; }}
        "#
        ));
        let Err(diagnostics) =
            crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        else {
            panic!("mutated entry equality must not prove {postcondition}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn generic_proof_isolation_rejects_stale_owners_missing_arguments_and_foreign_binders() {
    use typed_trees::types::TypeReferenceNode;
    let original = typed_program(
        r#"
        data Seq<T> { case Empty; case Cons(head: T, tail: Seq<T>); }
        data Other<T> { value: T; }
        machine terms(value: Seq<u64>) {}
        "#,
    );
    let state = &original.machine_states(&original.machines()[0])[0];
    let reference = original.state_parameters(state)[0].type_reference;
    for change in ["stale owner", "missing arguments", "foreign binder"] {
        let mut program = original.clone();
        match change {
            "stale owner" | "missing arguments" => {
                let mut node = program
                    .type_reference_table
                    .type_reference(reference)
                    .clone();
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = &mut node
                else {
                    panic!("generic sequence type");
                };
                if change == "stale owner" {
                    *base_symbol = arena::Handle::from_parts(
                        base_symbol.arena_index(),
                        base_symbol.generation() + 1,
                    );
                } else {
                    *arguments = arena::HandleSpan::empty();
                }
                program
                    .type_reference_table
                    .substitute_node(reference, node);
            }
            "foreign binder" => {
                let sequence = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Seq")
                    .expect("sequence");
                let sequence_parameter = sequence.type_parameters.start();
                let other = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Other")
                    .expect("other");
                let foreign_symbol = program.data_type_parameters(other)[0].symbol;
                program
                    .data_type_parameters
                    .get_mut(sequence_parameter)
                    .symbol = foreign_symbol;
            }
            _ => unreachable!(),
        }
        let resolver = validation::CallFrameResolver::new(&program).expect("symbol table");
        assert!(
            !resolver.proof_value_is_caller_isolated(reference),
            "{change}"
        );
    }
}
