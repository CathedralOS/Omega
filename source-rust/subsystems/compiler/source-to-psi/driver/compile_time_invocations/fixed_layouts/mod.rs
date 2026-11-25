//! Run the layout policy selected by an actual storage occurrence. The policy
//! receives its complete schema and target; only a checked result becomes
//! geometry for that application and target.

use checked_program_to_psi as production;
use std::sync::Arc;

use crate::source_state::references::{
    collect_program_type_occurrences, collect_type_symbol_dependencies,
};
use crate::source_state::{
    PlanLaidGeometry, PlanLaidLayout, PlanLaidRealizations, PlanLaidRetiredFieldIdentity,
    PlanLaidSchemaArgument, PlanLaidTargetLayout, TypedTrees,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use super::demanded_values::{DemandedValues, RoundProgram};
use super::{FixedLayoutTargetScope, PreparedBuildMachineProgram};
use crate::compile_time_invocations::{BuildTimeValue, InvocationTarget, SourceCompletionAccounting};

mod placements;
mod reflection;
mod schema;
#[cfg(test)]
mod tests;
mod validation;

/// One private slot observed while running this exact source layout policy.
#[derive(Debug, Clone, PartialEq)]
struct PrivateLayoutPlacementReceipt {
    operation_expression: crate::source_state::expression::ExpressionHandle,
    selected_slot: crate::source_state::expression::StaticMachineArgument,
    placement: BuildTimeValue,
}

/// A source invocation's ordinary return value and any sealed layout calls it executed.
#[derive(Debug, Clone, PartialEq)]
struct EvaluatedLayout {
    value: BuildTimeValue,
    private_layout_placements: Vec<PrivateLayoutPlacementReceipt>,
}

impl super::PreparedBuildMachineEntry {
    fn invoke_layout_policy(
        &self,
        produced: Arc<production::ProducedTerminalProgram>,
        target: InvocationTarget<'_>,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<EvaluatedLayout, String> {
        let sites = placements::executed_sites(self.accepted(), produced.view().module())?;
        let calls = sites.iter().map(|site| site.call).collect::<Vec<_>>();
        self.invoke_with_observations(
            Arc::clone(&produced),
            target,
            accounting,
            &calls,
            |value, events| {
                let private_layout_placements = placements::decode(
                    &terminal_interpreter::TerminalProgram::borrow(produced.view()),
                    &sites,
                    events,
                )?;
                Ok(EvaluatedLayout {
                    value,
                    private_layout_placements,
                })
            },
        )
    }
}

/// One storage application whose policy plans its layout for one target.
struct Demand {
    /// The deployment, as its position in the request's scopes.
    scope: usize,
    /// The first demanded declaration that writes the application. The
    /// policy runs as its request.
    requester: SymbolHandle,
    storage: crate::source_state::layout_storage::LayoutStorageArguments,
    /// Every declaration that writes the application, sorted. Each stores
    /// through the plan once it is installed.
    holders: Vec<SymbolHandle>,
    /// The declarations the application names: the sealed storage type, the
    /// stored type and the policy with its arguments.
    roots: Vec<SymbolHandle>,
}

/// The layout applications the request's deployments demand, one per
/// application and target.
pub(super) struct Layouts<'scope> {
    scopes: &'scope [FixedLayoutTargetScope],
    demands: Vec<Demand>,
}

impl<'scope> Layouts<'scope> {
    pub(super) fn demanded(
        typed: &mut TypedTrees,
        scopes: &'scope [FixedLayoutTargetScope],
    ) -> Result<Self, Vec<Diagnostic>> {
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
        let mut demanded = Self {
            scopes,
            demands: Vec::new(),
        };
        // Available target facts do not request geometry. With no computation
        // roots, retain the declarations' deferred requests without preparing
        // a whole-program occurrence inventory or running any layout policy.
        if scopes.iter().all(|scope| {
            scope.entry_declarations.is_empty() && scope.provider_declarations.is_empty()
        }) {
            super::frontier::refresh_evaluated_requests(typed)?;
            return Ok(demanded);
        }
        // Open library templates remain requests of their declarations.
        // Their concrete applications are judged when actually demanded.
        let closed_storage = |typed: &TypedTrees, reference| {
            let storage = typed.layout_storage_arguments(reference)?;
            let data = typed.closed_data_definition(storage.semantic_type)?;
            data.type_parameters.is_empty().then_some(storage)
        };
        let occurrences = collect_program_type_occurrences(typed);
        let mut requests = Vec::new();
        for (scope_index, scope) in scopes.iter().enumerate() {
            if scope.entry_declarations.is_empty() && scope.provider_declarations.is_empty() {
                continue;
            }
            let selection = scope
                .demanded_declarations(typed)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            for occurrence in &occurrences {
                if !selection.contains(typed.refusal_declaration(occurrence.lexical_owner)) {
                    continue;
                }
                let Some(storage) = closed_storage(typed, occurrence.reference) else {
                    continue;
                };
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
            super::frontier::refresh_evaluated_requests(typed)?;
            return Ok(demanded);
        }
        // Every declaration that writes a requested application holds its
        // plan, whichever deployment selects it.
        let key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut written = occurrences
            .iter()
            .filter_map(|occurrence| {
                let storage = closed_storage(typed, occurrence.reference)?;
                Some((
                    typed.normalized_type_identity(storage.application),
                    typed.refusal_declaration(occurrence.lexical_owner),
                ))
            })
            .collect::<Vec<_>>();
        written.sort_unstable_by_key(|(identity, holder)| (*identity, key(*holder)));
        written.dedup();
        demanded.demands = requests
            .into_iter()
            .map(|(scope, identity, requester, storage)| Demand {
                scope,
                requester,
                storage,
                holders: written
                    .iter()
                    .filter(|(written, _)| *written == identity)
                    .map(|(_, holder)| *holder)
                    .collect(),
                roots: collect_type_symbol_dependencies(typed, storage.application),
            })
            .collect();
        Ok(demanded)
    }

    fn data_name(&self, typed: &TypedTrees, value: usize) -> Option<String> {
        typed
            .closed_data_definition(self.demands[value].storage.semantic_type)
            .map(|data| data.name.to_string())
    }
}

impl DemandedValues for Layouts<'_> {
    fn len(&self) -> usize {
        self.demands.len()
    }

