//! Mathematical integer recognition and ordered Exact proof coercion.

use super::*;

fn proof_builtin_type(
    program: &TypedTrees,
    mut reference: psi_typed_trees::types::TypeReferenceHandle,
    builtin: psi_symbols::BuiltinType,
) -> bool {
    use psi_typed_trees::types::TypeReferenceNode;
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return symbol.is_valid()
                    && program.symbols.builtin_type_symbol(builtin) == Some(*symbol);
            }
            _ => return false,
        }
    }
}

fn proof_nat_type(
    program: &TypedTrees,
    mut reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    use psi_typed_trees::types::TypeReferenceNode;
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                // Nat is source-defined recursive proof data, not one of the
                // compiler-installed integer types. Retain its nominal symbol
                // and require the ordinary proof-only classification.
                return name.as_str() == "Nat"
                    && symbol.is_valid()
                    && match program.symbols.symbol_source_origin(*symbol) {
                        Some(psi_source::SourceOrigin::Toolchain) => true,
                        Some(psi_source::SourceOrigin::User) => false,
                        None => !program.symbols.has_source_metadata(),
                    }
                    && program.data_definitions().iter().find(|data| data.symbol == *symbol)
                        .is_some_and(|data| {
                            use psi_typed_trees::data::DataMember;
                            let members = program.data_members(data);
                            members.len() == 2 && members.iter().all(|member| {
                                let DataMember::Variant(variant) = member else { return false; };
                                let fields = program.data_payload_fields(variant);
                                match variant.name.as_str() {
                                    "Zero" => fields.is_empty(),
                                    "Succ" => matches!(fields, [field] if matches!(
                                        program.type_reference_table.type_reference(field.type_reference),
                                        TypeReferenceNode::Named { symbol: source, .. } if source == symbol)),
                                    _ => false,
                                }
                            })
                        });
            }
            _ => return false,
        }
    }
}

/// Recognize mathematical Int terms without borrowing a machine integer's
/// carrier or accidentally classifying a comparison as its operand type.
pub(crate) fn proof_integer_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if crate::proof_embeddings::is_exact_embed_call(program, call) =>
        {
            true
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) =>
        {
            let left = proof_integer_expression(program, binary.left);
            let right = proof_integer_expression(program, binary.right);
            (left && right)
                || (left
                    && matches!(
                        program.expression_table.expression(binary.right),
                        ExpressionNode::Integer(_)
                    ))
                || (right
                    && matches!(
                        program.expression_table.expression(binary.left),
                        ExpressionNode::Integer(_)
                    ))
        }
        _ => crate::proof_embeddings::expression_type_reference(program, expression).is_some_and(
            |reference| proof_builtin_type(program, reference, psi_symbols::BuiltinType::Int),
        ),
    }
}

pub(crate) fn proof_nat_cast(
    program: &TypedTrees,
    cast: &psi_typed_trees::expression::TableCastExpression,
) -> bool {
    !cast.form.is_recast()
        && cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Exact
        && cast.semantic_domain.is_empty()
        && cast.semantic_domain_arguments.is_empty()
        && proof_nat_type(program, cast.target_type)
        && proof_integer_expression(program, cast.value)
}

/// Formation is deliberately stricter than ordinary theorem validation:
/// unknown is not proof, and callers supply only facts preceding this term.
pub(crate) fn proof_integer_nonnegative(
    program: &TypedTrees,
    expression: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
) -> bool {
    if !proof_integer_expression(program, expression) {
        return false;
    }
    let mut engine = Engine::for_proof_integer_formation(program);
    let Some(polynomial) = engine.normalize(expression) else {
        return false;
    };
    let mut comparisons = Vec::new();
    engine.collect_comparisons(hypotheses, &mut comparisons);
    engine.install_hypotheses(comparisons);
    let polynomial = engine.substituted(&polynomial);
    engine.requires_unsatisfiable || engine.prove_at_least(&polynomial, &BigInt::zero())
}

pub(crate) fn validate_proof_integer_casts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    let mut children = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            children.push(cast.value);
            if program
                .named_type_reference(cast.target_type)
                .is_some_and(|name| name.as_str() == "Nat")
                && proof_integer_expression(program, cast.value)
                && (!proof_nat_cast(program, cast)
                    || !proof_integer_nonnegative(program, cast.value, hypotheses))
            {
                diagnostics.push(Diagnostic::error(
                    "Exact proof Int-to-Nat conversion requires a previously proven nonnegative value",
                ));
            }
        }
        ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
        ExpressionNode::Unary(unary) => children.push(unary.operand),
        ExpressionNode::Call(call) => {
            children.push(call.receiver);
            children.extend_from_slice(program.expression_table.expression_handles(call.arguments));
        }
        ExpressionNode::Borrow(value) => children.push(value.target),
        ExpressionNode::Member(member) => children.push(member.receiver),
        ExpressionNode::Indexed(indexed) => children.extend([indexed.collection, indexed.index]),
        ExpressionNode::Atomic(atomic) => children.extend([atomic.value, atomic.result]),
        ExpressionNode::Range(range) => children.extend([range.start, range.end]),
        ExpressionNode::ArrayLiteral(elements) => {
            children.extend_from_slice(program.expression_table.expression_handles(*elements))
        }
        ExpressionNode::StructLiteral(literal) => children.extend(
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| field.value),
        ),
        _ => {}
    }
    for child in children {
        validate_proof_integer_casts(program, child, hypotheses, diagnostics);
    }
}

/// Non-expression proof facts still contain proof-value arguments. Their
/// casts need formation checking even though the fact is not an arithmetic
/// hypothesis understood by this producer.
pub(crate) fn validate_proof_fact_integer_casts(
    program: &TypedTrees,
    fact: &ProofFact,
    hypotheses: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match fact {
        ProofFact::Expression(expression) => {
            validate_proof_integer_casts(program, *expression, hypotheses, diagnostics)
        }
        ProofFact::Membership(membership) => {
            validate_proof_integer_casts(program, membership.value, hypotheses, diagnostics)
        }
        ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                validate_proof_integer_casts(program, *argument, hypotheses, diagnostics);
            }
        }
    }
}
