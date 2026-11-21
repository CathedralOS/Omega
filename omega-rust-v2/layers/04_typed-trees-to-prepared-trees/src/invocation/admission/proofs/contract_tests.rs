use super::tests::{ADMISSION, WORD, assert_refused, binds, frozen, prepare_pass, range_in, typed};
use super::*;

#[path = "../../../../tests/support/source_package.rs"]
mod source_package;

const SOURCE: &str = "
let positive(n:u64):bool = n > 0u64;
machine demand(n:u64)->u64 requires positive(n) ensures result > 0u64 {n}
const VALUE:u64=demand(7u64);
";

/// The probe checks the named premise under the supplied input and the
/// guarantee under the supplied result. Each proves its own proposition.
fn check_contract<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    operand: u64,
    result: u64,
) -> Option<[CheckedInvocationFact<'trees, 'text>; 2]> {
    if operand == 0 || result == 0 {
        return None;
    }
    let call = trees.calls.iter().find(|call| call.machine.path == "demand")?;
    let signature = trees
        .signatures
        .iter()
        .find(|signature| signature.machine == call.machine)?;
    let [(_, WORD)] = signature.parameters.as_slice() else { return None };
    if signature.result != WORD {
        return None;
    }
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    let parameters = file.child(declaration, NodeKind::Params)?;
    let [parameter] = *file.nodes[parameters].children.as_slice() else {
        return None;
    };
    let body = file.child(declaration, NodeKind::Body)?;
    let invocation = InvocationSite {
        call: call.node,
        signature,
        body: file.node_id(body),
    };

    let requires = file.child(declaration, NodeKind::Requires)?;
    let [premise] = *file.nodes[requires].children.as_slice() else { return None };
    let [callee, arguments] = *file.nodes[premise].children.as_slice() else {
        return None;
    };
    let [argument] = *file.nodes[arguments].children.as_slice() else {
        return None;
    };
    let Some(Symbol::Declaration(predicate)) = resolved.symbol(callee) else {
        return None;
    };
    if !binds(&tree, argument, parameter)
        || !tree.semantics_at(premise).any(|record| {
            matches!(record, NodeSemantics::Obligation(o)
            if matches!(&o.kind, ObligationKind::MathematicalApplication {
                definition: Some(definition), generic_arguments, argument_count: 1
            } if definition == predicate && generic_arguments.is_empty()))
        })
    {
        return None;
    }
    let (_, _, assumptions) = trees
        .mathematical_interfaces
        .iter()
        .find(|(id, ..)| id == predicate)?;
    if !assumptions.is_empty() {
        return None;
    }
    let definition = (0..file.nodes.len()).find(|&node| resolved.declaration_at(node) == Some(predicate))?;
    let binding = file.math_binding(definition)?;
    let [bound] = *file.nodes[binding.parameters].children.as_slice() else {
        return None;
    };
    if !binds(&tree, greater_than_zero_subject(&tree, binding.body?)?, bound) {
        return None;
    }

    let ensures = file.child(declaration, NodeKind::Ensures)?;
    let [conclusion] = *file.nodes[ensures].children.as_slice() else {
        return None;
    };
    let subject = greater_than_zero_subject(&tree, conclusion)?;
    if !matches!(resolved.symbol(subject), Some(Symbol::Builtin(name)) if name == "result") {
        return None;
    }
    let obligation = trees.obligations.iter().find(|obligation| {
        obligation.node == file.node_id(conclusion)
            && matches!(&obligation.kind,
            ObligationKind::ProveGuarantee { machine, predicate } if machine == &signature.machine && predicate == &obligation.site)
    })?;
    let operands = vec![PreparedValue::Integer(operand.into())];
    Some([
        CheckedInvocationFact {
            trees,
            request: ProofRequest {
                invocation,
                claim: InvocationClaim::Requires(file.node_id(premise)),
            },
            operands: operands.clone(),
            result: None,
            application: None,
            observations: vec![],
        },
        CheckedInvocationFact {
            trees,
            request: ProofRequest {
                invocation,
                claim: InvocationClaim::Ensures(obligation),
            },
            operands,
            result: Some(PreparedValue::Integer(result.into())),
            application: None,
            observations: vec![],
        },
    ])
}

