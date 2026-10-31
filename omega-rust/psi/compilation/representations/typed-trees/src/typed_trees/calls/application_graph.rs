//! Source declaration topology for the ordinary proof-application walk.
//!
//! These are live typed relationships, not checked obligations or installed
//! provider choices. Checking and preparation enrich the same graph with their
//! own evidence; neither reconstructs calls or target membership by spelling.

use super::super::TypedTrees;
use semantic_vocabulary::proof_applications::{
    ProofApplicationMachine, ProofApplicationPlan, ProofApplicationTargetMember,
    ProofDeclarationDependencies,
};
use symbols::SymbolHandle;

pub fn source_application_plan(typed: &TypedTrees) -> ProofApplicationPlan<SymbolHandle> {
    let mut plan = ProofApplicationPlan::default();
    // The same application graph also discovers Build provider demand. A
    // program without conditional scalar debt still invokes requirements.
    let mut declarations = crate::references::collect_program_declarations(typed);
    declarations.retain(|owner| typed.machine_by_symbol(*owner).is_none());
    for owner in typed.traits() {
        declarations.extend(
            typed
                .trait_machine_signatures(owner)
                .iter()
                .map(|signature| signature.symbol),
        );
        declarations.extend(
            owner
                .refinement_clauses
                .iter()
                .map(|row| row.signature.symbol),
        );
    }
    declarations.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
    declarations.dedup();
    plan.declarations = plan.symbols.insert_many(declarations.iter().copied());
    for (declaration, dependencies) in
        crate::references::collect_declaration_symbol_dependencies(typed)
    {
        let dependencies = dependencies
            .into_iter()
            .map(|symbol| {
                typed
                    .trait_machine_signature(symbol)
                    .map_or(symbol, |(owner, _)| owner.symbol)
            })
            .filter(|dependency| {
                typed.machine_by_symbol(*dependency).is_none() && declarations.contains(dependency)
            });
        let dependencies = plan.symbols.insert_many(dependencies);
        let signatures = match typed.trait_definition_by_symbol(declaration) {
            Some(owner) => plan.symbols.insert_many(
                typed
                    .trait_machine_signatures(owner)
                    .iter()
                    .map(|signature| signature.symbol)
                    .chain(
                        owner
                            .refinement_clauses
                            .iter()
                            .map(|row| row.signature.symbol),
                    ),
            ),
            None => Default::default(),
        };
        plan.declaration_dependencies
            .insert(ProofDeclarationDependencies {
                declaration,
                dependencies,
                signatures,
            });
    }
    for machine in typed.machines() {
        let states = plan.symbols.insert_many(
            typed
                .machine_states(machine)
                .iter()
                .map(|state| state.symbol),
        );
        let binders = plan.symbols.insert_many(
            typed
                .machine_kind_type_parameters(machine)
                .map(|binder| binder.symbol),
        );
        let family_members =
            typed
                .target_machine_families
                .iter()
                .find(|family| family.canonical == machine.symbol)
                .map(|family| {
                    plan.target_members
                        .insert_many(family.members.iter().map(|member| {
                            ProofApplicationTargetMember {
                                target: member.target.as_str().to_owned(),
                                machine: member.machine,
                            }
                        }))
                })
                .unwrap_or_default();
        plan.machines.insert(ProofApplicationMachine {
            machine: machine.symbol,
            states,
            binders,
            authored_target: typed.machine_authored_target(machine).map(str::to_owned),
            source_target: typed.source_target_context(machine).map(str::to_owned),
            family_interface: machine.target_family_interface,
            family_members,
            dynamic_realizations: Default::default(),
            product_selected: true,
        });
    }
    plan.provider_candidates
        .insert_many(typed.conformances().iter().flat_map(|conformance| {
            conformance
                .rows
                .iter()
                .map(|row| (row.requirement, row.realization_machine))
        }));
    for machine in typed.machines() {
        plan.provider_candidates.insert_many(
            typed
                .machine_trait_conformances(machine)
                .iter()
                .filter(|clause| clause.requirement_symbol.is_valid())
                .map(|clause| (clause.requirement_symbol, machine.symbol)),
        );
    }
    plan
}
