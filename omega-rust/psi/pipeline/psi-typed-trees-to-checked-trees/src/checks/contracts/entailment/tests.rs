use super::*;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn parse(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn integral_reflexivity_requires_the_same_exact_parameter_and_builtin_type() {
    for (source_type, right, expected) in [
        ("u64", "left", true),
        ("u64", "right", false),
        ("f64", "left", false),
    ] {
        let program = parse(&format!(
            "machine theorem(left: {source_type}, right: {source_type}) ensures left == {right} {{}}"
        ));
        let (source, expression) = program
            .proof_facts
            .iter()
            .find_map(|(source, fact)| {
                let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    return None;
                };
                Some((source, *expression))
            })
            .expect("Boolean ensure");
        let fact = psi_facts::Fact {
            payload: psi_facts::FactPayload::ContractBooleanExpression {
                kind: psi_facts::ContractFactKind::Ensures,
                fact: source,
                expression,
                instantiated: psi_arena::Handle::invalid(),
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
fn recursive_resultless_identity_retains_its_positive_entailment_outcome() {
    let program = parse(
        r#"
        data Nat { case Zero; case Succ(prev: Nat); }
        machine copy(n: Nat) -> Nat terminates by n; {
            transition n {
                Nat::Zero -> Nat::Zero
                Nat::Succ { prev } -> Nat::Succ { prev: copy(prev) }
            }
        }
        machine copy_identity(n: Nat) terminates by n;
        ensures copy(n) == n
        {
            transition n {
                Nat::Zero -> base()
                Nat::Succ { prev } -> step(prev)
            }
            state base() {}
            state step(prev: Nat) { copy_identity(prev); }
        }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "copy_identity")
        .expect("identity theorem");
    assert_eq!(
        psi_validation::proven_machine_contract_expressions(&program, machine.symbol).len(),
        1,
        "exact recursive theorem is positively proved, not merely admitted"
    );
}

#[test]
fn proof_value_isolation_checks_reference_fields_beside_recursive_edges() {
    let program = parse(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        data BorrowingNat { case Zero; case Succ(previous: BorrowingNat, value: &mut u64); }
        boundary data Opaque;
        data OpaqueNat { case Zero; case Succ(previous: OpaqueNat, opaque: Opaque); }
        machine terms(plain: Nat, borrowing: BorrowingNat, opaque: OpaqueNat) {}
    "#,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let parameters = program.state_parameters(state);
    let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolved program");
    assert!(resolver.proof_value_is_caller_isolated(parameters[0].type_reference));
    assert!(!resolver.proof_value_is_caller_isolated(parameters[1].type_reference));
    assert!(!resolver.proof_value_is_caller_isolated(parameters[2].type_reference));
    assert!(
        resolver.local_requires_write_origin(parameters[0].type_reference),
        "proof-only policy must not change ordinary runtime frame conservatism"
    );
}

#[test]
fn projection_entailment_cannot_reuse_a_mutated_entry_premise() {
    for postcondition in [
        "make_pair(self.left, self.right).left == self.right",
        "projected_equal(self.left, self.right)",
    ] {
        let program = parse(&format!(
            r#"
            {PROJECTION_SOURCE}
            machine Input::break_equality(&mut self)
            requires make_pair(self.left, self.right).left == self.right
            ensures {postcondition}
            {{ self.left = 0; self.right = 1; }}
        "#
        ));
        let Err(diagnostics) = crate::lower_typed_trees(program) else {
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
fn generic_proof_isolation_requires_isolated_actual_arguments_before_closing_cycles() {
    let program = parse(
        r#"
        data Seq<T> { case Empty; case Cons(head: T, tail: Seq<T>); }
        data RefCarrier<T> { value: &mut T; }
        data Nest<T> { case Empty; case More(next: Nest<RefCarrier<T>>); }
        boundary data Opaque;
        machine terms<T>(
            plain: Seq<u64>,
            nested: Seq<Seq<u64>>,
            reference: Seq<RefCarrier<u64>>,
            opaque: Seq<Opaque>,
            unbound: Seq<T>,
            changing: Nest<u64>
        ) {}
        "#,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolved program");
    for (parameter, expected) in program
        .state_parameters(state)
        .iter()
        .zip([true, true, false, false, false, false])
    {
        assert_eq!(
            resolver.proof_value_is_caller_isolated(parameter.type_reference),
            expected,
            "{}",
            parameter.name
        );
    }
    assert!(
        resolver.local_requires_write_origin(program.state_parameters(state)[0].type_reference)
    );
}

#[test]
fn generic_recursive_copy_retains_actual_inductive_exit_proofs() {
    let program = parse(
        r#"
        data Seq<T> { case Empty; case Cons(head: T, tail: Seq<T>); }
        machine copy(items: Seq<u64>) -> Seq<u64> terminates by items; {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: copy(tail) }
            }
        }
        machine copy_identity(items: Seq<u64>) -> Seq<u64> terminates by items;
        ensures copy(items) == items
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons { head: head, tail: copy_identity(tail) }
            }
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "copy_identity")
        .expect("identity theorem");
    assert_eq!(
        psi_validation::proven_machine_contract_expressions(&program, machine.symbol).len(),
        1,
        "the actual induction engine proves this exact guarantee"
    );
    crate::lower_typed_trees(program).expect("generic recursive proof survives every exit check");
}

#[test]
fn generic_proof_isolation_rejects_stale_owners_missing_arguments_and_foreign_binders() {
    use psi_typed_trees::types::TypeReferenceNode;
    let original = parse(
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
                    *base_symbol = psi_arena::Handle::from_parts(
                        base_symbol.arena_index(),
                        base_symbol.generation() + 1,
                    );
                } else {
                    *arguments = psi_arena::HandleSpan::empty();
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
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("symbol table");
        assert!(
            !resolver.proof_value_is_caller_isolated(reference),
            "{change}"
        );
    }
}