fn greater_than_zero_subject(
    tree: &TypedTree<'_>,
    term: usize,
) -> Option<usize> {
    let file = tree.resolved().file;
    let [subject, zero] = *file.nodes[term].children.as_slice() else {
        return None;
    };
    let mut operations = tree.semantics_at(term).filter_map(|record| match record {
        NodeSemantics::Operation(op) if op.result == ValueType::Bool => Some(op),
        _ => None,
    });
    let selected = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    if selected.token != ">" || selected.operands != [WORD, WORD] || landed_value(tree, zero, &WORD).ok()?? != 0.into() {
        return None;
    }
    Some(subject)
}

#[test]
fn checks_general_precondition_and_result_postcondition_separately() {
    use crate::CheckedPreparationEvidence;
    let progress = omega_typer::TypingProgress::Complete(typed(SOURCE));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
    };
    // The checker's receipts for demand(operand) returning result; `claims`
    // selects which of [requires, ensures] the coordinator presents.
    let receipt = |operand: u64, result: u64, claims: [bool; 2]| {
        check_contract(trees, operand, result)
            .expect("checked contract")
            .into_iter()
            .zip(claims)
            .filter_map(|(fact, presented)| presented.then_some(fact))
            .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_invocation_fact)
    };
    let prepare = |operand, result, claims| prepare_pass(&progress, &[], None, || receipt(operand, result, claims));

    // Healthy control: both judgments for the actual operand and result.
    assert_eq!(frozen(prepare(7, 7, [true, true]), trees, 0), PreparedValue::Integer(7.into()));

    // The source program calls demand(7u64); the refusal is at that call.
    let range = range_in(SOURCE, "const VALUE:u64=", "demand(7u64)");
    let precondition = format!("{ADMISSION}the selected precondition requires a checked proof for the supplied arguments");
    let postcondition = format!("{ADMISSION}the selected postcondition requires a checked proof for the arguments and result");
    for (operand, result, claims, reason, message) in [
        // A precondition judgment cannot stand for the postcondition.
        (7, 7, [true, false], PreparationReason::PostconditionNeedsProof, &postcondition),
        // A postcondition judgment cannot stand for the precondition.
        (7, 7, [false, true], PreparationReason::PreconditionNeedsProof, &precondition),
        // Wrong result: the guarantee was checked for 8, the body returns 7.
        (7, 8, [true, true], PreparationReason::PostconditionNeedsProof, &postcondition),
        // Wrong operands: both judgments were checked for demand(6).
        (6, 7, [true, true], PreparationReason::PreconditionNeedsProof, &precondition),
    ] {
        assert_refused(prepare(operand, result, claims), reason, range, message);
    }
}

