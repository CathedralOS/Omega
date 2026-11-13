//! Source zero eligibility uses the lowered-form checker's proof operation.
//! A refusal keeps the declaration's establishment obligation in place.

pub fn data_predicates_admit_zero(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
) -> bool {
    lowered_discharge::prove_data_zero(program.typed_program(), definition, &mut Default::default())
        .is_ok()
}

pub(crate) fn domain_predicates_admit_zero(
    program: &dyn crate::CheckingInput,
    carrier: typed_trees::types::TypeReferenceHandle,
    constraint: &typed_trees::types::DomainConstraint,
) -> bool {
    lowered_discharge::prove_domain_zero(
        program.typed_program(),
        carrier,
        constraint,
        &mut Default::default(),
    )
    .is_ok()
}

pub fn is_arm_pattern_marker(statement: &typed_trees::statement::StatementNode) -> bool {
    matches!(
        statement,
        typed_trees::statement::StatementNode::LocalData(local)
            if local.role == language_core::LocalBindingRole::ArmPattern
    )
}
