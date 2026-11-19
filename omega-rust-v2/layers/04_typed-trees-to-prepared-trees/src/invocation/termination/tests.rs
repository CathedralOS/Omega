use super::*;

use omega_parser::TerminationClause;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypeError, resolve_source_package, type_source_package};

const WORD: ValueType = ValueType::Integer {
    signed: false,
    width: Some(64),
};
const COUNTDOWN: &str = "machine count(n:u64)->u64 terminates by n {
    match n == 0u64 {true -> 7u64, false -> count(n - 1u64),}
}
const VALUE:u64=count(3u64);";

fn typed<'text>(
    source: &'text str,
    package: &'text str,
) -> TypedTrees<'text> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package,
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    type_source_package(&resolve_source_package(&sources).unwrap()).unwrap()
}

/// Check the one countdown shape used to exercise admission without a
/// production checker. The selected core view orders the unsigned parameter
/// as a natural number. Only the nonzero branch calls back, subtracting one
/// exactly from that parameter, so its argument is natural and strictly smaller.
fn check_countdown<'trees, 'text>(trees: &'trees TypedTrees<'text>) -> Option<CheckedRecursiveBody<'trees, 'text>> {
    let signature = trees
        .signatures
        .iter()
        .find(|signature| signature.machine.path == "count")?;
    let [(_, WORD)] = signature.parameters.as_slice() else { return None };
    if signature.result != WORD {
        return None;
    }
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    let params = file.child(declaration, NodeKind::Params)?;
    let [parameter] = *file.nodes[params].children.as_slice() else { return None };
    let binds_parameter = |node| {
        matches!(
            resolved.symbol(node),
            Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == parameter
        )
    };
    let clause = file.child(declaration, NodeKind::Terminates)?;
    let TerminationClause::By {
        measure,
        order: None,
        bound: None,
    } = file.nodes[clause].termination_clause()?
    else {
        return None;
    };
    if !binds_parameter(measure)
        || !tree.semantics_at(clause).any(|record| {
            matches!(
                record, NodeSemantics::RankingWitness(witness)
                    if witness.machine == signature.machine && witness.rank == WORD
                        && witness.view.package == "@omega/toolchain"
                        && witness.view.path == "language::core::Nat::Descending"
            )
        })
    {
        return None;
    }

    let body = file.child(declaration, NodeKind::Body)?;
    let [tail] = *file.nodes[body].children.as_slice() else { return None };
    let [dispatch] = *file.nodes[tail].children.as_slice() else { return None };
    let [guard, base_arm, step_arm] = *file.nodes[dispatch].children.as_slice() else {
        return None;
    };
    if file.nodes[tail].kind != NodeKind::Tail || file.nodes[dispatch].kind != NodeKind::Match {
        return None;
    }
    let [subject, zero] = *file.nodes[guard].children.as_slice() else {
        return None;
    };
    let comparison = selected_boolean_operation(&tree, guard).ok()?;
    if comparison.token != "==" || comparison.operands != [WORD, WORD] || !binds_parameter(subject) || landed_value(&tree, zero, &WORD).ok()?? != 0.into() {
        return None;
    }
    let [base_pattern, base] = *file.nodes[base_arm].children.as_slice() else {
        return None;
    };
    let [step_pattern, step] = *file.nodes[step_arm].children.as_slice() else {
        return None;
    };
    if !matches!(resolved.symbol(base_pattern), Some(Symbol::Builtin(value)) if value == "true")
        || !matches!(resolved.symbol(step_pattern), Some(Symbol::Builtin(value)) if value == "false")
        || landed_value(&tree, base, &WORD).ok()?.is_none()
    {
        return None;
    }
    let call = selected_call(&tree, step, CallUse::Execution).ok()??;
    let [operand] = call.operands.as_slice() else { return None };
    if call.caller.as_ref() != Some(&signature.machine) || call.machine != signature.machine || operand.node.file != signature.node.file {
        return None;
    }
    let next = file.ungroup(operand.node.node);
    let [subject, decrement] = *file.nodes[next].children.as_slice() else {
        return None;
    };
    if selected_arithmetic(&tree, next, &WORD).ok()? != ("-", &WORD) || !binds_parameter(subject) || landed_value(&tree, decrement, &WORD).ok()?? != 1.into() {
        return None;
    }
    let call = trees.calls.iter().find(|record| std::ptr::eq(*record, call))?;
    Some(CheckedRecursiveBody {
        trees,
        signature,
        body: file.node_id(body),
        edges: vec![call],
    })
}

fn evaluate<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    evidence: Option<CheckedRecursiveBody<'trees, 'text>>,
    slots: &'trees [omega_typer::BoundarySlot],
) -> Result<PreparedValue, TypeError> {
    let mut evaluation = ConstantEvaluation::new(trees, slots);
    evaluation.termination.evidence = evidence;
    evaluation.evaluate(&trees.constants[0].initializer, None)
}

