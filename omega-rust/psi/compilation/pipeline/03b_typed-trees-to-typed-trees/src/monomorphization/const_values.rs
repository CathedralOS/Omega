//! Materialize selected const binders without capturing same-spelled names.

use super::Candidate;
use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstValue, DecodedCanonicalConstValue, boolean_literal_spelling,
};
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod structured;

pub(super) fn substitute(
    source: Option<&dyn ::validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    expression_start: Option<usize>,
    runtime_expression_roots: &[ExpressionHandle],
) -> Result<(), Diagnostic> {
    // Relocated operand graphs are inputs, not new binder occurrences in the
    // executable region being rewritten. Keep the original sweep boundary.
    let expression_end = program.expression_table.expression_count() + 1;
    for (index, ((binder, name, declared_type), binding)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
        .enumerate()
    {
        // A runtime-bound `Value` binder is realized as an ordinary parameter:
        // executable uses were remapped to it during cloning, so any occurrence
        // still reachable from the clone sits in a static context the runtime
        // subject cannot fill. Template-owned expressions keep their authored
        // binder spelling, so the scan follows clone expression roots rather
        // than an arena index bound.
        if candidate.runtime_value_bindings[index].is_some() {
            let stray = runtime_expression_roots.iter().any(|root| {
                let mut tree = Vec::new();
                validation::expression_traversal::collect_expression_tree(
                    program, *root, &mut tree,
                );
                tree.into_iter().any(|handle| {
                    matches!(
                        program.expression_table.expression(handle),
                        ExpressionNode::Name(path)
                            if binder.is_valid()
                                && path.symbol == *binder
                                && path.head_symbol == *binder
                                && program
                                    .expression_table
                                    .name_path_members(path.members)
                                    .len()
                                    == 1
                    )
                })
            });
            if stray {
                return Err(Diagnostic::error(format!(
                    "value parameter `{name}` of machine `{}` is bound to a runtime argument and \
                     cannot appear in a static type, contract, or const context",
                    candidate.template.template_name,
                )));
            }
            continue;
        }
        let occurrences = program
            .expression_table
            .iter_expressions()
            .filter(|(handle, _)| {
                expression_start.is_none_or(|start| handle.arena_index() as usize >= start)
                    && (handle.arena_index() as usize) < expression_end
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
        // A demanded application can bind against a declaration snapshot whose
        // appended payload rows are absent from the executable arena. Relocate
        // both actual and declared carrier through the same graph copier before
        // materialization; their source symbols and const meaning stay intact.
        // The selected candidate/application remains owned by its issuing graph.
        let (binding, declared_type) = if let Some(source) = source {
            let payload_expression_start = program.expression_table.expression_count() + 1;
            let declared_type = super::body_cloning::copy_type_reference(
                Some(source),
                program,
                *declared_type,
                &[],
            );
            super::body_cloning::copy_cloned_expression_type_payloads(
                Some(source),
                program,
                payload_expression_start,
                &[],
            )?;
            let binding = binding
                .map(|binding| {
                    super::body_cloning::copy_type_reference_payloads(
                        Some(source),
                        program,
                        binding,
                        &[],
                    )
                })
                .transpose()?;
            (binding, declared_type)
        } else {
            (*binding, *declared_type)
        };
        for occurrence in occurrences {
            // Each use owns its literal children and their checking context.
            let replacement = binding
                .and_then(|binding| value(program, binding, declared_type))
                .ok_or_else(|| Diagnostic::error(format!(
                    "machine `{}` cannot materialize const parameter `{name}` as its declared value type `{}`",
                    candidate.template.template_name,
                    program.display_type_reference(declared_type),
                )))?;
            *program.expression_table.expression_mut(occurrence) = replacement;
        }
    }
    Ok(())
}

pub(super) fn value(
    program: &mut TypedTrees,
    binding: TypeReferenceHandle,
    declared_type: TypeReferenceHandle,
) -> Option<ExpressionNode> {
    // Public header/result copying and ordinary inference retain the same
    // closed const operand graph. Contextual provider cloning can consume it
    // before the next round's selection scan, so value materialization owns
    // that shared normalization too. This preserves an independently typed
    // operand's canonical carrier; it never lands it under the destination.
    // Unknown/open graphs remain unchanged and unavailable below. The original
    // expression and tuple stay in their issuing arena; no source is replayed
    // or captured specialization identity changed by consuming its value.
    let binding = if let Some((TypeReferenceNode::Named { name, .. }, meaning)) =
        super::application_arguments::normalized_const_type_argument_with_meaning(program, binding)
    {
        // Reuse the existing canonical value row without replacing the actual
        // selected binding whose application/specialization already owns it.
        super::application_arguments::intern_closed_const_with_meaning(
            program,
            name.as_str(),
            meaning,
        )
    } else {
        binding
    };
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
        return boolean_literal_spelling(name.as_str()).map(ExpressionNode::Boolean);
    }
    let literal = program.static_const_literal_from_type_reference(binding)?;
    integer_value(program, declared_type, literal)
}

fn integer_value(
    program: &dyn ::validation::CheckingInput,
    declared_type: TypeReferenceHandle,
    literal: IntegerLiteral,
) -> Option<ExpressionNode> {
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    let landed_type = primitive.landed_landing()?;
    Some(ExpressionNode::Integer(
        literal.with_landing(IntegerLanding {
            landed_type,
            domain: program
                .type_reference_table
                .arithmetic_domain(declared_type),
        }),
    ))
}
