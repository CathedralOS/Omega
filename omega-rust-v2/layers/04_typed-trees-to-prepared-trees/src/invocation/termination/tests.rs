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
const MUTUAL_COUNTDOWN: &str = "machine first(n:u64)->u64 terminates by n {second(n)}
machine second(n:u64)->u64 terminates by n {
    match n == 0u64 {true -> 9u64, false -> first(n - 1u64),}
}
const VALUE:u64=first(3u64);";

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
fn check_countdown<'trees, 'text>(trees: &'trees TypedTrees<'text>) -> Option<CheckedRecursiveComponent<'trees, 'text>> {
    let (member, call) = countdown(trees, "count", "count", RecursiveApplication::default())?;
    Some(CheckedRecursiveComponent {
        trees,
        members: vec![member],
        edges: vec![CheckedRecursiveEdge { caller: 0, callee: 0, call }],
        observations: Vec::new(),
    })
}

fn ranked_member<'trees>(
    trees: &'trees TypedTrees<'_>,
    name: &str,
    application: RecursiveApplication,
) -> Option<(CheckedRecursiveMember<'trees>, usize)> {
    let signature = trees
        .signatures
        .iter()
        .find(|signature| signature.machine.path == name)?;
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
    Some((
        CheckedRecursiveMember {
            signature,
            body: file.node_id(body),
            application,
        },
        parameter,
    ))
}

fn countdown<'trees>(
    trees: &'trees TypedTrees<'_>,
    name: &str,
    next_machine: &str,
    application: RecursiveApplication,
) -> Option<(CheckedRecursiveMember<'trees>, &'trees TypedCall)> {
    let (member, parameter) = ranked_member(trees, name, application)?;
    let signature = member.signature;
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let binds_parameter = |node| matches!(resolved.symbol(node), Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == parameter);
    let body = member.body.node;
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
    if call.caller.as_ref() != Some(&signature.machine) || call.machine.path != next_machine || operand.node.file != signature.node.file {
        return None;
    }
    let next = file.ungroup(operand.node.node);
    let [subject, decrement] = *file.nodes[next].children.as_slice() else {
        return None;
    };
    if selected_arithmetic(&tree, next, &WORD, None).ok()? != ("-", WORD)
        || !binds_parameter(subject)
        || landed_value(&tree, decrement, &WORD).ok()?? != 1.into()
    {
        return None;
    }
    let call = trees.calls.iter().find(|record| std::ptr::eq(*record, call))?;
    Some((member, call))
}

/// The only transfer out of `first` preserves the rank. Every cycle must
/// then take `second`'s sole recursive edge, which strictly lowers it.
fn check_mutual_countdown<'trees, 'text>(trees: &'trees TypedTrees<'text>) -> Option<CheckedRecursiveComponent<'trees, 'text>> {
    let (first, parameter) = ranked_member(trees, "first", RecursiveApplication::default())?;
    let (second, decrement) = countdown(trees, "second", "first", RecursiveApplication::default())?;
    let tree = trees.trees().nth(first.body.file)?;
    let file = tree.resolved().file;
    let [tail] = *file.nodes[first.body.node].children.as_slice() else {
        return None;
    };
    let [call] = *file.nodes[tail].children.as_slice() else { return None };
    if file.nodes[tail].kind != NodeKind::Tail {
        return None;
    }
    let preserve = selected_call(&tree, call, CallUse::Execution).ok()??;
    let [operand] = preserve.operands.as_slice() else { return None };
    if preserve.caller.as_ref() != Some(&first.signature.machine)
        || preserve.machine != second.signature.machine
        || decrement.machine != first.signature.machine
        || operand.node.file != first.body.file
        || !matches!(tree.resolved().symbol(operand.node.node), Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == parameter)
    {
        return None;
    }
    let preserve = trees
        .calls
        .iter()
        .find(|record| std::ptr::eq(*record, preserve))?;
    Some(CheckedRecursiveComponent {
        trees,
        members: vec![first, second],
        edges: vec![
            CheckedRecursiveEdge {
                caller: 0,
                callee: 1,
                call: preserve,
            },
            CheckedRecursiveEdge {
                caller: 1,
                callee: 0,
                call: decrement,
            },
        ],
        observations: Vec::new(),
    })
}

