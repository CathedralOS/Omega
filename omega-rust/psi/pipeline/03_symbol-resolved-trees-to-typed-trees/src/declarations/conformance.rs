use crate::lowerer::{Lowerer, declaration_exposure};
use diagnostics::Diagnostic;
use symbol_resolved_trees::trait_definition::Conformance;

pub(crate) fn lower_conformance(
    lowerer: &mut Lowerer,
    conformance: &Conformance,
) -> Result<(), Diagnostic> {
    let symbol_resolved_trees = lowerer.source_trees;
    let conformance_exposure = declaration_exposure(conformance.is_public);
    if let symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(carrier_name) =
        &conformance.subject
    {
        crate::type_reference::retain_type_reference_selection(
            symbol_resolved_trees,
            &mut lowerer.typed_trees,
            carrier_name,
            conformance.carrier_symbol,
            conformance_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
    }
    if symbol_resolved_trees
        .roots
        .traits
        .iter()
        .find(|definition| definition.symbol == conformance.trait_symbol)
        .is_some_and(|definition| definition.refines.is_some())
    {
        return Err(Diagnostic::error(format!(
            "conformance target `{}` is a transparent refinement — a refinement bounds existing conformance evidence, it is not implemented directly",
            conformance.trait_name.as_str(),
        )));
    }
    crate::type_reference::retain_type_reference_selection(
        symbol_resolved_trees,
        &mut lowerer.typed_trees,
        &conformance.trait_name,
        conformance.trait_symbol,
        conformance_exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
    )?;
    let source_type_parameters = conformance.type_parameters;
    let mut arguments = arena::HandleSpan::empty();
    for argument in symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .span_or_empty(conformance.arguments)
    {
        let argument = lowerer.with_type_reference_exposure(conformance_exposure, |lowerer| {
            crate::type_reference::lower_type_reference_into_table(lowerer, argument)
        })?;
        lowerer
            .typed_trees
            .type_reference_table
            .push_type_reference_handle(&mut arguments, argument);
    }
    let trait_lifetime_arguments = conformance
        .trait_lifetime_arguments
        .iter()
        .map(|argument| {
            let ordinal = conformance
                .lifetime_parameters
                .iter()
                .position(|parameter| parameter == argument)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "conformance `{}` target trait lifetime `'{}' does not name an in-scope conformance lifetime binder",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed-conformance>", |name| name.as_str()),
                        argument.as_str(),
                    ))
                })?;
            u32::try_from(ordinal).map_err(|_| {
                Diagnostic::error(format!(
                    "conformance `{}` target trait lifetime ordinal exceeds the compiler limit",
                    conformance
                        .alias
                        .as_ref()
                        .map_or("<unnamed-conformance>", |name| name.as_str()),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut conformance = typed_trees::trait_definition::Conformance {
        symbol: conformance.symbol,
        is_public: conformance.is_public,
        lifetime_parameters: conformance
            .lifetime_parameters
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect(),
        type_parameters: arena::HandleSpan::empty(),
        subject: match &conformance.subject {
            symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                type_name,
            ) => typed_trees::trait_definition::ConformanceSubject::Carrier(
                crate::lowerer::name::lower_name(type_name),
            ),
            symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                typed_trees::trait_definition::ConformanceSubject::Subjectless
            }
        },
        carrier_symbol: conformance.carrier_symbol,
        trait_name: crate::lowerer::name::lower_name(&conformance.trait_name),
        trait_symbol: conformance.trait_symbol,
        trait_lifetime_arguments,
        arguments,
        alias: conformance.alias.as_ref().map(crate::lowerer::name::lower_name),
        implementation: match &conformance.implementation {
            symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
            }
            symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } => {
                typed_trees::trait_definition::ConformanceImplementation::Closed {
                    rows: rows
                        .iter()
                        .map(|row| typed_trees::trait_definition::ConformanceRow {
                            declaring_trait: row.declaring_trait,
                            declaring_trait_name: crate::lowerer::name::lower_name(&row.declaring_trait_name),
                            requirement: row.requirement,
                            requirement_name: crate::lowerer::name::lower_name(&row.requirement_name),
                            realization_machine: row.realization_machine,
                            realization_state: row.realization_state,
                            realization_name: crate::lowerer::name::lower_name(&row.realization_name),
                            source: match row.source {
                                symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => typed_trees::trait_definition::ConformanceRowSource::Inline,
                                symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => typed_trees::trait_definition::ConformanceRowSource::Reference,
                                symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => typed_trees::trait_definition::ConformanceRowSource::TraitDefault,
                            },
                        })
                        .collect(),
                }
            }
        },
    };
    conformance.type_parameters =
        lowerer.with_type_reference_exposure(conformance_exposure, |lowerer| {
            crate::signatures::type_parameters::lower_type_parameters(
                lowerer,
                source_type_parameters,
            )
        })?;
    // Inline/default realization machines close over the conformance
    // name's telescope. Publish that telescope as the machine template's
    // own generic surface as well, so ordinary specialization can clone
    // and substitute the selected row instead of leaving family symbols
    // such as `Element` in executable checked code. Referenced external
    // machines keep their independently declared telescope.
    let realization_machines = match &conformance.implementation {
        typed_trees::trait_definition::ConformanceImplementation::Closed { rows } => rows
            .iter()
            .filter(|row| {
                matches!(
                    row.source,
                    typed_trees::trait_definition::ConformanceRowSource::Inline
                        | typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                )
            })
            .map(|row| row.realization_machine)
            .collect::<Vec<_>>(),
        typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
            Vec::new()
        }
    };
    for machine in lowerer.typed_trees.machines_mut() {
        if realization_machines.contains(&machine.symbol) {
            machine.lifetime_parameters = conformance.lifetime_parameters.clone();
            machine.type_parameters = conformance.type_parameters;
        }
    }
    lowerer.typed_trees.push_conformance(conformance);
    Ok(())
}
