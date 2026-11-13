//! Retain source obligations which product preparation will need after checking.
//! Entry code, compiler-run calling policies and constant recipes share one
//! source census. Keeping their declarations adds no runtime invocation edge.

use compilation_products::ComputedBuildConfig;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) struct ProductCodeRoots {
    pub machines: Vec<SymbolHandle>,
    pub boundaries: Vec<SymbolHandle>,
}

pub(crate) fn collect(
    typed: &typed_trees::FrozenTypedTrees,
    build: &ComputedBuildConfig,
    defaults: &[build_time_evaluation::provider_declarations::TargetProviderDefaults],
) -> Result<ProductCodeRoots, Vec<Diagnostic>> {
    let mut machines = build.config.program_roots();
    machines.extend(calling_policy_roots(typed)?);
    machines.extend(
        typed
            .constant_recipe_closure
            .receipts
            .iter()
            .filter(|receipt| receipt.provider_machine.is_valid())
            .map(|receipt| receipt.provider_machine),
    );
    machines.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    machines.dedup();
    Ok(ProductCodeRoots {
        machines,
        boundaries: boundary_roots(typed, build, defaults)?,
    })
}

fn calling_policy_roots(
    typed: &typed_trees::FrozenTypedTrees,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let signatures = typed.boundary_signatures();
    let calling = signatures.calling();
    let mut machines = Vec::new();
    for request in calling.requests() {
        let Ok(instances) = calling.instances(request) else {
            continue;
        };
        for instance in instances {
            let Ok(policy) = instance.policy else {
                continue;
            };
            match typed.target_machine_families.iter().find(|family| {
                family.canonical == policy
                    || family.members.iter().any(|member| member.machine == policy)
            }) {
                Some(family) => machines.extend(family.members.iter().map(|member| member.machine)),
                None => machines.push(policy),
            }
        }
    }
    machines.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    machines.dedup();
    Ok(machines)
}

fn boundary_roots(
    typed: &typed_trees::FrozenTypedTrees,
    build: &ComputedBuildConfig,
    defaults: &[build_time_evaluation::provider_declarations::TargetProviderDefaults],
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let context =
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut roots = provider_planning::target_machines::provider_selection_roots(
        context,
        defaults
            .iter()
            .flat_map(|row| &row.selections)
            .chain(&build.config.provider_selections),
    )?;
    // Canonical float requirements are realized by the target's table
    // rather than selected, so they are boundary roots on their own.
    roots.extend(
        typed_trees::float_providers::catalog_realization_requirements(typed)
            .into_iter()
            .filter(|symbol| context.contains(*symbol)),
    );
    for relationship in &build.config.assumed_bindings {
        roots.extend([relationship.required, relationship.supplied]);
    }
    roots.extend(
        typed
            .constant_recipe_closure
            .receipts
            .iter()
            .filter(|receipt| receipt.provider_machine.is_valid())
            .map(|receipt| receipt.requirement),
    );
    roots.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    roots.dedup();
    Ok(roots)
}
