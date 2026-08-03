use crate::pipeline::stages::{
    source_files_to_syntax_trees_for_engine, symbol_resolved_trees_to_typed_trees,
    syntax_trees_to_symbol_resolved_trees, typed_trees_to_checked_trees,
};
use crate::pipeline::timing::CompileTimings;
use omega_core::diagnostics::Diagnostic;
use psi_checked_trees::CheckedTrees;
use std::path::Path;
use std::sync::Arc;

/// Psi-checked semantics paired with the Omega-owned provider realization
/// selected for one engine run. The semantic program deliberately does not
/// retain target/provider installation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompilation {
    program: CheckedTrees,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    task_activations: omega_task_plans::TaskActivationPlanSet,
}

impl CheckedCompilation {
    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub const fn task_activations(&self) -> &omega_task_plans::TaskActivationPlanSet {
        &self.task_activations
    }

    pub fn into_program(self) -> CheckedTrees {
        self.program
    }
}

impl std::ops::Deref for CheckedCompilation {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        &self.program
    }
}

impl std::ops::DerefMut for CheckedCompilation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.program
    }
}

/// Runs ONLY the four frontend stages (lex/parse -> symbol resolution -> typing ->
/// checking) and returns the in-memory `CheckedTrees` program. No backend lowering,
/// no file output. This is the source-of-truth semantic representation that the
/// reference interpreter (`omega-interpreter`) evaluates as a differential oracle for
/// the native backend.
pub fn compile_to_checked(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut timings = CompileTimings::default();

    // `native: false` — the interpreter path keeps the abstract `boundary trait Gui`
    // (its own headless stub, item #9); only the native-image pipeline substitutes the
    // darwin `MacosGui` provider (task #57).
    let (_source_file_count, mut syntax) =
        source_files_to_syntax_trees_for_engine(root_path, target_name, false, &mut timings)?;
    crate::pipeline::const_generic_calls::evaluate_const_generic_calls(&mut syntax.syntax_trees)?;
    crate::pipeline::trait_defaults::synthesize_trait_defaults(&mut syntax.syntax_trees)?;
    let placed_view_records =
        crate::pipeline::placed_views::desugar_placed_views(&mut syntax.syntax_trees)?;
    // PLAN-LAID VALUE TYPES (layouts L4), desugar half -- exactly as the full
    // `compile` pipeline does.
    crate::pipeline::generic_instances::desugar_generic_data_instances(&mut syntax.syntax_trees)?;
    let plan_laid_records =
        crate::pipeline::plan_laid::desugar_plan_laid_value_types(&mut syntax.syntax_trees)?;
    // TARGET-SCOPED MACHINES -- exactly as the full `compile` pipeline does:
    // the interpreter runs the SELECTED target's implementations.
    let target_default_machine_names = crate::pipeline::target_machines::filter_target_machines(
        &mut syntax.syntax_trees,
        target_name,
    )?;
    let build_file_machine_names: Vec<String> = syntax
        .files
        .iter()
        .filter(|file| file.path.file_name().and_then(|name| name.to_str()) == Some("build.omg"))
        .flat_map(|file| file.root_items.iter())
        .filter_map(|handle| match syntax.syntax_trees.root_item(*handle) {
            psi_syntax_trees::item::Item::Machine(machine) => {
                Some(machine.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect();
    let syntax_trees = syntax.syntax_trees.clone();
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
    let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
    // COMPTIME STAGE 1: substitute const-evaluated fixed-array lengths before
    // checking, exactly as the full `compile` pipeline does.
    crate::pipeline::const_lengths::evaluate_const_array_lengths(&mut typed)?;
    crate::pipeline::const_domain_facts::evaluate_const_domain_facts(&mut typed)?;
    // PLAN-LAID VALUE TYPES, plan half: evaluate + validate + record.
    crate::pipeline::plan_laid::compute_plan_laid_layouts(&mut typed, &plan_laid_records)?;
    crate::pipeline::placed_views::validate_placed_view_plans(&mut typed, &placed_view_records)?;
    // WIRE PLANS (mint arc rung 2a): mirror the full pipeline so tests see
    // the same derived plans the codec selection consumes.
    crate::pipeline::wire_plans::compute_wire_plans(&mut typed)?;
    let _boundary_calling_plan_realizations =
        crate::pipeline::calling_policy_plans::compute_boundary_calling_plans(&mut typed)?;
    let build_config =
        crate::pipeline::build_config::compute_build_config(&typed, &build_file_machine_names)?;
    let target_provider_defaults = crate::pipeline::build_config::compute_target_provider_defaults(
        &typed,
        &target_default_machine_names,
    )?;
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let provider_plans =
        crate::pipeline::provider_plans::derive_satisfies_plans(&syntax_trees, &typed, target_name);
    let selected_native_target = omega_target::NativeTarget::from_omega_target_name(target_name)
        .unwrap_or_else(|_| omega_target::NativeTarget::host());
    let diagnostics =
        crate::pipeline::provider_plans::validate_provider_plan_candidates(&typed, &provider_plans);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let selected_provider_plans = crate::pipeline::provider_plans::select_provider_plan_names(
        &provider_plans,
        selected_native_target,
        &target_provider_defaults,
        &build_config.provider_selections,
    )?;
    crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
        &typed,
        &provider_plans,
        &selected_provider_plans,
    )?;
    let mut checked = typed_trees_to_checked_trees(typed, &mut timings)?;
    let checked_program = Arc::get_mut(&mut checked.program)
        .expect("checked program must be uniquely owned before engine handoff");
    let selected_provider_plan_facts =
        crate::pipeline::provider_plans::bind_selected_provider_plan_facts(
            checked_program,
            &provider_plans,
            &selected_provider_plans,
            &build_config.grants,
        )?;
    crate::pipeline::operator_adapter_dispatch::rewrite_selected_operator_adapter_calls(
        checked_program,
        &selected_provider_plan_facts,
    )?;
    crate::pipeline::float_intrinsic_dispatch::rewrite_selected_float_intrinsic_calls(
        checked_program,
        &selected_provider_plan_facts,
    )?;
    // Preserve boundary-requirement proof/evidence at checking time, then
    // redirect only execution to the selected checked adapter.
    crate::pipeline::adapter_dispatch::rewrite_adapter_calls(
        &mut checked_program.typed,
        &selected_provider_plan_facts,
    )?;
    let task_activations = crate::pipeline::task_plans::elaborate_task_activation_plans(
        checked_program,
        &selected_provider_plan_facts,
        selected_native_target,
    )?;

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    Ok(CheckedCompilation {
        program: Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone()),
        selected_provider_plans: selected_provider_plan_facts,
        task_activations,
    })
}
