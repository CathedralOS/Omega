use super::*;
use crate::{CheckedPreparationEvidence, PreparationAnswers, prepare_typing_pass};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypingProgress, resolve_source_package, type_source_package};

fn typed(source: &str) -> TypingProgress<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "callback.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    TypingProgress::Complete(type_source_package(&package).unwrap())
}

fn caller_at(
    trees: &TypedTrees<'_>,
    obligation: &TypingObligation,
) -> CallbackCaller {
    let call = trees
        .calls
        .iter()
        .find(|call| call.node == obligation.node)
        .expect("the tested wrapper call");
    CallbackCaller::from_application(call)
}

/// Check reflexivity of integer equality on the same immutable parameter.
fn check_reflexivity<'stage, 'text>(trees: &'stage TypedTrees<'text>) -> Option<CheckedCallableRefinement<'stage, 'text>> {
    let obligation = trees
        .obligations
        .iter()
        .find(|obligation| matches!(obligation.kind, ObligationKind::RefineCallable { .. }))?;
    let ObligationKind::RefineCallable { contract, machine, arguments } = &obligation.kind else {
        return None;
    };
    if contract.requires || contract.has_guarantee || !arguments.is_empty() {
        return None;
    }
    let signature = trees
        .signatures
        .iter()
        .find(|signature| &signature.machine == machine)?;
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    let parameters = file.child(declaration, NodeKind::Params)?;
    let [formal] = *file.nodes[parameters].children.as_slice() else { return None };
    let mut requirements = file.nodes[declaration]
        .children
        .iter()
        .copied()
        .filter(|&node| file.nodes[node].kind == NodeKind::Requires);
    let [predicate] = *file.nodes[requirements.next()?].children.as_slice() else {
        return None;
    };
    if requirements.next().is_some() {
        return None;
    }
    let [left, right] = *file.nodes[predicate].children.as_slice() else {
        return None;
    };
    let operation = selected_boolean_operation(&tree, predicate).ok()?;
    let [left_type, right_type] = operation.operands.as_slice() else {
        return None;
    };
    if operation.token != "=="
        || left_type != right_type
        || IntegerRange::for_type(left_type).is_none()
        || [left, right]
            .into_iter()
            .any(|node| !matches!(resolved.symbol(node), Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == formal))
    {
        return None;
    }
    Some(CheckedCallableRefinement {
        trees,
        obligation,
        caller: caller_at(trees, obligation),
        bound: ValueType::Callable(contract.clone()),
        arguments: arguments.clone(),
        observations: Vec::new(),
    })
}

#[test]
fn a_checked_refinement_authorizes_only_its_exact_callable_binding() {
    let source = "machine identity(n:u64)->u64 requires n == n {n}
                  machine apply<const F:machine(n:u64)->u64>(n:u64)->u64 {F(n)}
                  const VALUE:u64=apply::<identity>(3u64);";
    let original = typed(source);
    let TypingProgress::Complete(trees) = &original else { unreachable!() };
    let evidence = || CheckedPreparationEvidence::default().with_callable_refinement(check_reflexivity(trees).unwrap());
    let missing = prepare_typing_pass(&original, &[], None, CheckedPreparationEvidence::default()).unwrap_err();
    assert!(missing.message.contains("needs a checked refinement"), "{}", missing.message);
    let unrelated = typed(source);
    let wrong_pass = prepare_typing_pass(&unrelated, &[], None, evidence()).unwrap_err();
    assert!(wrong_pass.message.contains("needs a checked refinement"), "{}", wrong_pass.message);
    let pass = prepare_typing_pass(&original, &[], None, evidence()).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected a frozen callback result")
    };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(3.into()));
}

