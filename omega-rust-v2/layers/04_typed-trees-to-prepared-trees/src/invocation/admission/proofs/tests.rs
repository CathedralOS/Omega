use super::*;

use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{BoundarySlot, RequirementSlot, SlotDirection, TypeError, resolve_source_package, type_source_package};

pub(super) const WORD: ValueType = ValueType::Integer {
    signed: false,
    width: Some(64),
};
const SOURCE: &str = "
let exceeds(n:u64,d:u64):bool = n < d;
machine demanded(n:u64,d:u64)->u64 requires d <= n crashes Trap (exceeds(n,d)) {7u64}
machine via(n:u64,d:u64)->u64 {demanded(n,d)}
machine second(n:u64,d:u64)->u64 {demanded(n,d)}
const PASS:u64=via(12u64,3u64);
const DIFFERENT:u64=via(12u64,4u64);
const OUTSIDE:u64=via(3u64,12u64);
const OTHER_SITE:u64=second(12u64,3u64);
";
const NEEDS_PROOF: &str = "the selected Trap route requires a checked disproof";

pub(super) fn typed(source: &str) -> TypedTrees<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    type_source_package(&resolve_source_package(&sources).unwrap()).unwrap()
}

/// A probe for one proof rule. The typed definition is transparent, so its
/// instantiated proposition n < d is the negation of the checked premise
/// d <= n on the identical ordered integer subjects.
fn check_order_disproof<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    actual: [u64; 2],
) -> Option<CheckedInvocationFact<'trees, 'text>> {
    let call = trees
        .calls
        .iter()
        .find(|call| call.machine.path == "demanded" && call.caller.as_ref().is_some_and(|caller| caller.path == "via"))?;
    let signature = trees
        .signatures
        .iter()
        .find(|row| row.machine == call.machine)?;
    let [(n_span, WORD), (d_span, WORD)] = signature.parameters.as_slice() else {
        return None;
    };
    let tree = trees.trees().nth(signature.node.file)?;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    let parameters = file.child(declaration, NodeKind::Params)?;
    let [n, d] = *file.nodes[parameters].children.as_slice() else { return None };
    let [left, right] = call.operands.as_slice() else { return None };
    if left.parameter.span != *n_span || right.parameter.span != *d_span || actual[1] > actual[0] {
        return None;
    }
    let body = file.child(declaration, NodeKind::Body)?;
    let requires = file.child(declaration, NodeKind::Requires)?;
    let [premise] = *file.nodes[requires].children.as_slice() else { return None };
    if !ordered_relation(&tree, premise, "<=", d, n) {
        return None;
    }

    let route = trees
        .crash_routes
        .iter()
        .find(|route| route.contract == signature.site && route.cause == CrashCause::Trap)?;
    let guard_site = route.guard.as_ref()?;
    let guard = trees
        .obligations
        .iter()
        .find(|obligation| obligation.site == *guard_site && matches!(obligation.kind, ObligationKind::TotalCrashGuard))?;
    let applied = file.ungroup(guard.node.node);
    let [callee, arguments] = *file.nodes[applied].children.as_slice() else {
        return None;
    };
    let [left, right] = *file.nodes[arguments].children.as_slice() else {
        return None;
    };
    let Some(Symbol::Declaration(predicate)) = resolved.symbol(callee) else {
        return None;
    };
    if !binds(&tree, left, n)
        || !binds(&tree, right, d)
        || !tree.semantics_at(applied).any(|record| {
            matches!(record,
                NodeSemantics::Obligation(o)
                    if matches!(&o.kind, ObligationKind::MathematicalApplication {
                        definition: Some(definition), generic_arguments, argument_count: 2
                    } if definition == predicate && generic_arguments.is_empty())
            )
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
    let [left, right] = *file.nodes[binding.parameters].children.as_slice() else {
        return None;
    };
    if !ordered_relation(&tree, binding.body?, "<", left, right) {
        return None;
    }
    Some(CheckedInvocationFact {
        trees,
        request: ProofRequest {
            invocation: InvocationSite {
                call: call.node,
                signature,
                body: file.node_id(body),
            },
            claim: InvocationClaim::CrashDisproved { route, guard },
        },
        result: None,
        application: None,
        observations: vec![],
        operands: actual.map(|value| PreparedValue::Integer(value.into())).into(),
    })
}

fn ordered_relation(
    tree: &TypedTree<'_>,
    term: usize,
    token: &str,
    left: usize,
    right: usize,
) -> bool {
    let file = tree.resolved().file;
    let term = file.ungroup(term);
    let [left_node, right_node] = *file.nodes[term].children.as_slice() else {
        return false;
    };
    selected_boolean_operation(tree, term).is_ok_and(|selected| selected.token == token && selected.operands == [WORD, WORD])
        && binds(tree, left_node, left)
        && binds(tree, right_node, right)
}

pub(super) fn binds(
    tree: &TypedTree<'_>,
    occurrence: usize,
    binder: usize,
) -> bool {
    matches!(tree.resolved().symbol(occurrence), Some(Symbol::Binding { binder_node, kind: NodeKind::Param, .. }) if *binder_node == binder)
}

fn evaluate<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    index: usize,
    evidence: Option<CheckedInvocationFact<'trees, 'text>>,
    slots: &'trees [BoundarySlot],
) -> Result<PreparedValue, TypeError> {
    let mut evaluation = ConstantEvaluation::new(trees, slots);
    evaluation.invocation_proofs.facts.extend(evidence);
    evaluation
        .evaluate(&trees.constants[index].initializer, None)
        .map(|value| PreparedValue::from(&value))
}

#[test]
fn admits_one_checked_mathematical_crash_route_at_its_exact_call_and_operands() {
    let trees = typed(SOURCE);
    let proof = || check_order_disproof(&trees, [12, 3]).expect("checked transparent order predicate");
    assert_eq!(evaluate(&trees, 0, Some(proof()), &[]).unwrap(), PreparedValue::Integer(7.into()));
    for (constant, proof) in [(0, None), (1, Some(proof()))] {
        let error = evaluate(&trees, constant, proof, &[]).unwrap_err();
        assert!(error.message.contains(NEEDS_PROOF), "{error:?}");
        assert_eq!(error.span.slice(SOURCE.as_bytes()), Some(&b"demanded(n,d)"[..]));
    }
    let error = evaluate(&trees, 2, Some(proof()), &[]).unwrap_err();
    assert!(error.message.contains("precondition is false"), "{error:?}");
    let error = evaluate(&trees, 3, Some(proof()), &[]).unwrap_err();
    assert!(error.message.contains(NEEDS_PROOF), "{error:?}");

    let changed = SOURCE.replace("n < d;", "n > d;");
    for source in [SOURCE, changed.as_str()] {
        let other = typed(source);
        let error = evaluate(&other, 0, Some(proof()), &[]).unwrap_err();
        assert!(error.message.contains(NEEDS_PROOF), "{error:?}");
    }
    assert!(check_order_disproof(&typed(&changed), [12, 3]).is_none());
}

#[test]
fn a_crash_disproof_does_not_admit_a_runtime_boundary() {
    let trees = typed(SOURCE);
    let proof = check_order_disproof(&trees, [12, 3]).unwrap();
    let slots = [BoundarySlot {
        requirement: RequirementSlot {
            declaration: proof.request.invocation.signature.machine.clone(),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    }];
    let error = evaluate(&trees, 0, Some(proof), &slots).unwrap_err();
    assert!(error.message.contains("runtime boundary slot cannot execute"), "{error:?}");
}
