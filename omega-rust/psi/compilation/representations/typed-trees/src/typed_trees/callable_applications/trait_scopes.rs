//! Prepare and rejoin exact logical trait applications for both named maps and
//! machine satisfaction edges. Consumers borrow these signatures after freezing;
//! they neither mutate declarations nor infer an authored fixed type argument.
//! The cache key and prepared carriers retain captured regions. Fresh member
//! binders stay independently fresh. Complete region refinement reads the same
//! original application scopes: flattening a mixed captured and fresh telescope
//! can recapture same-spelled operands.

use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::{TypedTrees, signature, trait_definition::AppliedTraitScope};
use symbols::SymbolHandle;

impl TypedTrees {
    /// The logical signature selected by this exact requirement invocation.
    /// Open trait operands still substitute the caller's binders; they do not
    /// need an executable instance to describe a result. Neither a provider's
    /// signature nor a receiving annotation can supply missing operands.
    pub fn requirement_call_signature(
        &self,
        lexical_owner: SymbolHandle,
        call: &crate::expression::TableCallExpression,
    ) -> Option<&signature::StateSignature> {
        self.resolve_requirement_call_signature(lexical_owner, call)
            .ok()
    }

    /// Copied contracts retain the full outer application even though their
    /// logical owner and bare sibling targets keep the declaration symbols.
    /// Explicit selections still own their result; only an implicit sibling
    /// may use the exact supplying scope, including inherited/nullary members.
    pub fn requirement_call_signature_in_scope(
        &self,
        lexical_owner: SymbolHandle,
        call: &crate::expression::TableCallExpression,
        supplying_scope: crate::trait_definition::RetainedTraitScope,
    ) -> Option<&signature::StateSignature> {
        if let Some(signature) = self.requirement_call_signature(lexical_owner, call) {
            return Some(signature);
        }
        if call.requirement_application.is_valid() || call.static_requirement_dispatch.is_some() {
            return None;
        }
        let (declaration, declared) = self.trait_machine_signature(call.target_symbol)?;
        let scope = supplying_scope
            .scope(self)?
            .declaring_scope(self, declaration.symbol)?;
        self.applied_callable_signature(&scope, declared.symbol)
            .filter(|signature| {
                signature.type_parameters.is_empty() && signature.lifetime_parameters.is_empty()
            })
    }

