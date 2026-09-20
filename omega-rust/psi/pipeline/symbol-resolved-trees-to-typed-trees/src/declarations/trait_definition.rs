use crate::lowerer::Lowerer;
use crate::signatures::callable_signature::lower_state_signature;
use crate::signatures::type_parameters::lower_type_parameters;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    trait_definition: &resolved::trait_definition::TraitDefinition,
) -> Result<typed::trait_definition::TraitDefinition, Diagnostic> {
    let mut typed_trait = typed::trait_definition::TraitDefinition {
        symbol: trait_definition.symbol,
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
        name: crate::lowerer::name::lower_name(&trait_definition.name),
        lifetime_parameters: trait_definition
            .lifetime_parameters
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect(),
        type_parameters: arena::HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        requires: arena::HandleSpan::empty(),
        machines: arena::HandleSpan::empty(),
        refines: None,
        refinement_clauses: Vec::new(),
    };

    typed_trait.type_parameters = lower_type_parameters(lowerer, trait_definition.type_parameters)?;

    for bound in &trait_definition.conformance_bounds {
        let mut arguments = Vec::new();
        for argument in lowerer.source_trees.child_type_references(bound.arguments) {
            arguments.push(crate::type_reference::lower_type_reference_into_table(
                lowerer, argument,
            )?);
        }
        let selected_conformance = bound
            .selected_conformance
            .as_ref()
            .map(|argument| {
                crate::expressions::expression::lower_static_machine_argument(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    argument,
                )
            })
            .transpose()?;
        typed_trait
            .conformance_bounds
            .push(typed::machine::GenericConformanceBound {
                binder: bound.binder,
                binder_name: bound
                    .binder_name
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                subject: bound.subject,
                subject_name: crate::lowerer::name::lower_name(&bound.subject_name),
                carrier: bound.carrier,
                carrier_name: crate::lowerer::name::lower_name(&bound.carrier_name),
                arguments,
                selected_conformance,
            });
    }

    for requirement in lowerer
        .source_trees
        .trait_requirements(trait_definition.requires)
    {
        crate::type_reference::retain_type_reference_selection(
            lowerer.source_trees,
            &mut lowerer.typed_trees,
            &requirement.name,
            requirement.symbol,
            lowerer.type_reference_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
        let mut arguments = arena::HandleSpan::empty();
        let source_arguments = lowerer
            .source_trees
            .child_type_references(requirement.arguments)
            .to_vec();
        for argument in &source_arguments {
            let argument =
                crate::type_reference::lower_type_reference_into_table(lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        lowerer.typed_trees.push_trait_requirement(
            &mut typed_trait,
            typed::trait_definition::TraitRequirement {
                symbol: requirement.symbol,
                name: crate::lowerer::name::lower_name(&requirement.name),
                lifetime_arguments: requirement
                    .lifetime_arguments
                    .iter()
                    .map(crate::lowerer::name::lower_name)
                    .collect(),
                arguments,
                source_span: requirement.name.source_span(),
            },
        );
    }

    for signature in lowerer
        .source_trees
        .trait_machine_signatures(trait_definition.machines)
    {
        let signature = lower_state_signature(lowerer, signature)?;
        lowerer
            .typed_trees
            .push_trait_machine_signature(&mut typed_trait, signature);
    }

    if let Some(base) = &trait_definition.refines {
        let base_definition = lowerer
            .source_trees
            .roots
            .traits
            .iter()
            .find(|definition| definition.symbol == base.symbol);
        let Some(base_definition) = base_definition else {
            return Err(Diagnostic::error(format!(
                "transparent refinement `{}` base `{}` does not name a trait",
                trait_definition.name.as_str(),
                base.name.as_str(),
            )));
        };
        crate::type_reference::retain_type_reference_selection(
            lowerer.source_trees,
            &mut lowerer.typed_trees,
            &base.name,
            base.symbol,
            lowerer.type_reference_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
        let mut base_arguments = arena::HandleSpan::empty();
        for argument in lowerer.source_trees.child_type_references(base.arguments) {
            let argument =
                crate::type_reference::lower_type_reference_into_table(lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut base_arguments, argument);
        }
        typed_trait.refines = Some(typed::trait_definition::TraitRequirement {
            symbol: base.symbol,
            name: crate::lowerer::name::lower_name(&base.name),
            lifetime_arguments: base
                .lifetime_arguments
                .iter()
                .map(crate::lowerer::name::lower_name)
                .collect(),
            arguments: base_arguments,
            source_span: base.name.source_span(),
        });
        let base_machines = lowerer
            .source_trees
            .trait_machine_signatures(base_definition.machines)
            .to_vec();
        let mut seen_named = std::collections::HashSet::new();
        for clause in &trait_definition.refinement_clauses {
            let signature = lower_state_signature(lowerer, &clause.signature)?;
            // A refinement narrows an existing base conformance: a named
            // clause must select a real base requirement, and its authored
            // axes may only restrict what the base already permits.
            let covered: Vec<&symbol_resolved_trees::signature::StateSignature> = match &clause
                .requirement
            {
                Some(requirement) => {
                    let matching = base_machines
                        .iter()
                        .filter(|machine| machine.name == *requirement)
                        .collect::<Vec<_>>();
                    if matching.is_empty() {
                        return Err(Diagnostic::error(format!(
                            "refinement clause `machine {}::{}` names no requirement of base trait `{}`",
                            base.name.as_str(),
                            requirement.as_str(),
                            base.name.as_str(),
                        )));
                    }
                    if !seen_named.insert(requirement.as_str().to_owned()) {
                        return Err(Diagnostic::error(format!(
                            "duplicate refinement clause for `{}`",
                            requirement.as_str(),
                        )));
                    }
                    matching
                }
                None => base_machines.iter().collect(),
            };
            if clause.signature.suspends
                && let Some(machine) = covered.iter().find(|machine| !machine.suspends)
            {
                return Err(Diagnostic::error(format!(
                    "refinement clause asserts `suspends` on `{}`, but base trait `{}` does not declare it — a refinement narrows, it cannot add an axis",
                    machine.name.as_str(),
                    base.name.as_str(),
                )));
            }
            if clause.signature.blocks
                && let Some(machine) = covered.iter().find(|machine| !machine.blocks)
            {
                return Err(Diagnostic::error(format!(
                    "refinement clause asserts `blocks` on `{}`, but base trait `{}` does not declare it — a refinement narrows, it cannot add an axis",
                    machine.name.as_str(),
                    base.name.as_str(),
                )));
            }
            if !clause.service_reaches.is_empty()
                && let Some(machine) = covered.iter().find(|machine| {
                    machine.service_reach_row == language_semantics::ServiceReachRowTable::EMPTY_ROW
                        || machine.service_reach_row
                            == language_semantics::ServiceReachRowId::default()
                })
            {
                return Err(Diagnostic::error(format!(
                    "refinement clause narrows `reaches` on `{}`, but base trait `{}` declares no reach set to narrow",
                    machine.name.as_str(),
                    base.name.as_str(),
                )));
            }
            for reach in &clause.service_reaches {
                // `reaches _;` is the independent abstract row bounded by the
                // inherited row; the clause-location row variant is pending.
                if reach.as_str() == "_" {
                    continue;
                }
                let service = lowerer
                    .source_trees
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        reach.as_str(),
                        &[symbols::SymbolKind::Trait],
                        reach.source_span(),
                    )
                    .and_then(|symbol| lowerer.source_trees.service_reaches.id_for_symbol(symbol));
                let Some(service) = service else {
                    return Err(Diagnostic::error(format!(
                        "refinement clause `reaches` names `{reach}`, which is not a boundary service",
                    )));
                };
                if let Some(machine) = covered.iter().find(|machine| {
                    !lowerer
                        .source_trees
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .contains(&service)
                }) {
                    return Err(Diagnostic::error(format!(
                        "refinement clause narrows `reaches` to `{reach}`, but `{}` of base trait `{}` does not reach it — a refinement narrows, it cannot add a reach",
                        machine.name.as_str(),
                        base.name.as_str(),
                    )));
                }
            }
            typed_trait
                .refinement_clauses
                .push(typed::trait_definition::TraitRefinementClause {
                    requirement: clause
                        .requirement
                        .as_ref()
                        .map(crate::lowerer::name::lower_name),
                    signature,
                    service_reaches: clause
                        .service_reaches
                        .iter()
                        .map(crate::lowerer::name::lower_name)
                        .collect(),
                });
        }
    }

    Ok(typed_trait)
}