fn evaluate<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    evidence: Option<CheckedRecursiveComponent<'trees, 'text>>,
    slots: &'trees [omega_typer::BoundarySlot],
) -> Result<PreparedValue, TypeError> {
    let mut evaluation = ConstantEvaluation::new(trees, slots);
    evaluation.termination.evidence = evidence.into_iter().collect();
    evaluation
        .evaluate(&trees.constants[0].initializer, None)
        .map(|value| PreparedValue::from(&value))
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
    assert_eq!(proof().members[0].signature.node, changed_signature.node);
    assert_eq!(proof().members[0].signature.machine, changed_signature.machine);

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
fn requires_one_joint_proof_for_every_mutual_recursion_edge() {
    let trees = typed(MUTUAL_COUNTDOWN, "app");
    let proof = || check_mutual_countdown(&trees).expect("the round trip strictly lowers a natural number");
    assert_eq!(evaluate(&trees, Some(proof()), &[]).unwrap(), PreparedValue::Integer(9.into()));

    let mut missing_edge = proof();
    missing_edge.edges.pop();
    let mut wrong_body = proof();
    wrong_body.members[0].body = wrong_body.members[1].body;
    let mut wrong_call = proof();
    wrong_call.edges[1].call = trees.calls.iter().find(|call| call.caller.is_none()).unwrap();
    for malformed in [missing_edge, wrong_body, wrong_call] {
        let error = evaluate(&trees, Some(malformed), &[]).unwrap_err();
        assert!(
            error
                .message
                .contains("recursive component: first -> second -> first"),
            "{}",
            error.message
        );
    }

    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    for member in proof().members {
        evaluator.termination.evidence.push(CheckedRecursiveComponent {
            trees: &trees,
            members: vec![member],
            edges: Vec::new(),
            observations: Vec::new(),
        });
    }
    let error = evaluator
        .evaluate(&trees.constants[0].initializer, None)
        .unwrap_err();
    assert!(error.message.contains(CYCLIC_INVOCATION));

    let no_descent = MUTUAL_COUNTDOWN
        .replace("n - 1u64", "n - 0u64")
        .replace("first(3u64)", "first(0u64)");
    let unchanged = typed(&no_descent, "app");
    assert!(check_mutual_countdown(&unchanged).is_none());
    assert!(
        evaluate(&unchanged, Some(proof()), &[])
            .unwrap_err()
            .message
            .contains(CYCLIC_INVOCATION)
    );
}

#[test]
fn independent_recursive_components_share_a_typing_pass() {
    let source = format!(
        "{COUNTDOWN}\n{}",
        COUNTDOWN
            .replace("count", "second")
            .replace("VALUE", "SECOND")
            .replace("7u64", "11u64")
    );
    let trees = typed(&source, "app");
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    for name in ["count", "second"] {
        let (member, call) = countdown(&trees, name, name, RecursiveApplication::default()).unwrap();
        evaluator.termination.evidence.push(CheckedRecursiveComponent {
            trees: &trees,
            members: vec![member],
            edges: vec![CheckedRecursiveEdge { caller: 0, callee: 0, call }],
            observations: Vec::new(),
        });
    }
    let frozen = evaluator.finish().unwrap();
    for (name, expected) in [("VALUE", 7), ("SECOND", 11)] {
        let (id, value) = frozen.iter().find(|(id, _)| id.path == name).unwrap();
        assert_eq!(value, &PreparedValue::Integer(expected.into()), "{}", id.path);
    }
}

#[test]
fn recursive_applications_need_proofs_for_their_exact_static_substitutions() {
    let source = "machine count<T>(n:u64)->u64 terminates by n {
        match n == 0u64 {true -> 7u64, false -> count::<T>(n - 1u64),}
    }
    const FIRST:u64=count::<u8>(3u64);
    const SECOND:u64=count::<u16>(0u64);";
    let trees = typed(source, "app");
    let constant = |name| {
        &trees
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap()
            .initializer
    };
    let proof = |name| {
        let request = constant(name);
        let entrance = trees
            .calls
            .iter()
            .find(|call| call.node == request.node)
            .unwrap();
        let application = RecursiveApplication::select(entrance, ValueType::clone).unwrap();
        let (member, call) = countdown(&trees, "count", "count", application).unwrap();
        CheckedRecursiveComponent {
            trees: &trees,
            members: vec![member],
            edges: vec![CheckedRecursiveEdge { caller: 0, callee: 0, call }],
            observations: Vec::new(),
        }
    };
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    evaluator.termination.evidence.push(proof("FIRST"));
    assert_eq!(
        evaluator.evaluate(constant("FIRST"), None).unwrap(),
        omega_typer::StaticValue::Integer(7.into())
    );
    assert!(
        evaluator
            .evaluate(constant("SECOND"), None)
            .unwrap_err()
            .message
            .contains(CYCLIC_INVOCATION)
    );

    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    evaluator
        .termination
        .evidence
        .extend([proof("FIRST"), proof("SECOND")]);
    assert_eq!(
        evaluator.evaluate(constant("SECOND"), None).unwrap(),
        omega_typer::StaticValue::Integer(7.into())
    );
}

