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
    EvaluatedConst, consider_generic_spelling, const_integer_type,
    evaluate_const_argument_expression, generic_const_integer_types,
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
    evaluate_const_fact_expression, evaluate_const_membership_fact,
    prove_const_literal_leaf_domain_constraints, prove_declared_const_domain_constraints,
};
use remainder::validate_anonymous_remainder;
pub(in crate::preparation) use templates::collect_type_positions;
pub(super) use templates::{
    capture_machine_runtime_template_names, collect_data_type_reference_positions,
    collect_machine_type_reference_positions, collect_selected_type_reference_positions,
    normalize_generic_template_const_expressions, replace_const_expression_names_from,
    replace_machine_const_expression_names_from,
};
pub(super) use values::{
    canonicalize_const_definition, canonicalize_const_expression,
    canonicalize_selected_const_definition, canonicalize_selected_declaration_value,
    canonicalize_selected_index_expression, checked_fact_integer, const_integer_in_envelope,
    syntax_type_identity, validate_const_index_type, validate_syntax_integer_range,
};

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
