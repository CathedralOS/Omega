//! Keep a selected callback's full required contract after its generic
//! parameter becomes a direct call. Recovered headers must still match the
//! original contract under the saved arguments.

use super::{
    AppliedCallableOwner, AppliedCallableSignature, CallableApplication, CallableLifetimeBinding,
    CallableLifetimeOrigin, TypedTrees,
};
use crate::source_state::signature;
use symbols::SymbolHandle;

/// The complete required contract remains with an application's bindings even
/// after specialization replaces its callback parameter with a direct call.
pub struct AppliedStaticCallbackRequirement<'a> {
    pub parameter: SymbolHandle,
    pub required: &'a signature::StateSignature,
    pub argument: &'a crate::source_state::expression::StaticMachineArgument,
}

impl TypedTrees {
    pub fn prepare_static_callback_requirements(
        &mut self,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) -> Result<(), &'static str> {
        use crate::source_state::static_applications::StaticMachineBindingValue;
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("callback requirements lost their source application");
        }
        let invocation = self.static_machine_application(application).clone();
        let bindings = self
            .static_machine_application_bindings(application)
            .to_vec();
        if !bindings
            .iter()
            .any(|binding| matches!(binding.value, StaticMachineBindingValue::Machine(_)))
        {
            return Ok(());
        }
        let declaration = self
            .machine_by_symbol(invocation.binding_declaration)
            .ok_or("callback requirements lost their binding declaration")?;
        let parameters = self.machine_callable_parameters(declaration);
        if parameters.len() != bindings.len()
            || parameters
                .iter()
                .zip(&bindings)
                .any(|(parameter, binding)| parameter.symbol != binding.parameter)
        {
            return Err("callback requirements have another binder tuple");
        }
        let lifetime_rows = self
            .static_application_lifetimes_for_declaration(
                application,
                invocation.binding_declaration,
            )?
            .into_owned();
        let arguments = self
            .static_machine_application_arguments(application)
            .to_vec();
        if arguments.len() != bindings.len() {
            return Err("callback requirements have an incomplete argument tuple");
        }
        let types = bindings
            .iter()
            .filter_map(|binding| match binding.value {
                StaticMachineBindingValue::Type(actual)
                | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let static_arguments = bindings
            .iter()
            .zip(arguments)
            .map(|(binding, argument)| (binding.parameter, argument))
            .collect::<Vec<_>>();
        if self.static_callback_requirements(application).is_ok() {
            return Ok(());
        }
        let previous = self
            .applied_callable_signatures
            .callback_rows(application)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in previous {
            self.applied_callable_signatures.free(handle);
        }
        let snapshot = self
            .expression_table
            .snapshot_static_machine_bindings(application);
        let retained_lifetimes = self
            .callable_lifetime_substitutions
            .insert_many(lifetime_rows.iter().cloned());
        for binding in &bindings {
            if !matches!(binding.value, StaticMachineBindingValue::Machine(_)) {
                continue;
            }
            let source = self
                .retained_static_machine_contract(binding.parameter)
                .ok_or("callback requirement lost its complete originating contract")?
                .signature()
                .clone();
            let lifetimes = lifetime_rows
                .iter()
                .map(|binding| {
                    let region = crate::source_state::type_identity::lifetime_parameter_identity(
                        self,
                        invocation.lexical_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or("callback requirement lifetime lost its supplying declaration")?;
                    Ok(CallableLifetimeBinding {
                        parameter: binding.parameter.clone(),
                        argument: binding.argument.clone(),
                        region,
                        origin: CallableLifetimeOrigin::Lexical,
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            let mut substitution = CallableApplication {
                region_scope: invocation.binding_declaration,
                types: types.clone(),
                static_arguments: static_arguments.clone(),
                static_argument_owner: invocation.lexical_owner,
                lifetimes,
                ..Default::default()
            };
            let signature = substitution.signature(self, &source)?;
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::StaticCallback {
                        parameter: binding.parameter,
                        application,
                        lexical_owner: invocation.lexical_owner,
                        source,
                        bindings: snapshot,
                        lifetimes: retained_lifetimes,
                    },
                    signature,
                });
        }
        Ok(())
    }

    fn static_callback_requirement_matches(
        &self,
        prepared: &AppliedCallableSignature,
        actual: crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) -> Result<bool, &'static str> {
        let AppliedCallableOwner::StaticCallback {
            application,
            lexical_owner,
            bindings,
            lifetimes,
            ..
        } = &prepared.owner
        else {
            return Ok(false);
        };
        if !self
            .expression_table
            .static_machine_application_is_valid(actual)
        {
            return Err("callback requirement has a stale source application");
        }
        let invocation = self.static_machine_application(actual);
        Ok(*application == actual
            && *lexical_owner == invocation.lexical_owner
            && self.symbols.symbols().nodes().is_valid(*lexical_owner)
            && self.static_callback_source_matches(prepared)?
            && self
                .expression_table
                .static_machine_binding_snapshot(*bindings)
                == self.static_machine_application_bindings(actual)
            && self
                .callable_lifetime_substitutions
                .span(*lifetimes)
                .is_some_and(|known| {
                    self.static_application_lifetimes_for_declaration(
                        actual,
                        invocation.binding_declaration,
                    )
                    .is_ok_and(|actual| known.iter().eq(actual.iter()))
                }))
    }

    /// Stored applied headers must still mean the original contract under
    /// their saved arguments. Live preparation already constructs that result.
    pub(crate) fn validate_recovered_callback_contracts(&self) -> Result<(), String> {
        use crate::source_state::static_applications::StaticMachineBindingValue;
        use crate::source_state::type_identity::IndexIdentityArgument;
        use crate::source_state::type_identity::identity_context::{
            SubstitutionSemantics, TypeIdentityContext, TypeIdentityQualification,
        };
        let mut applications = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(_, row)| match row.owner {
                AppliedCallableOwner::StaticCallback { application, .. } => Some(application),
                _ => None,
            })
            .collect::<Vec<_>>();
        applications.sort_unstable_by_key(|application| {
            (application.arena_index(), application.generation())
        });
        applications.dedup();
        if applications.is_empty() {
            return Ok(());
        }
        let exact_sources = source_admission::source_identity::toolchain_source_identities(
            self.symbols.source_files(),
        )
        .map_err(|reason| format!("recovered callback toolchain source identities: {reason}"))?;
        for application in applications {
            let contracts = self.static_callback_requirements(application)?;
            let invocation = self.static_machine_application(application);
            let bindings = self.static_machine_application_bindings(application);
            let arguments = self.static_machine_application_arguments(application);
            if bindings.len() != arguments.len() {
                return Err("recovered callback contract has an incomplete argument tuple".into());
            }
            let types = bindings
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let indices = bindings
                .iter()
                .zip(arguments)
                .map(|(binding, argument)| {
                    (
                        binding.parameter,
                        IndexIdentityArgument::StaticArgument(argument),
                    )
                })
                .collect::<Vec<_>>();
            let lifetimes = self.static_application_lifetimes_for_declaration(
                application,
                invocation.binding_declaration,
            )?;
            let regions = lifetimes
                .iter()
                .map(|binding| {
                    let formal = crate::source_state::type_identity::lifetime_parameter_identity(
                        self,
                        invocation.binding_declaration,
                        binding.parameter.as_str(),
                    )
                    .ok_or("recovered callback contract lost its declared lifetime")?;
                    let actual = crate::source_state::type_identity::lifetime_parameter_identity(
                        self,
                        invocation.lexical_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or("recovered callback contract lost its supplying lifetime")?;
                    Ok((formal, actual))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            for contract in contracts {
                let original = self
                    .retained_static_machine_contract(contract.parameter)
                    .ok_or("recovered callback contract lost its original binder")?
                    .signature();
                let owners = [(original.symbol, invocation.binding_declaration)];
                let invalid = std::cell::Cell::new(false);
                let context = TypeIdentityContext {
                    application_owner: invocation.binding_declaration,
                    application_argument_owner: invocation.lexical_owner,
                    callable_scope_owners: &owners,
                    resolved_lifetimes: &regions,
                    structural_callable_identity: true,
                    qualification: TypeIdentityQualification::PackageQualified,
                    exact_toolchain_sources: &exact_sources,
                    invalid_application: Some(&invalid),
                    invalid_index_argument: Some(&invalid),
                    missing_exact_nominal_owner: Some(&invalid),
                    ..Default::default()
                };
                let expected = crate::source_state::runtime_callable_identity::normalize_signature(
                    self,
                    original,
                    &TypeIdentityContext {
                        substitutions: &types,
                        substitution_scope_starts: &[0],
                        substitution_semantics: SubstitutionSemantics::Simultaneous,
                        index_substitutions: &indices,
                        require_owned_substitution_regions: true,
                        ..context
                    },
                )
                .map_err(|reason| reason.reason())?;
                let recovered =
                    crate::source_state::runtime_callable_identity::normalize_signature(
                        self,
                        contract.required,
                        &context,
                    )
                    .map_err(|reason| reason.reason())?;
                if invalid.get() || expected != recovered {
                    return Err(
                        "recovered callback contract differs from its original applied contract"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }

    fn static_callback_source_matches(
        &self,
        prepared: &AppliedCallableSignature,
    ) -> Result<bool, &'static str> {
        let AppliedCallableOwner::StaticCallback {
            parameter, source, ..
        } = &prepared.owner
        else {
            return Ok(false);
        };
        let original = self
            .retained_static_machine_contract(*parameter)
            .ok_or("callback requirement lost its original binder")?
            .signature();
        Ok(source == original && prepared.signature.symbol == source.symbol)
    }

    pub fn static_callback_requirements(
        &self,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) -> Result<Vec<AppliedStaticCallbackRequirement<'_>>, &'static str> {
        use crate::source_state::static_applications::StaticMachineBindingValue;
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("closed callback selection lost its source application");
        }
        let bindings = self.static_machine_application_bindings(application);
        let mut callbacks = bindings
            .iter()
            .filter_map(|binding| match &binding.value {
                StaticMachineBindingValue::Machine(argument) => Some((binding.parameter, argument)),
                _ => None,
            })
            .collect::<Vec<_>>();
        callbacks.sort_unstable_by_key(|(parameter, _)| {
            (parameter.arena_index(), parameter.generation())
        });
        let mut rows = self
            .applied_callable_signatures
            .callback_rows(application)
            .map(|(_, row)| {
                let AppliedCallableOwner::StaticCallback { parameter, .. } = row.owner else {
                    return Err("closed callback selection has another contract owner");
                };
                Ok(((parameter.arena_index(), parameter.generation()), row))
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        rows.sort_unstable_by_key(|(key, _)| *key);
        if rows.len() != callbacks.len() {
            return Err("closed callback selection has an incomplete or extra contract roster");
        }
        let mut required = Vec::with_capacity(rows.len());
        let mut shared = None;
        for ((parameter, argument), (_, row)) in callbacks.into_iter().zip(rows) {
            let AppliedCallableOwner::StaticCallback {
                parameter: retained,
                application: owner,
                lexical_owner,
                bindings: snapshot,
                lifetimes,
                ..
            } = row.owner
            else {
                return Err("closed callback selection has another contract owner");
            };
            if retained != parameter
                || owner != application
                || !self.static_callback_source_matches(row)?
            {
                return Err("closed callback selection changed its source contract or parameter");
            }
            let context = (lexical_owner, snapshot, lifetimes);
            if let Some(known) = shared {
                if known != context {
                    return Err("closed callback contracts do not share their actual application");
                }
            } else {
                if !self.static_callback_requirement_matches(row, application)? {
                    return Err("closed callback selection changed its source application");
                }
                shared = Some(context);
            }
            required.push(AppliedStaticCallbackRequirement {
                parameter,
                required: &row.signature,
                argument,
            });
        }
        Ok(required)
    }
}

#[cfg(test)]
mod callback_contract_recovery_tests {
    use super::*;
    use crate::source_state::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
    use crate::source_state::expression::{Expression, StaticMachineArgument};
    use crate::source_state::machine::Machine;
    use crate::source_state::name::Identifier;
    use crate::source_state::signature::{StateParameter, StateSignature};
    use crate::source_state::static_applications::{
        StaticMachineBinding, StaticMachineBindingValue,
    };
    use crate::source_state::types::TypeReferenceNode;
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    fn prepared() -> (
        TypedTrees,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) {
        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("invoke"));
        let binders = symbols.insert_children(
            owner,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::MachineParameter, SymbolNameRef::Static("Step")),
            ],
        );
        let element = binders.handle_at(0);
        let callback = binders.handle_at(1);
        let value = symbols
            .insert_children(
                callback,
                [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
            )
            .handle_at(0);
        let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
        let selected = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("finish"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let parameter = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: element,
                name: Identifier::from("T"),
            });
        let required = StateSignature {
            symbol: callback,
            parameters: program.state_parameters.insert_many([StateParameter {
                symbol: value,
                name: Identifier::from("value"),
                type_reference: parameter,
                ..Default::default()
            }]),
            return_type: unit,
            termination_guarantee: declaration_contracts::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        };
        let parameters = program.data_type_parameters.insert_many([
            TypeParameter {
                symbol: element,
                name: Identifier::from("T"),
                ..Default::default()
            },
            TypeParameter {
                symbol: callback,
                name: Identifier::from("Step"),
                kind: TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(required),
                },
                ..Default::default()
            },
        ]);
        program.push_machine(Machine {
            symbol: owner,
            name: Identifier::from("invoke"),
            type_parameters: parameters,
            ..Default::default()
        });
        program.push_machine(Machine {
            symbol: caller,
            name: Identifier::from("caller"),
            ..Default::default()
        });
        let chosen = StaticMachineArgument {
            symbol: selected,
            path: vec![Identifier::from("finish")].into_boxed_slice(),
            ..Default::default()
        };
        let arguments = [
            StaticMachineArgument {
                type_reference: unit,
                ..Default::default()
            },
            chosen.clone(),
        ];
        let application = program.expression_table.retain_static_machine_application(
            owner,
            Identifier::from("invoke"),
            &arguments,
        );
        let invocation = program
            .expression_table
            .static_machine_application_mut(application);
        invocation.lexical_owner = caller;
        invocation.binding_declaration = owner;
        program
            .expression_table
            .set_static_machine_application_bindings(
                application,
                &[
                    StaticMachineBinding {
                        parameter: element,
                        value: StaticMachineBindingValue::Type(unit),
                    },
                    StaticMachineBinding {
                        parameter: callback,
                        value: StaticMachineBindingValue::Machine(chosen),
                    },
                ],
            );
        program
            .prepare_static_callback_requirements(application)
            .unwrap();
        (program, application)
    }

    #[test]
    fn frozen_recovery_rejects_callback_contract_edits_with_the_same_symbol() {
        let (original, application) = prepared();
        let encoded = record_storage::encode(
            &crate::source_state::FrozenTypedTrees::from(original.clone()),
            Default::default(),
        )
        .unwrap();
        let recovered: crate::source_state::FrozenTypedTrees =
            record_storage::decode(&encoded, Default::default()).unwrap();
        assert_eq!(
            recovered
                .static_callback_requirements(application)
                .unwrap()
                .len(),
            1
        );
        for change in 0..2 {
            let mut forged = original.clone();
            let (handle, row) = forged
                .applied_callable_signatures
                .callback_rows(application)
                .next()
                .unwrap();
            let mut row = row.clone();
            if change == 0 {
                row.signature.termination_guarantee =
                    declaration_contracts::TerminationGuarantee::NoGuarantee;
            } else {
                let expression = forged
                    .expression_table
                    .insert_tree(&Expression::Boolean(false));
                let facts = forged.proof_facts.insert_many([
                    crate::source_state::domain::ProofFact::Expression(expression),
                ]);
                row.signature.contracts = forged.signature_contracts.insert_many([
                    crate::source_state::signature::SignatureContract {
                        kind: crate::source_state::signature::SignatureContractKind::Requires,
                        facts,
                        ..Default::default()
                    },
                ]);
            }
            forged
                .applied_callable_signatures
                .replace(handle, row)
                .unwrap();
            assert!(
                forged.static_callback_requirements(application).is_ok(),
                "the live association still has the original symbol and tuple"
            );
            let encoded = record_storage::encode(
                &crate::source_state::FrozenTypedTrees::from(forged),
                Default::default(),
            )
            .unwrap();
            let error = record_storage::decode::<crate::source_state::FrozenTypedTrees>(
                &encoded,
                Default::default(),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains(
                    "recovered callback contract differs from its original applied contract"
                ),
                "edit {change}: {error}"
            );
        }
    }
}
