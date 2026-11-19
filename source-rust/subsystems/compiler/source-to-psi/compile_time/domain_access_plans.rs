//! Run the access policy a demanded domain selects and record what its plan
//! says (`wiki/spec/language/domains.md`, "Domain-owned read and write
//! authority"; `wiki/spec/resources/access.md`, "Plans and admitted supply").
//!
//! `domain Ticket::Stamped access StampAccess;` leaves who may write
//! `ticket as +Stamped` to `StampAccess::plan`. Psi 05 cannot answer that
//! from the declaration, so before it runs, this pass calls the selected
//! `plan` once per demanded domain through the checked selected-call entry
//! (`PreparedBuildMachineProgram::entry_selected`), with the domain
//! subject's semantic schema as its argument. Only a result it can state in
//! full becomes `CheckedDomainAccessPlan` on the domain's policy:
//!
//! - the returned schema must be the argument the policy received;
//! - every route must be a description the program's own checked
//!   `reflect::callable(..)` queries produce, matched on declaration,
//!   callable application and source revision together, so a route names
//!   exactly one declaration of this program;
//! - reader and writer rules stay attached for the canonical operation
//!   checks against the actual function and borrowed storage;
//! - field observation demands and required reach remain unjudged until
//!   the backing and lifecycle checks can establish them.
//!
//! A plan this pass cannot judge is recorded as
//! `AccessPlanJudgment::Unjudged` with the reason: every use that needs the
//! plan stays refused, exactly as for a plan nobody ran.
//!
//! `Issuance::Only { routes: [] }` stays a deny-all list and
//! `Issuance::Unrestricted` stays unrestricted. Domains the Build prelude
//! declares (the Build seal) are supplied by the Build itself, not by
//! source issuance, and are left as they are. A domain whose policy
//! application already carries a deferred refusal stays refused where it is
//! demanded.

#[path = "domain_access_plans/reflection.rs"]
mod reflection;
use reflection::{
    Description, authenticated_route, callable_descriptions, core_reflection_data, subject_schema,
};

#[path = "domain_access_plans/returned_plan.rs"]
mod returned_plan;
#[cfg(test)]
#[path = "domain_access_plans/tests.rs"]
mod tests;

use std::sync::Arc;

use crate::source_state::TypedTrees;
use crate::source_state::domain::{
    AccessPlanJudgment, CheckedDomainAccessPlan, CheckedIssuance, DomainIssuance,
};
use diagnostics::Diagnostic;
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use symbols::SymbolHandle;

use crate::compile_time::PreparedBuildMachineProgram;
use crate::compile_time::{InvocationTarget, SourceCompletionAccounting};

/// Evaluates the access plan of every policy-governed domain `request`
/// demands and records each checked result on that domain.
pub(crate) fn evaluate(
    program: &mut TypedTrees,
    request: crate::production::TerminalSourceCompilation<'_>,
    authority: Option<Arc<dyn crate::compile_time::BuildTimeSelectionAuthority>>,
    source_consent: Option<Arc<dyn crate::compile_time::InvocationSourceConsent>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    let mut pending = program
        .domain_definitions
        .iter()
        .filter(|(_, domain)| {
            domain
                .access_policy
                .as_ref()
                .is_some_and(|policy| policy.plan == AccessPlanJudgment::Unevaluated)
                && !program.symbols.is_build_prelude_symbol(domain.symbol)
                && program.deferred_refusal(domain.symbol).is_none()
        })
        .map(|(handle, domain)| (handle, domain.symbol))
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(());
    }
    let demanded = crate::compile_time::demanded_declarations(program, request)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    pending.retain(|(_, domain)| demanded.contains(*domain));
    if pending.is_empty() {
        return Ok(());
    }
    // One immutable source owner serves every plan; recording one result
    // cannot change the program a later policy is judged in.
    let prepared = PreparedBuildMachineProgram::prepare_for(
        program,
        authority,
        request.purpose,
        source_consent,
    )?;
    let descriptions =
        callable_descriptions(program).map_err(|reason| vec![Diagnostic::error(reason)])?;
    // A plan this pass cannot judge stays refused at each use that needs
    // it (an `as +D`, a read or write inside a governed value), as an
    // unevaluated plan is; the reason is kept on the policy. The domain's
    // values still pass whole, and the rest of the program goes on.
    let mut judged = Vec::with_capacity(pending.len());
    for (handle, domain) in pending {
        let judgment = match evaluate_domain(program, &prepared, &descriptions, domain, accounting)
        {
            Ok(plan) => AccessPlanJudgment::Checked(plan),
            Err(reason) => AccessPlanJudgment::Unjudged(reason),
        };
        judged.push((handle, judgment));
    }
    for (handle, judgment) in judged {
        let policy = program
            .domain_definitions
            .get_mut(handle)
            .access_policy
            .as_mut()
            .ok_or_else(|| vec![Diagnostic::error("evaluated access plan lost its policy")])?;
        policy.plan = judgment;
    }
    Ok(())
}

