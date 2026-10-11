//! Enclosing records contribute the same simultaneous telescopes as inherited
//! traits. Retain their exact occurrence, including phantom regions; neither
//! the storage declaration nor its erased identity supplies those arguments.

use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::name::Identifier;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct DataApplication<'program> {
    pub declaration: &'program DataDefinition,
    pub arguments: &'program [TypeReferenceHandle],
    pub lifetimes: &'program [Identifier],
}

/// Read the authored nominal application even when storage preparation has
/// replaced its visible node. Both field discovery and trait scopes must use
/// this same decoding, not infer operands from a layout-equivalent record.
pub(super) fn data_application(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<DataApplication<'_>> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    // Prepared nominal storage retains its authored application. Following it
    // does not infer missing operands from a similarly shaped declaration.
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
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
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

pub(super) fn append_data_application(
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
    scope.region_scopes.push(AppliedDeclarationRegionScope::new(
        declaration.symbol,
        &declaration.lifetime_parameters,
        lifetimes,
    ));
    Some(())
}