#[test]
fn a_callable_bound_requires_its_own_checked_entry_at_the_actual_arguments() {
    let source = "data Nonzero = machine(n:u64)->u64 requires n > 0u64 reaches none crashes none;
                  machine identity(n:u64)->u64 reaches none crashes none {n}
                  machine apply<const F:Nonzero>(n:u64)->u64 {F(n)}
                  const VALUE:u64=apply::<identity>(3u64);";
    let progress = typed(source);
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let [call] = trees.indirect_calls.as_slice() else {
        panic!("expected one typed bound call")
    };
    let (obligation, contract, machine, arguments) = trees
        .obligations
        .iter()
        .find_map(|obligation| match &obligation.kind {
            ObligationKind::RefineCallable { contract, machine, arguments } => Some((obligation, contract, machine, arguments)),
            _ => None,
        })
        .unwrap();
    let body = trees
        .signatures
        .iter()
        .find(|signature| &signature.machine == machine)
        .unwrap();
    assert!(contract.requires && !contract.has_guarantee && !body.requires && contract.as_ref() == &call.contract);
    let bound = ValueType::Callable(contract.clone());
    // The selected body accepts the entire parameter carrier. The bound's
    // narrower entry requirement therefore establishes body admission.
    let evidence = || {
        CheckedPreparationEvidence::default().with_callable_refinement(CheckedCallableRefinement {
            trees,
            obligation,
            caller: caller_at(trees, obligation),
            bound: bound.clone(),
            arguments: arguments.clone(),
            observations: vec![],
        })
    };

    let tree = trees.trees().nth(call.node.file).unwrap();
    let resolved = tree.resolved();
    let file = resolved.file;
    let contract_node = (0..file.nodes.len())
        .find(|&node| file.nodes[node].kind == NodeKind::MachineType && file.span(node) == contract.contract.span)
        .unwrap();
    let parameters = file.child(contract_node, NodeKind::Params).unwrap();
    let [formal] = *file.nodes[parameters].children.as_slice() else {
        panic!("expected one bound parameter")
    };
    let requirement = file.child(contract_node, NodeKind::Requires).unwrap();
    let [predicate] = *file.nodes[requirement].children.as_slice() else {
        panic!("expected one bound predicate")
    };
    let [left, right] = *file.nodes[predicate].children.as_slice() else {
        panic!("expected a comparison")
    };
    let comparison = selected_boolean_operation(&tree, predicate).unwrap();
    assert_eq!(comparison.token, ">");
    assert!(matches!(resolved.symbol(left), Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == formal));
    assert_eq!(landed_value(&tree, right, &contract.parameters[0]).unwrap(), Some(0.into()));
    let certified_entry = |operand: u64| {
        assert!(operand > 0);
        CheckedCallableUse {
            trees,
            call,
            caller: caller_at(trees, obligation),
            bound: bound.clone(),
            machine: machine.clone(),
            arguments: arguments.clone(),
            operands: vec![PreparedValue::Integer(operand.into())],
            result: None,
            observations: vec![],
        }
    };
    for supplied in [evidence(), evidence().with_callable_use(certified_entry(2))] {
        let error = prepare_typing_pass(&progress, &[], None, supplied).unwrap_err();
        assert!(
            error
                .message
                .contains("bound's precondition needs a checked judgment"),
            "{}",
            error.message
        );
    }
    let pass = prepare_typing_pass(&progress, &[], None, evidence().with_callable_use(certified_entry(3))).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected a frozen callback result")
    };
    assert_eq!(constants[&trees.constants[0].declaration], PreparedValue::Integer(3.into()));
}