fn evaluate_domain(
    typed: &TypedTrees,
    prepared: &PreparedBuildMachineProgram,
    descriptions: &[Description],
    domain: SymbolHandle,
    accounting: &mut SourceCompletionAccounting,
) -> Result<CheckedDomainAccessPlan, String> {
    let definition = typed
        .domain_definition(domain)
        .ok_or("the domain has no declaration")?;
    let DomainIssuance::Planned(policy) = definition.issuance() else {
        return Err("the domain has no unevaluated policy".into());
    };
    let schema_type = core_reflection_data(typed, ReflectionDeclaration::TypeSchema)
        .ok_or("the core `TypeSchema` declaration is not in the program")?;
    let conformance = typed
        .conformance_by_symbol(policy.implementation())
        .ok_or("the selected policy is not a named implementation")?;
    // The `access` clause names an implementation of the toolchain's
    // `AccessPolicy`, whose one requirement is `plan`.
    if typed.core_vocabulary(conformance.trait_symbol) != Some(CoreVocabulary::AccessPolicy) {
        return Err("the selected policy does not implement the core `AccessPolicy`".into());
    }
    let [row] = conformance.rows.as_slice() else {
        return Err("the selected policy has no single `plan` requirement".into());
    };
    if typed.core_vocabulary(row.requirement) != Some(CoreVocabulary::AccessPolicyPlan) {
        return Err("the selected policy does not implement the core `AccessPolicy::plan`".into());
    }
    let (schema_graph, schema, revision) = subject_schema(typed, definition, schema_type)?;
    let expected_selection = policy_identity(typed, domain, &policy.selection)?;
    let entry = prepared.entry_selected(
        domain,
        domain,
        &policy.selection,
        row.requirement,
        std::slice::from_ref(&schema),
    )?;
    let accepted = entry.accepted_source();
    let (selected, member, _) = accepted
        .named_conformance_call_member(entry.call())?
        .ok_or("the plan call lost its selected policy member")?;
    let plan_machine = member.realization_machine;
    if selected.symbol != policy.implementation() || typed.machine_by_symbol(plan_machine).is_none()
    {
        return Err("the plan call lost its policy's source declaration".into());
    }
    let receiver = accepted
        .declaration_operand_from_expression(entry.call().receiver)
        .map_err(str::to_owned)?;
    if policy_identity(accepted, domain, &receiver)? != expected_selection {
        return Err("the plan call changed its domain's complete selected application".into());
    }
    let first_receipt = accounting.receipts().len();
    entry.invoke_recorded(
        InvocationTarget {
            name: None,
            layout_target: None,
            carrier_layout: None,
            address_model: None,
        },
        accounting,
    )?;
    let [crate::compile_time::PureEvaluationRecord::Invocation(receipt)] =
        &accounting.receipts()[first_receipt..]
    else {
        return Err("the plan evaluation lost its own invocation receipt".into());
    };
    if !std::ptr::eq(receipt.source(), entry.accepted_source())
        || !std::ptr::eq(receipt.request_source(), entry.source())
        || receipt.machine() != entry.symbol()
        || receipt.requester() != domain
        || receipt.call()? != entry.call()
        || receipt.arguments() != entry.arguments()
        || receipt.invocation()?.is_none()
    {
        return Err("the plan result belongs to another selected invocation".into());
    }
    let normalized =
        returned_plan::normalize_for_schema(receipt, schema_graph, &schema, &revision)?;
    // Complete data is retained by the common normalizer. Until operation
    // consumers enforce an axis, recording it as checked would drop a rule.
    if !normalized.observations.is_empty() {
        return Err("no product route yet: an access plan with a nonzero `field_count`".into());
    }
    if normalized.reach.services().len() != 0 {
        return Err("no product route yet: an access plan that requires boundary reach".into());
    }
    let issuance = match &normalized.issuance {
        None => CheckedIssuance::Unrestricted,
        Some(routes) => {
            let mut checked = Vec::with_capacity(routes.len());
            for route in routes {
                let route = authenticated_route(descriptions, route)?;
                if !checked.contains(&route) {
                    checked.push(route);
                }
            }
            CheckedIssuance::Only(checked)
        }
    };
    Ok(CheckedDomainAccessPlan {
        plan_machine,
        normalized,
        issuance,
    })
}

fn policy_identity(
    typed: &TypedTrees,
    author: SymbolHandle,
    selected: &crate::source_state::expression::StaticMachineArgument,
) -> Result<String, String> {
    let supplier = typed
        .static_operand_region_supplier(selected, author)
        .map_err(str::to_owned)?;
    crate::source_state::type_identity::portable_applied_index_identity(
        typed,
        crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(selected),
        supplier,
        &[],
    )
    .and_then(|identity| typed.type_identity_text(identity))
    .ok_or_else(|| "the access policy lost its complete selected application identity".into())
}
