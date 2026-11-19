/* Exact source-constant evaluation used by declaration retention and synthesis. */

use bignum::BigInt;

mod anonymous;
mod arguments;
mod domains;
mod fact_values;
mod facts;
mod initializers;
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
pub(in crate::resolution::declarations::generic_data) use facts::prove_const_leaf_domain_constraints;
pub(crate) use facts::{
    const_type_requires_declaration_proof, evaluate_const_fact_expression,
    evaluate_const_membership_fact,
};
pub(crate) use initializers::fold_closed_scalar_initializers;
use remainder::validate_anonymous_remainder;
pub(in crate::resolution::declarations) use templates::collect_type_positions;
pub(super) use templates::{
    capture_machine_runtime_template_names, collect_data_type_reference_positions,
    collect_selected_type_reference_positions, collect_stored_type_reference_positions,
    normalize_generic_template_const_expressions, replace_const_expression_names_from,
    replace_machine_const_expression_names_from,
};
pub(in crate::resolution::declarations) use values::canonicalize_selected_const_definition;
pub(crate) use values::closed_fixed_array_length;
pub(super) use values::{
    ConstPredicateAdmission, canonicalize_selected_declaration_value,
    canonicalize_selected_declaration_value_with_admission, canonicalize_selected_index_expression,
    const_integer_within_budget, syntax_type_identity, validate_syntax_integer_range,
};
