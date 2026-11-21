#[path = "support/source.rs"]
mod sources;

use omega_psi::{CallArgument, CallRequirement, CallSite, CallTarget, OperationKind, Parameter, PredicateOperator, PredicateStep, ScalarType, TypeShape};
use omega_typed::{CallUse, ObligationKind, ValueType};

#[test]
fn original_default_and_named_domain_calls_remain_on_the_declaring_owner() {
    for (source, callee, owner) in [
        (
            "machine allowed()->bool reaches none crashes none terminates {true}
                  domain i32::Guarded requires true && allowed();",
            "allowed",
            "i32::Guarded",
        ),
        (
            "machine predicate()->bool reaches none crashes none terminates {true}
                  data Guarded requires true && predicate() {}",
            "predicate",
            "Guarded",
        ),
    ] {
        let files = [("guard.omg", source)];
        let input = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&input).unwrap();
        let call = prepared
            .typed()
            .calls
            .iter()
            .find(|c| c.site.file == "guard.omg")
            .unwrap();
        assert_eq!(call.use_site, CallUse::Fact);
        assert_eq!(call.caller, None);
        assert_eq!(call.machine.path, callee);
        assert_eq!(call.result, ValueType::Bool);
        assert!(call.operands.is_empty());
        assert!(prepared.typed().obligations.iter().all(|o| o.node != call.node));
        let package = sources::lower_prepared(&prepared, &files).unwrap();
        let (decl, clauses) = match (package.domains.as_slice(), package.data.as_slice()) {
            ([d], []) => (&d.declaration, &d.requires),
            ([], [d]) => (&d.declaration, &d.requires),
            _ => panic!("expected only the authored domain or data"),
        };
        let [helper] = package.machines.as_slice() else {
            panic!("only the helper has executable backing")
        };
        assert_eq!(helper.declaration.path, callee);
        assert_eq!(decl.path, owner);
        let [clause] = clauses.as_slice() else {
            panic!("expected the authored clause")
        };
        assert_eq!(
            clause.predicate,
            [
                PredicateStep::Boolean(true),
                PredicateStep::MachineApplication {
                    callee: helper.declaration.clone(),
                    operands: vec![],
                    result: ScalarType::Boolean,
                    requirements: 0..0,
                    progress: omega_psi::PredicateProgress::CheckPromise,
                    site: call.site.clone(),
                },
                PredicateStep::Apply(PredicateOperator::And)
            ]
        );
    }
}

