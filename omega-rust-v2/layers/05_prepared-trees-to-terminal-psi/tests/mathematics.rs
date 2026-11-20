#[path = "support/source.rs"]
mod sources;

use MathematicalStep::{Apply, Declaration, FormProposition, Parameter, Predicate};
use TypeExpressionStep::{Function, GenericParameter, Level, LevelKind, Universe};
use omega_lowering::{MathematicalBody, MathematicalStep, PredicateOperator, PredicateStep, TypeExpressionStep};

#[test]
fn transparent_definitions_retain_unfoldable_terms_and_universe_bounds() {
    let text = "let greater_than(limit:i32,value:i32):core\\Strict<0> = value > limit;
                let compose<u:core\\Level,v:core\\Level,w:core\\Level,A:core\\Type<u>,B:core\\Type<v>,C:core\\Type<w>>
                    (f:B -> C,g:A -> B,value:A):C=f(g(value));";
    let package = sources::lower(&[("math.omg", text)]).unwrap();
    assert!(package.machines.is_empty());
    let [compose, greater] = package.mathematics.as_slice() else {
        panic!("expected both definitions")
    };
    assert_eq!(compose.parameter_count, 3);
    assert_eq!(greater.parameter_count, 2);
    assert!(compose.assumptions.is_empty() && greater.assumptions.is_empty());
    let MathematicalBody::Definition(composed) = &compose.body else {
        panic!("expected composition body")
    };
    let apply = Apply {
        generic_arguments: vec![],
        argument_count: 1,
        assumptions: vec![],
    };
    assert_eq!(composed, &[Parameter(0), Parameter(1), Parameter(2), apply.clone(), apply]);
    let bounds = &compose.generic_bounds;
    assert_eq!(bounds.len(), 6);
    let slot = |index| GenericParameter {
        owner: compose.declaration.clone(),
        index,
    };
    for index in 0..3 {
        assert_eq!(bounds[index].0, [LevelKind]);
        assert_eq!(bounds[index + 3].0, [slot(index), Universe { strict: false }]);
    }
    assert_eq!(
        compose.signature.0,
        [
            slot(4),
            slot(5),
            Function,
            slot(3),
            slot(4),
            Function,
            slot(3),
            slot(5),
            Function,
            Function,
            Function
        ]
    );
    let MathematicalBody::Definition(comparison) = &greater.body else {
        panic!("expected a strict proposition")
    };
    assert_eq!(
        comparison,
        &[
            Parameter(1),
            Parameter(0),
            Predicate(PredicateOperator::GreaterThan),
            FormProposition(omega_psi::TypeExpression(vec![Level(0u8.into()), Universe { strict: true }]))
        ]
    );
}

#[test]
fn a_boundary_assumption_keeps_its_exact_identity_through_a_definition_and_proof_use() {
    let definitions = "boundary let trusted(x:u8):u8; boundary let other(x:u8):u8; let wrap(x:u8):u8=trusted(x);";
    let text = format!("{definitions} machine claim(x:u8) ensures wrap(x)==x {{}}");
    let package = sources::lower(&[("trust.omg", &text)]).unwrap();
    let [other, trusted, wrap] = package.mathematics.as_slice() else {
        panic!("expected two assumptions and one definition")
    };
    assert_ne!(other.declaration, trusted.declaration);
    assert_eq!(trusted.body, MathematicalBody::BoundaryAssumption);
    assert_eq!(trusted.assumptions, std::slice::from_ref(&trusted.declaration));
    assert_eq!(wrap.assumptions, trusted.assumptions);
    let application = Apply {
        generic_arguments: vec![],
        argument_count: 1,
        assumptions: trusted.assumptions.clone(),
    };
    assert_eq!(
        wrap.body,
        MathematicalBody::Definition(vec![
            Declaration(trusted.declaration.clone()),
            Parameter(0),
            application.clone()
        ])
    );
    let [claim] = package.machines.as_slice() else {
        panic!("only the authored machine should have a runtime body")
    };
    assert_eq!(
        claim.contract.ensures[0].predicate,
        [
            PredicateStep::MathematicalTerm(vec![Declaration(wrap.declaration.clone()), Parameter(0), application]),
            PredicateStep::Formal(0),
            PredicateStep::Apply(PredicateOperator::Equal)
        ]
    );
    let executable = format!("{definitions} machine claim(x:u8)->u8 {{wrap(x)}}");
    let source = sources::resolve(&[("trust.omg", &executable)]);
    let error = omega_preparation::prepare_source_package(&source).unwrap_err();
    assert!(
        error
            .message
            .contains("mathematical assumption cannot supply executable data or control"),
        "{error:?}"
    );
}

#[test]
fn choice_preserves_the_dependent_interface_without_a_runtime_body() {
    let text = "boundary let choose<u:core\\Level,A:core\\Type<u>>(inhabited:core\\Squash<A>):A;";
    let package = sources::lower(&[("choose.omg", text)]).unwrap();
    let [choose] = package.mathematics.as_slice() else {
        panic!("expected the choice declaration")
    };
    assert_eq!(choose.body, MathematicalBody::BoundaryAssumption);
    let a = GenericParameter {
        owner: choose.declaration.clone(),
        index: 1,
    };
    assert_eq!(choose.signature.0, [a.clone(), TypeExpressionStep::Squash, a, Function]);
    assert!(package.machines.is_empty());
}