    fn holders(&self, value: usize) -> &[SymbolHandle] {
        &self.demands[value].holders
    }

    fn roots(&self, value: usize) -> &[SymbolHandle] {
        &self.demands[value].roots
    }

    fn description(&self, typed: &TypedTrees, value: usize) -> String {
        format!(
            "the layout of `{}` on `{}`",
            self.data_name(typed, value).unwrap_or_default(),
            self.scopes[self.demands[value].scope].target,
        )
    }

    fn source_span(&self, typed: &TypedTrees, value: usize) -> Option<source_text::SourceSpan> {
        typed
            .symbols
            .symbol_source_span(self.demands[value].requester)
    }

    /// The holders carry a layout-policy row until the plan is installed.
    fn serves_recorded_requests(&self) -> bool {
        true
    }

    fn evaluate(
        &mut self,
        value: usize,
        round: &RoundProgram,
        typed: &mut TypedTrees,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<(), Vec<Diagnostic>> {
        let demand = &self.demands[value];
        let scope = &self.scopes[demand.scope];
        if let Some(plan) = typed.plan_laid_layout_for_type(demand.storage.application)
            && plan
                .geometry_for_target(typed, Some(&scope.target))
                .is_some()
        {
            return Ok(());
        }
        let data_name = self
            .data_name(typed, value)
            .ok_or_else(|| vec![Diagnostic::error("demanded layout lost its semantic type")])?;
        // The policy is judged in the round's program. Installing an earlier
        // result of the same round cannot change it.
        evaluate_application(
            typed,
            round.layout_policies(),
            demand.requester,
            demand.storage,
            scope,
            accounting,
        )
        .map_err(|reason| {
            let mut diagnostic = Diagnostic::error(format!(
                "layout policy for `{data_name}` on `{}`: {reason}",
                scope.target,
            ));
            if let Some(span) = typed.symbols.symbol_source_span(demand.requester) {
                diagnostic = diagnostic.with_source_span(span);
            }
            vec![diagnostic]
        })
    }
}

fn evaluate_application(
    typed: &mut TypedTrees,
    program: &PreparedBuildMachineProgram,
    requester: SymbolHandle,
    storage: crate::source_state::layout_storage::LayoutStorageArguments,
    scope: &FixedLayoutTargetScope,
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), String> {
    let facts = scope
        .facts
        .as_ref()
        .ok_or("target has no complete layout facts")?;
    let policy = typed.static_operand_from_type_reference(storage.policy)?;
    let requirement_symbol = storage
        .plan_requirement(typed, requester)
        .map_err(|issue| issue.message())?;
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
    // The accepted call can use a captured body. Store the policy declaration
    // that the captured body implements in the published layout.
    let accepted = entry.accepted_source();
    let (selected, member, _) = accepted
        .named_conformance_call_member(entry.call())?
        .ok_or("layout invocation lost its selected policy member")?;
    let declared_machine = member.realization_machine;
    if selected.symbol != policy.symbol
        || typed.machine_by_symbol(declared_machine).is_none()
        || accepted
            .machine_instantiation_path(entry.symbol(), declared_machine)
            .is_none()
    {
        return Err("layout policy result lost its original source declaration".into());
    }
    let result = entry.invoke_layout_policy(
        Arc::new(entry.produce_invocation()?),
        InvocationTarget {
            name: Some(&scope.target),
            layout_target: Some(facts),
            carrier_layout: None,
            address_model: None,
        },
        accounting,
    )?;
    let validated_layout = validation::record_plan(typed, data, &fields, &result.value)?;
    let data_symbol = data.symbol;
    let data_name = data.name.to_string();
    let geometry = PlanLaidGeometry {
        validated_layout,
        private_callback_demands: placements::close(
            typed,
            accepted,
            &result.private_layout_placements,
            requester,
            &policy,
            storage.semantic_type,
            program.exact_toolchain_sources(),
        )?,
    };
    placements::validate(&geometry, &fields, facts)?;
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
        target: crate::source_state::name::Identifier::generated(&scope.target),
        machine: declared_machine,
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
            policy_plan_machine_symbol: declared_machine,
            schema_argument: None,
            realizations: PlanLaidRealizations::TargetInputs(span),
        });
    }
    Ok(())
}