#[test]
fn termination_replays_the_exact_target_plan_used_by_its_proof() {
    use crate::{LayoutExtent, LayoutGeometry, SelectedLayout, TargetIdentity, TargetSemantics};
    use omega_typer::LayoutQuery;

    let trees = typed(COUNTDOWN, "app");
    let identity = TargetIdentity {
        profile: "test".into(),
        semantics_version: 1,
        realization: (&b"test target"[..]).into(),
    };
    let target = |plan: &'static [u8]| {
        TargetSemantics::new(
            identity.clone(),
            [SelectedLayout {
                target: identity.clone(),
                subject: WORD,
                schema: (&b"u64"[..]).into(),
                plan: plan.into(),
                geometry: LayoutGeometry {
                    extent: LayoutExtent::Fixed(8),
                    alignment: Some(8),
                },
            }],
            [],
        )
        .unwrap()
    };
    let (original, different) = (target(b"first plan"), target(b"second plan"));
    let (size, observation) = original.observe_layout(LayoutQuery::Size, &WORD).unwrap();
    assert_eq!(different.observe_layout(LayoutQuery::Size, &WORD).unwrap().0, size);
    let mut proof = check_countdown(&trees).unwrap();
    proof.observations.push(observation.clone());
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);
    evaluator.termination.evidence.push(proof);
    evaluator.target = Some(&different);
    assert!(
        evaluator
            .evaluate(&trees.constants[0].initializer, None)
            .unwrap_err()
            .message
            .contains("different target selection")
    );
    assert!(evaluator.observations.borrow().is_empty());
    evaluator.target = Some(&original);
    assert_eq!(
        evaluator
            .evaluate(&trees.constants[0].initializer, None)
            .unwrap(),
        omega_typer::StaticValue::Integer(7.into())
    );
    assert_eq!(evaluator.observations.borrow().as_slice(), &[observation]);
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
    let signature = evidence.members[0].signature;
    evaluation.termination.evidence.push(evidence);
    let mut execution = Execution::default();
    let call = trees.calls.last().unwrap().node;

    execution.values.push(PreparedValue::Integer(0.into()));
    evaluation
        .enter_call(call, signature, None, None, &mut execution)
        .unwrap();
    let outer_steps = execution.steps.len();
    execution.values.push(PreparedValue::Integer(0.into()));
    evaluation
        .enter_call(call, signature, None, None, &mut execution)
        .unwrap();
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
            declaration: evidence.members[0].signature.machine.clone(),
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
    let signature = evidence.members[0].signature;
    let call = evidence.edges[0].call;
    let tree = trees.trees().nth(signature.node.file).unwrap();
    let index = evaluation.signatures()[&signature.machine].0;
    evaluation.termination.evidence.push(evidence);
    let mut execution = Execution::default();
    execution.active.insert(signature.node);
    let body = Scope::Body(Rc::new(CallBindings(RefCell::new(vec![]), None)));
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

