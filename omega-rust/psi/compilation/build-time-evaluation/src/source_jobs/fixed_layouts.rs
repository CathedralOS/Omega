//! Run the layout policy selected by an actual storage occurrence. The policy
//! receives its complete schema and target; only a checked result becomes
//! geometry for that application and target.

use std::sync::Arc;

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::references::{DeclarationSelection, collect_program_type_occurrences};
use typed_trees::{
    PlanLaidGeometry, PlanLaidLayout, PlanLaidRealizations, PlanLaidRetiredFieldIdentity,
    PlanLaidSchemaArgument, PlanLaidTargetLayout, TypedTrees,
};

use super::{FixedLayoutTargetScope, PreparedBuildMachineProgram};
use crate::{
    BuildTimeSelectionAuthority, BuildTimeValue, InvocationTarget, SourceCompletionAccounting,
};

mod reflection;
mod schema;
mod validation;

pub(super) fn evaluate(
    typed: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
    purpose: source::DependencyScope,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    if !typed.plan_laid_layouts.is_empty() || !typed.plan_laid_target_layouts.is_empty() {
        return Err(vec![Diagnostic::error(
            "layout policy input contains plans without their producing invocation",
        )]);
    }
    // A containing schema may consume another application's layout. The
    // profile must therefore name the same immutable facts across every
    // scope, before any policy runs or nested geometry is consulted.
    let mut targets = scopes.iter().collect::<Vec<_>>();
    targets.sort_unstable_by(|left, right| left.target.cmp(&right.target));
    for pair in targets.windows(2) {
        if pair[0].target == pair[1].target && pair[0].facts != pair[1].facts {
            return Err(vec![Diagnostic::error(
                "layout scopes disagree on one target's immutable facts",
            )]);
        }
    }
    for scope in targets {
        if let Some(facts) = &scope.facts {
            facts
                .validate()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if facts.profile != scope.target {
                return Err(vec![Diagnostic::error(
                    "layout scope and immutable target facts disagree",
                )]);
            }
        }
    }
    // Available target facts do not request geometry. With no computation
    // roots, retain the declarations' deferred requests without preparing
    // a whole-program occurrence inventory or running any layout policy.
    if scopes
        .iter()
        .all(|scope| scope.entry_declarations.is_empty() && scope.provider_declarations.is_empty())
    {
        return crate::source_frontier::refresh_layout_policy_requests(typed);
    }
    let occurrences = collect_program_type_occurrences(typed);
    let mut requests = Vec::new();
    for (scope_index, scope) in scopes.iter().enumerate() {
        if scope.entry_declarations.is_empty() && scope.provider_declarations.is_empty() {
            continue;
        }
        let selection = DeclarationSelection::from_computation_roots(
            typed,
            scope.entry_declarations.iter().copied().chain(
                scope
                    .provider_declarations
                    .iter()
                    .map(|row| row.provider_type.symbol),
            ),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for occurrence in &occurrences {
            if !selection.contains(typed.refusal_declaration(occurrence.lexical_owner)) {
                continue;
            }
            let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                continue;
            };
            let Some(data) = typed.closed_data_definition(storage.semantic_type) else {
                continue;
            };
            // Open library templates remain requests of their declarations.
            // Their concrete applications are judged when actually demanded.
            if !data.type_parameters.is_empty() {
                continue;
            }
            requests.push((
                scope_index,
                typed.normalized_type_identity(storage.application),
                occurrence.lexical_owner,
                storage,
            ));
        }
    }
    requests.sort_by_key(|(scope, identity, _, _)| (*scope, *identity));
    requests.dedup_by_key(|(scope, identity, _, _)| (*scope, *identity));
    if requests.is_empty() {
        return crate::source_frontier::refresh_layout_policy_requests(typed);
    }
    // Keep one immutable source owner for all policy calls. Installing an
    // earlier result cannot change the program used to judge a later policy.
    let program = PreparedBuildMachineProgram::prepare_for(typed, authority, purpose, consent)?;
    for (scope_index, _, requester, storage) in requests {
        let scope = &scopes[scope_index];
        if let Some(plan) = typed.plan_laid_layout_for_type(storage.application)
            && plan
                .geometry_for_target(typed, Some(&scope.target))
                .is_some()
        {
            continue;
        }
        let data_name = typed
            .closed_data_definition(storage.semantic_type)
            .ok_or_else(|| vec![Diagnostic::error("demanded layout lost its semantic type")])?
            .name
            .to_string();
        if let Err(reason) =
            evaluate_application(typed, &program, requester, storage, scope, accounting)
        {
            let mut diagnostic = Diagnostic::error(format!(
                "layout policy for `{data_name}` on `{}`: {reason}",
                scope.target,
            ));
            if let Some(span) = typed.symbols.symbol_source_span(requester) {
                diagnostic = diagnostic.with_source_span(span);
            }
            return Err(vec![diagnostic]);
        }
    }
    crate::source_frontier::refresh_layout_policy_requests(typed)
}

