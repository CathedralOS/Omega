//! Guard case tests that also bind the selected case's payload.
//!
//! A whole root's sum payload is observed as a parameter of the block a
//! `StructuralCase` terminator selected for that case, where the verifier
//! reconstructs `parameter == root.Case.field` (and the field's declared
//! range) from its own tag dispatch. A sum below record fields has no
//! dispatch terminator: its membership test stays an ordinary observation and
//! its payload is read in place below that test's true edge (see
//! `structural_fields::resolve_typed`). A guard such as
//! `Msg::Move { dx as step, .. } if step == 30` is lowered as the ordinary
//! short-circuit decision `m in Msg::Move && m.Move.dx == 30`; this planner
//! walks that decision and turns each root case test with an empty path into a
//! dispatch whose selected successor binds the case's scalar payload fields.
//! Direct scalar reads below that successor become those parameters. Nested
//! structural payload observations retain their checked carrier-copy source,
//! including the selecting root and case. They execute below the same selected
//! edge; preparing a copy source does not itself establish case knowledge.
//!
//! The dispatch is a value source, not case authority. Checking proved the
//! active case of every payload read it admitted; a read whose case came from
//! an entry `requires` or another dominating fact rather than a test in this
//! decision stays an in-place observation, and the planner does not refuse it.
//!
//! The dispatch neither consumes nor copies the root: every unselected case
//! takes the test's false outcome, so the decision's meaning is the original
//! membership test. Payload parameters stay valid exactly where the selected
//! block dominates, which the verifier checks as ordinary SSA dominance; the
//! planner additionally reports the payloads established on every path to the
//! guard's true outcome so the selected edge's arguments may read them.
use crate::emission::boolean_control::LoweredBooleanDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, PrimitiveReadSource,
};
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::lowering_error::{LoweringError, unsupported};
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
///
/// `outcome_reads_payloads` says the successor the guard's true outcome
/// selects may read a case payload, as a transition argument does: a true leaf
/// then stands for that read, so a whole-root case test reaching it becomes a
/// dispatch whose payloads that successor can use.
pub(crate) fn plan<'a>(
    decision: LoweredBooleanDecision,
    declared_cases: &impl Fn(PlaceId) -> Option<&'a [StructuralCaseDeclaration]>,
    namespace: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    outcome_reads_payloads: bool,
) -> Result<PlannedGuard, LoweringError> {
    let decision = plan_decision(
        decision,
        declared_cases,
        namespace,
        next_value,
        outcome_reads_payloads,
    )?;
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
    outcome_reads_payloads: bool,
) -> Result<LoweredBooleanDecision, LoweringError> {
    match decision {
        LoweredBooleanDecision::Value(expression) => Ok(LoweredBooleanDecision::Value(expression)),
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
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
                        outcome_reads_payloads,
                    )?),
                    when_false: Box::new(plan_decision(
                        *when_false,
                        declared_cases,
                        namespace,
                        next_value,
                        outcome_reads_payloads,
                    )?),
                });
            };
            // Only a whole root has a dispatch terminator, and only a test
            // whose true outcome reads that case's payload needs one; every
            // other test keeps its ordinary membership observation.
            let declared = if path.is_empty()
                && (decision_reads_case(&when_true, *source, *case)
                    || (outcome_reads_payloads && reaches_true_outcome(&when_true)))
            {
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
                        outcome_reads_payloads,
                    )?),
                    when_false: Box::new(plan_decision(
                        *when_false,
                        declared_cases,
                        namespace,
                        next_value,
                        outcome_reads_payloads,
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
                    StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                    _ => continue,
                };
                if field.relevance.is_erased() {
                    continue;
                }
                payloads.push((
                    field.id,
                    crate::terminal_identities::fresh_scalar_declaration(next_value, scalar_type)?,
                ));
            }
            let bound = payloads
                .iter()
                .map(|(field, value)| {
                    namespace.push(*value);
                    EstablishedCasePayload {
                        source,
                        path: Vec::new(),
                        case: selected,
                        field: *field,
                        position: namespace.len() - 1,
                    }
                })
                .collect::<Vec<_>>();
            let when_true = substitute_decision(*when_true, &bound);
            // Scalar fields become parameters; structural payload carriers
            // remain deferred checked copies. Both are authorized only below
            // this selected edge, including when the case has no scalar slots.
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
                    outcome_reads_payloads,
                )?),
                when_false: Box::new(plan_decision(
                    *when_false,
                    declared_cases,
                    namespace,
                    next_value,
                    outcome_reads_payloads,
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
                        path: Vec::new(),
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
    source: &PrimitiveReadSource,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<usize> {
    let PrimitiveReadSource::Place(source) = source else {
        return None;
    };
    let Some((CanonicalStructuralPathSegment::Case(case), prefix)) = path.split_last() else {
        return None;
    };
    // Guard planning above binds whole-root cases only. A projected case has
    // its own exact dispatch path and is resolved by the storage binding, not
    // by erasing that path into this guard's root namespace.
    if !prefix.is_empty() {
        return None;
    }
    bound
        .iter()
        .find(|row| {
            row.source == *source && row.path.is_empty() && row.case == *case && row.field == field
        })
        .map(|row| row.position)
}

fn substitute_boolean(
    expression: LoweredBooleanReturnExpression,
    bound: &[EstablishedCasePayload],
) -> LoweredBooleanReturnExpression {
    use LoweredBooleanReturnExpression as Boolean;
    match expression {
        Boolean::ScalarObservation { operand } => Boolean::ScalarObservation {
            operand: Box::new(substitute_direct(*operand, bound)),
        },
        Boolean::StructuralField {
            source,
            path,
            field,
        } => match bound_position(bound, &source, &path, field) {
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
        Boolean::IndexedCaseMembership {
            source,
            observes_element_view_length,
            path,
            index,
            element_path,
            case,
        } => Boolean::IndexedCaseMembership {
            source,
            observes_element_view_length,
            path,
            index: Box::new(substitute_direct(*index, bound)),
            element_path,
            case,
        },
        Boolean::ByteSequencePredicate {
            predicate,
            source,
            path,
            field,
        } => Boolean::ByteSequencePredicate {
            predicate,
            source,
            path: substitute_path(path, bound),
            field,
        },
        Boolean::ByteSequenceEqual { left, right } => Boolean::ByteSequenceEqual {
            left: substitute_byte_sequence_operand(left, bound),
            right: substitute_byte_sequence_operand(right, bound),
        },
        expression @ (Boolean::StructuralCaseMembership { .. }
        | Boolean::PrimitiveRead { .. }
        | Boolean::Constant { .. }
        | Boolean::Parameter { .. }
        | Boolean::Local { .. }
        | Boolean::UnresolvedStructuralParameterField { .. }) => expression,
    }
}

fn substitute_byte_sequence_operand(
    operand: crate::emission::operation_emission::boolean::LoweredByteSequenceOperand,
    bound: &[EstablishedCasePayload],
) -> crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
    crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
        source: operand.source,
        path: substitute_path(operand.path, bound),
        field: operand.field,
    }
}

fn substitute_path(
    path: Vec<crate::emission::operation_emission::expressions::LoweredElementPathSegment>,
    bound: &[EstablishedCasePayload],
) -> Vec<crate::emission::operation_emission::expressions::LoweredElementPathSegment> {
    use crate::emission::operation_emission::expressions::LoweredElementPathSegment;
    path.into_iter()
        .map(|segment| match segment {
            LoweredElementPathSegment::RuntimeIndex {
                selector,
                extent,
                bound_decided,
            } => LoweredElementPathSegment::RuntimeIndex {
                selector: substitute_direct(selector, bound),
                extent: extent.map(|extent| substitute_direct(extent, bound)),
                bound_decided,
            },
            segment => segment,
        })
        .collect()
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
        } => match bound_position(bound, &source, &path, field) {
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
        Direct::ElementViewRead {
            source,
            index,
            path,
            scalar_type,
            bound_decided,
        } => Direct::ElementViewRead {
            source,
            index: Box::new(substitute_direct(*index, bound)),
            path: substitute_path(path, bound),
            scalar_type,
            bound_decided,
        },
        Direct::IndexedPrimitiveRead {
            source,
            path,
            index,
            extent,
            element_path,
            scalar_type,
            bound_decided,
        } => Direct::IndexedPrimitiveRead {
            bound_decided,
            source,
            path,
            index: Box::new(substitute_direct(*index, bound)),
            extent: extent.map(|extent| Box::new(substitute_direct(*extent, bound))),
            element_path: substitute_path(element_path, bound),
            scalar_type,
        },
        Direct::ByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            scalar_type,
            bound_decided,
        } => Direct::ByteSequenceFieldRead {
            source,
            path: substitute_path(path, bound),
            field,
            index: Box::new(substitute_direct(*index, bound)),
            scalar_type,
            bound_decided,
        },
        Direct::ByteSequenceFieldLength {
            source,
            path,
            field,
            scalar_type,
        } => Direct::ByteSequenceFieldLength {
            source,
            path: substitute_path(path, bound),
            field,
            scalar_type,
        },
        Direct::SaturatingShiftLeft {
            scalar_type,
            left,
            right,
        } => Direct::SaturatingShiftLeft {
            scalar_type,
            left: Box::new(substitute_direct(*left, bound)),
            right: Box::new(substitute_direct(*right, bound)),
        },
        Direct::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
            formation_decided,
        } => Direct::IntegerBinary {
            kind,
            scalar_type,
            left: Box::new(substitute_direct(*left, bound)),
            right: Box::new(substitute_direct(*right, bound)),
            formation_decided,
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
            formation_decided,
        } => Direct::IntegerExactCast {
            scalar_type,
            operand: Box::new(substitute_direct(*operand, bound)),
            formation_decided,
        },
        Direct::RepresentationView {
            scalar_type,
            operand,
        } => Direct::RepresentationView {
            scalar_type,
            operand: Box::new(substitute_direct(*operand, bound)),
        },
        Direct::Boolean { expression } => Direct::Boolean {
            expression: Box::new(substitute_boolean(*expression, bound)),
        },
        Direct::ByteSequenceLength {
            source,
            path,
            access,
            scalar_type,
        } => Direct::ByteSequenceLength {
            source,
            path: substitute_path(path, bound),
            access,
            scalar_type,
        },
        Direct::ElementViewLength {
            source,
            path,
            access,
            scalar_type,
        } => Direct::ElementViewLength {
            source,
            path: substitute_path(path, bound),
            access,
            scalar_type,
        },
        expression @ (Direct::PrimitiveRead { .. }
        | Direct::Parameter { .. }
        | Direct::ErasedParameter { .. }
        | Direct::Local { .. }
        | Direct::IntegerLiteral { .. }
        | Direct::IeeeFloatLiteral { .. }
        | Direct::ImplementationSelection { .. }) => expression,
    }
}