#[test]
fn proof_inputs_replay_the_exact_layout_and_specialized_call() {
    use crate::{
        CheckedPreparationEvidence, ConstantMaterializationError, LayoutExtent, LayoutGeometry, PreparationProgress, PreparationRound, PreparationStep,
        PreparedValueOrigin, SelectedLayout, TargetIdentity, TargetSemantics,
    };
    use omega_typer::LayoutQuery;
    use std::sync::Arc;

    let source = "
        use omega\\language\\core;
        let within(n:u64):bool = n <= core\\size_of::<u64>();
        machine get<const K:u64>(n:u64)->u64 requires within(n) {n}
        machine via<const K:u64>(n:u64)->u64 {get::<K>(n)}
        const VALUE:u64=via::<1u64>(7u64);
    ";
    let identity = TargetIdentity {
        profile: "selected-target".into(),
        semantics_version: 1,
        realization: Arc::from(&b"probe-abi"[..]),
    };
    let layout = SelectedLayout {
        target: identity.clone(),
        subject: WORD,
        schema: Arc::from(&b"probe-u64-schema"[..]),
        plan: Arc::from(&b"probe-plan-one"[..]),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(8),
            alignment: Some(8),
        },
    };
    let capsule = TargetSemantics::new(identity.clone(), [layout.clone()], []).unwrap();
    let other_layout = SelectedLayout {
        plan: Arc::from(&b"probe-plan-two"[..]),
        ..layout.clone()
    };
    let other = TargetSemantics::new(identity, [other_layout], []).unwrap();
    let package = source_package::source_package([("main.omg", source)]);
    let round = PreparationRound::begin(&package, &[], Some(&capsule)).unwrap();
    let progress = round.typing();
    let omega_typer::TypingProgress::Complete(trees) = progress else {
        panic!("expected a complete typing pass")
    };
    let selected = trees
        .calls
        .iter()
        .find(|call| call.machine.path == "get")
        .unwrap();
    let caller = trees
        .calls
        .iter()
        .find(|call| call.machine.path == "via")
        .unwrap();
    let application = selected.in_caller_application(caller);
    assert_eq!(application.type_arguments, caller.type_arguments);
    let signature = trees
        .signatures
        .iter()
        .find(|signature| signature.machine == selected.machine)
        .unwrap();
    let tree = trees.trees().nth(signature.node.file).unwrap();
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    let params = file.child(declaration, NodeKind::Params).unwrap();
    let [n] = *file.nodes[params].children.as_slice() else { panic!("one input") };
    let requires = file.child(declaration, NodeKind::Requires).unwrap();
    let [premise] = *file.nodes[requires].children.as_slice() else {
        panic!("one premise")
    };
    let [callee, applied] = *file.nodes[premise].children.as_slice() else {
        panic!("predicate application")
    };
    assert!(binds(&tree, file.nodes[applied].children[0], n));
    let Some(Symbol::Declaration(predicate)) = resolved.symbol(callee) else {
        panic!("selected predicate")
    };
    assert!(
        trees
            .mathematical_interfaces
            .iter()
            .any(|(id, _, dependencies)| id == predicate && dependencies.is_empty())
    );
    assert!(
        tree.semantics_at(premise)
            .any(|record| matches!(record, NodeSemantics::Obligation(obligation)
        if matches!(&obligation.kind, ObligationKind::MathematicalApplication { definition: Some(id), generic_arguments, argument_count: 1 }
            if id == predicate && generic_arguments.is_empty())))
    );
    let definition = (0..file.nodes.len())
        .find(|&node| resolved.declaration_at(node) == Some(predicate))
        .unwrap();
    let binding = file.math_binding(definition).unwrap();
    let term = binding.body.unwrap();
    let [subject, layout_query] = *file.nodes[term].children.as_slice() else {
        panic!("typed comparison")
    };
    assert_eq!(selected_boolean_operation(&tree, term).unwrap().token, "<=");
    assert!(binds(&tree, subject, file.nodes[binding.parameters].children[0]));
    let query = selected_call(&tree, layout_query, CallUse::Fact)
        .unwrap()
        .unwrap();
    assert_eq!(query.layout_query, Some(LayoutQuery::Size));
    assert_eq!(query.type_arguments, [WORD]);

    let (size, observation) = capsule.observe_layout(LayoutQuery::Size, &WORD).unwrap();
    let operand = 7u64;
    assert!(operand <= size);
    let proof = |application| CheckedInvocationFact {
        trees,
        request: ProofRequest {
            invocation: InvocationSite {
                call: selected.node,
                signature,
                body: file.node_id(file.child(declaration, NodeKind::Body).unwrap()),
            },
            claim: InvocationClaim::Requires(file.node_id(premise)),
        },
        application,
        observations: vec![observation.clone()],
        operands: vec![PreparedValue::Integer(operand.into())],
        result: None,
    };
    let prepare = |target, application: &Option<TypedCall>| {
        prepare_pass(progress, &[], Some(target), || {
            CheckedPreparationEvidence::default().with_invocation_fact(proof(application.clone()))
        })
    };
    // The source program refuses at get::<K>(n) inside via, reached from VALUE.
    let range = range_in(source, "machine via<", "get::<K>(n)");

    // Healthy control: the receipt names this target, application and operand.
    let evaluated = round
        .evaluate(CheckedPreparationEvidence::default().with_invocation_fact(proof(Some(application.clone()))))
        .unwrap();

    // Wrong target: the receipt's layout observation is not the selected plan.
    spec_corpus::observe_handoff!(
        "static-invocation-admission/refusal",
        assert_refused(
            prepare(&other, &Some(application.clone())),
            PreparationReason::InvocationProofTargetChanged,
            range,
            &format!("{ADMISSION}the checked invocation proof depends on a different target selection\ncall chain: via -> get"),
        )
    );
    let mut wrong_application = application;
    let [ValueType::StaticInteger { value, .. }] = wrong_application.type_arguments.as_mut_slice() else {
        panic!("one closed const argument")
    };
    *value = 2u64.into();
    // Wrong application: the receipt was checked for get::<2>, the program calls get::<1>.
    assert_refused(
        prepare(&capsule, &Some(wrong_application)),
        PreparationReason::PreconditionNeedsProof,
        range,
        &format!("{ADMISSION}the selected precondition requires a checked proof for the supplied arguments\ncall chain: via -> get"),
    );

    let PreparationStep::Finished(PreparationProgress::Complete(prepared)) = round.advance(evaluated).unwrap() else {
        panic!("expected the frozen package")
    };
    let [constant] = prepared.typed().constants.as_slice() else {
        panic!("expected VALUE")
    };
    let declaration = &constant.declaration;
    assert_eq!(declaration.path, "VALUE");
    let inputs = prepared.dependencies().inputs(&PreparedValueOrigin::Constant(declaration.clone())).unwrap();
    spec_corpus::observe_handoff!(
        "static-invocation-admission/producer",
        assert_eq!(inputs.target_observations.as_slice(), std::slice::from_ref(&observation))
    );
    assert_eq!(prepared.target_observations(), std::slice::from_ref(&observation));

    // Same frozen package, target identity and geometry; only the plan that
    // justified the premise has changed. Materialization must replay it.
    let material = prepared.constant_for_materialization(declaration, &capsule).unwrap();
    spec_corpus::observe_handoff!(
        "invocation-proof/materialization",
        assert_eq!((material.definition, material.value, material.layout), (constant, &PreparedValue::Integer(7u64.into()), &layout))
    );
    spec_corpus::observe_handoff!(
        "invocation-proof/changed-materialization-plan",
        assert_eq!(
            prepared.constant_for_materialization(declaration, &other).unwrap_err(),
            ConstantMaterializationError::ChangedTargetDependency(&observation)
        )
    );
}

