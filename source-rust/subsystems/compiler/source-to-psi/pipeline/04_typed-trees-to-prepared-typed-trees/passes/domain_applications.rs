//! Apply each retained domain's argument telescope to its declaration after specialization.

use crate::source_state::{CallableApplication, TypedTrees, domain, types};
use diagnostics::Diagnostic;
use semantic_vocabulary::SemanticDomainId;
use symbols::SymbolHandle;

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // Copying a fact may retain more applications. Visit those rows in the same pass.
    let mut position = 0;
    while let Some(row) = program.applied_domain_applications.get(position) {
        if row.theory.is_none() {
            let declaration = row.declaration;
            let identity = row.identity;
            let arguments = row.arguments.clone();
            let theory = apply(program, declaration, identity, &arguments);
            program
                .applied_domain_applications
                .set_theory(position, theory.map_err(str::to_owned));
        }
        position += 1;
    }
    Ok(())
}

fn apply(
    program: &mut TypedTrees,
    declaration: SymbolHandle,
    identity: SemanticDomainId,
    arguments: &[types::TypeReferenceHandle],
) -> Result<domain::DomainDefinition, &'static str> {
    let mut theory = program
        .domain_definition(declaration)
        .ok_or("applied domain lost its declaration")?
        .clone();
    if !domain::has_direct_index_telescope(program, &theory) {
        return Err("domain index declaration is not its direct parameter telescope");
    }
    if program.domain_type_parameters(&theory).len() != arguments.len() {
        return Err("applied domain has an incomplete declaration telescope");
    }

    let source_subject = theory.subject_symbol;
    let subject = if source_subject.is_valid() {
        program.symbols.insert_generated_root_from(
            source_subject,
            program.symbols.get(source_subject).kind,
            theory.subject_name.as_str(),
        )
    } else {
        source_subject
    };
    let parameters = program.domain_type_parameters(&theory);
    let mut application = CallableApplication::for_type_parameters(
        declaration,
        parameters.iter().zip(arguments.iter().copied()),
        source_subject
            .is_valid()
            .then_some((source_subject, subject)),
    );
    theory.target_type = application.type_reference(program, theory.target_type)?;
    theory.subject_symbol = subject;
    theory.facts = application.facts(program, theory.facts)?;
    theory.type_parameters = Default::default();
    theory.index_arguments.clear();
    theory.semantic_id = identity;
    theory.semantic_roles.denotation_dimension =
        theory.semantic_roles.denotation_dimension.map(|_| identity);
    theory.semantic_roles.arithmetic_policy =
        theory.semantic_roles.arithmetic_policy.map(|_| identity);
    if let Some(selection) = theory.cleanup_selection {
        theory.cleanup_selection = Some(apply_cleanup_selection(
            program,
            &mut application,
            selection,
            declaration,
        )?);
    }
    Ok(theory)
}

fn apply_cleanup_selection(
    program: &mut TypedTrees,
    application: &mut CallableApplication,
    selection: domain::CleanupSelection,
    declaration: SymbolHandle,
) -> Result<domain::CleanupSelection, &'static str> {
    let original = program
        .expression_table
        .static_machine_application(selection.application)
        .clone();
    let mut arguments = program
        .expression_table
        .static_machine_application_arguments(selection.application)
        .to_vec();
    application.static_arguments(program, &mut arguments)?;
    let selected = program.expression_table.retain_static_machine_application(
        original.registration_operation,
        original.target,
        &arguments,
    );
    let lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(selection.application)
        .to_vec();
    program
        .expression_table
        .set_static_machine_application_lifetimes(
            selected,
            original.lifetime_binding_declaration,
            &lifetimes,
        );
    let selected_row = program
        .expression_table
        .static_machine_application_mut(selected);
    selected_row.lexical_owner = declaration;
    selected_row.binding_declaration = original.binding_declaration;
    Ok(domain::CleanupSelection {
        application: selected,
        authorization: selection.authorization,
        subject: application.type_reference(program, selection.subject)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::{data, name::Identifier, types::TypeReferenceNode};
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    #[test]
    fn a_refused_domain_coordinate_does_not_block_a_later_application_or_change_on_retry() {
        let mut symbols = SymbolTableBuilder::new();
        let declarations = ["MissingIndex", "GenericCarrier"].map(|name| {
            let declaration = symbols.insert_root(SymbolKind::Domain, SymbolNameRef::Static(name));
            let parameter = symbols
                .insert_children(
                    declaration,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
                )
                .start();
            (name, declaration, parameter)
        });
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let argument = program.type_reference_table.insert(TypeReferenceNode::Unit);
        for (name, declaration, parameter) in declarations {
            let index_is_missing = name == "MissingIndex";
            let target_type = if index_is_missing {
                argument
            } else {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: parameter,
                        name: Identifier::generated("T"),
                    })
            };
            let type_parameters = program
                .data_type_parameters
                .insert_many([data::TypeParameter {
                    name: Identifier::generated("T"),
                    symbol: parameter,
                    kind: data::TypeParameterKind::Type,
                    ..Default::default()
                }]);
            let identity = program.semantic_domains.intern(name);
            program.push_domain_definition(domain::DomainDefinition {
                name: Identifier::generated(name),
                symbol: declaration,
                target_type,
                type_parameters,
                semantic_id: identity,
                ..Default::default()
            });
            let indices = if index_is_missing {
                std::slice::from_ref(&argument)
            } else {
                &[]
            };
            program
                .retain_domain_application(declaration, argument, indices)
                .unwrap();
        }
        prepare(&mut program).unwrap();
        let rows = &program.applied_domain_applications;
        assert_eq!(
            rows.get(0)
                .unwrap()
                .theory
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err(),
            "domain index declaration is not its direct parameter telescope"
        );
        let valid = rows.get(1).unwrap();
        let theory = valid.theory.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(theory.semantic_id, valid.identity);
        assert!(theory.type_parameters.is_empty());
        assert_eq!(
            program
                .type_reference_table
                .type_reference(theory.target_type),
            &TypeReferenceNode::Unit
        );
        let prepared = rows.clone();
        prepare(&mut program).unwrap();
        assert_eq!(program.applied_domain_applications, prepared);
    }
}