    /// The same logical view with the reason its exact source cannot supply it.
    /// Optional queries above keep their existing refusal behavior.
    pub fn resolve_requirement_call_signature(
        &self,
        lexical_owner: SymbolHandle,
        call: &crate::expression::TableCallExpression,
    ) -> Result<&signature::StateSignature, &'static str> {
        self.requirement_invocation_signature(
            lexical_owner,
            call.target_symbol,
            call.static_requirement_dispatch.as_ref(),
            call.requirement_application,
            call.authored_machine_application,
        )
        .or_else(|reason| self.named_conformance_call_signature(call).ok_or(reason))
    }

    /// Statement calls select the same complete public logical view as an
    /// expression call. Their erased result syntax cannot discard the enclosing
    /// conformance application or the independently supplied member telescope.
    pub fn requirement_statement_call_signature(
        &self,
        lexical_owner: SymbolHandle,
        call: &crate::statement::TableCall,
    ) -> Option<&signature::StateSignature> {
        self.requirement_invocation_signature(
            lexical_owner,
            call.target_symbol,
            call.static_requirement_dispatch.as_ref(),
            Default::default(),
            call.authored_machine_application,
        )
        .ok()
    }

    fn requirement_invocation_signature(
        &self,
        lexical_owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: Option<&crate::typed_trees::StaticRequirementDispatch>,
        requirement_application: crate::types::TypeReferenceHandle,
        authored_application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Result<&signature::StateSignature, &'static str> {
        let target = dispatch.map_or(target, |dispatch| dispatch.contract_symbol());
        if let Some(closed) = self.closed_requirement_application(target) {
            // Closing the member's shape does not erase its source evidence
            // bound. A transparent refinement supplies a separate public
            // promise; the shared closed nominal signature remains unchanged.
            if dispatch.is_none() && authored_application.is_valid() {
                let source = self.static_machine_application(authored_application);
                let original = source.registration_operation;
                let binder = if original.is_valid() {
                    self.symbols.get(original).parent
                } else {
                    SymbolHandle::invalid()
                };
                if source.lexical_owner == lexical_owner
                    && binder.is_valid()
                    && self.symbols.get(binder).kind == symbols::SymbolKind::ConformanceParameter
                {
                    let mut owner = lexical_owner;
                    let mut visited = Vec::new();
                    while owner.is_valid() && !visited.contains(&owner) {
                        visited.push(owner);
                        if let Some(bound) = self
                            .evidence_bound_requirement(owner, binder, target)
                            .map_err(|_| "closed public view has unresolved source evidence")?
                        {
                            if bound
                                .application
                                .callable_refinement_clauses(self, bound.signature.name.as_str())
                                .is_empty()
                            {
                                break;
                            }
                            if let Some(signature) = self.applied_callable_signature(
                                &bound.application,
                                bound.signature.symbol,
                            ) && signature.type_parameters.is_empty()
                                && signature.lifetime_parameters.is_empty()
                            {
                                return Ok(signature);
                            }
                            break;
                        }
                        owner = self.declaration_scope_parent(owner);
                    }
                }
            }
            if requirement_application.is_valid() {
                let scope = self
                    .applied_requirement_scope(lexical_owner, requirement_application)
                    .ok_or("closed public view has no exact source application scope")?;
                if !self.requirement_application_matches_scope(target, &scope) {
                    return Err("closed public view does not match its source application");
                }
            }
            return Ok(closed.signature);
        }
        if let Some(signature) =
            self.applied_callable_signatures
                .iter()
                .find_map(|(_, prepared)| {
                    matches!(prepared.owner, AppliedCallableOwner::RequirementInvocation {
                application,
                lexical_owner: owner,
                ..
            } if application == authored_application
                && application.is_valid()
                && owner == lexical_owner
                && (prepared.signature.symbol == target
                    || {
                        let source = self.expression_table.static_machine_application(application);
                        source.lexical_owner == lexical_owner
                            && source.registration_operation == target
                            && source.binding_declaration == prepared.signature.symbol
                    }))
                    .then_some(&prepared.signature)
                })
        {
            return Ok(signature);
        }
        let outer = self.requirement_outer_signature(
            lexical_owner,
            target,
            dispatch,
            requirement_application,
        )?;
        (dispatch.is_none() || outer.type_parameters.is_empty())
            .then_some(outer)
            .ok_or("selected public view still has an unbound member telescope")
    }

    fn requirement_outer_signature(
        &self,
        lexical_owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: Option<&crate::typed_trees::StaticRequirementDispatch>,
        requirement_application: crate::types::TypeReferenceHandle,
    ) -> Result<&signature::StateSignature, &'static str> {
        // A source evidence child is not the declaring trait's state symbol.
        // Rejoin its exact authored bound before reading the prepared public
        // view, including when its caller-owned type operands remain symbolic.
        let binder = self.symbols.get(target).parent;
        let mut owner = lexical_owner;
        let mut visited = Vec::new();
        let bound = loop {
            if !owner.is_valid() || visited.contains(&owner) {
                break None;
            }
            visited.push(owner);
            if let Some(bound) = self
                .evidence_bound_requirement(owner, binder, target)
                .map_err(|_| "public view has unresolved source evidence")?
            {
                break Some(bound);
            }
            owner = self.declaration_scope_parent(owner);
        };
        if let Some(bound) = bound {
            if requirement_application.is_valid() {
                let written = self
                    .applied_requirement_scope(lexical_owner, requirement_application)
                    .ok_or("evidence public view has no exact written application scope")?
                    .declaring_scope(self, bound.application.declaration.symbol)
                    .ok_or("evidence public view has no declaring trait scope")?;
                if !bound.application.same_application(self, &written) {
                    return Err("evidence public view disagrees with its written application");
                }
            }
            return self
                .applied_callable_signature(&bound.application, bound.signature.symbol)
                .or_else(|| {
                    (bound.application.substitutions.is_empty()
                        && bound.application.declaration.lifetime_parameters.is_empty())
                    .then_some(bound.signature)
                })
                .ok_or("evidence public view has no prepared applied signature");
        }
        let (declaration, signature) = self
            .trait_machine_signature(target)
            .ok_or("public view has no source requirement declaration")?;
        if let Some(dispatch) = dispatch {
            // A map qualifier supplies an independent outer telescope even
            // inside an open generic body. Its exact retained selection, not
            // the realizer's private signature, owns this logical contract.
            let applications = self.selected_conformance_applications(
                lexical_owner,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                return Err("selected public view has no unique supplying map application");
            };
            let scope = application
                .trait_scope
                .scope(self)
                .ok_or("selected public view has no retained trait scope")?
                .declaring_scope(self, declaration.symbol)
                .ok_or("selected public view has no declaring trait scope")?;
            return self
                .applied_callable_signature(&scope, signature.symbol)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(signature)
                })
                .ok_or("selected public view has no prepared applied signature");
        }
        if requirement_application.is_valid() {
            let scope = self
                .applied_requirement_scope(lexical_owner, requirement_application)
                .ok_or("public view has no exact written application scope")?
                .declaring_scope(self, declaration.symbol)
                .ok_or("public view has no declaring trait scope")?;
            return self
                .applied_callable_signature(&scope, signature.symbol)
                .ok_or("public view has no prepared applied signature");
        }
        (declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty())
            .then_some(signature)
            .ok_or("public view has no complete outer application")
    }

    /// Prepare the remaining member telescope in this source invocation.
    /// The outer trait view alone cannot describe `Trait<Outer>::member<Inner>`:
    /// consumers need the same complete contract, including result memberships.
    /// This is a logical checking view and never an executable specialization.
    pub fn prepare_requirement_invocation(
        &mut self,
        lexical_owner: SymbolHandle,
        call: &crate::expression::TableCallExpression,
        bindings: &[crate::static_applications::StaticMachineBinding],
        selected: &signature::StateSignature,
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        let target = call
            .static_requirement_dispatch
            .as_ref()
            .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
        self.prepare_requirement_invocation_application(
            lexical_owner,
            target,
            call.authored_machine_application,
            bindings,
            selected,
        )
    }

    /// Statement and expression calls retain the same complete public view;
    /// a discarded result does not discard the invocation's static operands.
    pub fn prepare_requirement_statement_invocation(
        &mut self,
        lexical_owner: SymbolHandle,
        call: &crate::statement::TableCall,
        bindings: &[crate::static_applications::StaticMachineBinding],
        selected: &signature::StateSignature,
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        let target = call
            .static_requirement_dispatch
            .as_ref()
            .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
        self.prepare_requirement_invocation_application(
            lexical_owner,
            target,
            call.authored_machine_application,
            bindings,
            selected,
        )
    }

    fn prepare_requirement_invocation_application(
        &mut self,
        lexical_owner: SymbolHandle,
        target: SymbolHandle,
        authored_application: crate::static_applications::StaticMachineApplicationHandle,
        bindings: &[crate::static_applications::StaticMachineBinding],
        selected: &signature::StateSignature,
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        if self.closed_requirement_application(target).is_some() || !authored_application.is_valid()
        {
            return Ok(None);
        }
        let Some((declaration, declared)) = self.trait_machine_signature(selected.symbol) else {
            return Ok(None);
        };
        if declared.type_parameters.is_empty() {
            return Ok(None);
        }
        let declaration = declaration.symbol;
        let declared = declared.clone();
        // The producer supplies the canonical outer view, including an open
        // evidence-child's exact authored bound. A previous invocation output
        // cannot be the basis of another round of substitution.
        let mut application = CallableApplication {
            region_scope: lexical_owner,
            static_argument_owner: lexical_owner,
            ..CallableApplication::default()
        };
        for binding in bindings {
            let Some(parameter) =
                self.applied_signature_parameter(&declared, selected, binding.parameter)
            else {
                return Err("requirement invocation lost its prepared member binder");
            };
            match &binding.value {
                crate::static_applications::StaticMachineBindingValue::Type(actual)
                | crate::static_applications::StaticMachineBindingValue::Const(actual) => {
                    application.types.push((parameter, *actual));
                }
                crate::static_applications::StaticMachineBindingValue::Machine(actual)
                | crate::static_applications::StaticMachineBindingValue::Conformance(actual) => {
                    application
                        .static_arguments
                        .push((parameter, actual.clone()));
                }
                crate::static_applications::StaticMachineBindingValue::RuntimeValue { .. } => {
                    // A runtime subject supplies no static substitution. The
                    // invocation contract stays over the declared carrier and
                    // the call's own proof discharges roster membership.
                }
            }
        }
        let signature = application.signature(self, selected)?;
        let owner = AppliedCallableOwner::RequirementInvocation {
            declaration,
            application: authored_application,
            lexical_owner,
        };
        let previous = self
            .applied_callable_signatures
            .iter()
            .find_map(|(handle, previous)| {
                (previous.owner == owner && previous.signature.symbol == signature.symbol)
                    .then_some(handle)
            });
        if let Some(previous) = previous {
            self.applied_callable_signatures
                .replace_signature(previous, signature.clone())?;
        } else {
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner,
                    signature: signature.clone(),
                });
        }
        Ok(Some(signature))
    }

    /// Applied logical signature for one exact declaring scope. Its complete
    /// trait application remains separate from requirement authority.
    pub fn applied_callable_signature(
        &self,
        application: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        if let Some(closed) = self.closed_requirement_application(requirement)
            && application
                .callable_refinement_clauses(self, closed.signature.name.as_str())
                .is_empty()
        {
            return self
                .requirement_application_matches_scope(requirement, application)
                .then_some(closed.signature);
        }
        let (owner, identity) = application.callable_contract_key(self)?;
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::Trait {
                    declaration,
                    application: prepared_identity,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                if *declaration != owner
                    || *prepared_identity != identity
                    || prepared.signature.symbol != requirement
                {
                    return None;
                }
                Some(&prepared.signature)
            })
    }

    /// Prepare reachable declaring scopes once, including every inherited path.
    /// An application owns logical contracts, not executable realizations;
    /// constructing this row confers no conformance or proof authority.
    /// Return only newly substituted signatures so their producer can finalize
    /// derived domain identities before a caller reads the prepared contracts.
    pub fn prepare_applied_callable_signatures(
        &mut self,
    ) -> Result<Vec<signature::StateSignature>, &'static str> {
        self.prepare_applied_callable_signatures_reporting_change()
            .map(|(produced, _)| produced)
    }

    /// [`Self::prepare_applied_callable_signatures`], also reporting whether
    /// the call wrote anything. It writes only when some pending view has no
    /// prepared row yet; a call that writes nothing leaves the program as it
    /// found it, so the same call on that same program writes nothing again
    /// and its caller can pass that result forward instead of repeating it.
    pub fn prepare_applied_callable_signatures_reporting_change(
        &mut self,
    ) -> Result<(Vec<signature::StateSignature>, bool), &'static str> {
        let roots = self.applied_trait_roots();
        let mut pending = Vec::new();
        for scope in roots {
            scope.visit(self, &mut |scope| {
                for signature in self.trait_machine_signatures(scope.declaration) {
                    let clauses = scope.callable_refinement_clauses(self, signature.name.as_str());
                    // A completely monomorphic declaration needs no view.
                    // Outer arguments still substitute contracts when the
                    // requirement has no local binders, including phantom ones.
                    if signature.type_parameters.is_empty()
                        && scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty()
                        && clauses.is_empty()
                    {
                        continue;
                    }
                    let Some((owner, identity)) = scope.callable_contract_key(self) else {
                        continue;
                    };
                    // A refined bound supplies a logical view of a closed
                    // invocation too. Keep its exact ordinary Value formals;
                    // strengthening the shared nominal instance would grant
                    // those ceilings to unrelated base-trait callers.
                    if !clauses.is_empty() {
                        for closed in self.closed_requirement_applications().filter(|closed| {
                            closed.template == signature.symbol
                                && self.requirement_application_matches_scope(
                                    closed.signature.symbol,
                                    scope,
                                )
                        }) {
                            pending.push((
                                scope.declaration.symbol,
                                owner,
                                identity,
                                closed.signature.symbol,
                                scope.environment(),
                                clauses.clone(),
                            ));
                        }
                    }
                    pending.push((
                        scope.declaration.symbol,
                        owner,
                        identity,
                        signature.symbol,
                        scope.environment(),
                        clauses,
                    ));
                }
            });
        }
        // Every pending view checks for an existing row with its exact
        // requirement and trait key. Index those rows once, sorted, and add
        // each row as it is inserted: a scan of the whole table per pending
        // view was quadratic in the prepared roster. The index is guarded by
        // the table's revision: any table write other than this loop's own
        // insert, from any callee, rebuilds it before the next lookup.
        let row_key =
            |requirement: SymbolHandle,
             declaration: SymbolHandle,
             identity: crate::type_identity::NormalizedTypeIdentity| {
                (
                    (requirement.arena_index(), requirement.generation()),
                    (declaration.arena_index(), declaration.generation()),
                    identity,
                )
            };
        let index_rows = |program: &Self| {
            let mut rows = program
                .applied_callable_signatures
                .iter()
                .filter_map(|(_, prepared)| match prepared.owner {
                    AppliedCallableOwner::Trait {
                        declaration,
                        application,
                        ..
                    } => Some(row_key(prepared.signature.symbol, declaration, application)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            rows.sort_unstable();
            (rows, program.applied_callable_signatures.revision())
        };
        let (mut prepared_rows, mut indexed_revision) = index_rows(self);
        let mut wrote = false;
        let mut produced = Vec::new();
        for (trait_symbol, owner, identity, requirement, environment, clauses) in pending {
            if self.applied_callable_signatures.revision() != indexed_revision {
                (prepared_rows, indexed_revision) = index_rows(self);
            }
            if prepared_rows
                .binary_search(&row_key(requirement, owner, identity))
                .is_ok()
            {
                continue;
            }
            wrote = true;
            let mut applied = if let Some(closed) = self.closed_requirement_application(requirement)
            {
                closed.signature.clone()
            } else {
                let application = self.materialize_trait_application(&environment)?;
                let arguments = self.trait_application_arguments(trait_symbol, &application)?;
                // Ancestor substitutions resolve final operands without
                // capturing the declaring signature's independent telescope.
                let Some(applied) = CallableApplication {
                    trait_region_application: Some((
                        environment.clone(),
                        environment.frame_count(),
                    )),
                    ..CallableApplication::default()
                }
                .nominal(self, trait_symbol, requirement, &arguments)?
                else {
                    continue;
                };
                applied
            };
            self.apply_callable_refinements(&mut applied, &clauses)?;
            let scope = environment.retain(self)?;
            let inserted = row_key(applied.symbol, owner, identity);
            produced.push(applied.clone());
            let before_insert = self.applied_callable_signatures.revision();
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::Trait {
                        declaration: owner,
                        application: identity,
                        scope,
                    },
                    signature: applied,
                });
            // The insert moves the revision by exactly one. The index then
            // gains just this row, but only if nothing else wrote the table
            // since it was built; otherwise it is rebuilt from the table.
            let after_insert = self.applied_callable_signatures.revision();
            if before_insert == indexed_revision && after_insert == before_insert + 1 {
                if let Err(position) = prepared_rows.binary_search(&inserted) {
                    prepared_rows.insert(position, inserted);
                }
                indexed_revision = after_insert;
            } else {
                (prepared_rows, indexed_revision) = index_rows(self);
            }
        }
        Ok((produced, wrote))
    }

    /// Materialize each simultaneous telescope in its outer scopes. This shared
    /// derivation serves symbolic checking views and closed executable instances;
    /// it does not infer missing operands from a provider or runtime carrier.
    pub fn materialize_trait_application_arguments(
        &mut self,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
    ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
        let trait_symbol = environment
            .scope(self)
            .ok_or("applied callable lost its complete source environment")?
            .declaration
            .symbol;
        let application = self.materialize_trait_application(environment)?;
        self.trait_application_arguments(trait_symbol, &application)
    }

    pub(super) fn materialize_trait_application(
        &mut self,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
    ) -> Result<CallableApplication, &'static str> {
        let scope = environment
            .scope(self)
            .ok_or("applied callable lost its complete source environment")?;
        let substitutions = scope.substitutions;
        let scope_starts = scope.substitution_scope_starts;
        let argument_owner = scope.argument_owner;
        let mut application = CallableApplication {
            trait_region_application: Some((environment.clone(), 0)),
            ..CallableApplication::default()
        };
        for (ordinal, start) in scope_starts.iter().copied().enumerate() {
            if let Some((_, preceding_frames)) = &mut application.trait_region_application {
                *preceding_frames = ordinal;
            }
            let end = scope_starts
                .get(ordinal + 1)
                .copied()
                .unwrap_or(substitutions.len());
            let group = substitutions[start..end]
                .iter()
                .map(|(parameter, argument)| {
                    let applied = application.type_reference(self, *argument)?;
                    // Only copied supplying operands acquire this custody.
                    // Original declaration nodes remain lexical, and compound
                    // inherited arguments may contain differently owned children.
                    if ordinal == 0
                        && applied.is_valid()
                        && !self
                            .type_reference_table
                            .operand_lifetime_owner(applied)
                            .is_valid()
                    {
                        self.type_reference_table
                            .retain_operand_lifetime_owner(applied, argument_owner)?;
                    }
                    Ok((*parameter, applied))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            application.types.extend(group);
        }
        Ok(application)
    }

    fn trait_application_arguments(
        &self,
        trait_symbol: SymbolHandle,
        application: &CallableApplication,
    ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
        let parameters = self
            .trait_definition_by_symbol(trait_symbol)
            .map(|declaration| {
                self.trait_type_parameters(declaration)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>()
            })
            .ok_or("applied callable lost its declaring trait")?;
        let arguments = parameters
            .into_iter()
            .map(|symbol| {
                application
                    .types
                    .iter()
                    .rev()
                    .find(|(parameter, _)| *parameter == symbol)
                    .map(|(_, argument)| *argument)
                    .ok_or("applied callable lost a trait argument")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(arguments)
    }
}
