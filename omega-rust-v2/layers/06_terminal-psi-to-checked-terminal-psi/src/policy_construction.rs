//! Relate the frozen construction arguments to the checked open caller and callee.
//! This judgment establishes the data construction under its supplied schema.
//! Execution admission, schema validation and resource accounting remain separate.
use crate::{CheckedPolicyCall, FieldOrigin, storage};
use omega_psi::{PolicyCasePayload, PolicyConstructionField, PolicyConstructionInputs, TypeExpressionStep as Step};

#[derive(Debug, Eq, PartialEq)]
pub struct CheckedPolicyConstruction {
    inputs: PolicyConstructionInputs,
}

impl CheckedPolicyConstruction {
    pub fn inputs(&self) -> &PolicyConstructionInputs {
        &self.inputs
    }
}

/// Match each frozen constructor field to its proved transfer. Preserve the
/// package and revision for comparison by the preparation consuming this result.
pub fn check_policy_construction(
    call: &CheckedPolicyCall<'_>,
    inputs: &PolicyConstructionInputs,
) -> Result<CheckedPolicyConstruction, &'static str> {
    let selected = call.selection();
    let constructor = call.constructor();
    let result = constructor.result();
    if inputs.domain != selected.domain
        || inputs.conformance != selected.conformance.declaration
        || inputs.arguments != selected.arguments
        || inputs.plan != selected.plan.declaration
        || inputs.selection_site != selected.site
    {
        return Err("construction belongs to a different selected policy");
    }
    if inputs.constructor != constructor.machine().declaration
        || inputs.construction_site != result.site
        || inputs.result != result.ty
        || inputs.fields.len() != constructor.fields().len()
    {
        return Err("prepared result does not match the checked constructor occurrence");
    }
    let expected_payload = if call.empty_list_field().is_some() {
        PolicyCasePayload::EmptySlice
    } else {
        PolicyCasePayload::NoFields
    };
    for ((id, input), (field, origin)) in inputs.fields.iter().zip(constructor.fields()) {
        if id != *field {
            return Err("prepared field does not match the checked result field");
        }
        match (input, origin) {
            (PolicyConstructionField::Schema(copied), FieldOrigin::CopySharedFormal { formal: 0, ty })
                if copied.record == inputs.schema.record && std::sync::Arc::ptr_eq(&copied.storage, &inputs.schema.storage) =>
            {
                storage::check_schema_storage(constructor.unit(), ty, &copied.record)?;
            }
            (PolicyConstructionField::Issuance { data, case, payload }, FieldOrigin::MoveFormal { formal: 1, ty })
                if case == call.issuance_case()
                    && *payload == expected_payload
                    && matches!(ty.0.last(), Some(Step::Named { declaration, .. }) if declaration == data) => {}
            (PolicyConstructionField::Zero(actual), FieldOrigin::Zero(expected)) if actual == *expected => {}
            _ => return Err("prepared field does not implement the checked constructor transfer"),
        }
    }
    Ok(CheckedPolicyConstruction { inputs: inputs.clone() })
}
