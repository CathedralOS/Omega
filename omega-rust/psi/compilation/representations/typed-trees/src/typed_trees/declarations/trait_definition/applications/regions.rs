//! Exact region applications remain rooted in their supplying declaration.
//! Each inherited telescope is simultaneous: its operands see only earlier
//! scopes. Ordinary erased type identity remains available for layout; this
//! local key judges complete application equality and never grants a loan.

use super::{AppliedTraitScope, TraitDefinition};
use crate::TypedTrees;
use crate::name::Identifier;
use crate::data::TypeParameterKind;
use crate::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
    applied_lifetime_identity, applied_type_identity, local_trait_application_identity,
};
use symbols::SymbolHandle;

#[derive(Clone)]
pub(crate) struct AppliedTraitRegionScope {
    declaration: SymbolHandle,
    lifetimes: Vec<(String, String)>,
}

impl AppliedTraitRegionScope {
    pub(super) fn new(declaration: &TraitDefinition, arguments: &[Identifier]) -> Self {
        Self {
            declaration: declaration.symbol,
            lifetimes: declaration
                .lifetime_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (parameter.as_str().to_owned(), argument.as_str().to_owned())
                })
                .collect(),
        }
    }
}

impl AppliedTraitScope<'_> {
    /// Borrow the complete substitution route without manufacturing type nodes
    /// or flattening an inherited operand into the receiving telescope.
    pub fn with_identity_scopes<Result>(
        &self,
        program: &TypedTrees,
        use_scopes: impl FnOnce(&[AppliedTypeIdentityScope<'_>]) -> Result,
    ) -> Result {
        let arguments =
            self.substitution_scope_starts
                .iter()
                .enumerate()
                .map(|(ordinal, start)| {
                    let end = self
                        .substitution_scope_starts
                        .get(ordinal + 1)
                        .copied()
                        .unwrap_or(self.substitutions.len());
                    self.substitutions[*start..end]
                        .iter()
                        .map(|(parameter, argument)| {
                            let is_type = program
                                .trait_definition_by_symbol(self.region_scopes[ordinal].declaration)
                                .is_some_and(|declaration| {
                                    program.trait_type_parameters(declaration).iter().any(
                                        |binder| {
                                            binder.symbol == *parameter
                                                && binder.kind == TypeParameterKind::Type
                                        },
                                    )
                                });
                            (
                                *parameter,
                                if is_type {
                                    IndexIdentityArgument::TypeArgument(*argument)
                                } else {
                                    IndexIdentityArgument::TypeReference(*argument)
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
        let scopes = self
            .region_scopes
            .iter()
            .zip(&arguments)
            .map(|(region, arguments)| AppliedTypeIdentityScope {
                declaration: region.declaration,
                arguments,
                lifetimes: &region.lifetimes,
            })
            .collect::<Vec<_>>();
        use_scopes(&scopes)
    }

    /// All region-bearing arguments keep one root telescope. Independently
    /// normalizing each inherited path would collapse distinct root regions.
    pub fn application_identity(&self, program: &TypedTrees) -> Option<NormalizedTypeIdentity> {
        self.with_identity_scopes(program, |scopes| {
            let arguments = program
                .trait_type_parameters(self.declaration)
                .iter()
                .map(|parameter| {
                    let Some((ordinal, (_, reference))) = self
                        .substitutions
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, (binder, _))| *binder == parameter.symbol)
                    else {
                        return program.normalized_nominal_type_identity(parameter.symbol);
                    };
                    let scope = self
                        .substitution_scope_starts
                        .partition_point(|start| *start <= ordinal)
                        .saturating_sub(1);
                    applied_type_identity(
                        program,
                        *reference,
                        self.argument_owner,
                        &scopes[..scope],
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let lifetimes = self
                .declaration
                .lifetime_parameters
                .iter()
                .map(|parameter| {
                    applied_lifetime_identity(
                        program,
                        parameter.as_str(),
                        self.argument_owner,
                        scopes,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            local_trait_application_identity(
                program,
                self.declaration.symbol,
                &arguments,
                &lifetimes,
            )
        })
    }
}