#[test]
fn same_shape_fact_calls_keep_the_selected_helper_projected_actuals_and_each_premise() {
    let source = "machine first(a:u8,b:u8)->bool requires a != b requires b > 0 reaches none crashes none terminates {true}
        machine second(a:u8,b:u8)->bool requires a != b requires b > 0 reaches none crashes none terminates {false}
        data Pair requires first(self.right,self.left) && second(self.left,self.right) requires second(self.right,self.left) {left:u8;right:u8;}
        machine guarded(x:u8,y:u8)->u8 requires second(y,x) ensures first(result,y) {x}
        machine run(a:u8,b:u8)->u8 {guarded(b,a)}";
    let files = [("paths.omg", source)];
    let input = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&input).unwrap();
    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let machine = |path: &str| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == path)
            .unwrap()
    };
    let application = |name, spelling, actuals: [&str; 2], caller: Option<&str>| {
        let c = prepared
            .typed()
            .calls
            .iter()
            .find(|c| c.site.file == "paths.omg" && c.site.span.slice(source.as_bytes()) == Some(spelling))
            .unwrap();
        assert_eq!(c.use_site, CallUse::Fact);
        assert_eq!(c.machine.path, name);
        assert_eq!(c.caller.as_ref().map(|id| id.path.as_str()), caller);
        assert_eq!(c.result, ValueType::Bool);
        assert_eq!(c.operands.len(), 2);
        let sig = prepared
            .typed()
            .signatures
            .iter()
            .find(|sig| sig.machine == c.machine)
            .unwrap();
        for ((op, text), (span, ty)) in c.operands.iter().zip(actuals).zip(&sig.parameters) {
            assert_eq!(op.site.span.slice(source.as_bytes()), Some(text.as_bytes()));
            assert_eq!(op.parameter.span, *span);
            assert_eq!(*ty, ValueType::Integer { signed: false, width: Some(8) });
            if let Some(field) = text.strip_prefix("self.") {
                let (_, path) = prepared.typed().place_path(op.node);
                let [step] = path.as_slice() else { panic!("expected one field projection") };
                let Some(omega_typed::Projection::Field(id)) = &step.projection else {
                    panic!("expected a selected field")
                };
                assert_eq!(id.path, format!("Pair::{field}"));
                assert!(step.through.is_none());
            }
        }
        assert_eq!(
            prepared
                .typed()
                .obligations
                .iter()
                .filter(|o| o.node == c.node && o.site == c.site && matches!(&o.kind, ObligationKind::CallPrecondition(id) if id == &c.machine))
                .count(),
            1
        );
        let helper = machine(name);
        let u8_ty = ScalarType::Integer { signed: false, width: 8 };
        for ty in &helper.parameter_types {
            assert_eq!(package.types.get(*ty), Some(&TypeShape::PrimitiveScalar(u8_ty)));
        }
        assert_eq!(
            package.types.get(helper.result_type.unwrap()),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean))
        );
        let [one, two] = helper.contract.requires.as_slice() else {
            panic!("expected two premises")
        };
        assert_eq!(
            one.predicate,
            [
                PredicateStep::Formal(0),
                PredicateStep::Formal(1),
                PredicateStep::Apply(PredicateOperator::NotEqual)
            ]
        );
        assert_eq!(
            two.predicate,
            [
                PredicateStep::Formal(1),
                PredicateStep::Integer(0.into()),
                PredicateStep::Apply(PredicateOperator::GreaterThan)
            ]
        );
        PredicateStep::MachineApplication {
            callee: helper.declaration.clone(),
            operands: vec![u8_ty; 2],
            result: ScalarType::Boolean,
            requirements: 0..2,
            progress: omega_psi::PredicateProgress::CheckPromise,
            site: c.site.clone(),
        }
    };
    use PredicateStep::{Apply, Field, Formal, Result, SelfValue};
    let [pair] = package.data.as_slice() else { panic!("expected Pair") };
    assert_eq!(pair.declaration.path, "Pair");
    let [left, right] = pair.fields.as_slice() else {
        panic!("expected two fields")
    };
    assert_eq!((left.declaration.path.as_str(), right.declaration.path.as_str()), ("Pair::left", "Pair::right"));
    let left = Field(left.declaration.clone());
    let right = Field(right.declaration.clone());
    assert_eq!(
        pair.requires[0].predicate,
        [
            SelfValue,
            right.clone(),
            SelfValue,
            left.clone(),
            application("first", b"first(self.right,self.left)", ["self.right", "self.left"], None),
            SelfValue,
            left.clone(),
            SelfValue,
            right.clone(),
            application("second", b"second(self.left,self.right)", ["self.left", "self.right"], None),
            Apply(PredicateOperator::And)
        ]
    );
    // Hold both projected arguments fixed and change only the selected helper.
    spec_corpus::observe_handoff!(
        psi "psi-output/predicate-call-data-field-actuals",
        assert_eq!(pair.requires[1].predicate, [SelfValue, right, SelfValue, left,
            application("second", b"second(self.right,self.left)", ["self.right", "self.left"], None)]),
    );
    let guarded = machine("guarded");
    assert_eq!(
        guarded.contract.requires[0].predicate,
        [
            Formal(1),
            Formal(0),
            application("second", b"second(y,x)", ["y", "x"], Some("guarded"))
        ]
    );
    assert_eq!(
        guarded.contract.ensures[0].predicate,
        [
            Result,
            Formal(1),
            application("first", b"first(result,y)", ["result", "y"], Some("guarded"))
        ]
    );
    assert!(guarded.body.operations.is_empty());
    // The executable caller still owes the outer clause at its operation.
    let run = machine("run");
    let [Parameter::Scalar(a), Parameter::Scalar(b)] = run.body.parameters.as_slice() else {
        panic!("expected a,b")
    };
    let [op] = run.body.operations.as_slice() else {
        panic!("expected an executable call")
    };
    assert_eq!(
        op.kind,
        OperationKind::Call {
            callee: CallTarget::Machine(guarded.id),
            arguments: vec![CallArgument::Scalar(*b), CallArgument::Scalar(*a)],
            requirements: vec![CallRequirement {
                site: CallSite::Operation(op.id),
                clause: 0
            }]
        }
    );
}

