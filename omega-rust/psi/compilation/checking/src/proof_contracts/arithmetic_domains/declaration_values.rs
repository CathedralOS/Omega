//! Formation of scalar recipes retained in declaration type applications.
//! The supplying telescope establishes operands; the destination's predicate
//! never participates. Arithmetic uses the same walk as executable values.

use super::{ArithmeticDomain, Diagnostic, ExpressionHandle, PrimitiveType, ValueEnvironment};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;

pub(crate) fn validate_declaration_value_range(
    program: &dyn crate::CheckingInput,
    lexical_owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    value_environment: Option<&ValueEnvironment>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    crate::proof_contracts::contract_entailment::specification_calls::validate_declaration_expression_formation(
        program, lexical_owner, parameters, expression, &[], owner, diagnostics,
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
    // A local's type application uses its independently available flow facts.
    // Initializer writes have already conservatively invalidated this context.
    // Declaration-only applications retain the empty
    // environment; the destination's own predicate never supplies evidence.
    let declaration_environment = ValueEnvironment::new();
    super::expression_analysis::analyze_declaration(
        program,
        lexical_owner,
        parameters,
        expression,
        value_environment.unwrap_or(&declaration_environment),
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    );
}
