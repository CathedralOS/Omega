//! Exact source-constant evaluation used by declaration retention and synthesis.

use super::{ConstDefinition, Diagnostic, HashMap, Identifier, SyntaxTrees};

mod anonymous;
mod arguments;
mod domains;
mod fact_values;
mod facts;
mod remainder;
mod templates;
mod values;

pub(super) use arguments::{
    EvaluatedConst, const_integer_type, evaluate_const_argument_expression,
    generic_const_integer_types, prepare_closed_application,
};
pub(super) use domains::{
    canonicalize_closed_domain_indices, domain_index_parameters, integer_literal_value,
    qualified_const_name,
};
pub(crate) use fact_values::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, evaluate_const_fact_binary,
};
pub(in crate::preparation::generic_data) use facts::prove_const_leaf_domain_constraints;
pub(crate) use facts::{
    evaluate_const_fact_expression, evaluate_const_membership_fact, prove_const_literal_predicates,
    prove_declared_const_domain_constraints,
};
use remainder::validate_anonymous_remainder;
pub(in crate::preparation) use templates::collect_type_positions;
pub(super) use templates::{
    capture_machine_runtime_template_names, collect_data_type_reference_positions,
    collect_lexical_type_reference_positions, collect_selected_type_reference_positions,
    collect_stored_type_reference_positions, normalize_generic_template_const_expressions,
    replace_const_expression_names_from, replace_machine_const_expression_names_from,
};
pub(in crate::preparation) use values::canonicalize_selected_const_definition;
pub(super) use values::{
    canonicalize_const_definition, canonicalize_const_expression,
    canonicalize_selected_declaration_value, checked_fact_integer, const_integer_in_envelope,
    syntax_type_identity, validate_const_index_type, validate_syntax_integer_range,
};
pub(crate) use values::{canonicalize_selected_index_expression, closed_fixed_array_length};

/// A `name` selection that names a module-scoped const value or const
/// declaration is rejected up front — the shared module-constant gate both
/// evaluation walkers apply before their own normalization.
fn module_constant_selection_gate(
    syntax: &SyntaxTrees,
    name: &Identifier,
    const_values: &HashMap<String, i128>,
    const_definitions: &HashMap<String, ConstDefinition>,
) -> Result<(), Diagnostic> {
    if const_values.contains_key(name.as_str()) || const_definitions.contains_key(name.as_str()) {
        crate::preparation::generic_data::module_constants::reject_module_constant_selection(
            syntax,
            name.as_str(),
            name.source_span(),
        )
        .map_err(|reason| Diagnostic::error(reason).with_source_span(name.source_span()))?;
    }
    Ok(())
}

/// Keep syntax synthesis's evaluated display independently of its original
/// operand graph. The encoding checks only the completed numeric result; its
/// result label grants no width or policy. Typed admission receives the
/// original expression/declaration through the normalization's operand owner.
pub(super) fn replace_integer_argument(
    syntax: &mut SyntaxTrees,
    argument: syntax_trees::types::TypeReferenceHandle,
    value: i128,
) {
    let reference = match syntax.type_references.type_reference(argument) {
        syntax_trees::types::TypeReferenceNode::Named(name) => name.source_span(),
        syntax_trees::types::TypeReferenceNode::ConstExpression(expression) => {
            syntax.expressions.source_span(*expression)
        }
        _ => unreachable!("integer argument has no issuing operand"),
    };
    let encoding =
        language_semantics::const_value::CanonicalConstIdentity::integer("integer", value).encoding;
    syntax.type_references.retain_const_argument_normalization(
        argument,
        reference,
        encoding,
        [],
        [],
    );
    if let syntax_trees::types::TypeReferenceNode::ConstExpression(expression) =
        syntax.type_references.type_reference(argument)
    {
        let expression = *expression;
        syntax
            .type_references
            .retain_const_argument_expression(argument, expression);
    }
    syntax.type_references.replace_type_reference(
        argument,
        syntax_trees::types::TypeReferenceNode::Named(Identifier::generated(value.to_string())),
    );
}