#[test]
fn named_domain_substitutes_its_subject_inside_the_helper_application() {
    let source = "machine allowed(x:u8)->bool requires x > 0 reaches none crashes none terminates {true}
                  domain u8::Guarded requires allowed(self);
                  machine forward(x:u8 in Guarded)->u8 in Guarded {x}";
    let package = sources::lower(&[("domain.omg", source)]).unwrap();
    let [domain] = package.domains.as_slice() else { panic!("expected Guarded") };
    let [allowed, forward] = package.machines.as_slice() else {
        panic!("expected allowed and forward")
    };
    assert_eq!(allowed.declaration.path, "allowed");
    assert_eq!(forward.declaration.path, "forward");
    assert_eq!(domain.declaration.path, "u8::Guarded");
    let [
        PredicateStep::SelfValue,
        call @ PredicateStep::MachineApplication {
            callee,
            operands,
            result,
            requirements,
            progress: omega_psi::PredicateProgress::CheckPromise,
            site,
        },
    ] = domain.requires[0].predicate.as_slice()
    else {
        panic!("expected call on domain subject")
    };
    assert_eq!(callee, &allowed.declaration);
    assert_eq!(*operands, [ScalarType::Integer { signed: false, width: 8 }]);
    assert_eq!(*result, ScalarType::Boolean);
    assert_eq!(*requirements, 0..1);
    assert_eq!(site.span.slice(source.as_bytes()), Some(b"allowed(self)".as_slice()));
    spec_corpus::observe_handoff!(
        psi "psi-output/predicate-call-domain-substitution",
        assert_eq!(forward.contract.requires[0].predicate, [PredicateStep::Formal(0), call.clone()]),
    );
    assert_eq!(forward.contract.ensures[0].predicate, [PredicateStep::Result, call.clone()]);
}

#[test]
fn a_published_crash_route_prevents_a_total_predicate_application() {
    // The former CLI refusal now lowers. Change only the helper's published crash ceiling.
    let good = "machine predicate()->bool reaches none crashes none terminates {true}\r\ndomain i32::Guarded requires true && predicate();";
    let package = sources::lower(&[("case.omg", good)]).unwrap();
    assert!(
        matches!(&package.domains[0].requires[0].predicate[..], [PredicateStep::Boolean(true), PredicateStep::MachineApplication { callee, .. }, PredicateStep::Apply(PredicateOperator::And)] if callee == &package.machines[0].declaration)
    );
    let source = &good.replace("crashes none", "crashes Trap");
    sources::refuse(
        &[("case.omg", source)],
        "case.omg",
        sources::span_within(source, "&& predicate()", "predicate()"),
        "predicate application requires a terminating helper with an empty crash ceiling",
    );
}

#[test]
fn runtime_selector_inside_a_fact_call_still_refuses_at_that_selector() {
    let source = "machine check(x:u8)->bool reaches none crashes none terminates {true}
                  machine guarded(a:[u8;2], i:u64) requires check(a[i]) {}";
    sources::refuse(
        &[("paths.omg", source)],
        "paths.omg",
        sources::span_within(source, "a[i]", "i"),
        "contract index requires a fixed selector",
    );
}
