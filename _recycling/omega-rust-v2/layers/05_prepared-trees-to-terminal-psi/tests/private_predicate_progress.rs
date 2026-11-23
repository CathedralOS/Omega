#[path = "support/returns.rs"]
mod returns;
#[path = "support/source.rs"]
mod sources;

use omega_psi::{
    CallArgument, CallRequirement, CallSite, CallTarget, IntegerPrimitive, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter,
    PredicateOperator, PredicateProgress, PredicateStep, ReturnEdge, ScalarLiteral, ScalarType, Site, SourceSpan,
};

const PRIVATE: &str = "machine allowed(x:u8)->bool reaches none crashes none {x > 0}\ndomain u8::Mark requires allowed(self);";

fn check_application<'a>(psi: &'a LoweredPackage, source: &str, progress: PredicateProgress, premises: usize) -> &'a LoweredMachine {
    let [helper] = psi.machines.as_slice() else { panic!("expected only allowed") };
    let [domain] = psi.domains.as_slice() else { panic!("expected only Mark") };
    assert_eq!(helper.declaration.path, "allowed");
    assert_eq!(domain.declaration.path, "u8::Mark");
    let [clause] = domain.requires.as_slice() else { panic!("expected one predicate") };
    let span = sources::unique_span(source, "allowed(self)");
    assert_eq!(clause.predicate, [
        PredicateStep::SelfValue,
        PredicateStep::MachineApplication {
            callee: helper.declaration.clone(),
            operands: vec![ScalarType::Integer { signed: false, width: 8 }.into()],
            result: ScalarType::Boolean,
            requirements: 0..premises,
            progress,
            site: Site { package: "app".into(), file: "mark.omg".into(), span: SourceSpan::new(span.start, span.end) },
        },
    ]);
    assert_eq!(helper.contract.requires.len(), premises);
    assert!(helper.contract.has_empty_crash_ceiling);
    assert!(helper.contract.crash_routes.is_empty());
    helper
}

#[test]
fn private_body_and_authored_promise_are_distinct_progress_inputs() {
    let promised = PRIVATE.replace("none {x", "none terminates {x");
    let exported = format!("public {promised}");
    for (source, progress, public) in [
        (PRIVATE, PredicateProgress::CheckPrivateBody, false),
        (promised.as_str(), PredicateProgress::CheckPromise, false),
        (exported.as_str(), PredicateProgress::CheckPromise, true),
    ] {
        let psi = sources::lower(&[("mark.omg", source)]).unwrap();
        let helper = check_application(&psi, source, progress, 0);
        assert_eq!(helper.contract.public, public);
        assert_eq!(helper.contract.promises_termination, progress == PredicateProgress::CheckPromise);
        let [Parameter::Scalar(x)] = helper.body.parameters[..] else { panic!("expected x") };
        let [zero, comparison] = helper.body.operations.as_slice() else { panic!("expected x > 0") };
        assert_eq!(zero.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
        let OperationResult::Scalar(z) = zero.result else { panic!("expected scalar zero") };
        let OperationKind::Integer { operation, operands } = &comparison.kind else { panic!("expected comparison") };
        assert_eq!(operation.primitive, IntegerPrimitive::GreaterThan);
        assert_eq!(*operands, [x, z]);
        let OperationResult::Scalar(b) = comparison.result else { panic!("expected Boolean result") };
        assert_eq!(returns::sole_return(&helper.body), ReturnEdge::Scalar(b));
    }
}

#[test]
fn private_self_recursion_retains_the_edge_and_premise_not_a_progress_verdict() {
    let source = PRIVATE.replace("reaches none", "requires x != 0 reaches none").replace("{x > 0}", "{allowed(x)}");
    let psi = sources::lower(&[("mark.omg", &source)]).unwrap();
    let helper = check_application(&psi, &source, PredicateProgress::CheckPrivateBody, 1);
    assert!(!helper.contract.promises_termination);
    assert_eq!(helper.contract.ranking, None);
    assert_eq!(helper.contract.requires[0].predicate, [PredicateStep::Formal(0), PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::NotEqual)]);
    let [Parameter::Scalar(x)] = helper.body.parameters[..] else { panic!("expected x") };
    let [call] = helper.body.operations.as_slice() else { panic!("expected a recursive call") };
    assert_eq!(call.kind, OperationKind::Call {
        callee: CallTarget::Machine(helper.id),
        arguments: vec![CallArgument::Scalar(x)],
        requirements: vec![CallRequirement { site: CallSite::Operation(call.id), clause: 0 }],
    });
    let OperationResult::Scalar(b) = call.result else { panic!("expected call result") };
    assert_eq!(returns::sole_return(&helper.body), ReturnEdge::Scalar(b));
}

#[test]
fn a_silent_export_cannot_supply_private_progress_and_a_crash_route_is_not_an_empty_ceiling() {
    for (source, reason) in [
        (format!("public {PRIVATE}"), "predicate application of an exported helper requires a termination promise"),
        (PRIVATE.replace("crashes none", "crashes Trap"), "predicate application requires a terminating helper with an empty crash ceiling"),
    ] {
        sources::refuse(&[("mark.omg", &source)], "mark.omg", sources::unique_span(&source, "allowed(self)"), reason);
    }
}
