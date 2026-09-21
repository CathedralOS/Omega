//! Evidence argument and requirement rewrites.

use crate::monomorphization::{
    Candidate, StateSignature, StaticMachineArgument, SymbolHandle, TypedTrees,
};

pub(crate) fn evidence_argument_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)> {
    candidate
        .template
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
        .filter_map(|(parameter, binding)| {
            let binder = parameter.binder?;
            let binding = binding.as_ref()?;
            let selected = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == binding.symbol)?;
            Some((
                binder,
                binding.symbol,
                selected.alias.clone().unwrap_or_else(|| {
                    typed_trees::name::Identifier::generated("<unnamed-conformance>")
                }),
            ))
        })
        .collect()
}

pub(crate) struct EvidenceRequirementRewrite {
    pub(crate) placeholder: SymbolHandle,
    pub(crate) target: SymbolHandle,
    pub(crate) name: typed_trees::name::Identifier,
    pub(crate) application_arguments: Box<[StaticMachineArgument]>,
    pub(crate) dispatch: typed_trees::typed_trees::StaticRequirementDispatch,
}

pub(crate) fn evidence_requirement_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<EvidenceRequirementRewrite> {
    let mut rewrites = Vec::new();
    for (parameter, binding) in candidate
        .template
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
    {
        let (Some(binder), Some(binding)) = (parameter.binder, binding.as_ref()) else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == parameter.carrier)
        else {
            continue;
        };
        let Some(selected) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == binding.symbol)
        else {
            continue;
        };
        let Some(rows) = program.closed_conformance_rows(selected) else {
            continue;
        };
        let application =
            crate::conformance::conformance_applications::close_conformance_application(
                program, binding,
            )
            .expect("validated selected conformance must close during specialization");
        let mut requirements = Vec::new();
        collect_evidence_requirement_closure(
            program,
            trait_definition,
            &mut Vec::new(),
            &mut requirements,
        );
        let placeholders = program.symbols.child_handles(binder).into_iter().flatten();
        for (placeholder, requirement) in placeholders.zip(requirements) {
            let Some(row) = rows
                .iter()
                .find(|row| row.requirement == requirement.symbol)
            else {
                continue;
            };
            rewrites.push(EvidenceRequirementRewrite {
                placeholder,
                target: row.realization_state,
                name: typed_trees::name::Identifier::generated(
                    row.realization_name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .unwrap_or(row.realization_name.as_str()),
                ),
                application_arguments: binding
                    .application
                    .as_ref()
                    .map_or_else(Box::default, |application| application.arguments.clone()),
                dispatch: typed_trees::typed_trees::StaticRequirementDispatch {
                    application_report_fingerprint: application.report_fingerprint,
                    application_commitment: application.commitment,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                },
            });
        }
    }
    rewrites
}

pub(crate) fn collect_evidence_requirement_closure<'program>(
    program: &'program TypedTrees,
    trait_definition: &'program typed_trees::trait_definition::TraitDefinition,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);
    output.extend(program.trait_machine_signatures(trait_definition).iter());
    // A transparent refinement declares no requirements of its own; a binder
    // over it selects the base conformance's requirement namespace. This
    // order must mirror the binder placeholder symbols inserted at symbol
    // resolution (own machines, then the refined base, then parents).
    if let Some(base) = &trait_definition.refines
        && let Some(base_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == base.symbol)
    {
        collect_evidence_requirement_closure(program, base_trait, visited, output);
    }
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == parent.symbol)
        else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, visited, output);
    }
}
