//! Retained generic applications share one complete binder tuple, regardless
//! of whether the declaration combines types, scalar consts or schema consts.

use super::Program;
use crate::lowering::body::ValueType;
use crate::source_state::trait_definition::AppliedTraitScope;
use semantic_vocabulary::{MachineId, ScalarTerm};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

#[derive(Debug)]
pub(super) struct ApplicationArguments<'source> {
    /// Original source scope for callbacks nested in this application.
    pub(super) lexical_owner: SymbolHandle,
    pub(super) types: Vec<ValueType>,
    pub(super) type_sources: Vec<(
        SymbolHandle,
        crate::source_state::types::TypeReferenceHandle,
    )>,
    pub(super) values: Vec<ScalarTerm>,
    pub(super) proofs: Vec<terminal_psi::TheoremSource>,
    pub(super) const_sources: Vec<(
        SymbolHandle,
        crate::source_state::types::TypeReferenceHandle,
    )>,
    pub(super) callables: Vec<(
        SymbolHandle,
        &'source crate::source_state::expression::StaticMachineArgument,
    )>,
}

/// Source-only authentication is shared with static partition discovery,
/// which runs before any checking machine or scalar input is allocated.
pub(super) struct ApplicationBindings<'source> {
    lexical_owner: SymbolHandle,
    pub(super) parameters: arena::RootRows<'source, crate::source_state::data::TypeParameter>,
    pub(super) inherited: bool,
    pub(super) bindings: arena::HandleKeyMap<
        SymbolHandle,
        &'source crate::source_state::static_applications::StaticMachineBinding,
    >,
}

