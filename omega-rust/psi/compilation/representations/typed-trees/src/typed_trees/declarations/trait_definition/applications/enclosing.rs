//! Enclosing records contribute the same simultaneous telescopes as inherited
//! traits. Retain their exact occurrence, including phantom regions; neither
//! the storage declaration nor its erased identity supplies those arguments.

use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::name::Identifier;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

impl TypedTrees {
    /// Project a declaration operand through its exact enclosing applications.
    /// The returned frame count is the supplying prefix: when a Type binder is
    /// replaced, its compound actual must not see the telescope it replaces.
    /// Fields remain original declaration nodes rather than prepared storage
    /// copies whose substituted region spelling has lost that route.
    pub fn project_enclosing_data_type_reference(
        &self,
        reference: TypeReferenceHandle,
        applications: &[TypeReferenceHandle],
    ) -> Option<(TypeReferenceHandle, usize)> {
        let mut substitutions = Vec::new();
        let mut starts = Vec::new();
        for application in applications {
            let application = data_application(self, *application)?;
            starts.push(substitutions.len());
            substitutions.extend(
                self.data_type_parameters(application.declaration)
                    .iter()
                    .zip(application.arguments)
                    .filter(|(parameter, _)| parameter.kind == crate::data::TypeParameterKind::Type)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
        }
        let mut applied = super::project_type_reference(self, reference, &substitutions, &starts);
        let mut visited = Vec::new();
        loop {
            if !self.contains_type_reference(applied.reference)
                || visited.contains(&(applied.reference, applied.scope_starts.len()))
            {
                return None;
            }
            visited.push((applied.reference, applied.scope_starts.len()));
            let TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(applied.reference)
            else {
                break;
            };
            let Some(origin) = self
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            else {
                break;
            };
            // Prepared nominal fields are storage copies. Their retained
            // origin rejoins the original declaration without reapplying its
            // frames to already-substituted field operands.
            applied = super::project_type_reference(
                self,
                origin,
                applied.substitutions,
                applied.scope_starts,
            );
        }
        Some((applied.reference, applied.scope_starts.len()))
    }

    /// Borrow original enclosing data telescopes without materializing fields
    /// before constant and generic preparation settles. The callback sees
    /// simultaneous actuals, not a layout-equivalent record's field names.
    pub fn with_enclosing_data_identity_scopes<Result>(
        &self,
        applications: &[TypeReferenceHandle],
        use_scopes: impl FnOnce(&[crate::type_identity::AppliedTypeIdentityScope<'_>]) -> Result,
    ) -> Option<Result> {
        let applications = applications
            .iter()
            .map(|reference| data_application(self, *reference))
            .collect::<Option<Vec<_>>>()?;
        let arguments = applications
            .iter()
            .map(|application| {
                self.data_type_parameters(application.declaration)
                    .iter()
                    .zip(application.arguments)
                    .map(|(parameter, actual)| {
                        let actual = if parameter.kind == crate::data::TypeParameterKind::Type {
                            crate::type_identity::IndexIdentityArgument::TypeArgument(*actual)
                        } else {
                            crate::type_identity::IndexIdentityArgument::TypeReference(*actual)
                        };
                        (parameter.symbol, actual)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let lifetimes = applications
            .iter()
            .map(|application| {
                application
                    .declaration
                    .lifetime_parameters
                    .iter()
                    .zip(application.lifetimes)
                    .map(|(parameter, actual)| {
                        (parameter.as_str().to_owned(), actual.as_str().to_owned())
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let scopes = applications
            .iter()
            .zip(&arguments)
            .zip(&lifetimes)
            .map(|((application, arguments), lifetimes)| {
                crate::type_identity::AppliedTypeIdentityScope {
                    declaration: application.declaration.symbol,
                    arguments,
                    lifetimes,
                }
            })
            .collect::<Vec<_>>();
        Some(use_scopes(&scopes))
    }
}

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
    if !program.contains_type_reference(reference) {
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
