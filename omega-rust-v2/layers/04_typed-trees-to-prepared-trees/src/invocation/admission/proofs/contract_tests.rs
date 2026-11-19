use super::tests::{WORD, binds, typed};
use super::*;

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
    use crate::{CheckedPreparationEvidence, PreparationAnswers, prepare_typing_pass};
    let progress = omega_typer::TypingProgress::Complete(typed(SOURCE));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
    };
    let facts = |result| check_contract(trees, 7, result).unwrap();
    let prepare = |proofs: Vec<_>| {
        prepare_typing_pass(
            &progress,
            &[],
            None,
            proofs
                .into_iter()
                .fold(CheckedPreparationEvidence::default(), CheckedPreparationEvidence::with_invocation_fact),
        )
    };

    let prepared = prepare(facts(7).into()).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = prepared.answers else {
        panic!("complete package")
    };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(7.into()));

    let [requires, ensures] = facts(7);
    for (proof, message) in [
        (requires, "postcondition requires"),
        (ensures, "precondition requires"),
    ] {
        let refusal = prepare(vec![proof]).unwrap_err();
        assert!(refusal.message.contains(message), "{refusal:?}");
    }

    let refusal = prepare(facts(8).into()).unwrap_err();
    assert!(
        refusal
            .message
            .contains("postcondition requires a checked proof for the arguments and result"),
        "{refusal:?}"
    );
}

#[test]
fn proof_inputs_replay_the_exact_layout_and_specialized_call() {
    use crate::{
        CheckedPreparationEvidence, LayoutExtent, LayoutGeometry, PreparationAnswers, PreparedValueOrigin, SelectedLayout, TargetIdentity, TargetSemantics,
        prepare_typing_pass,
    };
    use omega_typer::LayoutQuery;
    use std::sync::Arc;

    let source = "
        use omega::language::core;
        let within(n:u64):bool = n <= core::size_of::<u64>();
        machine get<const K:u64>(n:u64)->u64 requires within(n) {n}
        machine via<const K:u64>(n:u64)->u64 {get::<K>(n)}
        const VALUE:u64=via::<1u64>(7u64);
    ";
    let progress = omega_typer::TypingProgress::Complete(typed(source));
    let omega_typer::TypingProgress::Complete(trees) = &progress else {
        unreachable!()
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
    assert_eq!(query.layout_query.as_ref().map(|query| query.projection), Some(LayoutQuery::Size));
    assert_eq!(query.type_arguments, [WORD]);

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
        ..layout
    };
    let other = TargetSemantics::new(identity, [other_layout], []).unwrap();
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
    let prepare = |target, proof| prepare_typing_pass(&progress, &[], Some(target), CheckedPreparationEvidence::default().with_invocation_fact(proof));

    let prepared = prepare(&capsule, proof(Some(application.clone()))).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = prepared.answers else {
        panic!("constant was frozen")
    };
    let declaration = &trees.constants[0].declaration;
    assert_eq!(constants[declaration], PreparedValue::Integer(operand.into()));
    let inputs = prepared
        .dependencies
        .inputs(&PreparedValueOrigin::Constant(declaration.clone()))
        .unwrap();
    assert_eq!(inputs.target_observations.as_slice(), std::slice::from_ref(&observation));
    assert_eq!(prepared.target_observations.as_slice(), std::slice::from_ref(&observation));

    let error = prepare(&other, proof(Some(application.clone()))).unwrap_err();
    assert!(
        error
            .message
            .contains("proof depends on a different target selection"),
        "{error:?}"
    );
    let mut wrong_application = application;
    let [ValueType::StaticInteger { value, .. }] = wrong_application.type_arguments.as_mut_slice() else {
        panic!("one closed const argument")
    };
    *value = 2u64.into();
    let error = prepare(&capsule, proof(Some(wrong_application))).unwrap_err();
    assert!(error.message.contains("precondition requires a checked proof"), "{error:?}");
}

#[test]
fn a_static_callback_uses_the_selected_body_contract_at_its_indirect_occurrence() {
    use crate::{CheckedPreparationEvidence, PreparationAnswers, prepare_typing_pass};
    let source = "machine leaf(n:u64)->u64 requires true ensures result == 7u64 {7u64}
                  machine apply<const F:machine(n:u64)->u64>(n:u64)->u64 {F(n)}
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
    let without = prepare_typing_pass(&progress, &[], None, CheckedPreparationEvidence::default()).unwrap_err();
    assert!(
        without
            .message
            .contains("postcondition requires a checked proof"),
        "{}",
        without.message
    );
    let invocation = InvocationSite {
        call: call.node,
        signature,
        body,
    };
    let proof = probe_literal_guarantee(trees, invocation, vec![PreparedValue::Integer(3.into())], &7.into()).unwrap();
    let evidence = CheckedPreparationEvidence {
        invocation_proofs: proof,
        ..CheckedPreparationEvidence::default()
    };
    let pass = prepare_typing_pass(&progress, &[], None, evidence).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected a frozen constant")
    };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(7.into()));
}
