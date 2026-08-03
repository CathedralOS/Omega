use psi_arena::Arena;
use psi_checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedNamedOperatorUseFact, CheckedOperatorCandidateFact,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedValueFacts, CheckedValueOrigin,
};
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::float_semantics::FloatFormat;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
};
use psi_typed_trees::operator::{SpelledOperator, resolve_spelling_for_operands};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle};

mod receiver;
mod selection;

use receiver::expression_type_reference_for_origin;
pub(crate) use selection::select_pending_domain_operator_meanings;

pub(crate) fn build_operator_facts(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut named_uses = Arena::default();
    let mut candidates = Arena::default();
    let mut seen = Vec::new();

    for (_, value) in values.values.iter() {
        collect_expression_operator_use(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
            &mut named_uses,
            &mut candidates,
        );
    }

    CheckedOperatorFacts::with_roots(uses, named_uses, candidates)
}

fn collect_expression_operator_use(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut Vec<(ExpressionHandle, CheckedValueOrigin)>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    named_uses: &mut Arena<CheckedNamedOperatorUseFact>,
    candidates: &mut Arena<CheckedOperatorCandidateFact>,
) {
    if !expression.is_valid() || seen.iter().any(|seen| *seen == (expression, origin)) {
        return;
    }
    seen.push((expression, origin));

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_expression_operator_use(
            program,
            atomic.value,
            origin,
            seen,
            uses,
            named_uses,
            candidates,
        ),
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            let operand_types = indexed_operand_types(program, indexed, origin);
            uses.append(operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                &operand_types,
                candidates,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                indexed.index,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(
                    program, *value, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // A spelled binary use participates in operator resolution only
            // when the left operand type is known and at least one spelled
            // candidate matches it: builtin-only arithmetic stays unrecorded
            // (and untouched), exactly as before spelled binary dispatch.
            if let Some(spelling) = binary_operator_spelling(binary.operator)
                && let Some(receiver_type) =
                    expression_type_reference_for_origin(program, binary.left, origin)
                && let right_type =
                    expression_type_reference_for_origin(program, binary.right, origin)
                && let Some(fact) = binary_operator_use_fact(
                    program,
                    expression,
                    origin,
                    spelling,
                    receiver_type,
                    right_type,
                    candidates,
                )
            {
                uses.append(fact);
            }
            collect_expression_operator_use(
                program,
                binary.left,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                binary.right,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(
                program, cast.value, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::Call(call) => {
            if let Some(named_use) = named_operator_use_fact(program, expression, origin, call) {
                named_uses.append(named_use);
            }
            collect_expression_operator_use(
                program,
                call.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(
                    program, *argument, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_operator_use(
                program, *inner, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_operator_use(
                program,
                unary.operand,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(
                program,
                range.start,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program, range.end, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_operator_use(
                    program,
                    field.value,
                    origin,
                    seen,
                    uses,
                    named_uses,
                    candidates,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Retain the selected identity of one unambiguously resolved named operator
/// call. Every boundary-operator provider path consumes this common fact;
/// numeric policy adaptation remains specific to the normalized float surface.
/// Policy adaptation is operand-driven for float-returning F32/F64 operations;
/// classification and destination-owned float-to-integer conversions carry no
/// float result adapter.
fn named_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedOperatorUseFact> {
    let operator = psi_typed_trees::operator::resolve_named_expression_call(program, call)?;
    let path = program.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let format = match namespace.as_str() {
        "F32" => Some(FloatFormat::BINARY32),
        "F64" => Some(FloatFormat::BINARY64),
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
            if matches!(requirement.as_str(), "from_f32" | "from_f64") =>
        {
            None
        }
        _ => None,
    };
    let policy_adapter = match format {
        Some(format)
            if matches!(
                (
                    format,
                    program.primitive_type_reference(operator.return_type)
                ),
                (FloatFormat::BINARY32, Some(PrimitiveType::F32))
                    | (FloatFormat::BINARY64, Some(PrimitiveType::F64))
            ) =>
        {
            named_float_policy_adapter(program, call, origin, format)
        }
        _ => CheckedArithmeticPolicyAdapter::None,
    };

    Some(CheckedNamedOperatorUseFact {
        expression,
        origin,
        selected_operator_symbol: operator.symbol,
        policy_adapter,
        provider_plan_identity: 0,
    })
}

fn named_float_policy_adapter(
    program: &TypedTrees,
    call: &TableCallExpression,
    origin: CheckedValueOrigin,
    format: FloatFormat,
) -> CheckedArithmeticPolicyAdapter {
    let mut selected_domain = ArithmeticDomain::Exact;
    for argument in program.expression_table.expression_handles(call.arguments) {
        let Some(type_reference) = expression_type_reference_for_origin(program, *argument, origin)
        else {
            continue;
        };
        let domain = program
            .type_reference_table
            .arithmetic_domain(type_reference);
        if domain == ArithmeticDomain::Exact {
            continue;
        }
        if selected_domain != ArithmeticDomain::Exact && selected_domain != domain {
            // Validation rejects mixed explicit arithmetic policies. Checked
            // evidence fails closed if lowering is invoked without that gate.
            return CheckedArithmeticPolicyAdapter::None;
        }
        selected_domain = domain;
    }
    float_policy_adapter(format, selected_domain)
}

/// The fixed operator spelling for a binary operator, when one exists.
/// Logical/shift operators have no spelling surface (frozen Wave 0 decision
/// #3) and never participate in spelled dispatch.
fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    })
}

/// Records a spelled binary use when spelled candidates match the left
/// operand type. Root-only candidate sets resolve immediately (root spelled
/// operators are the declared surface of the builtin operation). Any
/// domain-owned candidate defers to the binding-site selection pass
/// (`select_pending_domain_operator_meanings`), which reads declarations,
/// mints, and signature `requires` but never flow facts.
fn binary_operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    receiver_type: TypeReferenceHandle,
    right_type: Option<TypeReferenceHandle>,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> Option<CheckedOperatorUseFact> {
    let candidates =
        resolve_spelling_for_operands(program, spelling, &[Some(receiver_type), right_type]);
    if candidates.is_empty() {
        return None;
    }

    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let (status, selected_operator_symbol) = if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        (
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        )
    } else if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    Some(CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        policy_adapter: arithmetic_policy_adapter(program, spelling, receiver_type),
        provider_plan_identity: 0,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    })
}

fn indexed_operator_spelling(program: &TypedTrees, index: ExpressionHandle) -> OperatorSpelling {
    if index.is_valid()
        && matches!(
            program.expression_table.expression(index),
            ExpressionNode::Range(_)
        )
    {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    }
}

fn indexed_operand_types(
    program: &TypedTrees,
    indexed: &TableIndexedExpression,
    origin: CheckedValueOrigin,
) -> Vec<Option<TypeReferenceHandle>> {
    let mut operand_types = vec![expression_type_reference_for_origin(
        program,
        indexed.collection,
        origin,
    )];
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(range) => {
            operand_types.push(expression_type_reference_for_origin(
                program,
                range.start,
                origin,
            ));
            operand_types.push(expression_type_reference_for_origin(
                program, range.end, origin,
            ));
        }
        _ => operand_types.push(expression_type_reference_for_origin(
            program,
            indexed.index,
            origin,
        )),
    }
    operand_types
}

/// Records the typed-trees resolution outcome for one use site as checked
/// evidence. Every known operand position participates, including both range
/// bounds for `[..]`.
fn operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let candidates = resolve_spelling_for_operands(program, spelling, operand_types);
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| checked_candidate(program, candidate)),
    );
    let (status, selected_operator_symbol) = if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        (
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        )
    } else if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        )
    } else if candidates.is_empty() {
        (
            CheckedOperatorResolutionStatus::Missing,
            SymbolHandle::invalid(),
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        policy_adapter: CheckedArithmeticPolicyAdapter::None,
        provider_plan_identity: 0,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    }
}

fn arithmetic_policy_adapter(
    program: &TypedTrees,
    spelling: OperatorSpelling,
    receiver_type: TypeReferenceHandle,
) -> CheckedArithmeticPolicyAdapter {
    if !matches!(
        spelling,
        OperatorSpelling::Add
            | OperatorSpelling::Subtract
            | OperatorSpelling::Multiply
            | OperatorSpelling::Divide
    ) {
        return CheckedArithmeticPolicyAdapter::None;
    }

    let format = match program.primitive_type_reference(receiver_type) {
        Some(PrimitiveType::F32) => FloatFormat::BINARY32,
        Some(PrimitiveType::F64) => FloatFormat::BINARY64,
        _ => return CheckedArithmeticPolicyAdapter::None,
    };
    float_policy_adapter(
        format,
        program
            .type_reference_table
            .arithmetic_domain(receiver_type),
    )
}

fn float_policy_adapter(
    format: FloatFormat,
    domain: ArithmeticDomain,
) -> CheckedArithmeticPolicyAdapter {
    match domain {
        ArithmeticDomain::Saturating => {
            CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
        }
        ArithmeticDomain::Trapping => {
            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
        }
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
            CheckedArithmeticPolicyAdapter::None
        }
    }
}

fn checked_candidate(
    program: &TypedTrees,
    candidate: &SpelledOperator<'_>,
) -> CheckedOperatorCandidateFact {
    let fact = if let Some(domain) = candidate.domain {
        CheckedOperatorCandidateFact::domain(candidate.operator.symbol, domain.symbol)
    } else {
        CheckedOperatorCandidateFact::root(candidate.operator.symbol)
    };
    fact.with_signature(
        program
            .operator_parameters(candidate.operator)
            .first()
            .map(|parameter| parameter.type_reference)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        candidate.operator.return_type,
        candidate.operator.contracts,
        program.operator_type_parameters(candidate.operator).len(),
        program.operator_parameters(candidate.operator).len(),
        candidate.operator.is_boundary,
    )
}
