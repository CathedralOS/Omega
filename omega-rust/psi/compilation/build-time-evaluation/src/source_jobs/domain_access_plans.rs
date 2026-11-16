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

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use language_semantics::DomainEstablishmentRoute;
use language_semantics::reflection::{CallableDescription, SemanticSchemaGraph};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::{
    AccessPlanJudgment, CheckedDomainAccessPlan, CheckedIssuance, DomainIssuance,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::references::StaticArgumentCall;

use super::PreparedBuildMachineProgram;
use crate::{BuildTimeValue, InvocationTarget, SourceCompletionAccounting};

/// Evaluates the access plan of every policy-governed domain `request`
/// demands and records each checked result on that domain.
pub fn evaluate(
    program: &mut TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    source_consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
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
    let demanded = super::constant_jobs::demanded_declarations(program, request)
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
    let [crate::PureEvaluationRecord::Invocation(receipt)] =
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
        super::access_plan_values::normalize_for_schema(receipt, schema_graph, &schema, &revision)?;
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
    selected: &typed_trees::expression::StaticMachineArgument,
) -> Result<String, String> {
    let supplier = typed
        .static_operand_region_supplier(selected, author)
        .map_err(str::to_owned)?;
    typed_trees::type_identity::portable_applied_index_identity(
        typed,
        typed_trees::type_identity::IndexIdentityArgument::StaticArgument(selected),
        supplier,
        &[],
    )
    .and_then(|identity| typed.type_identity_text(identity))
    .ok_or_else(|| "the access policy lost its complete selected application identity".into())
}

/// The data the core reflection vocabulary names `which`.
fn core_reflection_data(typed: &TypedTrees, which: ReflectionDeclaration) -> Option<SymbolHandle> {
    typed
        .data_definitions()
        .iter()
        .find(|data| typed.core_vocabulary(data.symbol) == Some(CoreVocabulary::Reflection(which)))
        .map(|data| data.symbol)
}

/// The `TypeSchema` of the domain's subject, asked for by the domain's
/// author: the same graph `reflect::schema<Subject>()` returns there.
fn subject_schema(
    typed: &TypedTrees,
    definition: &typed_trees::domain::DomainDefinition,
    schema_type: SymbolHandle,
) -> Result<(SemanticSchemaGraph, BuildTimeValue, Vec<u8>), String> {
    let subject = typed
        .closed_data_definition(definition.target_type)
        .ok_or("no product route yet: an access plan for a subject that is not closed data")?;
    let authority = typed_trees::reflection::construction::schema_query_authority(
        typed,
        definition.symbol,
        subject.symbol,
    )?;
    let graph = typed_trees::reflection::construction::construct_semantic_schema_graph_for_type(
        typed,
        definition.target_type,
        authority,
    )?;
    let revision =
        typed_trees::reflection::construction::declaration_source_revision(typed, subject.symbol)?;
    // Ordinary result snapshots use the canonical record identity, not its
    // short source name. Construct the input with that same identity so the
    // complete returned value can be compared without discarding its type.
    let schema_identity = typed
        .normalized_nominal_type_identity(schema_type)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or("the core TypeSchema has no canonical record identity")?;
    let schema = BuildTimeValue::Struct {
        type_name: schema_identity,
        fields: vec![(
            "storage".to_owned(),
            BuildTimeValue::Text(graph.runtime_image()),
        )],
    };
    Ok((graph, schema, revision))
}

/// One description a `reflect::callable(..)` query of this program returns,
/// with the route it names.
struct Description {
    route: DomainEstablishmentRoute,
    case: &'static str,
    identities: [Vec<u8>; 3],
}

/// Every callable description the program's checked `reflect::callable`
/// queries produce. A plan's route is accepted only as one of these: the
/// bytes alone grant nothing (`reflect.omg`, `CallableInfo`).
fn callable_descriptions(typed: &TypedTrees) -> Result<Vec<Description>, String> {
    let mut descriptions = Vec::new();
    for (expression, node) in typed.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let query = typed
            .machine_by_symbol(call.target_symbol)
            .or_else(|| typed.machine_holding_state(call.target_symbol));
        if !query.is_some_and(|query| {
            typed.core_vocabulary(query.symbol)
                == Some(CoreVocabulary::Reflection(ReflectionDeclaration::Callable))
        }) {
            continue;
        }
        // Read the query as Psi 05 does (`read/reflection.rs`,
        // `declaration_query`): one declaration path as its only argument,
        // described for the call's lexical author.
        let [operand] = typed.expression_table.expression_handles(call.arguments) else {
            continue;
        };
        let Ok(selected) = typed.declaration_operand_from_expression(*operand) else {
            continue;
        };
        let owner = StaticArgumentCall::Expression(expression, call).lexical_owner(typed);
        let in_scope = typed
            .symbols
            .symbol_provenance_source_span(owner)
            .is_some_and(|author| {
                selected.symbol.is_valid()
                    && typed
                        .symbols
                        .source_reference_may_select_symbol(author, selected.symbol)
            });
        // A query Psi 05 would refuse produces no description either.
        let Ok((requirement, identities)) =
            typed_trees::reflection::construction::callable_description_identity(
                typed, &selected, owner,
            )
        else {
            continue;
        };
        if !in_scope {
            continue;
        }
        let route = if requirement {
            let (owner, _) = typed
                .trait_machine_signature(selected.symbol)
                .ok_or("a requirement description lost its trait")?;
            if owner.is_boundary {
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait: owner.symbol,
                    requirement: selected.symbol,
                }
            } else {
                DomainEstablishmentRoute::CheckedRequirement {
                    trait_definition: owner.symbol,
                    requirement: selected.symbol,
                }
            }
        } else {
            let machine = typed
                .machine_by_symbol(selected.symbol)
                .or_else(|| typed.machine_holding_state(selected.symbol))
                .ok_or("a machine description lost its declaration")?;
            DomainEstablishmentRoute::ExactMachine {
                machine: machine.symbol,
            }
        };
        descriptions.push(Description {
            route,
            case: if requirement {
                "Requirement"
            } else {
                "Machine"
            },
            identities,
        });
    }
    Ok(descriptions)
}

/// The declaration a returned `CallableInfo` names, matched on every byte
/// it carries.
fn authenticated_route(
    descriptions: &[Description],
    route: &CallableDescription,
) -> Result<DomainEstablishmentRoute, String> {
    let variant = if route.0 { "Requirement" } else { "Machine" };
    let identities = route.1.each_ref().map(Vec::as_slice);
    let mut routes = descriptions
        .iter()
        .filter(|description| {
            description.case == variant
                && description
                    .identities
                    .iter()
                    .zip(identities)
                    .all(|(expected, actual)| expected.as_slice() == actual)
        })
        .map(|description| description.route);
    let route = routes
        .next()
        .ok_or("a plan route names no callable this program describes")?;
    if routes.any(|other| other != route) {
        return Err("a plan route names more than one declaration".into());
    }
    Ok(route)
}
