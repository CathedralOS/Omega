//! Guard case tests that also bind the selected case's payload.
//!
//! Terminal has no case-qualified place read. A sum payload is observable only
//! as a parameter of the block a `StructuralCase` terminator selected for that
//! case, where the verifier reconstructs `parameter == root.Case.field` (and
//! the field's declared range) from its own tag dispatch. A guard such as
//! `Msg::Move { dx as step, .. } if step == 30` is lowered as the ordinary
//! short-circuit decision `m in Msg::Move && m.Move.dx == 30`; this planner
//! walks that decision and turns each root case test with an empty path into a
//! dispatch whose selected successor binds the case's scalar payload fields.
//! Deferred reads below that successor become those parameters; any other
//! payload read has no dominating case knowledge and is refused.
//!
//! The dispatch neither consumes nor copies the root: every unselected case
//! takes the test's false outcome, so the decision's meaning is the original
//! membership test. Payload parameters stay valid exactly where the selected
//! block dominates, which the verifier checks as ordinary SSA dominance; the
//! planner additionally reports the payloads established on every path to the
//! guard's true outcome so the selected edge's arguments may read them.
use crate::emission::boolean_control::LoweredBooleanDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, value_id};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralFieldId,
};
use terminal_psi::{StructuralCaseDeclaration, StructuralFieldType, ValueDeclaration};

/// One case test realized as a tag dispatch on a whole sum root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaseDispatch {
    pub(crate) source: PlaceId,
    pub(crate) selected: StructuralCaseId,
    /// Every declared case in declaration order, as the terminator requires.
    pub(crate) cases: Vec<StructuralCaseId>,
    /// Payload fields bound as the selected block's parameters, in order.
    pub(crate) payloads: Vec<(StructuralFieldId, ValueDeclaration)>,
}

pub(crate) struct PlannedGuard {
    pub(crate) decision: LoweredBooleanDecision,
    /// Payloads bound on every path to the guard's true outcome.
    pub(crate) established: Vec<EstablishedCasePayload>,
}

/// Plan the dispatches of one guard decision. `declared_cases` answers the
/// exact declared sum roster of a whole structural root; payload parameters
/// are appended to `namespace`, the scalar namespace the decision blocks and
/// the selected edge share.
pub(crate) fn plan<'a>(
    decision: LoweredBooleanDecision,
    declared_cases: &impl Fn(PlaceId) -> Option<&'a [StructuralCaseDeclaration]>,
    namespace: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
) -> Result<PlannedGuard, LoweringError> {
    let decision = plan_decision(decision, declared_cases, namespace, next_value)?;
    let established = established_on_true(&decision, namespace).unwrap_or_default();
    Ok(PlannedGuard {
        decision,
        established,
    })
}

