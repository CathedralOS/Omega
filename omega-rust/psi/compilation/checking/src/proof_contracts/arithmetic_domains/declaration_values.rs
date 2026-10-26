//! Formation of scalar recipes retained in declaration type applications.
//! The supplying telescope establishes operands; the destination's predicate
//! never participates. Arithmetic uses the same walk as executable values.

use super::{ArithmeticDomain, Diagnostic, ExpressionHandle, PrimitiveType, ValueEnvironment};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;

/// A local type recipe is formed at this statement's current value epoch.
/// Its environment has already lost facts invalidated by earlier effects;
/// declaration entry assumptions cannot stand in for this prefix.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalValueFormation<'program> {
    pub machine: &'program typed_trees::machine::Machine,
    pub state: Option<&'program typed_trees::state::State>,
    pub environment: &'program ValueEnvironment,
}

pub(crate) fn validate_declaration_value_range(
    program: &dyn crate::CheckingInput,
    lexical_owner: SymbolHandle,
    parameters: &[StateParameter],
    prior_facts: &[ExpressionHandle],
    local_formation: Option<LocalValueFormation<'_>>,
    expression: ExpressionHandle,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    crate::proof_contracts::contract_entailment::specification_calls::validate_declaration_expression_formation(
        program, lexical_owner, parameters, expression, prior_facts, owner, diagnostics,
    );
    // A known scalar projection does not waive formation of a constructor
    // evaluated to obtain its receiver. Closed structural values use the
    // canonical carrier/predicate judgment; an open construction needs its
    // ordinary declaration construction proof before it can supply an index.
    let mut pending = vec![expression];
    while let Some(operand) = pending.pop() {
        let node = program.expression_table.expression(operand);
        // These operations carry access, bounds or custody obligations beyond
        // scalar arithmetic and selected call contracts. An unused declaration
        // must decline missing formation evidence just as an executed one does.
        let missing_formation = match node {
            typed_trees::expression::ExpressionNode::Indexed(_) => Some("indexed access bounds"),
            typed_trees::expression::ExpressionNode::Atomic(_) => Some("atomic access authority"),
            typed_trees::expression::ExpressionNode::Borrow(_) => Some("borrow custody"),
            typed_trees::expression::ExpressionNode::ZeroValue(_) => {
                Some("zero construction membership")
            }
            typed_trees::expression::ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                Some("recast access and representation")
            }
            typed_trees::expression::ExpressionNode::Cast(cast)
                if cast.semantic_domain_symbol.is_valid() =>
            {
                Some("domain qualification membership")
            }
            typed_trees::expression::ExpressionNode::Member(member) => {
                let receiver = crate::parameter_expression_result_type_reference(
                    program,
                    lexical_owner,
                    parameters,
                    member.receiver,
                );
                receiver
                    .and_then(|reference| crate::reference_shell(program, reference))
                    .filter(|reference| !reference.access.is_readable())
                    .map(|_| "readable projected storage")
            }
            _ => None,
        };
        if let Some(obligation) = missing_formation {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{owner} has no declaration formation evidence for {obligation}"
                ))
                .with_source_span(program.expression_table.source_span(operand)),
            );
        }
        if matches!(
            node,
            typed_trees::expression::ExpressionNode::StructLiteral(_)
                | typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        ) {
            let reference = crate::parameter_expression_result_type_reference(
                program,
                lexical_owner,
                parameters,
                operand,
            )
            .or_else(|| {
                let reference = program
                    .expression_table
                    .constructed_value_type_reference(operand);
                reference.is_valid().then_some(reference)
            });
            let formation = reference.and_then(|reference| {
                crate::normalize_closed_const_expression(program, lexical_owner, operand, reference)
                    .transpose()
            });
            match formation {
                Some(Ok(_)) => {}
                Some(Err(reason)) => diagnostics.push(Diagnostic::error(format!(
                    "{owner} has an invalid construction: {reason}"
                ))),
                None => diagnostics.push(Diagnostic::error(format!(
                    "{owner} has an open construction without a declaration construction judgment"
                ))),
            }
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    if let Some(local) = local_formation {
        super::expression_analysis::analyze(
            program,
            local.machine,
            local.state,
            expression,
            local.environment,
            target_primitive,
            target_domain,
            owner,
            false,
            diagnostics,
        );
        return;
    }
    super::expression_analysis::analyze_declaration(
        program,
        lexical_owner,
        parameters,
        prior_facts,
        expression,
        &ValueEnvironment::new(),
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    );
}

/// Prove an Exact result from the supplying declaration's formed operands and
/// incoming requirements. The result is mathematical only for this formation
/// question: it does not replace the executable operation or its value identity.
/// The common walk has already checked children and selected builtin meaning.
pub(super) fn binary_result_fits(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    prior_facts: &[ExpressionHandle],
    expression: ExpressionHandle,
    primitive: PrimitiveType,
) -> bool {
    use crate::proof_contracts::contract_entailment::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        scoped_arithmetic_values_relation_for_declaration,
    };
    use typed_trees::expression::BinaryOperator;
    if prior_facts.is_empty() {
        return false;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
    ) {
        return false;
    }
    let Some((minimum, maximum)) = primitive.integer_bounds() else {
        return false;
    };
    let bindings = parameters
        .iter()
        .filter_map(|parameter| {
            let primitive = crate::value_custody::recasts::exact_primitive_type(
                program,
                parameter.type_reference,
            )?;
            let (minimum, maximum) = primitive.integer_bounds()?;
            Some(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(ScopedArithmeticValue::Atom {
                        identity: format!("\0declaration-value:{:?}", parameter.symbol),
                        unsigned: !primitive.is_signed_integer(),
                    }),
                    minimum: Some(numerics::bignum::BigInt::from_i128(minimum)),
                    maximum: Some(numerics::bignum::BigInt::from_i128(maximum)),
                },
            })
        })
        .collect::<Vec<_>>();
    let term = |expression| ScopedArithmeticExpression {
        owner,
        expression,
        bindings: bindings.clone(),
    };
    let hypotheses = prior_facts
        .iter()
        .map(|&expression| ScopedArithmeticHypothesis {
            proposition: term(expression),
            holds: true,
        })
        .collect::<Vec<_>>();
    let value = ScopedArithmeticValue::IntegerOperation {
        operator: binary.operator,
        left: Box::new(ScopedArithmeticValue::Term(term(binary.left))),
        right: Box::new(ScopedArithmeticValue::Term(term(binary.right))),
    };
    [
        (BinaryOperator::GreaterOrEqual, minimum),
        (BinaryOperator::LessOrEqual, maximum),
    ]
    .into_iter()
    .all(|(operator, bound)| {
        matches!(
            scoped_arithmetic_values_relation_for_declaration(
                program,
                owner,
                &hypotheses,
                operator,
                &value,
                &ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(bound)),
            ),
            StrictArithmeticImplicationJudgment::Proven
        )
    })
}
