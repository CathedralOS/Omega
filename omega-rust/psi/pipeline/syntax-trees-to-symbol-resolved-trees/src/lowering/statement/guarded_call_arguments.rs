//! Value captures for an arm-local return call, without moving place access.

use super::*;

pub(super) fn capture(
    lowerer: &Lowerer,
    arguments: &[ExpressionHandle],
) -> Option<Vec<(String, TypeReference)>> {
    let mut captures = Vec::new();
    for argument in arguments {
        capture_expression(lowerer, *argument, &mut captures)?;
    }
    Some(captures)
}

fn capture_expression(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    captures: &mut Vec<(String, TypeReference)>,
) -> Option<()> {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            let [member] = expressions.name_path_members(path.members) else {
                return None;
            };
            let name = member.as_str();
            // A primitive local is copied at the selected edge. Capturing a
            // borrowed or aggregate local here could instead move its place,
            // alter aliasing, or schedule its cleanup before the return call.
            let source = if let Some(local) = lowerer
                .current_state_locals
                .iter()
                .rev()
                .find(|(local, _, _)| local == name)
            {
                local.1.primitive_type()?;
                local
            } else {
                lowerer
                    .current_state_parameters
                    .iter()
                    .find(|(parameter, _, _)| parameter == name)?
            };
            if !captures.iter().any(|(captured, _)| captured == name) {
                captures.push((source.0.clone(), source.1.clone()));
            }
        }
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {}
        ExpressionNode::Binary(binary) => {
            capture_expression(lowerer, binary.left, captures)?;
            capture_expression(lowerer, binary.right, captures)?;
        }
        ExpressionNode::Unary(unary) => {
            capture_expression(lowerer, unary.operand, captures)?;
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
            capture_expression(lowerer, cast.value, captures)?;
        }
        // Calls, borrows, projections and recasts cannot be replaced by a
        // value snapshot without retaining their evaluation/place custody.
        _ => return None,
    }
    Some(())
}
