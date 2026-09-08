use checked_trees::{ContractProofFactKind, ContractProofFactOwner, ProofFacts};
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolved source");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed source")
}

fn proof(program: &TypedTrees) -> ProofFacts {
    let plan = proof::obligations::build_proof_plan(program);
    let borrow = crate::build_borrow_facts(program);
    crate::build_proof_facts(program, &plan, &borrow)
}

fn recursive_scalar(argument: &str) -> String {
    format!(
        r#"
        machine walk(remaining: u64) -> u64
        requires remaining > 0
        ensures result > 0
        {{
            transition remaining > 1 {{
                true -> walk({argument})
                false -> 1
            }}
        }}
        "#
    )
}

#[test]
fn scalar_entry_backedges_reestablish_machine_requires() {
    crate::lower_typed_trees(typed(&recursive_scalar("1")))
        .expect("the recursive argument establishes the entry requirement");
    let diagnostics = crate::lower_typed_trees(typed(&recursive_scalar("0")))
        .expect_err("the entry assumption cannot prove the recursive argument's requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call walk")),
        "{diagnostics:#?}"
    );
}

#[test]
fn entry_backedge_attaches_requires_without_importing_ensures() {
    let program = typed(&recursive_scalar("1"));
    let machine = &program.machines()[0];
    let entry = &program.machine_states(machine)[0];
    let facts = proof(&program);
    let calls = facts.contract_calls.iter().collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "the entry transfer retains its contract call"
    );
    let call = calls[0].1;
    assert_eq!(call.target_machine_symbol, machine.symbol);
    assert_eq!(call.target_state_symbol, entry.symbol);
    assert!(call.ensures.is_empty(), "a backedge has no normal return");
    let requirements = facts.contract_fact_refs.span_or_empty(call.requires);
    assert_eq!(requirements.len(), 1);
    let requirement = facts.contract_facts.get(requirements[0].fact);
    assert_eq!(requirement.kind, ContractProofFactKind::Requires);
    assert_eq!(
        requirement.owner,
        ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        }
    );
}

#[test]
fn internal_named_state_uses_only_its_own_requires_and_no_machine_ensures() {
    let program = typed(
        r#"
        machine choose(entry: u64) -> u64
        requires entry == 7
        ensures result == 8
        {
            transition { _ -> finish(8) }
            state finish(value: u64) -> u64
            requires value == 8
            { value }
        }
        "#,
    );
    let machine = &program.machines()[0];
    let destination = &program.machine_states(machine)[1];
    let facts = proof(&program);
    let calls = facts.contract_calls.iter().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let call = calls[0].1;
    assert_eq!(call.target_state_symbol, destination.symbol);
    assert!(call.ensures.is_empty());
    let requirements = facts.contract_fact_refs.span_or_empty(call.requires);
    assert_eq!(requirements.len(), 1);
    assert_eq!(
        facts.contract_facts.get(requirements[0].fact).owner,
        ContractProofFactOwner::MachineState {
            machine_symbol: machine.symbol,
            state_symbol: destination.symbol,
        }
    );
    crate::lower_typed_trees(program)
        .expect("a sibling state need not re-establish the machine's entry == 7");
}

#[test]
fn ordinary_invocations_retain_requires_and_ensures() {
    for body in ["restricted(1)", "transition { _ -> restricted(1) }"] {
        let program = typed(&format!(
            "machine restricted(value: u64) -> u64 requires value > 0; ensures result > 0; {{ 1 }}
             machine caller() -> u64 {{ {body} }}"
        ));
        let facts = proof(&program);
        let calls = facts.contract_calls.iter().collect::<Vec<_>>();
        assert_eq!(calls.len(), 1, "{body}");
        let call = calls[0].1;
        assert_eq!(
            facts.contract_fact_refs.span_or_empty(call.requires).len(),
            1
        );
        assert_eq!(
            facts.contract_fact_refs.span_or_empty(call.ensures).len(),
            1
        );
        crate::lower_typed_trees(program).expect(body);
    }
}

fn entry_requirement(program: &TypedTrees) -> typed_trees::expression::ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("an authored entry requirement")
}

#[test]
fn good_ranking_cannot_discharge_an_unsupported_boolean_requirement() {
    let source = r#"
        machine walk(remaining: u64 [0..=5], allowed: bool)
        requires remaining <= 5 && allowed;
        terminates by remaining in 0..=5;
        -> u64 {
            transition remaining > 0 {
                true -> walk(remaining - 1, false)
                false -> 0
            }
        }
    "#;
    let program = typed(source);
    crate::checks::termination::check_machine_termination(&program)
        .expect("the numeric ranking is valid independently of allowed");
    let goal = entry_requirement(&program);
    let machine = &program.machines()[0];
    assert!(!validation::arithmetic_entry_requirement_is_covered(
        &program, machine, goal,
    ));
    assert!(!crate::checks::termination::proves_ranked_entry_requirement(&program, machine, goal,));
    let diagnostics = crate::lower_typed_trees(program)
        .expect_err("strict descent cannot establish allowed for a false actual");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call walk")),
        "{diagnostics:#?}"
    );
}

#[test]
fn good_ranking_cannot_discharge_a_noninductive_numeric_requirement() {
    let source = r#"
        machine walk(remaining: u64 [0..=5], permission: u64 [0..=5])
        requires permission > 0;
        terminates by remaining in 0..=5;
        -> u64 {
            transition remaining > 0 {
                true -> walk(remaining - 1, 0)
                false -> 0
            }
        }
    "#;
    let program = typed(source);
    crate::checks::termination::check_machine_termination(&program)
        .expect("remaining decreases even though permission is lost");
    let goal = entry_requirement(&program);
    let machine = &program.machines()[0];
    assert!(validation::arithmetic_entry_requirement_is_covered(
        &program, machine, goal,
    ));
    assert!(!crate::checks::termination::proves_ranked_entry_requirement(&program, machine, goal,));
    let diagnostics = crate::lower_typed_trees(program)
        .expect_err("a readable requirement still needs inductive preservation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call walk")),
        "{diagnostics:#?}"
    );
}

#[test]
fn graph_requirement_reuse_checks_internal_arrivals_before_root_reentry() {
    let source = r#"
        machine walk(remaining: u64 [0..=5], permission: u64 [0..=5])
        requires permission > 0;
        terminates by remaining in 0..=5;
        -> u64 {
            transition remaining > 0 && permission > 0 {
                true -> step(remaining - 1, permission)
                false -> 0
            }
            state step(pending: u64 [0..=5], ticket: u64 [0..=5]) {
                transition pending > 0 {
                    true -> walk(pending - 1, ticket)
                    false -> 0
                }
            }
        }
    "#;
    let program = typed(source);
    let goal = entry_requirement(&program);
    assert!(crate::checks::termination::proves_ranked_entry_requirement(
        &program,
        &program.machines()[0],
        goal,
    ));
    crate::lower_typed_trees(program).expect("both arrivals preserve permission");

    let changed = source.replace(
        "step(remaining - 1, permission)",
        "step(remaining - 1, permission - 1)",
    );
    let program = typed(&changed);
    crate::checks::termination::check_machine_termination(&program)
        .expect("every cyclic edge still decreases the numeric rank");
    let goal = entry_requirement(&program);
    assert!(
        !crate::checks::termination::proves_ranked_entry_requirement(
            &program,
            &program.machines()[0],
            goal,
        )
    );
    let diagnostics = crate::lower_typed_trees(program)
        .expect_err("an earlier internal arrival can lose the requirement before root reentry");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call walk")),
        "{diagnostics:#?}"
    );
}