#[test]
fn a_callable_bound_checks_the_actual_result_against_its_guarantee_judgment() {
    let source = "data Positive = machine(n:u64)->u64 reaches none crashes none ensures result > 0u64;
                  machine leaf(n:u64)->u64 reaches none crashes none {7u64}
                  machine apply<const F:Positive>(n:u64)->u64 {F(n)}
                  const VALUE:u64=apply::<leaf>(3u64);";
    let progress = typed(source);
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let [call] = trees.indirect_calls.as_slice() else {
        panic!("expected one typed bound call")
    };
    let (obligation, contract, machine, arguments) = trees
        .obligations
        .iter()
        .find_map(|obligation| match &obligation.kind {
            ObligationKind::RefineCallable { contract, machine, arguments } => Some((obligation, contract, machine, arguments)),
            _ => None,
        })
        .unwrap();
    assert!(!contract.requires && contract.has_guarantee && contract.as_ref() == &call.contract);

    let body_signature = trees
        .signatures
        .iter()
        .find(|signature| &signature.machine == machine)
        .unwrap();
    let tree = trees.trees().nth(call.node.file).unwrap();
    let resolved = tree.resolved();
    let file = resolved.file;
    let contract_node = (0..file.nodes.len())
        .find(|&node| file.nodes[node].kind == NodeKind::MachineType && file.span(node) == contract.contract.span)
        .unwrap();
    let guarantee = file.child(contract_node, NodeKind::Ensures).unwrap();
    let [predicate] = *file.nodes[guarantee].children.as_slice() else {
        panic!("expected one bound guarantee predicate")
    };
    let [left, right] = *file.nodes[predicate].children.as_slice() else {
        panic!("expected a comparison")
    };
    let comparison = selected_boolean_operation(&tree, predicate).unwrap();
    assert_eq!(comparison.token, ">");
    assert!(matches!(resolved.symbol(left), Some(Symbol::Builtin(name)) if name == "result"));
    assert_eq!(landed_value(&tree, right, &contract.result).unwrap(), Some(0.into()));

    let body = file.child(body_signature.node.node, NodeKind::Body).unwrap();
    let [tail] = *file.nodes[body].children.as_slice() else {
        panic!("expected a single body result")
    };
    assert_eq!(file.nodes[tail].kind, NodeKind::Tail);
    let [literal] = *file.nodes[tail].children.as_slice() else {
        panic!("expected a literal body result")
    };
    let body_result = landed_value(&tree, literal, &body_signature.result)
        .unwrap()
        .unwrap();
    assert!(body_result > 0.into());

    let bound = ValueType::Callable(contract.clone());
    let evidence = |result: PreparedValue| {
        CheckedPreparationEvidence::default()
            .with_callable_refinement(CheckedCallableRefinement {
                trees,
                obligation,
                caller: caller_at(trees, obligation),
                bound: bound.clone(),
                arguments: arguments.clone(),
                observations: vec![],
            })
            .with_callable_use(CheckedCallableUse {
                trees,
                call,
                caller: caller_at(trees, obligation),
                bound: bound.clone(),
                machine: machine.clone(),
                arguments: arguments.clone(),
                operands: vec![PreparedValue::Integer(3.into())],
                result: Some(result),
                observations: vec![],
            })
    };
    // Both positive results satisfy the bound. Only the actual returned
    // value can reuse a judgment at this occurrence and these arguments.
    let other_result = PreparedValue::Integer(&body_result + 1u64);
    let error = prepare_typing_pass(&progress, &[], None, evidence(other_result)).unwrap_err();
    assert!(
        error
            .message
            .contains("bound's guarantee needs a checked judgment"),
        "{}",
        error.message
    );
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"F(n)".as_slice()));
    let expected = PreparedValue::Integer(body_result);
    let pass = prepare_typing_pass(&progress, &[], None, evidence(expected.clone())).unwrap();
    let PreparationAnswers::Frozen { constants, .. } = pass.answers else {
        panic!("expected a frozen callback result")
    };
    assert_eq!(constants[&trees.constants[0].declaration], expected);
}