/// Retain the selecting origin even when emission first copies a payload
/// carrier. The copy needs the same dominating case edge as a direct read;
/// preparing its source is not evidence that the case is active.
fn collect_source_case_reads(
    source: &PrimitiveReadSource,
    path: &[CanonicalStructuralPathSegment],
    reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) {
    let origin = source.field_origin(path);
    if origin
        .1
        .iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
    {
        reads.push(origin);
    }
}

/// Every case-qualified read `(root, path)` still awaiting a dispatch.
fn case_reads(
    expression: &LoweredBooleanReturnExpression,
    reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) {
    use LoweredBooleanReturnExpression as Boolean;
    match expression {
        Boolean::ScalarObservation { operand } => collect_direct_case_reads(operand, reads),
        Boolean::PrimitiveRead { source, .. } => collect_source_case_reads(source, &[], reads),
        Boolean::StructuralField { source, path, .. } => {
            collect_source_case_reads(source, path, reads);
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
        Boolean::IndexedCaseMembership { index, .. } => collect_direct_case_reads(index, reads),
        Boolean::ByteSequencePredicate { path, .. } => {
            for segment in path {
                if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } =
                    segment
                {
                    collect_direct_case_reads(selector, reads);
                    if let Some(extent) = extent {
                        collect_direct_case_reads(extent, reads);
                    }
                }
            }
        }
        Boolean::ByteSequenceEqual { left, right } => {
            for operand in [left, right] {
                for segment in &operand.path {
                    if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } =
                        segment
                    {
                        collect_direct_case_reads(selector, reads);
                        if let Some(extent) = extent {
                            collect_direct_case_reads(extent, reads);
                        }
                    }
                }
            }
        }
        Boolean::StructuralCaseMembership { .. }
        | Boolean::Constant { .. }
        | Boolean::Parameter { .. }
        | Boolean::Local { .. }
        | Boolean::UnresolvedStructuralParameterField { .. } => {}
    }
}