#[test]
fn consumes_only_evidence_for_the_exact_typed_body() {
    let trees = typed(COUNTDOWN, "app");
    let proof = || check_countdown(&trees).expect("the nonzero branch lowers the rank strictly");
    assert_eq!(evaluate(&trees, Some(proof()), &[]).unwrap(), PreparedValue::Integer(7.into()));
    assert!(
        evaluate(&trees, None, &[])
            .unwrap_err()
            .message
            .contains(CYCLIC_INVOCATION)
    );

    let changed = COUNTDOWN.replace("n - 1u64", "n - 0u64");
    let changed_trees = typed(&changed, "app");
    assert!(check_countdown(&changed_trees).is_none());
    let changed_signature = changed_trees
        .signatures
        .iter()
        .find(|signature| signature.machine.path == "count")
        .unwrap();
    assert_eq!(proof().signature.node, changed_signature.node);
    assert_eq!(proof().signature.machine, changed_signature.machine);

    let second_body = COUNTDOWN.replace("count", "second");
    let second_body = second_body.trim_end_matches("const VALUE:u64=second(3u64);");
    let two_bodies = format!("{}\n{second_body}", COUNTDOWN.replace("VALUE:u64=count(3u64)", "VALUE:u64=second(3u64)"));
    let both = typed(&two_bodies, "app");
    let evidence_for_count = check_countdown(&both).unwrap();
    let error = evaluate(&both, Some(evidence_for_count), &[]).unwrap_err();
    assert!(error.message.contains("recursive component: second -> second"), "{}", error.message);

    for other in [
        changed_trees,
        typed(COUNTDOWN, "app"),
        typed(COUNTDOWN, "other"),
    ] {
        assert!(
            evaluate(&other, Some(proof()), &[])
                .unwrap_err()
                .message
                .contains(CYCLIC_INVOCATION)
        );
    }
}

#[test]
fn checks_preconditions_and_crash_routes_independently_of_the_rank() {
    for (clause, diagnostic) in [
        ("requires n < 3u64", "the selected integer precondition is false"),
        ("crashes Trap (n == 3u64)", "the selected Trap route remains possible"),
    ] {
        let source = COUNTDOWN.replace("terminates by n", &format!("{clause} terminates by n"));
        let trees = typed(&source, "app");
        let evidence = check_countdown(&trees).expect("the body still descends");
        assert!(
            evaluate(&trees, Some(evidence), &[])
                .unwrap_err()
                .message
                .contains(diagnostic),
            "{diagnostic}"
        );
    }
}

#[test]
fn one_body_proof_does_not_cover_a_recursive_precondition() {
    let source = COUNTDOWN.replace("terminates by n", "requires guard(n) reaches none crashes none terminates by n");
    let source = format!(
        "{source}
        machine guard(n:u64)->bool reaches none crashes none terminates {{count(n) == 7u64}}"
    );
    let trees = typed(&source, "app");
    let evidence = check_countdown(&trees).expect("the body still descends");
    let error = evaluate(&trees, Some(evidence), &[]).unwrap_err();
    assert!(
        error
            .message
            .contains("recursive component: count -> guard -> count"),
        "{}",
        error.message
    );
}

#[test]
fn a_recursive_return_keeps_the_enclosing_activation_active() {
    let trees = typed(COUNTDOWN, "app");
    let mut evaluation = ConstantEvaluation::new(&trees, &[]);
    let evidence = check_countdown(&trees).unwrap();
    let signature = evidence.signature;
    evaluation.termination.evidence = Some(evidence);
    let mut execution = Execution::default();

    execution.values.push(PreparedValue::Integer(0.into()));
    evaluation.enter_call(signature, &mut execution).unwrap();
    let outer_steps = execution.steps.len();
    execution.values.push(PreparedValue::Integer(0.into()));
    evaluation.enter_call(signature, &mut execution).unwrap();
    while execution.steps.len() > outer_steps {
        let step = execution.steps.pop().unwrap();
        evaluation.evaluate_step(step, &mut execution).unwrap();
    }
    assert!(execution.active.contains(&signature.node));

    while let Some(step) = execution.steps.pop() {
        evaluation.evaluate_step(step, &mut execution).unwrap();
    }
    assert!(!execution.active.contains(&signature.node));
}

#[test]
fn checking_a_rank_does_not_allow_runtime_boundary_selection() {
    use omega_typer::{BoundarySlot, RequirementSlot, SlotDirection};
    let trees = typed(COUNTDOWN, "app");
    let evidence = check_countdown(&trees).unwrap();
    let slots = [BoundarySlot {
        requirement: RequirementSlot {
            declaration: evidence.signature.machine.clone(),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    }];
    let error = evaluate(&trees, Some(evidence), &slots).unwrap_err();
    assert!(
        error
            .message
            .contains("the selected runtime boundary slot cannot execute"),
        "{}",
        error.message
    );
}

#[test]
fn evidence_for_a_body_edge_does_not_authorize_a_distinct_call_in_an_argument() {
    let source = format!(
        "{}
         machine keep(n:u64)->u64{{n}}
         machine outer()->u64{{keep(count(3u64))}}",
        COUNTDOWN.replace("VALUE:u64=count(3u64)", "VALUE:u64=outer()")
    );
    let trees = typed(&source, "app");
    let mut evaluation = ConstantEvaluation::new(&trees, &[]);
    let evidence = check_countdown(&trees).unwrap();
    let signature = evidence.signature;
    let call = evidence.edges[0];
    let tree = trees.trees().nth(signature.node.file).unwrap();
    let index = evaluation.signatures()[&signature.machine].0;
    evaluation.termination.evidence = Some(evidence);
    let mut execution = Execution::default();
    execution.active.insert(signature.node);
    let body = Scope::Body(Rc::new(CallBindings(RefCell::new(vec![]))));
    evaluation
        .admit_termination(index, signature, call, &body, &tree, &execution)
        .unwrap();

    let nested_call = trees
        .calls
        .iter()
        .find(|candidate| {
            candidate.machine == signature.machine
                && candidate
                    .caller
                    .as_ref()
                    .is_some_and(|caller| caller.path == "outer")
        })
        .unwrap();
    assert_eq!(nested_call.use_site, CallUse::Execution);
    let error = evaluation
        .admit_termination(index, signature, nested_call, &body, &tree, &execution)
        .unwrap_err();
    assert!(matches!(error, EvaluationError::Refused(CYCLIC_INVOCATION)));
}