fn plan_decision<'a>(
    decision: LoweredBooleanDecision,
    declared_cases: &impl Fn(PlaceId) -> Option<&'a [StructuralCaseDeclaration]>,
    namespace: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
) -> Result<LoweredBooleanDecision, LoweringError> {
    match decision {
        LoweredBooleanDecision::Value(expression) => {
            if boolean_reads_case_payload(&expression) {
                return unsupported("case payload observation requires an established case");
            }
            Ok(LoweredBooleanDecision::Value(expression))
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            if boolean_reads_case_payload(&condition) {
                return unsupported("case payload observation requires an established case");
            }
            let LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case } =
                &condition
            else {
                return Ok(LoweredBooleanDecision::Test {
                    condition,
                    when_true: Box::new(plan_decision(
                        *when_true,
                        declared_cases,
                        namespace,
                        next_value,
                    )?),
                    when_false: Box::new(plan_decision(
                        *when_false,
                        declared_cases,
                        namespace,
                        next_value,
                    )?),
                });
            };
            // Only a whole root has a dispatch terminator, and only a test
            // whose true outcome reads that case's payload needs one; every
            // other test keeps its ordinary membership observation.
            let declared = if path.is_empty() && decision_reads_case(&when_true, *source, *case) {
                declared_cases(*source)
            } else {
                None
            };
            let Some(declared) = declared else {
                return Ok(LoweredBooleanDecision::Test {
                    when_true: Box::new(plan_decision(
                        *when_true,
                        declared_cases,
                        namespace,
                        next_value,
                    )?),
                    when_false: Box::new(plan_decision(
                        *when_false,
                        declared_cases,
                        namespace,
                        next_value,
                    )?),
                    condition,
                });
            };
            let (source, selected) = (*source, *case);
            let selected_case = declared
                .iter()
                .find(|declared| declared.id == selected)
                .ok_or(LoweringError::Unsupported(
                    "case dispatch selects a case outside its root's sum",
                ))?;
            // The true outcome reads the payload, so it is a test with its own
            // selected block for the payload parameters to enter.
            let mut payloads = Vec::new();
            for field in &selected_case.fields {
                let scalar_type = match field.field_type {
                    StructuralFieldType::Scalar(scalar) => scalar,
                    StructuralFieldType::BoundedInteger(integer) => {
                        semantic_vocabulary::ScalarType::Integer(integer.integer_type())
                    }
                    _ => continue,
                };
                if field.relevance.is_erased() {
                    continue;
                }
                payloads.push((
                    field.id,
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type,
                    },
                ));
            }
            let bound = payloads
                .iter()
                .map(|(field, value)| {
                    namespace.push(*value);
                    EstablishedCasePayload {
                        source,
                        case: selected,
                        field: *field,
                        position: namespace.len() - 1,
                    }
                })
                .collect::<Vec<_>>();
            let when_true = substitute_decision(*when_true, &bound);
            Ok(LoweredBooleanDecision::CaseDispatch {
                dispatch: CaseDispatch {
                    source,
                    selected,
                    cases: declared.iter().map(|declared| declared.id).collect(),
                    payloads,
                },
                when_true: Box::new(plan_decision(
                    when_true,
                    declared_cases,
                    namespace,
                    next_value,
                )?),
                when_false: Box::new(plan_decision(
                    *when_false,
                    declared_cases,
                    namespace,
                    next_value,
                )?),
            })
        }
        LoweredBooleanDecision::CaseDispatch { .. } => {
            unsupported("guard case dispatch was planned twice")
        }
    }
}

/// Payloads bound on every path reaching a true outcome, or `None` when no
/// path does. Rows name exact namespace positions, so an intersection keeps
/// only a binding whose single dispatch dominates every true outcome.
fn established_on_true(
    decision: &LoweredBooleanDecision,
    namespace: &[ValueDeclaration],
) -> Option<Vec<EstablishedCasePayload>> {
    let intersect = |left: Option<Vec<EstablishedCasePayload>>,
                     right: Option<Vec<EstablishedCasePayload>>| match (
        left, right,
    ) {
        (Some(left), Some(right)) => {
            Some(left.into_iter().filter(|row| right.contains(row)).collect())
        }
        (left, None) => left,
        (None, right) => right,
    };
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
            value.then(Vec::new)
        }
        // A non-constant value outcome is not a guard exit; keep nothing.
        LoweredBooleanDecision::Value(_) => Some(Vec::new()),
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => intersect(
            established_on_true(when_true, namespace),
            established_on_true(when_false, namespace),
        ),
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true,
            when_false,
        } => {
            let selected = established_on_true(when_true, namespace).map(|mut rows| {
                rows.extend(dispatch.payloads.iter().map(|(field, value)| {
                    EstablishedCasePayload {
                        source: dispatch.source,
                        case: dispatch.selected,
                        field: *field,
                        position: namespace
                            .iter()
                            .position(|candidate| candidate.id == value.id)
                            .expect("planned payload parameters join the guard namespace"),
                    }
                }));
                rows
            });
            intersect(selected, established_on_true(when_false, namespace))
        }
    }
}

fn substitute_decision(
    decision: LoweredBooleanDecision,
    bound: &[EstablishedCasePayload],
) -> LoweredBooleanDecision {
    if bound.is_empty() {
        return decision;
    }
    match decision {
        LoweredBooleanDecision::Value(expression) => {
            LoweredBooleanDecision::Value(substitute_boolean(expression, bound))
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => LoweredBooleanDecision::Test {
            condition: substitute_boolean(condition, bound),
            when_true: Box::new(substitute_decision(*when_true, bound)),
            when_false: Box::new(substitute_decision(*when_false, bound)),
        },
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true,
            when_false,
        } => LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true: Box::new(substitute_decision(*when_true, bound)),
            when_false: Box::new(substitute_decision(*when_false, bound)),
        },
    }
}