fn evaluate_application(
    typed: &mut TypedTrees,
    program: &PreparedBuildMachineProgram,
    requester: SymbolHandle,
    storage: typed_trees::layout_storage::LayoutStorageArguments,
    scope: &FixedLayoutTargetScope,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), String> {
    let facts = scope
        .facts
        .as_ref()
        .ok_or("target has no complete layout facts")?;
    let policy = typed.static_operand_from_type_reference(storage.policy)?;
    let (requirement_symbol, environment) = {
        let contract = typed
            .trait_definition_by_symbol(storage.contract)
            .ok_or("layout storage lost its exact core contract")?;
        let [requirement] = typed.trait_machine_signatures(contract) else {
            return Err("fixed layout contract has no exact plan requirement".into());
        };
        if typed.trait_type_parameters(contract).len() != 1 {
            return Err("core layout contract lost its exact semantic parameter".into());
        }
        // Authenticate the actual applied policy, including generic maps.
        // Its declaring trait supplies T; an unrelated map argument does not.
        let applied = typed
            .applied_requirement_scope(requester, storage.policy)
            .and_then(|scope| scope.declaring_scope(typed, storage.contract))
            .ok_or("selected policy does not retain the core layout application")?;
        (requirement.symbol, applied.environment())
    };
    let arguments = typed.materialize_trait_application_arguments(&environment)?;
    let [semantic] = arguments.as_slice() else {
        return Err("core layout contract lost its exact semantic argument".into());
    };
    if !typed
        .type_reference_table
        .contains_type_reference(*semantic)
        || typed.normalized_type_identity(*semantic)
            != typed.normalized_type_identity(storage.semantic_type)
    {
        return Err("selected layout policy belongs to another semantic type".into());
    }
    let data = typed
        .closed_data_definition(storage.semantic_type)
        .ok_or("layout application lost its closed semantic type")?;
    let fields = reflection::record_fields(typed, data, facts)?;
    let mut retired = data.retired_identities.clone();
    retired.sort_unstable();
    if retired.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("layout schema repeats a retired identity".into());
    }
    let argument = schema::record_argument(&fields, &retired)?;
    // The public call entrance checks the selected plan and its body using
    // this exact policy application.
    let entry = program.entry_selected(
        requester,
        requester,
        &policy,
        requirement_symbol,
        &[argument, BuildTimeValue::LayoutTarget(facts.clone())],
    )?;
    if !typed.symbols.symbols().nodes().is_valid(entry.symbol()) {
        return Err("layout policy result lost its original source declaration".into());
    }
    let result = entry.invoke_recorded(
        InvocationTarget {
            name: Some(&scope.target),
            layout_target: Some(facts),
            carrier_layout: None,
            address_model: None,
        },
        accounting,
    )?;
    let geometry = PlanLaidGeometry {
        validated_layout: validation::record_plan(typed, data, &fields, result.value())?,
        private_callback_demands: Vec::new(),
    };
    let data_symbol = data.symbol;
    let data_name = data.name.to_string();
    let retired = retired
        .into_iter()
        .map(|identity| PlanLaidRetiredFieldIdentity { identity })
        .collect::<Vec<_>>();
    let field_symbols = fields
        .iter()
        .map(|field| field.schema_field)
        .collect::<Vec<_>>();
    let schema_argument = PlanLaidSchemaArgument {
        abi_version: 4,
        fields: typed.plan_laid_schema_fields.insert_many(fields),
        retired_field_identities: typed
            .plan_laid_retired_field_identities
            .insert_many(retired),
    };
    let target_layout = PlanLaidTargetLayout {
        schema_argument: Some(schema_argument),
        target_facts: facts.clone(),
        target: typed_trees::name::Identifier::generated(&scope.target),
        machine: entry.symbol(),
        geometry,
    };
    // A later target extends this same application, retaining each target's
    // schema and geometry separately. No target overwrites another's plan.
    if let Some(index) = typed.plan_laid_layouts.iter().position(|plan| {
        typed.normalized_type_identity(plan.application)
            == typed.normalized_type_identity(storage.application)
    }) {
        let mut alternatives = match typed.plan_laid_layouts[index].realizations {
            PlanLaidRealizations::TargetInputs(span) => {
                typed.plan_laid_target_layouts.span_or_empty(span).to_vec()
            }
            PlanLaidRealizations::Invariant(_) => {
                return Err(
                    "target-specific evaluation cannot overwrite an invariant layout".into(),
                );
            }
        };
        if alternatives
            .iter()
            .any(|row| row.target.as_str() == scope.target)
        {
            return Err("layout target has more than one producing invocation".into());
        }
        alternatives.push(target_layout);
        let span = typed.plan_laid_target_layouts.insert_many(alternatives);
        let plan = &mut typed.plan_laid_layouts[index];
        plan.target_inputs.push(facts.clone());
        plan.realizations = PlanLaidRealizations::TargetInputs(span);
    } else {
        let span = typed.plan_laid_target_layouts.insert_many([target_layout]);
        typed.plan_laid_layouts.push(PlanLaidLayout {
            application: storage.application,
            semantic_type: storage.semantic_type,
            target_inputs: vec![facts.clone()],
            data_name,
            data_symbol,
            schema_symbol: data_symbol,
            schema_field_symbols: field_symbols.clone(),
            field_symbols,
            policy_symbol: policy.symbol,
            policy_plan_machine_symbol: entry.symbol(),
            schema_argument: None,
            realizations: PlanLaidRealizations::TargetInputs(span),
        });
    }
    Ok(())
}
