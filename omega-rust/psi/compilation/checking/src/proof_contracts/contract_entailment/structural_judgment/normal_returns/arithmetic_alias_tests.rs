//! The numeric consumer receives authenticated completed-value equations.
//! These tests start after call admission; source fixtures cover the producer,
//! actual argument capture, storage epochs and ordinary executable completion.

use super::{CapturedResultEquation, StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::name::Identifier;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

fn references(program: &mut TypedTrees) -> (TypeReferenceHandle, TypeReferenceHandle) {
    let exact = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::ArithmeticDomain(
                ArithmeticDomain::Wrapping,
            )]);
    let wrapping = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: exact,
            constraints,
        });
    (exact, wrapping)
}

fn field(name: &str) -> StructuralTerm {
    StructuralTerm::Projection {
        subject: Box::new(StructuralTerm::Variable("main".to_owned())),
        path: name.to_owned(),
    }
}

fn integer(value: i64) -> StructuralTerm {
    StructuralTerm::Integer(BigInt::from_i64(value))
}

fn compound(
    offset: StructuralTerm,
    exact: TypeReferenceHandle,
    wrapping: TypeReferenceHandle,
) -> StructuralTerm {
    StructuralTerm::ScalarBinary {
        operator: BinaryOperator::Add,
        references: [wrapping; 2],
        meaning: [(PrimitiveType::I32, ArithmeticDomain::Wrapping); 2],
        left: Box::new(StructuralTerm::ScalarBinary {
            operator: BinaryOperator::Multiply,
            references: [wrapping, exact],
            meaning: [
                (PrimitiveType::I32, ArithmeticDomain::Wrapping),
                (PrimitiveType::I32, ArithmeticDomain::Exact),
            ],
            left: Box::new(field("b")),
            right: Box::new(integer(20)),
        }),
        right: Box::new(offset),
    }
}

fn assume_bound(
    judge: &mut StructuralJudge<'_>,
    operator: BinaryOperator,
    value: StructuralTerm,
    limit: i64,
    exact: TypeReferenceHandle,
    wrapping: TypeReferenceHandle,
) {
    judge.intake_equation(
        StructuralTerm::ScalarBinary {
            operator,
            references: [wrapping, exact],
            meaning: [
                (PrimitiveType::I32, ArithmeticDomain::Wrapping),
                (PrimitiveType::I32, ArithmeticDomain::Exact),
            ],
            left: Box::new(value),
            right: Box::new(integer(limit)),
        },
        super::boolean_term(true),
        0,
    );
}

fn bound(
    judge: &StructuralJudge<'_>,
    value: StructuralTerm,
    wrapping: TypeReferenceHandle,
    operator: BinaryOperator,
    limit: i64,
) -> StructuralJudgment {
    judge.judge_observed_predicate(
        StructuralTerm::MathematicalBinary {
            operator,
            left: Box::new(StructuralTerm::IntegerEmbedding {
                reference: wrapping,
                value: Box::new(value),
            }),
            right: Box::new(integer(limit)),
        },
        0,
    )
}

fn captured(wrapping: TypeReferenceHandle) -> StructuralTerm {
    StructuralTerm::CapturedValue {
        expression: Default::default(),
        statement: Default::default(),
        callable: SymbolHandle::invalid(),
        occurrence: 1,
        result_type: wrapping,
        integer_range: None,
    }
}

#[test]
fn completed_wrapping_alias_preserves_compound_guard() {
    let mut program = TypedTrees::default();
    let (exact, wrapping) = references(&mut program);
    let mut judge = StructuralJudge::from_declaration_requires(
        &program,
        SymbolHandle::invalid(),
        &[],
        &[],
        false,
    );
    judge.runtime_body_values = true;
    let original = compound(field("a"), exact, wrapping);
    assume_bound(
        &mut judge,
        BinaryOperator::GreaterOrEqual,
        original.clone(),
        0,
        exact,
        wrapping,
    );
    assume_bound(
        &mut judge,
        BinaryOperator::Less,
        original.clone(),
        200,
        exact,
        wrapping,
    );
    assert!(matches!(
        bound(
            &judge,
            original.clone(),
            wrapping,
            BinaryOperator::Less,
            200
        ),
        StructuralJudgment::Proven
    ));
    assert!(matches!(
        bound(
            &judge,
            original,
            wrapping,
            BinaryOperator::GreaterOrEqual,
            0
        ),
        StructuralJudgment::Proven
    ));
    let result = captured(wrapping);
    judge.intake_normal_call_equations(vec![CapturedResultEquation::public((
        result.clone(),
        field("a"),
    ))]);
    assert!(matches!(
        bound(
            &judge,
            compound(result, exact, wrapping),
            wrapping,
            BinaryOperator::Less,
            200
        ),
        StructuralJudgment::Proven
    ));
}

