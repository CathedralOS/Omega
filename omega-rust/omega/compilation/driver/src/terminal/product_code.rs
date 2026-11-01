//! Publish the complete Product code universe independently of selected execution.
//!
//! Authored entry roots, policy bodies, constant providers and boundary catalogs
//! belong to source completion. A target may later select and admit a use of
//! this code, but cannot change its retention roots or trigger policy-only code.

use compilation_products::{CheckedProductSource, ComputedBuildConfig, ProductTerminalPublication};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::{
    PortableTerminalProgram, PsiCompilationObserver, TerminalMachineSelection, TerminalProgramRoots,
};
use std::sync::Arc;
use symbols::SymbolHandle;

pub(crate) struct ProductCodeSource<'a> {
    pub program: &'a Arc<checked_trees::CheckedTrees>,
    pub inventory: &'a checked_trees::PreparedProviderInventory,
    pub sources: &'a package_compilation::CheckedSourceCustody,
    pub build: &'a ComputedBuildConfig,
    pub defaults: &'a [build_time_evaluation::provider_declarations::TargetProviderDefaults],
    pub publication: &'a ProductTerminalPublication,
}

impl<'a> ProductCodeSource<'a> {
    pub(crate) fn checked(source: &'a CheckedProductSource) -> Self {
        Self {
            program: &source.program,
            inventory: &source.provider_inventory,
            sources: &source.sources,
            build: &source.computed_build_config,
            defaults: &source.provider_defaults,
            publication: &source.terminal_publication,
        }
    }

    pub(crate) fn publish(
        &self,
        selections: &OptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.publication.retain(selections, || {
            let typed = &self.program.typed;
            let entries = self.build.config.program_roots();
            let default = entries.first().copied().map(|default| {
                typed
                    .target_machine_families
                    .iter()
                    .find(|family| {
                        (family.canonical == default
                            || family
                                .members
                                .iter()
                                .any(|member| member.machine == default))
                            && typed
                                .machine_by_symbol(family.canonical)
                                .is_some_and(|machine| machine.target_family_interface)
                    })
                    .map_or(default, |family| family.canonical)
            });
            let mut machines = entries;
            machines.extend(self.calling_policy_roots()?);
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
            let boundaries = self.boundary_roots()?;
            let machine = match default {
                Some(machine) => TerminalMachineSelection::Symbol(machine),
                None => TerminalMachineSelection::Declarations,
            };
            self.produce(
                machine,
                TerminalProgramRoots {
                    machines: &machines,
                    boundaries: &boundaries,
                },
                selections,
                observer,
            )
        })
    }

    /// Named inspection is a different requested root, not a policy compiler.
    /// It uses the same Psi operation but cannot replace Product publication.
    pub(crate) fn inspect(
        &self,
        name: &str,
        selections: &OptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        let boundaries = self.boundary_roots()?;
        self.produce(
            TerminalMachineSelection::Name(name),
            TerminalProgramRoots {
                machines: &[],
                boundaries: &boundaries,
            },
            selections,
            observer,
        )
        .map(Arc::new)
    }

    fn produce(
        &self,
        machine: TerminalMachineSelection<'_>,
        roots: TerminalProgramRoots<'_>,
        selections: &OptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PortableTerminalProgram, Vec<Diagnostic>> {
        psi_compiler::TerminalProductionRequest {
            publication: terminal_codec::TerminalPublication::Portable,
            checked: self.program,
            external_declarations: None,
            provider_inventory: Some(self.inventory),
            exact_toolchain_sources: self.sources.exact_toolchain_sources(),
            machine,
            roots,
            optimization_selections: selections.project_psi().selections().clone(),
        }
        .produce(Default::default(), observer)
        .map_err(|error| error.diagnostics())?
        .into_shared_portable()
        .map_err(|error| vec![Diagnostic::error(error.to_string())])
    }

    pub(crate) fn calling_policy_roots(&self) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        let calling = self
            .inventory
            .services_for(self.program)
            .map_err(|error| vec![error])?
            .signatures()
            .calling();
        let typed = &self.program.typed;
        let mut machines = Vec::new();
        for request in calling.requests() {
            // Admission consumes these refusals in authored order. Retention
            // must not turn an unavailable relationship into an executable row.
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
                    Some(family) => {
                        machines.extend(family.members.iter().map(|member| member.machine))
                    }
                    None => machines.push(policy),
                }
            }
        }
        machines.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        machines.dedup();
        Ok(machines)
    }

    fn boundary_roots(&self) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        let typed = &self.program.typed;
        let context = provider_planning::ProviderPlanningContext::new(
            typed,
            source::DependencyScope::Product,
        );
        let mut roots = provider_planning::target_machines::provider_selection_roots(
            context,
            self.defaults
                .iter()
                .flat_map(|row| &row.selections)
                .chain(&self.build.config.provider_selections),
        )?;
        // Nonexecuting assumption operands retain intermediate declarations;
        // retention is not runtime supply or root consent.
        for relationship in &self.build.config.assumed_bindings {
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
}
