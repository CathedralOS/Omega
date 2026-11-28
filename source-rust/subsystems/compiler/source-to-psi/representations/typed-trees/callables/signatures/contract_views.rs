use super::{
    AppliedCallableOwner, AppliedCallableSignature, CallableContractScope, SymbolHandle,
    TypedTrees, signature,
};

impl TypedTrees {
    pub fn applied_trait_contracts(
        &self,
        trait_symbol: SymbolHandle,
    ) -> impl Iterator<Item = CallableContractScope<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter(|(_, prepared)| self.callable_application_owner_is_live(prepared))
            .filter_map(move |(_, prepared)| match prepared.owner {
                AppliedCallableOwner::Trait {
                    declaration, scope, ..
                }
                | AppliedCallableOwner::RequirementInstance {
                    declaration,
                    trait_scope: scope,
                    ..
                } if declaration == trait_symbol => Some(CallableContractScope {
                    signature: &prepared.signature,
                    trait_scope: Some(scope),
                }),
                _ => None,
            })
    }

    pub fn requirement_invocation_signatures(
        &self,
        caller: SymbolHandle,
    ) -> impl Iterator<Item = &signature::StateSignature> {
        self.requirement_invocation_signature_rows()
            .filter(move |&(owner, _)| owner == caller)
            .map(move |(_, row)| self.requirement_invocation_signature(row))
    }

    /// Every invocation view keeps its complete lexical owner and arena order.
    /// Reference walks may prepare these associations once, without narrowing
    /// them to live machines or merging signatures that name one declaration.
    pub(crate) fn requirement_invocation_signature_rows(
        &self,
    ) -> impl Iterator<Item = (SymbolHandle, arena::Handle<AppliedCallableSignature>)> {
        self.applied_callable_signatures
            .iter()
            .filter_map(|(row, prepared)| match prepared.owner {
                AppliedCallableOwner::RequirementInvocation { lexical_owner, .. }
                | AppliedCallableOwner::Realization { lexical_owner, .. } => {
                    Some((lexical_owner, row))
                }
                _ => None,
            })
    }

    pub(crate) fn requirement_invocation_signature(
        &self,
        row: arena::Handle<AppliedCallableSignature>,
    ) -> &signature::StateSignature {
        &self.applied_callable_signatures.get(row).signature
    }

    pub fn prepared_callable_signatures(&self) -> impl Iterator<Item = &signature::StateSignature> {
        self.prepared_callable_contracts()
            .map(|contract| contract.signature)
    }

    /// The copied telescope and its supplying trait application travel
    /// together. Logical declaration symbols deliberately remain unchanged
    /// when preparation substitutes a concrete outer application.
    pub fn prepared_callable_contracts(&self) -> impl Iterator<Item = CallableContractScope<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter(|(_, prepared)| self.callable_application_owner_is_live(prepared))
            .flat_map(|(_, prepared)| {
                let counterparts = match &prepared.owner {
                    AppliedCallableOwner::MachineSelection {
                        required,
                        source_required,
                        ..
                    } => [required.as_ref(), Some(source_required)],
                    AppliedCallableOwner::StaticCallback { source, .. } => [Some(source), None],
                    _ => [None, None],
                };
                let trait_scope = match prepared.owner {
                    AppliedCallableOwner::Trait { scope, .. } => Some(scope),
                    AppliedCallableOwner::RequirementInstance { trait_scope, .. } => {
                        Some(trait_scope)
                    }
                    _ => None,
                };
                [Some(&prepared.signature), counterparts[0], counterparts[1]]
                    .into_iter()
                    .flatten()
                    .map(move |signature| CallableContractScope {
                        signature,
                        trait_scope,
                    })
            })
    }

    fn callable_application_owner_is_live(&self, prepared: &AppliedCallableSignature) -> bool {
        match prepared.owner {
            AppliedCallableOwner::RequirementInvocation { lexical_owner, .. }
            | AppliedCallableOwner::StaticCallback { lexical_owner, .. }
            | AppliedCallableOwner::Realization { lexical_owner, .. } => {
                self.machine_by_symbol(lexical_owner).is_some()
            }
            _ => true,
        }
    }

    pub fn retain_live_trait_callable_applications(&mut self) {
        let mut live = Vec::new();
        for root in self.applied_trait_roots() {
            root.visit(self, &mut |scope| {
                if let Some(key) = scope.callable_contract_key(self) {
                    live.push(key);
                }
            });
        }
        let removed = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, prepared)| match prepared.owner {
                AppliedCallableOwner::Trait {
                    declaration,
                    application,
                    ..
                } if !live.contains(&(declaration, application)) => Some(handle),
                _ => None,
            })
            .collect::<Vec<_>>();
        for handle in removed {
            self.applied_callable_signatures.free(handle);
        }
    }

    pub fn remove_trait_declaration_applications(
        &mut self,
        removed: impl Fn(SymbolHandle) -> bool,
    ) {
        let applications = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, application)| match application.owner {
                AppliedCallableOwner::Trait { declaration, .. }
                | AppliedCallableOwner::RequirementInstance { declaration, .. }
                | AppliedCallableOwner::RequirementInvocation { declaration, .. }
                | AppliedCallableOwner::StaticCallback {
                    parameter: declaration,
                    ..
                }
                | AppliedCallableOwner::Realization { declaration, .. }
                    if removed(declaration) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for application in applications {
            self.applied_callable_signatures.free(application);
        }
    }
}
