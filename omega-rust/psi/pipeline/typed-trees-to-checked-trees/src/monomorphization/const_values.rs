//! Materialize selected const binders without capturing same-spelled names.

use super::{Candidate, static_const_literal_from_type_reference};
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod structured;

pub(super) fn substitute(
    program: &mut TypedTrees,
    candidate: &Candidate,
    expression_start: Option<usize>,
) -> Result<(), Diagnostic> {
    for ((binder, name, declared_type), binding) in candidate
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
    {
        let occurrences = program
            .expression_table
            .iter_expressions()
            .filter(|(handle, _)| {
                expression_start.is_none_or(|start| handle.arena_index() as usize >= start)
            })
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return None;
                };
                (binder.is_valid()
                    && path.symbol == *binder
                    && path.head_symbol == *binder
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1)
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        if occurrences.is_empty() {
            continue;
        }
        for occurrence in occurrences {
            // Each use owns its literal children and their checking context.
            let replacement = binding
                .and_then(|binding| value(program, binding, *declared_type))
                .ok_or_else(|| Diagnostic::error(format!(
                    "machine `{}` cannot materialize const parameter `{name}` as its declared value type `{}`",
                    candidate.template_name,
                    program.display_type_reference(*declared_type),
                )))?;
            *program.expression_table.expression_mut(occurrence) = replacement;
        }
    }
    Ok(())
}

fn value(
    program: &mut TypedTrees,
    binding: TypeReferenceHandle,
    declared_type: TypeReferenceHandle,
) -> Option<ExpressionNode> {
    let TypeReferenceNode::Named { name, .. } =
        program.type_reference_table.type_reference(binding)
    else {
        return None;
    };
    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        return structured::materialize(program, declared_type, &value.decode_encoding()?);
    }
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    if primitive == PrimitiveType::Bool {
        return match name.as_str() {
            "true" => Some(ExpressionNode::Boolean(true)),
            "false" => Some(ExpressionNode::Boolean(false)),
            _ => None,
        };
    }
    let literal = static_const_literal_from_type_reference(program, binding)?;
    integer_value(program, declared_type, literal)
}

fn integer_value(
    program: &TypedTrees,
    declared_type: TypeReferenceHandle,
    literal: IntegerLiteral,
) -> Option<ExpressionNode> {
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    let landed_type = match primitive {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        PrimitiveType::Addr => LandedIntegerType::Addr,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some(ExpressionNode::Integer(
        literal.with_landing(IntegerLanding {
            landed_type,
            domain: program
                .type_reference_table
                .arithmetic_domain(declared_type),
        }),
    ))
}
