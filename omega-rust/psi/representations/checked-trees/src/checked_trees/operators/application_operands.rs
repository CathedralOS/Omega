//! The authored operands of one selected boundary-operator application.
//!
//! The values stage binds each scalar operand's checked expression to its
//! authored source before provider settlement, and Checked-to-Lowered Psi
//! replays that binding after settlement. Both stages read the operand roster
//! and each operand's carrier here, so the two sides cannot disagree about
//! which expression is operand `n` or what it lands as.
//!
//! Settlement replaces a spelled application (`items[offset]`) in place with a
//! call to its realization over the same operand expression handles, and a
//! named application is already a call. The roster is therefore the call's
//! argument list when the node is a call, and the spelled operator's operands
//! otherwise. The carrier comes from the boundary operator's declared formal:
//! a primitive formal lands as itself, and a formal naming one of the
//! operator's type binders lands as the primitive type argument the checked
//! application demand closed it with. Any other formal is structural and owns
//! no scalar carrier.

use super::{CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorResolutionStatus};
use crate::{CheckedBoundaryOperatorApplicationArgument, CheckedValueOrigin};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// One selected boundary operator applied at an exact authored site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryApplicationOperands {
    pub requirement_operator: SymbolHandle,
    /// Authored operands in position order. `Some` is a scalar operand's
    /// declared carrier; `None` is a structural operand.
    pub operands: Vec<(ExpressionHandle, Option<PrimitiveType>)>,
}

impl CheckedBoundaryApplicationOperands {
    /// The scalar operand at one authored position, with its carrier.
    pub fn scalar_operand(&self, position: usize) -> Option<(ExpressionHandle, PrimitiveType)> {
        let (expression, primitive_type) = self.operands.get(position)?;
        Some((*expression, (*primitive_type)?))
    }
}

impl CheckedOperatorFacts {
    /// The selected boundary operator applied at `expression` in `origin`,
    /// or `None` unless exactly one spelled or named use selected one.
    pub fn boundary_application_operands(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<CheckedBoundaryApplicationOperands> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let spelled = self
            .uses
            .iter()
            .map(|(_, operator_use)| operator_use)
            .filter(|operator_use| {
                operator_use.expression == expression
                    && operator_use.origin == origin
                    && operator_use.occurrence == CheckedOperatorOccurrence::Expression
                    && operator_use.status == CheckedOperatorResolutionStatus::Resolved
                    && operator_use.selected_operator_symbol.is_valid()
            })
            .collect::<Vec<_>>();
        let named = self
            .named_uses
            .iter()
            .map(|(_, named_use)| named_use)
            .filter(|named_use| {
                named_use.expression == expression
                    && named_use.origin == origin
                    && named_use.selected_operator_symbol.is_valid()
            })
            .collect::<Vec<_>>();
        let (requirement_operator, spelled_use) = match (spelled.as_slice(), named.as_slice()) {
            ([operator_use], []) => (operator_use.selected_operator_symbol, Some(*operator_use)),
            ([], [named_use]) => (named_use.selected_operator_symbol, None),
            _ => return None,
        };
        let operator = typed_trees::operator::declaration_by_symbol(program, requirement_operator)?;
        if !operator.is_boundary {
            return None;
        }
        // A named call's receiver slot holds its qualified owner path, which
        // settlement clears; it is never an operand. A formal that would
        // bind a receiver operand (`self`) is refused below.
        let operand_expressions = match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec(),
            _ => spelled_use?.operands(program)?,
        };
        let parameters = program.operator_parameters(operator);
        if parameters.len() != operand_expressions.len()
            || parameters
                .iter()
                .any(|parameter| parameter.is_self || parameter.is_const)
        {
            return None;
        }
        let binders = program.operator_type_parameters(operator);
        let type_arguments = self
            .boundary_applications
            .iter()
            .filter(|demand| {
                demand.requirement_symbol == requirement_operator
                    && matches!(
                        demand.site,
                        crate::CheckedBoundaryOperatorApplicationUseSite::Expression {
                            expression: site,
                            origin: site_origin,
                        } if site == expression && site_origin == origin
                    )
            })
            .flat_map(|demand| &demand.arguments)
            .filter_map(|argument| match argument {
                CheckedBoundaryOperatorApplicationArgument::Type {
                    binder_owner,
                    binder_symbol,
                    type_reference,
                    ..
                } if *binder_owner == requirement_operator => {
                    Some((*binder_symbol, *type_reference))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let operands = operand_expressions
            .into_iter()
            .zip(parameters)
            .map(|(operand, parameter)| {
                let declared = program.primitive_type_reference(parameter.type_reference);
                let closed = || match program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                {
                    TypeReferenceNode::Named { symbol, .. }
                        if binders.iter().any(|binder| binder.symbol == *symbol) =>
                    {
                        let mut closing =
                            type_arguments.iter().filter(|(binder, _)| binder == symbol);
                        let (_, type_reference) = closing.next()?;
                        if closing.next().is_some() {
                            return None;
                        }
                        program.primitive_type_reference(*type_reference)
                    }
                    _ => None,
                };
                (operand, declared.or_else(closed))
            })
            .collect();
        Some(CheckedBoundaryApplicationOperands {
            requirement_operator,
            operands,
        })
    }
}