impl<'source> Program<'source> {
    pub(super) fn application_bindings(
        &self,
        id: MachineId,
        caller: SymbolHandle,
        target: SymbolHandle,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        authored: &[crate::source_state::expression::StaticMachineArgument],
    ) -> Checked<ApplicationBindings<'source>> {
        let unavailable = |reason| super::services::unavailable(id, reason);
        let typed = self.typed;
        let parameters = if let Some(machine) = typed.machine_by_symbol(target) {
            typed.machine_type_parameters(machine)
        } else if let Some((_, method)) = typed.trait_machine_signature(target) {
            typed.state_signature_type_parameters(method)
        } else {
            return Err(unavailable("generic application has no actual callee"));
        };
        let source = if application.is_valid() {
            if !typed
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err(unavailable(
                    "generic application has a stale retained source",
                ));
            }
            let source = typed
                .expression_table
                .static_machine_application(application);
            if typed
                .machine_by_own_or_state_symbol(source.lexical_owner)
                .is_none_or(|machine| source.lexical_owner != caller && machine.symbol != caller)
                || (source.registration_operation != target
                    && typed
                        .machine_by_own_or_state_symbol(source.registration_operation)
                        .map(|machine| machine.symbol)
                        != Some(target))
            {
                return Err(unavailable(
                    "generic application has another source owner or declaration",
                ));
            }
            if !source.selected_machine_arguments.is_empty()
                || !source.input_selections.is_empty()
                || !source.lifetime_bindings.is_empty()
                || source.conformance_selection.is_some()
                || source.callable_contract_source.is_valid()
            {
                return Err(unavailable(
                    "generic application has another static feature",
                ));
            }
            Some(source)
        } else {
            None
        };
        let arguments = source.map_or(&[][..], |_| {
            typed
                .expression_table
                .static_machine_application_arguments(application)
        });
        let bindings = source.map_or(&[][..], |_| {
            typed
                .expression_table
                .static_machine_application_bindings(application)
        });
        if source.is_some_and(|source| {
            arguments.len() != source.arguments.len() || bindings.len() != source.bindings.len()
        }) {
            return Err(unavailable(
                "generic application has a stale argument or binding span",
            ));
        }
        if !authored.is_empty() && authored != arguments {
            return Err(unavailable(
                "generic actuals differ from their retained source tuple",
            ));
        }
        let inherited = bindings.is_empty()
            && parameters.iter().all(|parameter| {
                typed.symbols.symbols().nodes().is_valid(parameter.symbol)
                    && typed.symbols.get(parameter.symbol).parent != target
                    && typed
                        .declaration_type_parameters(caller)
                        .iter()
                        .any(|actual| actual == parameter)
            })
            && match source {
                // Named transitions have no separate static application. Their
                // inherited binders are the same original caller inputs.
                None => arguments.is_empty(),
                Some(source) => {
                    // Source preparation retains captured arguments before it
                    // can prepare bindings. Each capture must still name this
                    // exact original binder in its original position.
                    source.binding_declaration == target
                        && arguments.len() == parameters.len()
                        && parameters
                            .iter()
                            .zip(arguments)
                            .all(|(parameter, argument)| {
                                argument.symbol == parameter.symbol
                                    && argument.application.is_none()
                                    && argument.evidence_projection.is_none()
                                    && argument.const_literal.is_none()
                                    && (!argument.type_reference.is_valid()
                                        || typed
                                            .static_operand_from_type_reference(
                                                argument.type_reference,
                                            )
                                            .is_ok_and(|retained| {
                                                retained.symbol == parameter.symbol
                                                    && retained.application.is_none()
                                                    && retained.evidence_projection.is_none()
                                                    && retained.const_literal.is_none()
                                            }))
                                    && typed
                                        .static_operand_region_supplier(
                                            argument,
                                            source.lexical_owner,
                                        )
                                        .is_ok()
                            })
                }
            };
        if !inherited
            && (source.is_none_or(|source| source.binding_declaration != target)
                || arguments.len() != parameters.len()
                || bindings.len() != parameters.len())
        {
            return Err(unavailable(
                "generic application has no complete exact binder tuple",
            ));
        }
        let mut retained = arena::HandleKeyMap::<SymbolHandle, _>::default();
        for binding in bindings {
            if !typed.symbols.symbols().nodes().is_valid(binding.parameter) {
                return Err(unavailable(
                    "generic application has a foreign retained binder",
                ));
            }
            if retained.insert(binding.parameter, binding).is_some() {
                return Err(unavailable("generic application repeats a retained binder"));
            }
        }
        if !inherited
            && parameters
                .iter()
                .any(|parameter| !retained.contains_key(&parameter.symbol))
        {
            return Err(unavailable("generic application lost a declared binder"));
        }
        Ok(ApplicationBindings {
            lexical_owner: source.map_or(caller, |source| source.lexical_owner),
            parameters,
            inherited,
            bindings: retained,
        })
    }

    /// Authenticate the whole tuple once, then read each argument by its binder kind.
    /// Types select a checking instance; scalar consts become its explicit inputs.
    /// The checking frame supplies values; the lexical caller authenticates the source tuple.
    pub(super) fn application_arguments(
        &mut self,
        id: MachineId,
        caller: SymbolHandle,
        target: SymbolHandle,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        authored: &[crate::source_state::expression::StaticMachineArgument],
    ) -> Checked<ApplicationArguments<'source>> {
        let unavailable = |reason| super::services::unavailable(id, reason);
        let typed = self.typed;
        let prepared = self.application_bindings(id, caller, target, application, authored)?;
        let parameters = prepared.parameters;
        let inherited = prepared.inherited;
        let retained = prepared.bindings;
        let mut types = Vec::new();
        let mut type_sources = Vec::new();
        let mut values = Vec::new();
        let mut const_sources = Vec::new();
        let mut callables = Vec::new();
        let mut proofs = Vec::new();
        for parameter in parameters {
            if matches!(
                parameter.kind,
                crate::source_state::data::TypeParameterKind::Proof { .. }
            ) {
                if inherited {
                    let formal = self
                        .theorem_parameters
                        .iter()
                        .zip(&self.theorem_parameter_sources)
                        .find(|(formal, source)| {
                            formal.machine == id && source.binder == parameter.symbol
                        })
                        .map(|(formal, _)| formal.proof_position)
                        .ok_or_else(|| {
                            unavailable("forwarded theorem has no exact caller formal")
                        })?;
                    proofs.push(terminal_psi::TheoremSource::Formal(formal));
                } else {
                    let binding = retained.get(&parameter.symbol).ok_or_else(|| {
                        unavailable("proof parameter has no unique explicit actual")
                    })?;
                    let crate::source_state::static_applications::StaticMachineBindingValue::Proof(
                        argument,
                    ) = &binding.value
                    else {
                        return Err(unavailable(
                            "proof parameter was supplied with an executable or type binding",
                        ));
                    };
                    proofs.push(self.theorem_argument(id, argument)?);
                }
                continue;
            }
            if let crate::source_state::data::TypeParameterKind::Machine { contract } =
                &parameter.kind
            {
                if typed.machine_parameter_contract_view(contract).is_none() {
                    return Err(unavailable(
                        "generic callable lost its complete declared contract",
                    ));
                }
                if !inherited {
                    let binding = retained.get(&parameter.symbol).ok_or_else(|| {
                        unavailable("callable parameter has no unique retained actual")
                    })?;
                    let crate::source_state::static_applications::StaticMachineBindingValue::Machine(
                        argument,
                    ) = &binding.value
                    else {
                        return Err(unavailable(
                            "callable parameter was supplied with another binding kind",
                        ));
                    };
                    callables.push((parameter.symbol, argument));
                }
                continue;
            }
            if parameter.kind == crate::source_state::data::TypeParameterKind::Type {
                let value = if inherited {
                    self.scope
                        .iter()
                        .find_map(|(binder, value)| (*binder == parameter.symbol).then_some(*value))
                        .ok_or_else(|| {
                            unavailable("inherited type lost its exact caller binding")
                        })?
                } else {
                    let binding = retained.get(&parameter.symbol).ok_or_else(|| {
                        unavailable("type parameter has no unique retained actual")
                    })?;
                    let crate::source_state::static_applications::StaticMachineBindingValue::Type(
                        reference,
                    ) = binding.value
                    else {
                        return Err(unavailable(
                            "type parameter was supplied with another binding kind",
                        ));
                    };
                    type_sources.push((parameter.symbol, reference));
                    self.value_type(id, reference)?
                };
                types.push(value);
                continue;
            }
            let (crate::source_state::data::TypeParameterKind::Const { type_reference }
            | crate::source_state::data::TypeParameterKind::Value { type_reference }) =
                parameter.kind
            else {
                return Err(unavailable(
                    "generic application changed its declared binder kind",
                ));
            };
            let reference = if inherited {
                None
            } else {
                let binding = retained
                    .get(&parameter.symbol)
                    .ok_or_else(|| unavailable("Const parameter has no unique retained actual"))?;
                let crate::source_state::static_applications::StaticMachineBindingValue::Const(
                    reference,
                ) = binding.value
                else {
                    return Err(unavailable(
                        "Const parameter was supplied with another binding kind",
                    ));
                };
                if !typed
                    .type_reference_table
                    .contains_type_reference(reference)
                {
                    return Err(unavailable("Const actual has a stale value reference"));
                }
                Some(reference)
            };
            if let Some(record) = self.record_parameter(parameter.symbol) {
                if inherited {
                    continue;
                } // The exact same source value, already formed above.
                let operand = typed
                    .static_operand_from_type_reference(
                        reference.ok_or_else(|| unavailable("record actual lost its reference"))?,
                    )
                    .map_err(unavailable)?;
                let actual = if let Some(binding) = self.record_parameter(operand.symbol) {
                    if !typed
                        .declaration_type_parameters(caller)
                        .iter()
                        .any(|parameter| parameter.symbol == operand.symbol)
                    {
                        return Err(unavailable(
                            "forwarded record is outside the caller's exact telescope",
                        ));
                    }
                    binding.account.application.clone()
                } else {
                    let value = typed.conformance_by_symbol(operand.symbol).ok_or_else(|| {
                        unavailable("record actual has no represented complete schema construction")
                    })?;
                    AppliedTraitScope::from_conformance(typed, value)
                        .and_then(|scope| scope.portable_application_identity(typed))
                        .and_then(|identity| typed.type_identity_text(identity))
                        .ok_or_else(|| {
                            unavailable("record actual lost its exact schema application")
                        })?
                };
                if actual != record.account.application {
                    return Err(unavailable(
                        "record actual has another exact schema application",
                    ));
                }
            } else {
                if let Some(reference) = reference {
                    const_sources.push((parameter.symbol, reference));
                }
                let scalar_type = self.scalar_type(id, type_reference)?;
                values.push(if let Some(reference) = reference {
                    self.bound_scalar_value(id, scalar_type, reference, true)?
                } else {
                    let input = self
                        .const_scope
                        .iter()
                        .find(|input| {
                            input.symbol == parameter.symbol
                                && input.owner == id
                                && input.scalar_type == scalar_type
                        })
                        .ok_or_else(|| {
                            unavailable("inherited scalar Const lost its exact caller input")
                        })?;
                    semantic_vocabulary::ScalarTerm::value(input.value, scalar_type)
                });
            }
        }
        Ok(ApplicationArguments {
            lexical_owner: prepared.lexical_owner,
            types,
            type_sources,
            values,
            proofs,
            const_sources,
            callables,
        })
    }
}
