//! Check function table bindings against their exact Product signature and storage declaration.

use super::exact_vocabulary;
use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataMember, DataShapeKind};
use crate::source_state::reflection::construction::{
    field_description_for_application, parameter_description_identity,
};
use crate::source_state::types::{PrimitiveType, TypeReferenceNode};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub fn is_function_field_binding(
    typed: &TypedTrees,
    declaration: SymbolHandle,
) -> Result<bool, Vec<Diagnostic>> {
    use language_core::core_vocabulary::{
        CoreVocabulary, ExternalBindingVocabulary, ReflectionDeclaration,
    };
    let declaration = typed
        .unique_data_definition_by_symbol(declaration)
        .and_then(|data| data.generic_instance)
        .map_or(declaration, |application| {
            typed.type_reference_table.type_symbol(application)
        });
    let role = ExternalBindingVocabulary::FunctionFieldBinding;
    if typed.symbols.core_vocabulary(declaration) != Some(CoreVocabulary::ExternalBinding(role)) {
        return Ok(false);
    }
    let definition = exact_vocabulary(typed, declaration, role)?;
    let fields = typed.data_members(definition);
    let expected = [
        ("table", ReflectionDeclaration::ParameterInfo),
        ("field", ReflectionDeclaration::FieldInfo),
    ];
    if fields.len() != expected.len()
        || !fields.iter().zip(expected).all(|(member, (name, role))| {
            member.as_field().is_some_and(|field| {
                field.name.as_str() == name
                    && !field.relevance.is_erased()
                    && typed.symbols.core_vocabulary(
                        typed.type_reference_table.type_symbol(field.type_reference),
                    ) == Some(CoreVocabulary::Reflection(role))
            })
        })
    {
        return Err(vec![Diagnostic::error(
            "FunctionFieldBinding requires the checked ParameterInfo table and FieldInfo field",
        )]);
    }
    Ok(true)
}

/// Compare the Build value with the receiving Product's own signature
/// and schema. Equal names or declaration positions in another table
/// cannot authorize reading that table or calling one of its fields.
pub fn validate_function_field_binding(
    typed: &TypedTrees,
    requirement: SymbolHandle,
    supplier: semantic_vocabulary::PackageKeyIdentity,
    binding: &semantic_vocabulary::FunctionFieldBinding,
) -> Result<(), String> {
    let (_, signature) = typed
        .trait_machine_signature(requirement)
        .ok_or("function table binding lost its boundary requirement")?;
    let position = usize::try_from(binding.table.declaration_position)
        .map_err(|_| "table parameter position does not fit")?;
    let parameter = typed
        .state_signature_parameters(signature)
        .get(position)
        .ok_or("function table binding names a parameter outside the requirement")?;
    let expected = parameter_description_identity(typed, parameter.symbol, requirement)?;
    if binding.table != expected {
        return Err(
            "function table binding names a different requirement parameter or source revision"
                .into(),
        );
    }

    let types = &typed.type_reference_table;
    let parameter_type = types.unconstrained_type_reference(parameter.type_reference);
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = types.type_reference(parameter_type)
    else {
        return Err("function table dispatch requires a borrowed table".into());
    };
    let referee = types.unconstrained_type_reference(*referee);
    if !access.is_readable() {
        return Err("function table dispatch requires permission to read the table".into());
    }
    let data = typed
        .closed_data_definition(referee)
        .ok_or("function table dispatch requires a closed record application")?;
    if DataDefinition::shape_kind_from_members(typed.data_members(data)) != DataShapeKind::Record {
        return Err("function table dispatch requires a record table".into());
    }
    let declaration = typed
        .symbols
        .symbol_provenance_source_span(data.symbol)
        .and_then(|span| typed.symbols.source_file(span))
        .ok_or("function table binding lost its Product table source")?;
    if !typed.symbols.source_map().is_some_and(|sources| {
        sources.product_description_visible(supplier, declaration, data.is_public)
    }) {
        return Err(
            "function table binding selects a table outside its supplier's Product scope".into(),
        );
    }
    let position = usize::try_from(binding.field.declaration_position)
        .map_err(|_| "function field position does not fit")?;
    let field = typed
        .data_members(data)
        .get(position)
        .and_then(DataMember::as_field)
        .ok_or("function table binding names a field outside the borrowed record")?;
    if field.relevance.is_erased()
        || typed.primitive_type_reference(field.type_reference) != Some(PrimitiveType::Addr)
    {
        return Err("function table binding requires a live, non-atomic addr field".into());
    }
    let expected = field_description_for_application(typed, field.symbol, referee, requirement)?;
    if binding.field != expected {
        return Err(
            "function table binding names a different field, owner application or source revision"
                .into(),
        );
    }
    Ok(())
}
