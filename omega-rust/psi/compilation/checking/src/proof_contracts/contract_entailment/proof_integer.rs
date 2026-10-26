//! Mathematical integer recognition and ordered proof-term formation.

use super::{
    BigInt, BinaryOperator, Diagnostic, Engine, ExpressionHandle, ExpressionNode, ProofFact,
};
use crate::proof_contracts::arithmetic_domains::{DispatchSelection, dispatch_arm_contexts};
fn proof_builtin_type(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
    builtin: symbols::BuiltinType,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
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
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
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
                        Some(source::SourceOrigin::Toolchain) => true,
                        Some(source::SourceOrigin::User) => false,
                        None => !program.symbols.has_source_metadata(),
                    }
                    && crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                        .is_some_and(|data| {

                            let members = program.data_members(data);
                            members.len() == 2 && members.iter().all(|member| {
                                let Some(variant) = member.as_variant() else { return false; };
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
pub fn proof_integer_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call) =>
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
                || (left && anonymous_integer_value(program, binary.right).is_some())
                || (right && anonymous_integer_value(program, binary.left).is_some())
        }
        _ => {
            crate::proof_contracts::proof_embeddings::expression_type_reference(program, expression)
                .is_some_and(|reference| {
                    proof_builtin_type(program, reference, symbols::BuiltinType::Int)
                })
        }
    }
}

/// Land the complete anonymous peer to unbounded Int. Fractional intermediate
/// values remain exact; a fractional final value cannot become an integer.
pub(crate) fn anonymous_integer_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    crate::value_custody::literals::anonymous_numeric_value(
        program,
        expression,
        &mut |expression| {
            crate::value_custody::literals::has_anonymous_operator_meaning(program, expression)
        },
    )?
    .value
    .to_integer_exact()
}

/// Operand compatibility after builtin selection, shared by result typing and
/// structural proof terms. An anonymous peer must land exactly in Int; a
/// finite runtime value enters this arithmetic only through its embedding.
pub(crate) fn mathematical_integer_operands(
    program: &dyn crate::CheckingInput,
    operands: [ExpressionHandle; 2],
) -> bool {
    let mathematical = operands.map(|operand| proof_integer_expression(program, operand));
    mathematical.iter().any(|value| *value)
        && operands
            .into_iter()
            .zip(mathematical)
            .all(|(operand, mathematical)| {
                mathematical || anonymous_integer_value(program, operand).is_some()
            })
}

pub(crate) fn proof_nat_cast(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
) -> bool {
    !cast.form.is_recast()
        && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
        && cast.semantic_domain.is_empty()
        && cast.semantic_domain_arguments.is_empty()
        && proof_nat_type(program, cast.target_type)
        && proof_integer_expression(program, cast.value)
}

/// Formation is deliberately stricter than ordinary theorem validation:
/// unknown is not proof, and callers supply only facts preceding this term.
pub(crate) fn proof_integer_nonnegative(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
    selections: &[DispatchSelection],
) -> bool {
    if !proof_integer_expression(program, expression) {
        return false;
    }
    let mut engine =
        Engine::for_proof_integer_formation(program, owner_symbol, hypotheses, selections);
    let Some(polynomial) = engine.normalize(expression) else {
        return false;
    };
    let polynomial = engine.substituted(&polynomial);
    engine.requires_unsatisfiable || engine.prove_at_least(&polynomial, &BigInt::zero())
}

/// Nonzero is a formation premise, not a theorem the containing term may
/// establish for itself. Only independently admitted preceding facts enter.
pub(crate) fn proof_integer_nonzero(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
    selections: &[DispatchSelection],
) -> bool {
    let mut engine =
        Engine::for_proof_integer_formation(program, owner_symbol, hypotheses, selections);
    let Some(polynomial) = engine.normalize(expression) else {
        return false;
    };
    let polynomial = engine.substituted(&polynomial);
    engine.requires_unsatisfiable || engine.proves_nonzero(&polynomial)
}