fn bound_position(
    bound: &[EstablishedCasePayload],
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<usize> {
    let [CanonicalStructuralPathSegment::Case(case)] = path else {
        return None;
    };
    bound
        .iter()
        .find(|row| row.source == source && row.case == *case && row.field == field)
        .map(|row| row.position)
}

fn substitute_boolean(
    expression: LoweredBooleanReturnExpression,
    bound: &[EstablishedCasePayload],
) -> LoweredBooleanReturnExpression {
    use LoweredBooleanReturnExpression as Boolean;
    match expression {
        Boolean::StructuralField {
            source,
            path,
            field,
        } => match bound_position(bound, source, &path, field) {
            Some(position) => Boolean::Parameter { position },
            None => Boolean::StructuralField {
                source,
                path,
                field,
            },
        },
        Boolean::Not { operand } => Boolean::Not {
            operand: Box::new(substitute_boolean(*operand, bound)),
        },
        Boolean::Equal { left, right } => Boolean::Equal {
            left: Box::new(substitute_boolean(*left, bound)),
            right: Box::new(substitute_boolean(*right, bound)),
        },
        Boolean::And { left, right } => Boolean::And {
            left: Box::new(substitute_boolean(*left, bound)),
            right: Box::new(substitute_boolean(*right, bound)),
        },
        Boolean::Or { left, right } => Boolean::Or {
            left: Box::new(substitute_boolean(*left, bound)),
            right: Box::new(substitute_boolean(*right, bound)),
        },
        Boolean::IntegerComparison { kind, left, right } => Boolean::IntegerComparison {
            kind,
            left: Box::new(substitute_direct(*left, bound)),
            right: Box::new(substitute_direct(*right, bound)),
        },
        expression @ (Boolean::StructuralCaseMembership { .. }
        | Boolean::PrimitiveRead { .. }
        | Boolean::Constant { .. }
        | Boolean::Parameter { .. }
        | Boolean::Local { .. }
        | Boolean::UnresolvedStructuralParameterField { .. }) => expression,
    }
}

pub(crate) fn substitute_direct(
    expression: LoweredDirectExpression,
    bound: &[EstablishedCasePayload],
) -> LoweredDirectExpression {
    use LoweredDirectExpression as Direct;
    match expression {
        Direct::StructuralField {
            source,
            path,
            field,
            scalar_type,
        } => match bound_position(bound, source, &path, field) {
            Some(position) => Direct::Parameter {
                position,
                scalar_type,
            },
            None => Direct::StructuralField {
                source,
                path,
                field,
                scalar_type,
            },
        },
        Direct::ByteSequenceRead {
            source,
            index,
            scalar_type,
        } => Direct::ByteSequenceRead {
            source,
            index: Box::new(substitute_direct(*index, bound)),
            scalar_type,
        },
        Direct::ElementViewRead {
            source,
            index,
            scalar_type,
        } => Direct::ElementViewRead {
            source,
            index: Box::new(substitute_direct(*index, bound)),
            scalar_type,
        },
        Direct::ByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            scalar_type,
        } => Direct::ByteSequenceFieldRead {
            source,
            path,
            field,
            index: Box::new(substitute_direct(*index, bound)),
            scalar_type,
        },
        Direct::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => Direct::IntegerBinary {
            kind,
            scalar_type,
            left: Box::new(substitute_direct(*left, bound)),
            right: Box::new(substitute_direct(*right, bound)),
        },
        Direct::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => Direct::IntegerBitwiseNot {
            scalar_type,
            operand: Box::new(substitute_direct(*operand, bound)),
        },
        Direct::IntegerWiden {
            scalar_type,
            operand,
        } => Direct::IntegerWiden {
            scalar_type,
            operand: Box::new(substitute_direct(*operand, bound)),
        },
        Direct::IntegerExactCast {
            scalar_type,
            operand,
        } => Direct::IntegerExactCast {
            scalar_type,
            operand: Box::new(substitute_direct(*operand, bound)),
        },
        Direct::Boolean { expression } => Direct::Boolean {
            expression: Box::new(substitute_boolean(*expression, bound)),
        },
        expression @ (Direct::PrimitiveRead { .. }
        | Direct::ByteSequenceLength { .. }
        | Direct::ElementViewLength { .. }
        | Direct::ByteSequenceFieldLength { .. }
        | Direct::Parameter { .. }
        | Direct::ErasedParameter { .. }
        | Direct::Local { .. }
        | Direct::IntegerLiteral { .. }
        | Direct::IeeeFloatLiteral { .. }) => expression,
    }
}

