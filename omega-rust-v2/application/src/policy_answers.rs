//! Bind checked policy executions to frozen source occurrences and validate their returns.
use omega_interpreter::{Ceilings, PolicyReceipt, Usage, Value};
use omega_lowering::{FrozenPolicyInvocation, LoweredPackage, LoweringError, PolicyUnit};
use omega_preparation::{PlanIssuance, PolicyConstructionOccurrence, PreparedTrees, PreparedValue};
use omega_resolved::DeclarationId;
use std::sync::Arc;

/// Take the return of the execution `receipt` records as this unit's
/// answer. The receipt must name this unit's package and plan body by
/// address and this invocation's schema by allocation. A receipt for an
/// equal unit lowered again, or for a copy of the schema, is refused.
pub fn answer_policy(
    unit: &PolicyUnit,
    receipt: PolicyReceipt<'_>,
) -> Result<PolicyAnswer, LoweringError> {
    let refuse = |reason: &str| LoweringError::new(unit.selection_site(), reason);
    let invocation = unit
        .invocation()
        .ok_or_else(|| refuse("policy unit has no frozen invocation to answer"))?;
    if !std::ptr::eq(receipt.unit(), unit.package()) {
        return Err(refuse("policy receipt records the execution of a different unit"));
    }
    if !std::ptr::eq(receipt.entry(), unit.plan()) {
        return Err(refuse("policy receipt records the execution of a different entry"));
    }
    if !Arc::ptr_eq(
        receipt.schema(),
        &unit
            .schema()
            .expect("frozen invocation includes its schema")
            .storage,
    ) {
        return Err(refuse("policy receipt records an execution on a different schema backing"));
    }
    let (ceilings, usage) = (receipt.ceilings(), receipt.usage().clone());
    let returned = freeze(unit.package(), receipt.into_result()).map_err(refuse)?;
    Ok(PolicyAnswer {
        invocation: Arc::clone(invocation),
        returned: Arc::new(returned),
        ceilings,
        usage,
    })
}

/// What one admitted execution of a policy unit returned, in the vocabulary
/// preparation validates. It grants nothing by existing: lowering still asks
/// L4 to validate [`PolicyAnswer::returned`] before it publishes the domain.
#[derive(Debug)]
pub struct PolicyAnswer {
    invocation: Arc<FrozenPolicyInvocation>,
    returned: Arc<PreparedValue>,
    ceilings: Ceilings,
    usage: Usage,
}

impl PolicyAnswer {
    pub fn domain(&self) -> &DeclarationId {
        self.invocation.occurrence().domain()
    }

    /// The frozen invocation this answers.
    pub fn occurrence(&self) -> &PolicyConstructionOccurrence {
        self.invocation.occurrence()
    }

    /// The returned plan. Lowering validates this allocation and no copy of it.
    pub fn returned(&self) -> &Arc<PreparedValue> {
        &self.returned
    }

    /// The ceilings the root set for the execution.
    pub fn ceilings(&self) -> Ceilings {
        self.ceilings
    }

    /// The work and cells the execution used, with the schedules they were counted under.
    pub fn usage(&self) -> &Usage {
        &self.usage
    }
}

/// Raise an executed value into the frozen form preparation reads. A record
/// or case names its schema by the row of the unit that ran, and that row
/// names the source declaration. A byte view keeps its backing.
fn freeze(
    unit: &LoweredPackage,
    value: Value,
) -> Result<PreparedValue, &'static str> {
    let fields = |values: Vec<Value>| {
        values
            .into_iter()
            .map(|value| freeze(unit, value))
            .collect::<Result<Arc<[_]>, _>>()
    };
    let schema = |id: omega_lowering::DataId| {
        let row = unit.data.get(id.get() - 1).filter(|row| row.id == id);
        row.ok_or("policy return names a data row its unit does not have")
    };
    let declaration =
        |id: &omega_lowering::DeclarationId| omega_lowering::source_declaration(id).ok_or("policy return names a declaration with no source identity");
    Ok(match value {
        Value::Boolean(value) => PreparedValue::Bool(value),
        Value::Integer(value) => PreparedValue::Integer(value),
        Value::Bytes(backing) => PreparedValue::SharedBytes(backing),
        Value::Array(values) => PreparedValue::Array(fields(values)?),
        Value::Record { data, fields: values } => {
            let row = schema(data)?;
            if !row.cases.is_empty() || row.fields.len() != values.len() {
                return Err("policy return disagrees with its record schema");
            }
            PreparedValue::Record {
                owner: declaration(&row.declaration)?,
                fields: fields(values)?,
            }
        }
        Value::Case { data, case, fields: values } => {
            let row = schema(data)?;
            let alternative = row
                .cases
                .get(case)
                .filter(|alternative| alternative.fields.len() == values.len())
                .ok_or("policy return disagrees with its sum schema")?;
            PreparedValue::Sum {
                owner: declaration(&row.declaration)?,
                case: declaration(&alternative.declaration)?,
                fields: fields(values)?,
            }
        }
        // Validation reads no float and no reference in an access plan.
        Value::Shared(_) | Value::Float(_) => return Err("policy return holds a value no access plan has"),
    })
}

