//! Owner tests for structural crash predicates, not fabricated source evidence.
use super::{
    CheckedBooleanExpression, LoweringError, ScalarType, TerminalCrashCause, ValueDeclaration,
    lower_structural_crash_route_buckets,
};
use crate::proofs::{
    IeeeFloatFormat, PrimitiveType, Proposition, StructuralFieldType,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    integer_scalar_type,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{StructuralAccess, StructuralFieldDeclaration, StructuralMultiplicity};

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).unwrap()
}

fn path(name: &str) -> Vec<checked_trees::CheckedStructuralPredicatePathSegment> {
    vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
        name.into(),
    )]
}

fn field(name: &str) -> CheckedBooleanExpression {
    CheckedBooleanExpression::StructuralParameterField {
        parameter_position: 2,
        path: path(name),
    }
}

fn and(
    left: CheckedBooleanExpression,
    right: CheckedBooleanExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn equal(
    left: CheckedBooleanExpression,
    right: CheckedBooleanExpression,
) -> CheckedBooleanExpression {
    CheckedBooleanExpression::Equal {
        left: Box::new(left),
        right: Box::new(right),
    }
}

struct Fixture {
    scalars: Vec<ValueDeclaration>,
    parameters: Vec<StructuralParameterDeclaration>,
    types: Vec<StructuralTypeDeclaration>,
}

impl Fixture {
    fn new() -> Self {
        let fields = [
            ("flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
            ("other", StructuralFieldType::Scalar(ScalarType::Boolean)),
            (
                "count",
                StructuralFieldType::Scalar(integer_scalar_type(PrimitiveType::I32).unwrap()),
            ),
            (
                "first",
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            ),
            (
                "second",
                StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, field_type))| StructuralFieldDeclaration {
            id: id(index as u64 + 1),
            identity: name.into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type,
        })
        .collect();
        Self {
            scalars: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: id(7),
                scalar_type: ScalarType::Boolean,
            }],
            parameters: vec![StructuralParameterDeclaration {
                place: id(11),
                position: 2,
                is_self: false,
                structural_type: id(3),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            types: vec![StructuralTypeDeclaration {
                id: id(3),
                identity: "test::Record".into(),
                shape: StructuralTypeShape::Record { fields },
            }],
        }
    }

    fn lower(&self, expression: CheckedBooleanExpression) -> Result<Proposition, LoweringError> {
        // This unit owns checked-to-terminal conversion only. The opaque
        // authored identity does not claim compilation or admission authority.
        let identity = checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
            checked_trees::CrashPredicateExpression::Parameter(0),
            expression,
        );
        let bucket = checked_trees::CrashRouteBucket::new(
            checked_trees::CrashCause::Trap,
            vec![checked_trees::CrashRouteGuard::Predicate(identity)],
        )
        .unwrap();
        let lowered = lower_structural_crash_route_buckets(
            &[bucket],
            &self.scalars,
            &self.parameters,
            &self.types,
            &[],
        )?;
        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].cause, TerminalCrashCause::Trap);
        let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
            lowered[0].alternatives.as_slice()
        else {
            panic!("one checked predicate remains one published alternative")
        };
        Ok(predicate.proposition().clone())
    }
}

#[test]
fn malformed_structural_and_scalar_operands_do_not_gain_equality_meaning() {
    let fixture = Fixture::new();
    for wrong in [
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path: path("flag"),
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 2,
            path: path("absent"),
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 2,
            path: Vec::new(),
        },
        field("count"),
        CheckedBooleanExpression::Parameter { position: 1 },
        CheckedBooleanExpression::StorageRead {
            symbol: symbols::SymbolHandle::invalid(),
        },
    ] {
        assert!(
            fixture
                .lower(equal(and(field("flag"), field("other")), wrong))
                .is_err()
        );
    }
    let mut wrong_type = Fixture::new();
    wrong_type.parameters[0].structural_type = id(99);
    assert!(wrong_type.lower(field("flag")).is_err());
    let mut wrong_scalar = Fixture::new();
    wrong_scalar.scalars[0].scalar_type = integer_scalar_type(PrimitiveType::I32).unwrap();
    assert!(
        wrong_scalar
            .lower(equal(
                and(field("flag"), field("other")),
                CheckedBooleanExpression::Parameter { position: 0 }
            ))
            .is_err()
    );
}

#[test]
fn structural_composition_has_one_depth_and_expansion_budget() {
    let fixture = Fixture::new();
    let mut deep = and(field("flag"), field("other"));
    for _ in 0..64 {
        deep = CheckedBooleanExpression::Not(Box::new(deep));
    }
    assert!(
        matches!(fixture.lower(deep), Err(LoweringError::Unsupported(message)) if message.contains("depth limit"))
    );
    let mut expensive = and(field("flag"), field("other"));
    for _ in 0..12 {
        expensive = equal(
            expensive,
            CheckedBooleanExpression::Parameter { position: 0 },
        );
    }
    assert!(
        matches!(fixture.lower(expensive), Err(LoweringError::Unsupported(message)) if message.contains("lowering budget"))
    );
}