/// Every case-qualified read `(root, path)` still awaiting a dispatch.
fn case_reads<'e>(
    expression: &'e LoweredBooleanReturnExpression,
    reads: &mut Vec<(PlaceId, &'e [CanonicalStructuralPathSegment])>,
) {
    use LoweredBooleanReturnExpression as Boolean;
    match expression {
        Boolean::StructuralField { source, path, .. } | Boolean::PrimitiveRead { source, path } => {
            if path
                .iter()
                .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
            {
                reads.push((*source, path));
            }
        }
        Boolean::Not { operand } => case_reads(operand, reads),
        Boolean::Equal { left, right }
        | Boolean::And { left, right }
        | Boolean::Or { left, right } => {
            case_reads(left, reads);
            case_reads(right, reads);
        }
        Boolean::IntegerComparison { left, right, .. } => {
            collect_direct_case_reads(left, reads);
            collect_direct_case_reads(right, reads);
        }
        Boolean::StructuralCaseMembership { .. }
        | Boolean::Constant { .. }
        | Boolean::Parameter { .. }
        | Boolean::Local { .. }
        | Boolean::UnresolvedStructuralParameterField { .. } => {}
    }
}

/// Whether any lowered direct expression still observes a bound payload of
/// `case` on `source` — the scalar-graph dispatch admission check.
pub(crate) fn direct_case_reads(
    expressions: &[LoweredDirectExpression],
    source: PlaceId,
    case: StructuralCaseId,
) -> bool {
    expressions.iter().any(|expression| {
        let mut reads = Vec::new();
        collect_direct_case_reads(expression, &mut reads);
        reads.iter().any(|(root, path)| {
            *root == source && path.first() == Some(&CanonicalStructuralPathSegment::Case(case))
        })
    })
}

fn collect_direct_case_reads<'e>(
    expression: &'e LoweredDirectExpression,
    reads: &mut Vec<(PlaceId, &'e [CanonicalStructuralPathSegment])>,
) {
    use LoweredDirectExpression as Direct;
    match expression {
        Direct::StructuralField { source, path, .. }
        | Direct::PrimitiveRead { source, path, .. } => {
            if path
                .iter()
                .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
            {
                reads.push((*source, path));
            }
        }
        Direct::ByteSequenceRead { index, .. }
        | Direct::ElementViewRead { index, .. }
        | Direct::ByteSequenceFieldRead { index, .. } => collect_direct_case_reads(index, reads),
        Direct::IntegerBinary { left, right, .. } => {
            collect_direct_case_reads(left, reads);
            collect_direct_case_reads(right, reads);
        }
        Direct::IntegerBitwiseNot { operand, .. }
        | Direct::IntegerWiden { operand, .. }
        | Direct::IntegerExactCast { operand, .. } => collect_direct_case_reads(operand, reads),
        Direct::Boolean { expression } => case_reads(expression, reads),
        Direct::ByteSequenceLength { .. }
        | Direct::ByteSequenceFieldLength { .. }
        | Direct::ElementViewLength { .. }
        | Direct::Parameter { .. }
        | Direct::ErasedParameter { .. }
        | Direct::Local { .. }
        | Direct::IntegerLiteral { .. }
        | Direct::IeeeFloatLiteral { .. } => {}
    }
}

fn boolean_reads_case_payload(expression: &LoweredBooleanReturnExpression) -> bool {
    let mut reads = Vec::new();
    case_reads(expression, &mut reads);
    !reads.is_empty()
}

