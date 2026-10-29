//! Fixed storage selects an ordinary map application as compile-time work.
//! Source type occurrences remain the independent reason each capture exists.
use super::{Diagnostic, TypedTrees};
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

pub(super) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let occurrences = typed_trees::references::collect_program_type_occurrences(program);
    for occurrence in occurrences {
        let Some(storage) = program.layout_storage_arguments(occurrence.reference) else {
            continue;
        };
        if program
            .data_implementation_applications
            .iter()
            .any(|(_, selected)| {
                selected.source_application == occurrence.reference
                    && selected.lexical_owner == occurrence.lexical_owner
            })
        {
            continue;
        }
        let operand = program
            .static_operand_from_type_reference(storage.policy)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if !operand.symbol.is_valid()
            || program.symbols.get(operand.symbol).kind == symbols::SymbolKind::ConformanceParameter
        {
            continue;
        }
        let application = match validation::conformance_applications::close_conformance_application(
            program, &operand,
        ) {
            Ok(application) => application,
            // Ordinary source formation diagnoses a wrong or open selection;
            // no executable demand may be manufactured for it here.
            Err(_) => continue,
        };
        let constructor = program
            .type_reference_table
            .type_symbol(storage.application);
        let parameters = program
            .data_definition_by_symbol(constructor)
            .map(|data| program.data_type_parameters(data).to_vec())
            .unwrap_or_default();
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter.kind == typed_trees::data::TypeParameterKind::Implementation
        }) else {
            continue;
        };
        let parameter = parameter.symbol;
        let supplying = program.expression_table.retain_static_machine_application(
            constructor,
            typed_trees::name::Identifier::generated("LayoutStorage"),
            std::slice::from_ref(&operand),
        );
        let source = program
            .expression_table
            .static_machine_application_mut(supplying);
        source.lexical_owner = occurrence.lexical_owner;
        source.binding_declaration = constructor;
        program
            .expression_table
            .set_static_machine_application_bindings(
                supplying,
                &[StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Conformance(operand),
                }],
            );
        program.data_implementation_applications.insert(
            typed_trees::typed_trees::DataImplementationApplication {
                source_application: occurrence.reference,
                lexical_owner: occurrence.lexical_owner,
                parameter,
                argument: storage.policy,
                supplying_application: supplying,
                application,
            },
        );
    }
    Ok(())
}
