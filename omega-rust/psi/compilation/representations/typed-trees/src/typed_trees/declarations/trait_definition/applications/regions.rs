//! Exact region applications remain rooted in their supplying declaration.
//! Each inherited telescope is simultaneous: its operands see only earlier
//! scopes. Ordinary erased type identity remains available for layout; this
//! local key judges complete application equality and never grants a loan.

use super::AppliedTraitScope;
use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::name::Identifier;
use crate::type_identity::lifetime_parameter_identity;
use crate::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
    applied_index_identity, applied_lifetime_identity, applied_type_identity,
    local_trait_application_identity, portable_applied_index_identity,
    portable_applied_lifetime_identity, portable_applied_type_identity,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedDeclarationRegionScope {
    pub(super) declaration: SymbolHandle,
    pub(super) lifetimes: Vec<(String, String)>,
}

impl AppliedDeclarationRegionScope {
    pub(super) fn new(
        declaration: SymbolHandle,
        parameters: &[Identifier],
        arguments: &[Identifier],
    ) -> Self {
        Self {
            declaration,
            lifetimes: parameters
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
    /// A binder inside a generic call uses that call's actual types and regions.
    /// Prepend their simultaneous scope so a child trait retains those actuals
    /// without changing the inherited requirement's own parameter ownership.
    pub fn under_invocation(
        mut self,
        program: &TypedTrees,
        declaration: SymbolHandle,
        argument_owner: SymbolHandle,
        arguments: &[(SymbolHandle, crate::types::TypeReferenceHandle)],
        bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
    ) -> Option<Self> {
        let region = invocation_region_scope(program, declaration, argument_owner, bindings)?;
        self.argument_owner = argument_owner;
        self.substitutions.splice(0..0, arguments.iter().copied());
        for start in &mut self.substitution_scope_starts {
            *start += arguments.len();
        }
        self.substitution_scope_starts.insert(0, 0);
        self.region_scopes.insert(0, region);
        Some(self)
    }

    /// Publish the same complete application the local checking key names.
    /// Every operand keeps its own simultaneous prefix and supplying regions.
    pub fn portable_application_identity(
        &self,
        program: &TypedTrees,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_identity_scopes(program, |scopes| {
            self.portable_application_identity_in_scopes(program, self.argument_owner, scopes, 0)
        })
    }

    /// Identify a receiving trait with every selection made by its calling
    /// invocation, including complete machine and conformance applications.
    /// The supplied frame is simultaneous and precedes the trait's own bindings.
    pub fn portable_application_identity_under_invocation(
        &self,
        program: &TypedTrees,
        declaration: SymbolHandle,
        argument_owner: SymbolHandle,
        arguments: &[(SymbolHandle, IndexIdentityArgument<'_>)],
        bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
    ) -> Option<NormalizedTypeIdentity> {
        let region = invocation_region_scope(program, declaration, argument_owner, bindings)?;
        self.with_identity_scopes(program, |scopes| {
            let mut complete = Vec::with_capacity(scopes.len() + 1);
            complete.push(AppliedTypeIdentityScope {
                declaration: region.declaration,
                arguments,
                lifetimes: &region.lifetimes,
            });
            complete.extend_from_slice(scopes);
            self.portable_application_identity_in_scopes(program, argument_owner, &complete, 1)
        })
    }

    fn portable_application_identity_in_scopes(
        &self,
        program: &TypedTrees,
        argument_owner: SymbolHandle,
        scopes: &[AppliedTypeIdentityScope<'_>],
        preceding: usize,
    ) -> Option<NormalizedTypeIdentity> {
        let arguments = program
            .trait_type_parameters(self.declaration)
            .iter()
            .map(|parameter| {
                let (reference, outer) = self.parameter_application(parameter.symbol)?;
                let supplying = &scopes[..outer + preceding];
                if parameter.kind.is_const_or_value()
                    || matches!(parameter.kind, TypeParameterKind::Machine { .. })
                {
                    portable_applied_index_identity(
                        program,
                        IndexIdentityArgument::TypeReference(reference),
                        argument_owner,
                        supplying,
                    )
                } else {
                    portable_applied_type_identity(program, reference, argument_owner, supplying)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let lifetimes = self
            .declaration
            .lifetime_parameters
            .iter()
            .map(|parameter| {
                portable_applied_lifetime_identity(
                    program,
                    parameter.as_str(),
                    argument_owner,
                    scopes,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        crate::type_identity::portable_trait_application_identity(
            program,
            self.declaration.symbol,
            &arguments,
            &lifetimes,
        )
    }

    /// Publish an actual in its supplying scope, before the simultaneous
    /// telescope whose parameter it replaces. This retains reference access,
    /// nominal phantom regions and the complete inherited substitution route.
    pub fn portable_parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        let (reference, outer) = self.parameter_application(parameter)?;
        self.with_identity_scopes(program, |scopes| {
            let binder = program
                .trait_type_parameters(self.declaration)
                .iter()
                .find(|binder| binder.symbol == parameter)?;
            if matches!(
                binder.kind,
                TypeParameterKind::Const { .. }
                    | TypeParameterKind::Value { .. }
                    | TypeParameterKind::Machine { .. }
            ) {
                portable_applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(reference),
                    self.argument_owner,
                    &scopes[..outer],
                )
            } else {
                portable_applied_type_identity(
                    program,
                    reference,
                    self.argument_owner,
                    &scopes[..outer],
                )
            }
        })
    }

    /// Public lifetime operands participate even when no method mentions them.
    /// An inherited region is resolved through each edge, never recaptured by
    /// a similarly spelled binder in the receiving trait.
    pub fn portable_lifetime_arguments(
        &self,
        program: &TypedTrees,
    ) -> Option<Vec<NormalizedTypeIdentity>> {
        self.with_identity_scopes(program, |scopes| {
            self.declaration
                .lifetime_parameters
                .iter()
                .map(|parameter| {
                    portable_applied_lifetime_identity(
                        program,
                        parameter.as_str(),
                        self.argument_owner,
                        scopes,
                    )
                })
                .collect()
        })
    }

    /// Borrow the actual and the number of preceding simultaneous scopes it
    /// sees. Compound actuals must retain that prefix during structural walks;
    /// their existing arena handle alone does not carry the substitutions.
    pub fn parameter_application(
        &self,
        parameter: SymbolHandle,
    ) -> Option<(crate::types::TypeReferenceHandle, usize)> {
        let (ordinal, (_, reference)) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)?;
        let outer = self
            .substitution_scope_starts
            .partition_point(|start| *start <= ordinal)
            .checked_sub(1)?;
        Some((*reference, outer))
    }

    /// Exact local argument identity before its simultaneous receiving frame.
    /// Compound operands retain all supplying substitutions and region owners.
    pub fn argument_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        let (reference, outer) = self.parameter_application(parameter)?;
        let binder = program
            .trait_type_parameters(self.declaration)
            .iter()
            .find(|binder| binder.symbol == parameter)?;
        self.with_identity_scopes(program, |scopes| {
            if binder.kind.is_const_or_value()
                || matches!(binder.kind, TypeParameterKind::Machine { .. })
            {
                applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(reference),
                    self.argument_owner,
                    &scopes[..outer],
                )
            } else {
                applied_type_identity(program, reference, self.argument_owner, &scopes[..outer])
            }
        })
    }

    /// Borrow the complete substitution route without manufacturing type nodes
    /// or flattening an inherited operand into the receiving telescope.
    pub fn with_identity_scopes<Result>(
        &self,
        program: &TypedTrees,
        use_scopes: impl FnOnce(&[AppliedTypeIdentityScope<'_>]) -> Result,
    ) -> Result {
        let arguments = self
            .substitution_scope_starts
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
                            .data_type_parameter_by_symbol(*parameter)
                            .is_some_and(|binder| binder.kind == TypeParameterKind::Type);
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
                    let Some((reference, scope)) = self.parameter_application(parameter.symbol)
                    else {
                        return program.normalized_nominal_type_identity(parameter.symbol);
                    };
                    // The declared slot fixes the operand role before identity:
                    // a named const and its literal share one value key, while
                    // type operands retain nominal and region ownership.
                    if matches!(
                        parameter.kind,
                        TypeParameterKind::Const { .. }
                            | TypeParameterKind::Value { .. }
                            | TypeParameterKind::Machine { .. }
                    ) {
                        applied_index_identity(
                            program,
                            IndexIdentityArgument::TypeReference(reference),
                            self.argument_owner,
                            &scopes[..scope],
                        )
                    } else {
                        applied_type_identity(
                            program,
                            reference,
                            self.argument_owner,
                            &scopes[..scope],
                        )
                    }
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

fn invocation_region_scope(
    program: &TypedTrees,
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
) -> Option<AppliedDeclarationRegionScope> {
    let declaration = program.lifetime_declaration_owner(declaration);
    let mut lifetimes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let parameter =
            lifetime_parameter_identity(program, declaration, binding.parameter.as_str())?;
        if parameter.declaration != declaration
            || lifetimes
                .iter()
                .any(|(previous, _)| previous == binding.parameter.as_str())
        {
            return None;
        }
        lifetime_parameter_identity(program, argument_owner, binding.argument.as_str())?;
        lifetimes.push((
            binding.parameter.as_str().to_owned(),
            binding.argument.as_str().to_owned(),
        ));
    }
    Some(AppliedDeclarationRegionScope {
        declaration,
        lifetimes,
    })
}
