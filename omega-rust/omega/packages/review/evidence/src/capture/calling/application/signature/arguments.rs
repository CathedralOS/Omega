//! Declared argument categories and exact inherited substitution environments.

use super::rejected;
use crate::capture::semantics::types::{
    review_signature_const_argument_identity,
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
};
use crate::record::PackageReviewTypeIdentity;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::{data::TypeParameterKind, name::Identifier, types::TypeReferenceHandle};

pub(super) fn project(
    compilation: &CheckedCompilation,
    owner: &TraitDefinition,
    arguments: &[TypeReferenceHandle],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[Identifier],
) -> Result<Vec<PackageReviewTypeIdentity>, Vec<Diagnostic>> {
    let parameters = compilation.trait_type_parameters(owner);
    if parameters.len() != arguments.len() {
        return Err(rejected(
            "calling trait application has an incomplete static telescope",
        ));
    }
    let binders = substitutions
        .iter()
        .enumerate()
        .map(|(ordinal, (symbol, _))| (*symbol, format!("inherited-parameter:{ordinal}")))
        .collect::<Vec<_>>();
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                review_signature_const_argument_identity(
                    compilation,
                    *argument,
                    &binders,
                    lifetimes,
                    substitutions,
                )
            } else {
                review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                    compilation,
                    *argument,
                    &binders,
                    lifetimes,
                    substitutions,
                    &[],
                )
            }
        })
        .collect()
}