#[test]
fn callable_evidence_cannot_cross_applications_with_equal_operand_and_result_types() {
    let source = "machine leaf(n:u64)->u64 {n}
                  machine apply<const K:u64, const F:machine(n:u64)->u64 requires K == 0u64>(n:u64)->u64 {F(n)}
                  machine run<const K:u64>(n:u64)->u64 {apply::<K,leaf>(n)}
                  const ZERO:u64=run::<0>(3u64);
                  const FOUR:u64=run::<4>(3u64);";
    let progress = typed(source);
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let [call] = trees.indirect_calls.as_slice() else {
        panic!("expected one typed bound invocation")
    };
    let (obligation, contract, machine, arguments) = trees
        .obligations
        .iter()
        .find_map(|obligation| match &obligation.kind {
            ObligationKind::RefineCallable { contract, machine, arguments } => Some((obligation, contract, machine, arguments)),
            _ => None,
        })
        .unwrap();
    assert!(contract.requires && !contract.has_guarantee);
    let signature = trees
        .signatures
        .iter()
        .find(|signature| &signature.machine == machine)
        .unwrap();
    assert!(!signature.requires);
    let template = trees
        .calls
        .iter()
        .find(|call| call.node == obligation.node)
        .unwrap();
    let constant = |name: &str| {
        &trees
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap()
            .initializer
    };
    let wrapper = |name: &str| {
        let select = trees
            .calls
            .iter()
            .find(|call| call.node == constant(name).node)
            .unwrap();
        template.in_caller_application(select)
    };
    let zero = wrapper("ZERO");
    let four = wrapper("FOUR");
    let [ValueType::StaticInteger { value: zero_k, .. }, zero_selection] = zero.type_arguments.as_slice() else {
        panic!("expected K and its callback")
    };
    let [ValueType::StaticInteger { value: four_k, .. }, four_selection] = four.type_arguments.as_slice() else {
        panic!("expected K and its callback")
    };
    assert_eq!(zero_k, &BigInt::from(0u64));
    assert_eq!(four_k, &BigInt::from(4u64));
    assert_eq!(zero_selection, four_selection);
    assert_eq!(zero.node, four.node);
    let bound = ValueType::Callable(contract.clone());
    assert_eq!(zero.instantiate(&bound), four.instantiate(&bound));

    let tree = trees.trees().nth(call.node.file).unwrap();
    let file = tree.resolved().file;
    let contract_node = (0..file.nodes.len())
        .find(|&node| file.nodes[node].kind == NodeKind::MachineType && file.span(node) == contract.contract.span)
        .unwrap();
    let clause = file.child(contract_node, NodeKind::Requires).unwrap();
    let [predicate] = *file.nodes[clause].children.as_slice() else {
        panic!("expected one bound precondition")
    };
    let [left, right] = *file.nodes[predicate].children.as_slice() else {
        panic!("expected a comparison")
    };
    assert_eq!(selected_boolean_operation(&tree, predicate).unwrap().token, "==");
    assert!(matches!(
        tree.resolved().symbol(left),
        Some(Symbol::Binding {
            kind: NodeKind::ConstParam,
            ..
        })
    ));
    assert_eq!(landed_value(&tree, right, &contract.parameters[0]).unwrap(), Some(zero_k.clone()));

    let refinement = |caller: &TypedCall| CheckedCallableRefinement {
        trees,
        obligation,
        caller: CallbackCaller::from_application(caller),
        bound: caller.instantiate(&bound),
        arguments: arguments.clone(),
        observations: vec![],
    };
    let mut evaluator = ConstantEvaluation::new(trees, &[]);
    evaluator.callable_refinements.checked.push(refinement(&zero));
    evaluator.callable_refinements.uses.push(CheckedCallableUse {
        trees,
        call,
        caller: CallbackCaller::from_application(&zero),
        bound: zero.instantiate(&bound),
        machine: machine.clone(),
        arguments: arguments.clone(),
        operands: vec![PreparedValue::Integer(3.into())],
        result: None,
        observations: vec![],
    });
    assert_eq!(
        evaluator
            .evaluate_value_request(constant("ZERO"), None)
            .unwrap(),
        PreparedValue::Integer(3.into())
    );
    let error = evaluator
        .evaluate_value_request(constant("FOUR"), None)
        .unwrap_err();
    assert!(matches!(error, EvaluationError::AtCall(error) if error.message.contains("needs a checked refinement")));
    // Refinement is valid under K=4 because the leaf accepts every input.
    // Its caller still cannot reuse the K=0 entry judgment.
    evaluator.callable_refinements.checked.push(refinement(&four));
    let error = evaluator
        .evaluate_value_request(constant("FOUR"), None)
        .unwrap_err();
    assert!(matches!(error, EvaluationError::AtCall(error) if error.message.contains("bound's precondition needs a checked judgment")));
}
