use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
use crate::source_state::TypedTrees;
use crate::source_state::data::DataDefinition;
use crate::source_state::name::Identifier;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct DataApplication<'program> {
    pub declaration: &'program DataDefinition,
    pub arguments: &'program [TypeReferenceHandle],
    pub lifetimes: &'program [Identifier],
}

pub(crate) fn data_application(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<DataApplication<'_>> {
    if !program.contains_type_reference(reference) {
        return None;
    }
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
        && let Some(origin) = program.data_definition_by_symbol(*symbol)?.generic_instance
    {
        if origin == reference || !program.type_reference_table.contains_type_reference(origin) {
            return None;
        }
        reference = origin;
    }
    let (declaration, arguments, lifetimes) =
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => (
                program.data_definition_by_symbol(*symbol)?,
                &[][..],
                &[][..],
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => (
                program.data_definition_by_symbol(*base_symbol)?,
                program.type_reference_handles(*arguments),
                lifetime_arguments.as_slice(),
            ),
            _ => return None,
        };
    let parameters = program.data_type_parameters(declaration);
    if parameters.len() != arguments.len()
        || declaration.lifetime_parameters.len() != lifetimes.len()
    {
        return None;
    }
    Some(DataApplication {
        declaration,
        arguments,
        lifetimes,
    })
}

pub(crate) fn append_data_application(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    scope: &mut AppliedTraitScope<'_>,
) -> Option<()> {
    let DataApplication {
        declaration,
        arguments,
        lifetimes,
    } = data_application(program, reference)?;
    let parameters = program.data_type_parameters(declaration);
    scope
        .substitution_scope_starts
        .push(scope.substitutions.len());
    scope.substitutions.extend(
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument)),
    );
    let reference_owner = program
        .type_reference_table
        .operand_lifetime_owner(reference);
    scope.region_scopes.push(AppliedDeclarationRegionScope::new(
        declaration.symbol,
        if reference_owner.is_valid() {
            reference_owner
        } else {
            scope.argument_owner
        },
        &declaration.lifetime_parameters,
        lifetimes,
    ));
    Some(())
}
