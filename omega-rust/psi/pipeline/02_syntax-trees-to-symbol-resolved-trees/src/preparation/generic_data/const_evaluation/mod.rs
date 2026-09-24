//! Exact source-constant evaluation used by declaration retention and synthesis.

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
    canonicalize_closed_domain_indices, const_expression_contains_name, domain_index_parameters,
    integer_literal_value, qualified_const_name,
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
    collect_machine_type_reference_positions, collect_type_reference_positions,
    normalize_generic_template_const_expressions, replace_const_expression_names_from,
    replace_machine_const_expression_names_from,
};
pub(super) use values::{
    canonicalize_const_definition, canonicalize_const_expression,
    canonicalize_selected_const_definition, canonicalize_selected_declaration_value,
    canonicalize_selected_index_expression, checked_fact_integer, const_integer_in_envelope,
    syntax_type_identity, validate_const_index_type, validate_syntax_integer_range,
};
