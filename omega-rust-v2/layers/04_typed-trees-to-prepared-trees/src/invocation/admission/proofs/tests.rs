use super::*;

use crate::preparation::typing_pass::evaluate_pass;
use crate::{CheckedPreparationEvidence, PreparationAnswers, PreparedPass, TargetSemantics, prepare_typing_pass};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{BoundarySlot, RequirementSlot, SlotDirection, TypingProgress, resolve_source_package, type_source_package};

pub(super) const WORD: ValueType = ValueType::Integer {
    signed: false,
    width: Some(64),
};

/// The origin `typed` gives every program in these tests.
pub(super) const PACKAGE: &str = "app";
pub(super) const FILE: &str = "main.omg";
/// Written here, independently of the renderer's prefix and the reason inventory.
pub(super) const ADMISSION: &str = "static evaluation cannot admit this invocation: ";
const TRAP_NEEDS_DISPROOF: &str = "the selected Trap route requires a checked disproof for the supplied arguments";

/// The source program. Each case appends only the constant it evaluates, so
/// the complete pass refuses or freezes exactly that constant.
const DECLARATIONS: &str = "
let exceeds(n:u64,d:u64):bool = n < d;
machine demanded(n:u64,d:u64)->u64 requires d <= n crashes Trap (exceeds(n,d)) {7u64}
machine via(n:u64,d:u64)->u64 {demanded(n,d)}
machine second(n:u64,d:u64)->u64 {demanded(n,d)}
";
const PASS: &str = "const PASS:u64=via(12u64,3u64);\n";
const DIFFERENT: &str = "const DIFFERENT:u64=via(12u64,4u64);\n";
const OUTSIDE: &str = "const OUTSIDE:u64=via(3u64,12u64);\n";
const OTHER_SITE: &str = "const OTHER_SITE:u64=second(12u64,3u64);\n";

fn program(constant: &str) -> String {
    format!("{DECLARATIONS}{constant}")
}

pub(super) fn typed(source: &str) -> TypedTrees<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: PACKAGE,
        source: ParsedSource {
            logical_path: FILE,
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    type_source_package(&resolve_source_package(&sources).unwrap()).unwrap()
}

/// The byte range of the first `occurrence` after `after`, located in the
/// program text rather than taken from the diagnostic under test.
pub(super) fn range_in(
    source: &str,
    after: &str,
    occurrence: &str,
) -> (usize, usize) {
    let from = source
        .find(after)
        .unwrap_or_else(|| panic!("{after:?} does not occur in the program"))
        + after.len();
    let start = from
        + source[from..]
            .find(occurrence)
            .unwrap_or_else(|| panic!("{occurrence:?} does not follow {after:?}"));
    (start, start + occurrence.len())
}

/// Present one receipt to the complete typing pass. `evaluate_pass` is the
/// cause-preserving form of the public `prepare_typing_pass`; both receive an
/// equal receipt and must reach the same decision and diagnostic.
pub(super) fn prepare_pass<'stage, 'text>(
    progress: &'stage TypingProgress<'text>,
    slots: &'stage [BoundarySlot],
    target: Option<&'stage TargetSemantics>,
    receipt: impl Fn() -> CheckedPreparationEvidence<'stage, 'text>,
) -> Result<PreparedPass, PreparationRefusal> {
    let caused = evaluate_pass(progress, slots, target, receipt(), &HashSet::new());
    let public = prepare_typing_pass(progress, slots, target, receipt());
    match (&caused, &public) {
        (Err(refusal), Err(diagnostic)) => assert_eq!(&refusal.diagnostic, diagnostic, "the public typing pass renders another diagnostic"),
        (Ok(_), Ok(_)) => {}
        _ => panic!("the public typing pass decides differently: {caused:?} / {public:?}"),
    }
    caused
}

/// Check the returned cause, origin and complete diagnostic, then report it.
pub(super) fn assert_refused(
    result: Result<PreparedPass, PreparationRefusal>,
    reason: PreparationReason,
    range: (usize, usize),
    message: &str,
) {
    let refusal = result.expect_err("the pass must refuse");
    spec_corpus::assert_preparation_cause!(refusal, RefusalCause::Preparation(reason), PACKAGE, FILE, range, message);
}

/// The frozen value of one local constant after an admitted pass.
pub(super) fn frozen(
    result: Result<PreparedPass, PreparationRefusal>,
    trees: &TypedTrees<'_>,
    index: usize,
) -> PreparedValue {
    let PreparationAnswers::Frozen { constants, .. } = result.expect("the pass must admit").answers else {
        panic!("complete package")
    };
    constants[&trees.constants[index].declaration].clone()
}

