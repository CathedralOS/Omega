//! Declared argument categories and exact inherited substitution environments.

use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::types::signature_type_identity;
use crate::record::PackageReviewTypeIdentity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::{name::Identifier, types::TypeReferenceHandle};

pub(super) fn project(
    typed: &dyn ::validation::CheckingInput,
    compilation: &PackageReviewInput<'_>,
    owner: &TraitDefinition,
    arguments: &[TypeReferenceHandle],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[Identifier],
    root_binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewTypeIdentity>, Vec<Diagnostic>> {
    let parameters = compilation.trait_type_parameters(owner);
    if parameters.len() != arguments.len() {
        return Err(rejected(
            "calling trait application has an incomplete static telescope",
        ));
    }
    let mut binders = root_binders.to_vec();
    binders.extend(
        substitutions
            .iter()
            .enumerate()
            .map(|(ordinal, (symbol, _))| (*symbol, format!("inherited-parameter:{ordinal}")))
            .collect::<Vec<_>>(),
    );
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let const_argument = parameter.kind.is_const_or_value();
            signature_type_identity(
                typed,
                compilation.custody.sources().exact_toolchain_sources(),
                *argument,
                &binders,
                lifetimes,
                substitutions,
                &[],
                const_argument,
            )
        })
        .collect()
}