#[test]
fn completed_wrapping_alias_keeps_exact_actual_and_required_upper_bound() {
    let mut program = TypedTrees::default();
    let (exact, wrapping) = references(&mut program);
    for (has_upper, actual) in [(true, "b"), (false, "a")] {
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            false,
        );
        judge.runtime_body_values = true;
        let original = compound(field("a"), exact, wrapping);
        assume_bound(
            &mut judge,
            BinaryOperator::GreaterOrEqual,
            original.clone(),
            0,
            exact,
            wrapping,
        );
        if has_upper {
            assume_bound(
                &mut judge,
                BinaryOperator::Less,
                original,
                200,
                exact,
                wrapping,
            );
        }
        let result = captured(wrapping);
        judge.intake_normal_call_equations(vec![CapturedResultEquation::public((
            result.clone(),
            field(actual),
        ))]);
        assert!(!matches!(
            bound(
                &judge,
                compound(result, exact, wrapping),
                wrapping,
                BinaryOperator::Less,
                200
            ),
            StructuralJudgment::Proven
        ));
    }
}

#[test]
fn completed_wrapping_alias_keeps_completed_operation_identity() {
    let mut program = TypedTrees::default();
    let (exact, wrapping) = references(&mut program);
    let mut judge = StructuralJudge::from_declaration_requires(
        &program,
        SymbolHandle::invalid(),
        &[],
        &[],
        false,
    );
    judge.runtime_body_values = true;
    let original = compound(field("a"), exact, wrapping);
    assume_bound(
        &mut judge,
        BinaryOperator::GreaterOrEqual,
        original.clone(),
        0,
        exact,
        wrapping,
    );
    assume_bound(
        &mut judge,
        BinaryOperator::Less,
        original.clone(),
        200,
        exact,
        wrapping,
    );
    let result = captured(wrapping);
    judge.intake_normal_call_equations(vec![
        CapturedResultEquation::public((result.clone(), field("a"))),
        CapturedResultEquation::completed_call(
            result.clone(),
            StructuralTerm::Application {
                result_type: wrapping,
                target: SymbolHandle::invalid(),
                selections: Vec::new(),
                machine: "coordinate".to_owned(),
                arguments: vec![field("a")],
            },
        ),
    ]);
    let current = compound(result, exact, wrapping);
    std::sync::Arc::make_mut(&mut judge.normal_results).push((Default::default(), original));
    std::sync::Arc::make_mut(&mut judge.normal_results).push((Default::default(), current.clone()));
    for (operator, limit) in [
        (BinaryOperator::GreaterOrEqual, 0),
        (BinaryOperator::Less, 200),
    ] {
        assert!(matches!(
            bound(&judge, current.clone(), wrapping, operator, limit),
            StructuralJudgment::Proven
        ));
    }
}

#[test]
fn completed_wrapping_alias_preserves_conjunctive_guard() {
    let mut program = TypedTrees::default();
    let (exact, wrapping) = references(&mut program);
    let boolean = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("bool"),
        });
    let mut judge = StructuralJudge::from_declaration_requires(
        &program,
        SymbolHandle::invalid(),
        &[],
        &[],
        false,
    );
    judge.runtime_body_values = true;
    let original = compound(field("a"), exact, wrapping);
    std::sync::Arc::make_mut(&mut judge.normal_results)
        .push((Default::default(), original.clone()));
    let comparison = |operator, limit| StructuralTerm::ScalarBinary {
        operator,
        references: [wrapping, exact],
        meaning: [
            (PrimitiveType::I32, ArithmeticDomain::Wrapping),
            (PrimitiveType::I32, ArithmeticDomain::Exact),
        ],
        left: Box::new(original.clone()),
        right: Box::new(integer(limit)),
    };
    judge.intake_equation(
        StructuralTerm::ScalarBinary {
            operator: BinaryOperator::And,
            references: [boolean; 2],
            meaning: [(PrimitiveType::Bool, ArithmeticDomain::Exact); 2],
            left: Box::new(comparison(BinaryOperator::GreaterOrEqual, 0)),
            right: Box::new(comparison(BinaryOperator::Less, 200)),
        },
        super::boolean_term(true),
        0,
    );
    for (operator, limit) in [
        (BinaryOperator::GreaterOrEqual, 0),
        (BinaryOperator::Less, 200),
    ] {
        let judgment = bound(&judge, original.clone(), wrapping, operator, limit);
        assert!(
            matches!(judgment, StructuralJudgment::Proven),
            "before call: {judgment:?}"
        );
    }
    let result = captured(wrapping);
    judge.intake_normal_call_equations(vec![
        CapturedResultEquation::public((result.clone(), field("a"))),
        CapturedResultEquation::completed_call(
            result.clone(),
            StructuralTerm::Application {
                result_type: wrapping,
                target: SymbolHandle::invalid(),
                selections: Vec::new(),
                machine: "coordinate".to_owned(),
                arguments: vec![field("a")],
            },
        ),
    ]);
    let current = compound(result, exact, wrapping);
    std::sync::Arc::make_mut(&mut judge.normal_results)
        .push((Default::default(), original.clone()));
    std::sync::Arc::make_mut(&mut judge.normal_results).push((Default::default(), current.clone()));
    for value in [original, current] {
        for (operator, limit) in [
            (BinaryOperator::GreaterOrEqual, 0),
            (BinaryOperator::Less, 200),
        ] {
            let judgment = bound(&judge, value.clone(), wrapping, operator, limit);
            assert!(
                matches!(judgment, StructuralJudgment::Proven),
                "{value:?}: {judgment:?}"
            );
        }
    }
}