#[test]
fn a_static_callback_uses_the_selected_body_contract_at_its_indirect_occurrence() {
    use crate::CheckedPreparationEvidence;
    let source = "machine leaf(n:u64)->u64 requires true ensures result == 7u64 {7u64}
                  data NumberStep=machine(n:u64)->u64; machine apply<const F:NumberStep>(n:u64)->u64 {F(n)}
                  const VALUE:u64=apply::<leaf>(3u64);";
    let progress = omega_typer::TypingProgress::Complete(typed(source));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
    };
    let signature = trees
        .signatures
        .iter()
        .find(|signature| signature.machine.path == "leaf")
        .unwrap();
    let [call] = trees.indirect_calls.as_slice() else {
        panic!("expected one static callback")
    };
    let tree = trees.trees().nth(signature.node.file).unwrap();
    let file = tree.resolved().file;
    let body = file.node_id(file.child(signature.node.node, NodeKind::Body).unwrap());
    // The guarantee is checked at the indirect occurrence F(n) inside apply.
    // That occurrence has no direct call record, so the chain names it generically.
    let range = range_in(source, "machine apply<", "F(n)");
    let postcondition = format!(
        "{ADMISSION}the selected postcondition requires a checked proof for the arguments and result\ncall chain: apply -> <indirect or unsupported call>"
    );
    // The source program alone has no checked guarantee.
    assert_refused(
        prepare_pass(&progress, &[], None, CheckedPreparationEvidence::default),
        PreparationReason::PostconditionNeedsProof,
        range,
        &postcondition,
    );
    let invocation = InvocationSite {
        call: call.node,
        signature,
        body,
    };
    let receipt = |invocation, operand: u64| CheckedPreparationEvidence {
        invocation_proofs: probe_literal_guarantee(trees, invocation, vec![PreparedValue::Integer(operand.into())], &7.into()).unwrap(),
        ..CheckedPreparationEvidence::default()
    };
    // Healthy control: the guarantee for leaf(3) at the indirect occurrence.
    let admitted = prepare_pass(&progress, &[], None, || receipt(invocation, 3));
    assert_eq!(frozen(admitted, trees, 0), PreparedValue::Integer(7.into()));
    // Wrong operands: the guarantee was checked for leaf(4).
    assert_refused(
        prepare_pass(&progress, &[], None, || receipt(invocation, 4)),
        PreparationReason::PostconditionNeedsProof,
        range,
        &postcondition,
    );
    // Wrong occurrence: the same body and guarantee attributed to the direct call of apply.
    let direct = trees
        .calls
        .iter()
        .find(|call| call.machine.path == "apply")
        .unwrap();
    let elsewhere = InvocationSite {
        call: direct.node,
        ..invocation
    };
    assert_refused(
        prepare_pass(&progress, &[], None, || receipt(elsewhere, 3)),
        PreparationReason::PostconditionNeedsProof,
        range,
        &postcondition,
    );
}
