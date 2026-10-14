use crate::lowerer::{Lowerer, declaration_exposure};
use diagnostics::Diagnostic;
use symbol_resolved_trees::trait_definition::Conformance;

pub(crate) fn lower_conformance(
    lowerer: &mut Lowerer,
    conformance: &Conformance,
) -> Result<(), Diagnostic> {
    let symbol_resolved_trees = lowerer.source_trees;
    let conformance_exposure = declaration_exposure(conformance.is_public);
    if symbol_resolved_trees
        .roots
        .trait_definition(conformance.trait_symbol)
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
        trait_name: crate::lowerer::name::lower_name(&conformance.trait_name),
        trait_symbol: conformance.trait_symbol,
        trait_lifetime_arguments,
        arguments,
        alias: conformance
            .alias
            .as_ref()
            .map(crate::lowerer::name::lower_name),
        rows: conformance
            .rows
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
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => {
                        typed_trees::trait_definition::ConformanceRowSource::Inline
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => {
                        typed_trees::trait_definition::ConformanceRowSource::Reference
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                        typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Synthesized => {
                        typed_trees::trait_definition::ConformanceRowSource::Synthesized
                    }
                },
            })
            .collect(),
    };
    conformance.type_parameters =
        lowerer.with_type_reference_exposure(conformance_exposure, |lowerer| {
            crate::signatures::type_parameters::lower_type_parameters(
                lowerer,
                source_type_parameters,
            )
        })?;
    // The row keeps its independently authored invocation telescope. Capture
    // belongs to the exact owned-row relationship, queried through the map's
    // declaration scope; replacing the row's binders would erase generic
    // method authority and recapture same-spelled local parameters.
    lowerer.typed_trees.push_conformance(conformance);
    Ok(())
}
