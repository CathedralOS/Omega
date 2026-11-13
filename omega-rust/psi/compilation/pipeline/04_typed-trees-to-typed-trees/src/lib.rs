//! The retired source-preparation entry points preserve request data and refuse
//! work that has no structural producer. They publish no checking judgments.

#![forbid(unsafe_code)]

mod preparation;

pub use preparation::{
    PreparationAdmission, PreparationRequest, PreparedProductRequirement, TypedPreparation,
    bind_pre_specialization_authored_selections, prepare_constant_execution,
    prepare_execution_applications, prepare_product_operand_application,
    prepare_product_requirement_operand, prepare_requirement_expression_views,
    prepare_retained_product_application, prepare_retained_product_requirement_operand,
    prepare_typed_trees, restore_copied_call_sources,
};