/// Whether a still-deferred read below this outcome observes `case` of `source`.
fn decision_reads_case(
    decision: &LoweredBooleanDecision,
    source: PlaceId,
    case: StructuralCaseId,
) -> bool {
    let reads_case = |expression: &LoweredBooleanReturnExpression| {
        let mut reads = Vec::new();
        case_reads(expression, &mut reads);
        reads.iter().any(|(root, path)| {
            *root == source && path.first() == Some(&CanonicalStructuralPathSegment::Case(case))
        })
    };
    match decision {
        LoweredBooleanDecision::Value(expression) => reads_case(expression),
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            reads_case(condition)
                || decision_reads_case(when_true, source, case)
                || decision_reads_case(when_false, source, case)
        }
        LoweredBooleanDecision::CaseDispatch {
            when_true,
            when_false,
            ..
        } => {
            decision_reads_case(when_true, source, case)
                || decision_reads_case(when_false, source, case)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseDispatch, plan};
    use crate::emission::boolean_control::LoweredBooleanDecision;
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, PlaceId,
        ScalarType, StructuralCaseId, StructuralFieldId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType,
    };

    fn integer() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"))
    }

    fn roster() -> Vec<StructuralCaseDeclaration> {
        let field = |id, identity: &str| StructuralFieldDeclaration {
            id: StructuralFieldId::new(id).unwrap(),
            identity: identity.into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(integer()),
        };
        vec![
            StructuralCaseDeclaration {
                id: StructuralCaseId::new(1).unwrap(),
                identity: "Move".into(),
                fields: vec![field(1, "dx"), field(2, "dy")],
            },
            StructuralCaseDeclaration {
                id: StructuralCaseId::new(2).unwrap(),
                identity: "Stop".into(),
                fields: vec![field(3, "code")],
            },
        ]
    }

    fn outcome(value: bool) -> LoweredBooleanDecision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value })
    }

    /// `root in tested && root.Move.dx == 30`, with the read on `read_when_true`.
    fn guard(tested: u64, read_when_true: bool) -> LoweredBooleanDecision {
        let root = PlaceId::new(7).unwrap();
        let read = LoweredBooleanDecision::Test {
            condition: LoweredBooleanReturnExpression::IntegerComparison {
                kind: LoweredIntegerComparisonKind::Equal,
                left: Box::new(LoweredDirectExpression::StructuralField {
                    source: root,
                    path: vec![CanonicalStructuralPathSegment::Case(
                        StructuralCaseId::new(1).unwrap(),
                    )],
                    field: StructuralFieldId::new(1).unwrap(),
                    scalar_type: integer(),
                }),
                right: Box::new(LoweredDirectExpression::IntegerLiteral {
                    value: IntegerValue::Signed(30),
                    scalar_type: integer(),
                }),
            },
            when_true: Box::new(outcome(true)),
            when_false: Box::new(outcome(false)),
        };
        let (when_true, when_false) = if read_when_true {
            (read, outcome(false))
        } else {
            (outcome(false), read)
        };
        LoweredBooleanDecision::Test {
            condition: LoweredBooleanReturnExpression::StructuralCaseMembership {
                source: root,
                path: Vec::new(),
                case: StructuralCaseId::new(tested).unwrap(),
            },
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        }
    }

    #[test]
    fn selected_case_binds_its_payload_for_the_read_and_the_true_outcome() {
        let cases = roster();
        let mut namespace = Vec::new();
        let mut next_value = 100;
        let planned = plan(
            guard(1, true),
            &|_| Some(cases.as_slice()),
            &mut namespace,
            &mut next_value,
        )
        .expect("the read sits below its own case test");
        let LoweredBooleanDecision::CaseDispatch {
            dispatch: CaseDispatch {
                selected, payloads, ..
            },
            when_true,
            ..
        } = &planned.decision
        else {
            panic!("the case test became a dispatch: {:?}", planned.decision);
        };
        assert_eq!(*selected, StructuralCaseId::new(1).unwrap());
        assert_eq!(payloads.len(), 2);
        let LoweredBooleanDecision::Test {
            condition: LoweredBooleanReturnExpression::IntegerComparison { left, .. },
            ..
        } = when_true.as_ref()
        else {
            panic!("the payload comparison survives");
        };
        assert!(matches!(
            left.as_ref(),
            LoweredDirectExpression::Parameter { position: 0, .. }
        ));
        assert_eq!(
            planned
                .established
                .iter()
                .map(|row| row.position)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn payload_read_without_its_case_established_is_refused() {
        let cases = roster();
        // Another case of the same sum, and the tested case's false outcome,
        // both reach the `Move` read without `Move` selected.
        for (tested, read_when_true) in [(2, true), (1, false)] {
            let error = plan(
                guard(tested, read_when_true),
                &|_| Some(cases.as_slice()),
                &mut Vec::new(),
                &mut 100,
            )
            .err()
            .expect("an unestablished payload read is refused");
            assert_eq!(
                format!("{error:?}"),
                "Unsupported(\"case payload observation requires an established case\")"
            );
        }
    }
}