fn collect_path_case_reads(
    path: &[crate::emission::operation_emission::expressions::LoweredElementPathSegment],
    reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) {
    for segment in path {
        if let crate::emission::operation_emission::expressions::LoweredElementPathSegment::RuntimeIndex { selector, extent, .. } = segment {
            collect_direct_case_reads(selector, reads);
            if let Some(extent) = extent { collect_direct_case_reads(extent, reads); }
        }
    }
}

fn collect_direct_case_reads(
    expression: &LoweredDirectExpression,
    reads: &mut Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) {
    use LoweredDirectExpression as Direct;
    match expression {
        Direct::PrimitiveRead { source, .. } => collect_source_case_reads(source, &[], reads),
        Direct::StructuralField { source, path, .. } => {
            collect_source_case_reads(source, path, reads);
        }
        Direct::ElementViewRead { index, path, .. } => {
            collect_direct_case_reads(index, reads);
            collect_path_case_reads(path, reads);
        }
        Direct::IndexedPrimitiveRead {
            index,
            extent,
            element_path,
            ..
        } => {
            if let Some(extent) = extent {
                collect_direct_case_reads(extent, reads);
            }
            collect_direct_case_reads(index, reads);
            collect_path_case_reads(element_path, reads);
        }
        Direct::ByteSequenceFieldRead { path, index, .. } => {
            collect_path_case_reads(path, reads);
            collect_direct_case_reads(index, reads);
        }
        Direct::ByteSequenceLength { path, .. }
        | Direct::ByteSequenceFieldLength { path, .. }
        | Direct::ElementViewLength { path, .. } => collect_path_case_reads(path, reads),
        Direct::IntegerBinary { left, right, .. }
        | Direct::SaturatingShiftLeft { left, right, .. } => {
            collect_direct_case_reads(left, reads);
            collect_direct_case_reads(right, reads);
        }
        Direct::IntegerBitwiseNot { operand, .. }
        | Direct::IntegerWiden { operand, .. }
        | Direct::IntegerExactCast { operand, .. }
        | Direct::RepresentationView { operand, .. } => collect_direct_case_reads(operand, reads),
        Direct::Boolean { expression } => case_reads(expression, reads),
        Direct::Parameter { .. }
        | Direct::ErasedParameter { .. }
        | Direct::Local { .. }
        | Direct::IntegerLiteral { .. }
        | Direct::IeeeFloatLiteral { .. }
        | Direct::ImplementationSelection { .. } => {}
    }
}

/// Whether a decision can end in the guard's true outcome: any leaf but a
/// constant false.
fn reaches_true_outcome(decision: &LoweredBooleanDecision) -> bool {
    match decision {
        LoweredBooleanDecision::Value(expression) => !matches!(
            expression,
            LoweredBooleanReturnExpression::Constant { value: false }
        ),
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        }
        | LoweredBooleanDecision::CaseDispatch {
            when_true,
            when_false,
            ..
        } => reaches_true_outcome(when_true) || reaches_true_outcome(when_false),
    }
}

/// Whether a read below this outcome observes `case` of `source`.
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