/// Validate every answer against the frozen package that will consume it.
fn admitted_issuers<'a>(
    prepared: &PreparedTrees<'_>,
    answers: &'a [PolicyAnswer],
) -> Result<std::collections::BTreeMap<&'a DeclarationId, PlanIssuance>, LoweringError> {
    let mut domains = std::collections::BTreeMap::new();
    for answer in answers {
        let domain = answer.domain();
        let refuse = |reason: String| LoweringError::new(answer.invocation.selection_site(), reason);
        if !prepared
            .typed()
            .access_policies
            .iter()
            .any(|selection| &selection.domain == domain)
        {
            return Err(refuse("policy answer names no selected policy of this package".into()));
        }
        if !answer.invocation.belongs_to(prepared) {
            return Err(refuse("policy answer belongs to a different preparation of this domain".into()));
        }
        let issuance = omega_preparation::read_returned_access_plan(prepared, domain, &answer.returned)
            .map_err(|reason| refuse(format!("executed access plan is refused: {reason}")))?;
        if domains.insert(domain, issuance).is_some() {
            return Err(refuse("domain access policy received more than one answer".into()));
        }
    }
    Ok(domains)
}

/// Complete the pending Psi domain rows using the returns of the selected checked policies.
pub fn lower_package_with_policy_answers(
    prepared: &PreparedTrees<'_>,
    answers: &[PolicyAnswer],
) -> Result<LoweredPackage, LoweringError> {
    if answers.is_empty() {
        return omega_lowering::lower_package(prepared);
    }
    let mut issuers = admitted_issuers(prepared, answers)?;
    for selection in &prepared.typed().access_policies {
        if !issuers.contains_key(&selection.domain) {
            return Err(omega_lowering::unanswered_policy_refusal(prepared, selection));
        }
    }
    let (mut package, domains) = omega_lowering::lower_pending_policies(prepared)?.into_parts();
    let denied_cast = domains
        .iter()
        .filter(|domain| issuers.get(&domain.source) == Some(&PlanIssuance::Nobody))
        .flat_map(|domain| domain.introductions.iter().map(move |cast| (cast, domain)))
        .min_by_key(|(cast, _)| (cast.machine.get(), &cast.site));
    if let Some((cast, domain)) = denied_cast {
        return Err(cast.refusal(domain));
    }
    for domain in domains {
        let decision = issuers
            .remove(&domain.source)
            .ok_or_else(|| LoweringError::new(&domain.selection, "pending domain has no executed issuer decision"))?;
        let decision = match decision {
            PlanIssuance::Unrestricted => omega_lowering::DomainIssuers::Unrestricted,
            PlanIssuance::Nobody => omega_lowering::DomainIssuers::Nobody,
        };
        let row = domain.row;
        package.domains.push(omega_lowering::LoweredDomain {
            issuers: decision,
            parameters: row.parameters,
            index_binders: row.index_binders,
            declaration: row.declaration,
            carrier: row.carrier,
            cleanup_target: row.cleanup_target,
            public: row.public,
            requires: row.requires,
            site: row.site,
        });
    }
    if let Some((domain, _)) = issuers.first_key_value() {
        let selection = prepared
            .typed()
            .access_policies
            .iter()
            .find(|selection| &selection.domain == *domain)
            .expect("admission verified the selected declaration");
        return Err(LoweringError::new(&selection.site, "executed policy has no pending Psi domain row"));
    }
    package
        .domains
        .sort_unstable_by(|left, right| left.declaration.cmp(&right.declaration));
    Ok(package)
}
