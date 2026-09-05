//! Declared argument categories and exact inherited substitution environments.

use super::rejected;
use crate::capture::semantics::types::{
    review_signature_const_argument_identity,
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
};
use crate::record::PackageReviewTypeIdentity;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::{data::TypeParameterKind, name::Identifier, types::TypeReferenceHandle};

pub(super) fn project(
    compilation: &CheckedCompilation,
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
