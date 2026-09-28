use super::validate_machine_call_cycles;
use crate::declarations::symbols::TopLevelSymbols;
use crate::validated_runtime_recursive_components;
use diagnostics::Diagnostic;
use source::SourceMap;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

fn typed_source(source: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("runtime_component_query.omg".into(), source.to_owned())
        .source_id;
    crate::front_end::typed_program_from_source_map(sources, &[(source_id, source)])
}

fn typed_pair(body: &str, extra: &str) -> TypedTrees {
    typed_source(&format!(
        "machine unrelated() -> u64 {{ transition {{ _ -> 0 }} }}
         machine scan_a(remaining: u64) -> u64
         terminates by remaining;
         {{ transition remaining == 0 {{ true -> 0 false -> scan_b(remaining) }} }}
         machine scan_b(remaining: u64) -> u64
         terminates by remaining;
         {{ {body} }}
         {extra}"
    ))
}

const DECREASE: &str = "transition remaining == 0 { true -> 0 false -> scan_a(remaining - 1) }";

fn machine_symbol(program: &TypedTrees, name: &str) -> SymbolHandle {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("fixture machine")
        .symbol
}

fn assert_admitted_pair(program: &TypedTrees) {
    assert_eq!(
        validated_runtime_recursive_components(program),
        vec![vec![
            machine_symbol(program, "scan_a"),
            machine_symbol(program, "scan_b"),
        ]]
    );
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

fn assert_cycle_rejected(program: &TypedTrees) {
    assert!(validated_runtime_recursive_components(program).is_empty());
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
        "{diagnostics:#?}"
    );
}

#[test]
fn query_returns_exact_runtime_members_without_acyclic_neighbors() {
    assert_admitted_pair(&typed_pair(DECREASE, ""));
}

#[test]
fn removing_strict_descent_rejects_the_preserving_cycle() {
    let mut program = typed_pair(DECREASE, "");
    assert_admitted_pair(&program);
    let decreases = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| match expression {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Subtract => {
                Some((handle, binary.left))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!decreases.is_empty());
    for (handle, left) in decreases {
        let preserved = program.expression_table.expression(left).clone();
        *program.expression_table.expression_mut(handle) = preserved;
    }
    assert_cycle_rejected(&program);
}

#[test]
fn strict_parallel_call_does_not_hide_a_preserving_cycle() {
    let program = typed_pair(
        "transition remaining > 0 {
            true -> scan_a(remaining - 1)
            false -> scan_a(remaining)
        }",
        "",
    );
    assert_cycle_rejected(&program);
}

#[test]
fn missing_target_identity_cannot_borrow_a_parallel_call() {
    let mut program = typed_pair(
        "transition remaining > 1 { true -> scan_a(remaining - 1) }
         transition remaining > 0 { true -> scan_a(remaining - 1) false -> 0 }",
        "",
    );
    assert_admitted_pair(&program);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "scan_b")
        .unwrap();
    let statements = program.machine_states(machine)[0].statement_nodes;
    let (statement, mut target) = program
        .statement_table
        .iter_statements(statements)
        .find_map(|(handle, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            let target = program.statement_table.transition_target(transition.target);
            matches!(target, TransitionTargetNode::Named { .. }).then(|| (handle, target.clone()))
        })
        .expect("parallel recursive call");
    let TransitionTargetNode::Named { path, .. } = &mut target else {
        unreachable!()
    };
    path.symbol = SymbolHandle::invalid();
    path.head_symbol = SymbolHandle::invalid();
    let target = program.statement_table.insert_transition_target(target);
    let StatementNode::Transition(transition) = program.statement_table.statement_mut(statement)
    else {
        unreachable!()
    };
    transition.target = target;
    assert_cycle_rejected(&program);
}

#[test]
fn missing_member_ranking_rejects_the_component() {
    let mut program = typed_pair(DECREASE, "");
    assert_admitted_pair(&program);
    let symbol = machine_symbol(&program, "scan_b");
    program
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == symbol)
        .unwrap()
        .termination_plan
        .implementation_witness = None;
    assert_cycle_rejected(&program);
}

#[test]
fn symbol_construction_errors_grant_no_components() {
    let mut program = typed_pair(DECREASE, "data Duplicate {}");
    assert_admitted_pair(&program);
    let duplicate = program.data_definitions()[0].clone();
    program.push_data_definition(duplicate);
    let mut diagnostics = Vec::new();
    TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.iter().any(Diagnostic::is_error));
    assert!(validated_runtime_recursive_components(&program).is_empty());
}

#[test]
fn normalized_proof_self_tail_edges_still_require_strict_descent() {
    for (name, witness, returned, accepted) in [
        ("unmeasured", "", "ProofNat::spin(n)", false),
        (
            "nondecreasing",
            "terminates by n;",
            "ProofNat::spin(n)",
            false,
        ),
        (
            "decreasing",
            "terminates by n;",
            "ProofNat::spin(prev)",
            true,
        ),
    ] {
        let program = typed_source(&format!(
            "data ProofNat {{ case Zero; case Succ(prev: ProofNat); }}
             machine ProofNat::spin(n: ProofNat) -> ProofNat {witness} {{
                 transition n {{
                     ProofNat::Zero -> (n)
                     ProofNat::Succ {{ prev }} -> ({returned})
                 }}
             }}"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ProofNat::spin")
            .unwrap();
        assert!(
            crate::proof_only_classification(&program).is_proof_machine(&program, machine),
            "{name}: proof classification"
        );
        let mut diagnostics = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                crate::machine_calls::calls::validate_proof_machine_recursion(
                    &program,
                    machine,
                    state,
                    statement,
                    &mut diagnostics,
                );
            }
        }
        assert_eq!(diagnostics.is_empty(), accepted, "{name}: {diagnostics:#?}");
    }
}
