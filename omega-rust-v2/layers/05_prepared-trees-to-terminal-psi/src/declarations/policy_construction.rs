//! Project the arguments observed in preparation into their defining Psi identities.
use super::type_expression::{declaration_term, type_expression};
use crate::{LoweringError, PolicyCasePayload, PolicyConstructionField, PolicyConstructionInputs, PolicySchemaValue, identity};
use omega_prepared::{PolicyFieldInput, PreparedTrees, PreparedValue};
use omega_typed::{Site, ValueType};

pub(super) fn schema_value(
    value: &PreparedValue,
    site: &Site,
) -> Result<PolicySchemaValue, LoweringError> {
    if let PreparedValue::Record { owner, fields } = value
        && let [PreparedValue::SharedBytes(bytes)] = fields.as_ref()
    {
        return Ok(PolicySchemaValue {
            record: identity::declaration(owner, site)?,
            storage: bytes.clone(),
        });
    }
    Err(LoweringError::new(site, "policy schema must carry its immutable byte storage"))
}

/// Supply the values of the constructor reached in the domain's actual
/// invocation. No selected policy is authorized by these inputs.
pub fn lower_policy_construction_inputs(
    prepared: &PreparedTrees<'_>,
    selection: &omega_typed::AccessPolicySelection,
) -> Result<PolicyConstructionInputs, LoweringError> {
    let domain = &selection.domain;
    if !prepared.typed().access_policies.contains(selection) {
        return Err(LoweringError::new(&selection.site, "policy selection is absent from the prepared package"));
    }
    let at = &selection.site;
    let invocation = prepared
        .policy_invocation(domain)
        .ok_or_else(|| LoweringError::new(at, "access policy construction inputs are pending"))?;
    let c = invocation.construction;
    let region = ValueType::InferredLifetime {
        site: Box::new(at.clone()),
        index: 0,
    };
    if c.machine.arguments.as_slice() != std::slice::from_ref(&region) || !c.machine.enclosing_arguments.is_empty() {
        return Err(LoweringError::new(&c.site, "policy constructor does not use the selected schema region"));
    }
    // Bind the known concrete region back to its position in the checked
    // constructor signature. Any other inferred lifetime remains unencodable.
    let bound_type = |ty: &ValueType| {
        let rebound = ty.rewrite(0, &|subterm, _| {
            (subterm == &region).then(|| ValueType::Parameter {
                owner: c.machine.machine.clone(),
                index: 0,
            })
        });
        type_expression(&rebound, &c.site)
    };
    let mut fields = Vec::with_capacity(c.fields.len());
    for (field, input) in &c.fields {
        let value = match input {
            PolicyFieldInput::Zero(ty) => PolicyConstructionField::Zero(bound_type(ty)?),
            PolicyFieldInput::Written(PreparedValue::Sum { owner, case, fields }) => {
                let payload = match fields.as_ref() {
                    [] => PolicyCasePayload::NoFields,
                    [PreparedValue::Array(routes)] if routes.is_empty() => PolicyCasePayload::EmptySlice,
                    _ => return Err(LoweringError::new(&c.site, "unsupported prepared policy case payload")),
                };
                PolicyConstructionField::Issuance {
                    data: identity::declaration(owner, &c.site)?,
                    case: identity::declaration(case, &c.site)?,
                    payload,
                }
            }
            PolicyFieldInput::Written(value) => PolicyConstructionField::Schema(schema_value(value, &c.site)?),
        };
        fields.push((identity::declaration(field, &c.site)?, value));
    }
    Ok(PolicyConstructionInputs {
        domain: identity::declaration(domain, at)?,
        conformance: identity::declaration(&selection.policy, at)?,
        arguments: selection
            .arguments
            .iter()
            .map(|arg| declaration_term(arg, at))
            .collect::<Result<_, _>>()?,
        plan: identity::declaration(&selection.plan_machine, at)?,
        selection_site: at.clone(),
        package_key: *invocation.package_key,
        subject_revision: invocation.subject_revision.to_vec(),
        schema: schema_value(invocation.schema, at)?,
        constructor: identity::declaration(&c.machine.machine, &c.site)?,
        construction_site: c.site.clone(),
        result: bound_type(&c.carrier)?,
        fields,
    })
}
