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
        self.requirement_invocation_signature(
            lexical_owner,
            call.target_symbol,
            call.static_requirement_dispatch.as_ref(),
            call.requirement_application,
            call.authored_machine_application,
        )
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
    }

    fn requirement_invocation_signature(
        &self,
        lexical_owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: Option<&crate::typed_trees::StaticRequirementDispatch>,
        requirement_application: crate::types::TypeReferenceHandle,
        authored_application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<&signature::StateSignature> {
        let target = dispatch.map_or(target, |dispatch| dispatch.contract_symbol());
        if let Some(closed) = self.closed_requirement_application(target) {
            if requirement_application.is_valid() {
                let scope =
                    self.applied_requirement_scope(lexical_owner, requirement_application)?;
                if !self.requirement_application_matches_scope(target, &scope) {
                    return None;
                }
            }
            return Some(closed.signature);
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
            return Some(signature);
        }
        let outer = self.requirement_outer_signature(
            lexical_owner,
            target,
            dispatch,
            requirement_application,
        )?;
        (dispatch.is_none() || outer.type_parameters.is_empty()).then_some(outer)
    }

    fn requirement_outer_signature(
        &self,
        lexical_owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: Option<&crate::typed_trees::StaticRequirementDispatch>,
        requirement_application: crate::types::TypeReferenceHandle,
    ) -> Option<&signature::StateSignature> {
        // A source evidence child is not the declaring trait's state symbol.
        // Rejoin its exact authored bound before reading the prepared public
        // view, including when its caller-owned type operands remain symbolic.
        if let Some(machine) = self.machine_by_symbol(lexical_owner)
            && let Some(bound) = self.conformance_target_requirement(machine, target).ok()?
        {
            if requirement_application.is_valid() {
                let written = self
                    .applied_requirement_scope(lexical_owner, requirement_application)?
                    .declaring_scope(self, bound.application.declaration.symbol)?;
                if !bound.application.same_application(self, &written) {
                    return None;
                }
            }
            return self
                .applied_callable_signature(&bound.application, bound.signature.symbol)
                .or_else(|| {
                    (bound.application.substitutions.is_empty()
                        && bound.application.declaration.lifetime_parameters.is_empty())
                    .then_some(bound.signature)
                });
        }
        let (declaration, signature) = self.trait_machine_signature(target)?;
        if let Some(dispatch) = dispatch {
            // A map qualifier supplies an independent outer telescope even
            // inside an open generic body. Its exact retained selection, not
            // the realizer's private signature, owns this logical contract.
            let applications = self.selected_conformance_applications(
                lexical_owner,
                dispatch.selection_application,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                return None;
            };
            let scope = application
                .trait_scope
                .scope(self)?
                .declaring_scope(self, declaration.symbol)?;
            return self
                .applied_callable_signature(&scope, signature.symbol)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(signature)
                });
        }
        if requirement_application.is_valid() {
            let scope = self
                .applied_requirement_scope(lexical_owner, requirement_application)?
                .declaring_scope(self, declaration.symbol)?;
            return self.applied_callable_signature(&scope, signature.symbol);
        }
        (declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty())
            .then_some(signature)
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
        if self.closed_requirement_application(target).is_some()
            || !call.authored_machine_application.is_valid()
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
            application: call.authored_machine_application,
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
            self.applied_callable_signatures.get_mut(previous).signature = signature.clone();
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
        let identity = application.application_identity(self)?;
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::Trait {
                    declaration,
                    application: prepared_identity,
                } = &prepared.owner
                else {
                    return None;
                };
                if *declaration != application.declaration.symbol
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
    pub fn prepare_applied_callable_signatures(&mut self) -> Result<(), &'static str> {
        let roots = self.applied_trait_roots();
        let mut pending = Vec::new();
        for scope in roots {
            scope.visit(self, &mut |scope| {
                for signature in self.trait_machine_signatures(scope.declaration) {
                    // A completely monomorphic declaration needs no view.
                    // Outer arguments still substitute contracts when the
                    // requirement has no local binders, including phantom ones.
                    if signature.type_parameters.is_empty()
                        && scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty()
                    {
                        continue;
                    }
                    let Some(identity) = scope.application_identity(self) else {
                        continue;
                    };
                    pending.push((
                        scope.declaration.symbol,
                        identity,
                        signature.symbol,
                        scope.environment(),
                    ));
                }
            });
        }
        for (trait_symbol, identity, requirement, environment) in pending {
            if self
                .applied_callable_signatures.iter().any(|(_, prepared)| {
                    prepared.signature.symbol == requirement
                        && matches!(prepared.owner, AppliedCallableOwner::Trait { application, .. } if application == identity)
                })
            {
                continue;
            }
            let application = self.materialize_trait_application(&environment)?;
            let arguments = self.trait_application_arguments(trait_symbol, &application)?;
            // Ancestor substitutions resolve the final operands, but are not
            // the declaring signature's telescope. Reusing that scratch scope
            // would capture unrelated binders while copying local contracts.
            let Some(applied) = CallableApplication {
                trait_region_application: Some((environment.clone(), environment.frame_count())),
                ..CallableApplication::default()
            }
            .nominal(self, trait_symbol, requirement, &arguments)?
            else {
                continue;
            };
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::Trait {
                        declaration: trait_symbol,
                        application: identity,
                    },
                    signature: applied,
                });
        }
        Ok(())
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

    fn materialize_trait_application(
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
