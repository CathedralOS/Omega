//! Identify sealed Core instruction-model propositions by resolved declaration
//! custody and exact parameter carriers. This forms a query; only executed
//! instruction flow can supply an event, and no query disposes a resource.
use language_core::inline_assembly::AsmRegisterOperation;
use std::path::Path;
use symbols::SymbolHandle;
use typed_trees::proposition::PropositionBody;
use typed_trees::types::PrimitiveType;

pub fn instruction_effect_model(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<facts::InstructionEffectOperation> {
    let declaration = program.proposition_definition_by_symbol(symbol)?;
    if !declaration.is_public
        || !matches!(declaration.body, PropositionBody::Primitive)
        || !program
            .symbol_toolchain_relative_path(symbol)
            .is_some_and(|path| path == Path::new("assembly_model.omg"))
    {
        return None;
    }
    if declaration.name.as_str() == "atomic_release_written" {
        use typed_trees::proposition::PropositionBinderKind;
        use typed_trees::types::TypeReferenceNode;
        let [cell, value] = program.proposition_binders(declaration) else {
            return None;
        };
        let [destination, stored] = program.proposition_parameters(declaration) else {
            return None;
        };
        if !matches!(cell.kind, PropositionBinderKind::Type)
            || !matches!(value.kind, PropositionBinderKind::Type)
        {
            return None;
        }
        let TypeReferenceNode::Reference {
            referee,
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } = program
            .type_reference_table
            .type_reference(destination.type_reference)
        else {
            return None;
        };
        if !matches!(program.type_reference_table.type_reference(*referee), TypeReferenceNode::Named { symbol, .. } if *symbol == cell.symbol)
            || !matches!(program.type_reference_table.type_reference(stored.type_reference), TypeReferenceNode::Named { symbol, .. } if *symbol == value.symbol)
        {
            return None;
        }
        return Some(facts::InstructionEffectOperation::AtomicReleaseStore);
    }
    if !program.proposition_binders(declaration).is_empty() {
        return None;
    }
    let operation = match declaration.name.as_str() {
        "port_written" => AsmRegisterOperation::PortWrite,
        "model_specific_written" => AsmRegisterOperation::ModelSpecificWrite,
        _ => return None,
    };
    let expected = operation.input_carriers();
    let parameters = program.proposition_parameters(declaration);
    (parameters.len() == expected.len()
        && parameters
            .iter()
            .zip(expected)
            .all(|(parameter, expected)| {
                program.primitive_type_reference(parameter.type_reference)
                    == PrimitiveType::from_name(expected.name())
            }))
    .then_some(facts::InstructionEffectOperation::Register(operation))
}

/// The resolved transparent alias chain ending in a sealed instruction model.
/// Keep declarations and actuals, rather than parsing normalized proof labels.
pub struct InstructionEffectContract<'program> {
    pub operation: facts::InstructionEffectOperation,
    pub applications: Vec<&'program typed_trees::proposition::PropositionApplication>,
}
pub fn instruction_effect_contract<'program>(
    program: &'program dyn crate::CheckingInput,
    application: &'program typed_trees::proposition::PropositionApplication,
) -> Option<InstructionEffectContract<'program>> {
    let mut applications = Vec::new();
    let mut current = application;
    loop {
        if applications.iter().any(
            |previous: &&typed_trees::proposition::PropositionApplication| {
                previous.proposition == current.proposition
            },
        ) {
            return None;
        }
        applications.push(current);
        if let Some(operation) = instruction_effect_model(program, current.proposition) {
            return Some(InstructionEffectContract {
                operation,
                applications,
            });
        }
        let declaration = program.proposition_definition_by_symbol(current.proposition)?;
        let PropositionBody::Transparent {
            proposition: typed_trees::proposition::PropositionFormula::Application(next),
        } = &declaration.body
        else {
            return None;
        };
        current = next;
    }
}