fn validate_proof_integer_formation(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
    hypotheses: &[ExpressionHandle],
    selections: &[DispatchSelection],
    owner: &str,
    builtin_arithmetic: &impl Fn(ExpressionHandle) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
        let diagnostics_before = diagnostics.len();
        validate_proof_integer_formation(
            program,
            owner_symbol,
            dispatch.subject,
            hypotheses,
            selections,
            owner,
            builtin_arithmetic,
            diagnostics,
        );
        let subject_formed = diagnostics.len() == diagnostics_before;
        // A proof position does not itself establish an empty write frame.
        // Consume the same selected callees' complete frames, including for
        // abstract owners which have no executable caller to fabricate.
        let meaning =
            Engine::for_proof_integer_formation(program, owner_symbol, hypotheses, selections);
        let stable = subject_formed
            && crate::CallFrameResolver::new(program).is_some_and(|frames| {
                frames.declaration_expression_has_empty_write_frame(expression, &|expression| {
                    program
                        .expression_table
                        .binary_node(expression)
                        .is_some_and(|binary| meaning.selected_binary_meaning(expression, binary))
                })
            });
        let initial = if stable {
            selections.to_vec()
        } else {
            Vec::new()
        };
        let contexts = dispatch_arm_contexts(
            program,
            expression,
            dispatch,
            &initial,
            |context, pattern| {
                let diagnostics_before = diagnostics.len();
                validate_proof_integer_formation(
                    program,
                    owner_symbol,
                    pattern,
                    hypotheses,
                    context,
                    owner,
                    builtin_arithmetic,
                    diagnostics,
                );
                subject_formed && diagnostics.len() == diagnostics_before
            },
            |context, selection| {
                if stable {
                    context.push(selection);
                }
            },
        );
        for (arm, context) in program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .zip(&contexts)
        {
            validate_proof_integer_formation(
                program,
                owner_symbol,
                arm.value,
                hypotheses,
                context,
                owner,
                builtin_arithmetic,
                diagnostics,
            );
        }
        return;
    }
    // Formation is child-first even when a parent is reflexive or its proof
    // does not normalize the child (notably integer quotient and remainder).
    crate::expression_traversal::children(
        program,
        program.expression_table.expression(expression),
        |child| {
            validate_proof_integer_formation(
                program,
                owner_symbol,
                child,
                hypotheses,
                selections,
                owner,
                builtin_arithmetic,
                diagnostics,
            );
        },
    );
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            if program
                .named_type_reference(cast.target_type)
                .is_some_and(|name| name.as_str() == "Nat")
                && proof_integer_expression(program, cast.value)
                && (!proof_nat_cast(program, cast)
                    || !proof_integer_nonnegative(
                        program,
                        owner_symbol,
                        cast.value,
                        hypotheses,
                        selections,
                    ))
            {
                diagnostics.push(Diagnostic::error(
                    "Exact proof Int-to-Nat conversion requires a previously proven nonnegative value",
                ));
            }
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Divide | BinaryOperator::Modulo
            ) && mathematical_integer_operands(program, [binary.left, binary.right])
                && builtin_arithmetic(expression) =>
        {
            crate::proof_contracts::arithmetic_domains::division_definedness::validate_proof_integer(
                program, owner_symbol, binary, hypotheses, selections, owner, diagnostics,
            );
        }
        _ => {}
    }
}

/// Non-expression proof facts still contain proof-value arguments. Their
/// terms need formation checking even though the fact is not an arithmetic
/// hypothesis understood by this producer.
pub(crate) fn validate_proof_fact_integer_formation(
    program: &dyn crate::CheckingInput,
    owner_symbol: symbols::SymbolHandle,
    fact: &ProofFact,
    hypotheses: &[ExpressionHandle],
    owner: &str,
    builtin_arithmetic: &impl Fn(ExpressionHandle) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match fact {
        ProofFact::Expression(expression) => validate_proof_integer_formation(
            program,
            owner_symbol,
            *expression,
            hypotheses,
            &[],
            owner,
            builtin_arithmetic,
            diagnostics,
        ),
        ProofFact::Membership(membership) => validate_proof_integer_formation(
            program,
            owner_symbol,
            membership.value,
            hypotheses,
            &[],
            owner,
            builtin_arithmetic,
            diagnostics,
        ),
        ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                validate_proof_integer_formation(
                    program,
                    owner_symbol,
                    *argument,
                    hypotheses,
                    &[],
                    owner,
                    builtin_arithmetic,
                    diagnostics,
                );
            }
        }
    }
}

/// One anonymous integral peer in mathematical arithmetic. Its value is
/// evaluated by the same exact anonymous-number owner as proof admission;
/// the portable literal representation retains a canonical sign and magnitude.
pub fn anonymous_integer_math_literal(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<semantic_vocabulary::IntegerMathLiteral> {
    let value = anonymous_integer_value(program, expression)?;
    let magnitude = value.abs();
    let (high, low) = magnitude.div_rem(&BigInt::from_u128(1u128 << 64))?;
    let magnitude = (u128::from(high.to_u64()?) << 64) | u128::from(low.to_u64()?);
    semantic_vocabulary::IntegerMathLiteral::new(value.is_negative(), magnitude).ok()
}