/// A probe for one proof rule. The typed definition is transparent, so its
/// instantiated proposition n < d is the negation of the checked premise
/// d <= n on the identical ordered integer subjects.
fn check_order_disproof<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    caller: &str,
    actual: [u64; 2],
) -> Option<CheckedInvocationFact<'trees, 'text>> {
    let call = trees.calls.iter().find(|call| {
        call.machine.path == "demanded"
            && call
                .caller
                .as_ref()
                .is_some_and(|selected| selected.path == caller)
    })?;
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

/// The receipt the checker issues for one call of `demanded` and its operands.
fn disproof<'trees, 'text>(
    trees: &'trees TypedTrees<'text>,
    caller: &str,
    operands: [u64; 2],
) -> CheckedPreparationEvidence<'trees, 'text> {
    CheckedPreparationEvidence::default().with_invocation_fact(check_order_disproof(trees, caller, operands).expect("checked transparent order predicate"))
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

/// The refusal at the call of `demanded` inside `caller`, reached from the constant.
fn trap_refusal(
    source: &str,
    caller: &str,
) -> ((usize, usize), String) {
    (
        range_in(source, &format!("machine {caller}("), "demanded(n,d)"),
        format!("{ADMISSION}{TRAP_NEEDS_DISPROOF}\ncall chain: {caller} -> demanded"),
    )
}

#[test]
fn admits_one_checked_mathematical_crash_route_at_its_exact_call_and_operands() {
    // Healthy control: the receipt names this call, route and operands.
    let source = program(PASS);
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let admitted = prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 3]));
    assert_eq!(frozen(admitted, trees, 0), PreparedValue::Integer(7.into()));
    // The source program alone leaves the Trap route open.
    let (range, message) = trap_refusal(&source, "via");
    assert_refused(
        prepare_pass(&progress, &[], None, CheckedPreparationEvidence::default),
        PreparationReason::TrapRouteNeedsDisproof,
        range,
        &message,
    );

    // Wrong operands: a receipt for (12,3) presented to via(12,4).
    let source = program(DIFFERENT);
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let (range, message) = trap_refusal(&source, "via");
    assert_refused(
        prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 3])),
        PreparationReason::TrapRouteNeedsDisproof,
        range,
        &message,
    );
    let admitted = prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 4]));
    assert_eq!(frozen(admitted, trees, 0), PreparedValue::Integer(7.into()));

    // The source program's own premise is false; no receipt is issued for it
    // and a receipt for other operands does not replace the premise.
    let source = program(OUTSIDE);
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    assert!(check_order_disproof(trees, "via", [3, 12]).is_none());
    assert_refused(
        prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 3])),
        PreparationReason::IntegerPreconditionFalse,
        range_in(&source, "machine via(", "demanded(n,d)"),
        &format!("{ADMISSION}the selected integer precondition is false for the supplied arguments\ncall chain: via -> demanded"),
    );

    // Wrong target occurrence: the receipt for the call in via presented to the call in second.
    let source = program(OTHER_SITE);
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let (range, message) = trap_refusal(&source, "second");
    assert_refused(
        prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 3])),
        PreparationReason::TrapRouteNeedsDisproof,
        range,
        &message,
    );
    let admitted = prepare_pass(&progress, &[], None, || disproof(trees, "second", [12, 3]));
    assert_eq!(frozen(admitted, trees, 0), PreparedValue::Integer(7.into()));

    // Wrong typing pass: a receipt checked against one typing of a program is
    // presented to another typing of the same program and to a program whose
    // guard predicate changed. The checker cannot issue the changed program's receipt.
    let source = program(PASS);
    let changed = format!("{}{PASS}", DECLARATIONS.replace("n < d;", "n > d;"));
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    for other_source in [source.as_str(), changed.as_str()] {
        let other = TypingProgress::Complete(typed(other_source));
        let (range, message) = trap_refusal(other_source, "via");
        assert_refused(
            prepare_pass(&other, &[], None, || disproof(trees, "via", [12, 3])),
            PreparationReason::TrapRouteNeedsDisproof,
            range,
            &message,
        );
    }
    assert!(check_order_disproof(&typed(&changed), "via", [12, 3]).is_none());
}

#[test]
fn a_crash_disproof_does_not_admit_a_runtime_boundary() {
    let source = program(PASS);
    let progress = TypingProgress::Complete(typed(&source));
    let TypingProgress::Complete(trees) = &progress else { unreachable!() };
    let proof = check_order_disproof(trees, "via", [12, 3]).unwrap();
    let slots = [BoundarySlot {
        requirement: RequirementSlot {
            declaration: proof.request.invocation.signature.machine.clone(),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    }];
    // Healthy control: the same receipt admits the local body without the slot.
    let admitted = prepare_pass(&progress, &[], None, || disproof(trees, "via", [12, 3]));
    assert_eq!(frozen(admitted, trees, 0), PreparedValue::Integer(7.into()));
    assert_refused(
        prepare_pass(&progress, &slots, None, || disproof(trees, "via", [12, 3])),
        PreparationReason::UnselectedBoundarySlot,
        range_in(&source, "machine via(", "demanded(n,d)"),
        &format!("{ADMISSION}the selected runtime boundary slot cannot execute during preparation\ncall chain: via -> demanded"),
    );
}