pub(crate) fn countdown_evidence<'stage, 'text>(trees: &'stage TypedTrees<'text>) -> crate::CheckedPreparationEvidence<'stage, 'text> {
    let component = check_countdown(trees).expect("the supplied countdown body decreases its natural rank");
    crate::CheckedPreparationEvidence::default().with_recursive_component(component)
}

/// Entering through a static binder uses the same checked component as a
/// direct call. Its indirect occurrence cannot authorize a recursive edge.
#[test]
fn a_ranked_static_callback_requires_the_exact_component() {
    let source = COUNTDOWN.replace(
        "const VALUE:u64=count(3u64);",
        "machine apply<const F:machine(n:u64)->u64>(n:u64)->u64 {F(n)}
         const VALUE:u64=apply::<count>(3u64);",
    );
    let progress = omega_typer::TypingProgress::Complete(typed(&source, "app"));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
    };
    let error = crate::prepare_typing_pass(&progress, &[], None, crate::CheckedPreparationEvidence::default()).unwrap_err();
    assert!(error.message.contains("requires a checked termination proof"), "{}", error.message);
    let pass = crate::prepare_typing_pass(&progress, &[], None, countdown_evidence(trees)).unwrap();
    let crate::PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected frozen countdown")
    };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(7.into()));
}

#[test]
fn a_static_callback_wrapper_requires_its_exact_reachable_recursive_proof() {
    let source = "machine count<const Tag:u64>(n:u64)->u64 terminates by n {
            match n == 0u64 {true -> 7u64, false -> count::<Tag>(n - 1u64),}
        }
        machine relay<const Tag:u64>(n:u64)->u64 {
            match n == 0u64 {true -> 9u64, false -> count::<Tag>(n),}
        }
        machine apply<const F:machine(n:u64)->u64>(n:u64)->u64 {F(n)}
        const SKIP:u64=apply::<relay::<2>>(0u64);
        const RUN:u64=apply::<relay::<2>>(3u64);";
    let progress = omega_typer::TypingProgress::Complete(typed(source, "app"));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
    };
    let proof = |tag: u8| {
        let argument = ValueType::StaticInteger {
            carrier: Box::new(WORD),
            value: tag.into(),
        };
        let application = RecursiveApplication {
            arguments: vec![argument],
            enclosing: vec![],
        };
        let (member, call) = countdown(trees, "count", "count", application).unwrap();
        CheckedRecursiveComponent {
            trees,
            members: vec![member],
            edges: vec![CheckedRecursiveEdge { caller: 0, callee: 0, call }],
            observations: vec![],
        }
    };
    // Evaluate the skipped branch first. Its direct call is absent from the
    // execution trace, but it is still part of the callback's checked closure.
    let skip = &trees
        .constants
        .iter()
        .find(|constant| constant.declaration.path == "SKIP")
        .unwrap()
        .initializer;
    for evidence in [None, Some(proof(1))] {
        let mut evaluator = ConstantEvaluation::new(trees, &[]);
        evaluator.termination.evidence = evidence.into_iter().collect();
        let error = evaluator.evaluate(skip, None).unwrap_err();
        assert!(error.message.contains(CYCLIC_INVOCATION), "{}", error.message);
    }
    let checked = crate::CheckedPreparationEvidence::default().with_recursive_component(proof(2));
    let pass = crate::prepare_typing_pass(&progress, &[], None, checked).unwrap();
    let crate::PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected closed callback values")
    };
    for (name, expected) in [("SKIP", 9), ("RUN", 7)] {
        let declaration = &trees
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap()
            .declaration;
        assert_eq!(constants[declaration], PreparedValue::Integer(expected.into()));
    }
}
