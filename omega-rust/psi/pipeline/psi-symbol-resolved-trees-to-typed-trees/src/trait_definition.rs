use crate::data::lower_type_parameter;
use crate::lowerer::Lowerer;
use crate::state::lower_state_signature;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_trait_definition(
    lowerer: &mut Lowerer,
    trait_definition: &resolved::trait_definition::TraitDefinition,
) -> Result<typed::trait_definition::TraitDefinition, Diagnostic> {
    let mut typed_trait = typed::trait_definition::TraitDefinition {
        symbol: trait_definition.symbol,
        is_boundary: trait_definition.is_boundary,
        is_public: trait_definition.is_public,
        name: crate::name::lower_name(&trait_definition.name),
        lifetime_parameters: trait_definition
            .lifetime_parameters
            .iter()
            .map(crate::name::lower_name)
            .collect(),
        type_parameters: psi_arena::HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        requires: psi_arena::HandleSpan::empty(),
        machines: psi_arena::HandleSpan::empty(),
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(trait_definition.type_parameters)
    {
        let type_parameter = lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_trait_type_parameter(&mut typed_trait, type_parameter);
    }

    for bound in &trait_definition.conformance_bounds {
        let mut arguments = Vec::new();
        for argument in lowerer.source_trees.child_type_references(bound.arguments) {
            arguments.push(crate::type_reference::lower_type_reference_into_table(
                lowerer, argument,
            )?);
        }
        typed_trait
            .conformance_bounds
            .push(typed::machine::GenericConformanceBound {
                binder: bound.binder,
                binder_name: bound.binder_name.as_ref().map(crate::name::lower_name),
                subject: bound.subject,
                subject_name: crate::name::lower_name(&bound.subject_name),
                carrier: bound.carrier,
                carrier_name: crate::name::lower_name(&bound.carrier_name),
                arguments,
                selected_conformance: bound
                    .selected_conformance
                    .as_ref()
                    .map(crate::expression::lower_static_machine_argument),
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
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
        let mut arguments = psi_arena::HandleSpan::empty();
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
                name: crate::name::lower_name(&requirement.name),
                lifetime_arguments: requirement
                    .lifetime_arguments
                    .iter()
                    .map(crate::name::lower_name)
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

    Ok(typed_trait)
}
