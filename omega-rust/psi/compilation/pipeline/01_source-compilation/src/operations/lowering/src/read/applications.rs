//! Retained generic applications share one complete binder tuple, regardless
//! of whether the declaration combines types, scalar consts or schema consts.

use super::Program;
use crate::lowering::body::ValueType;
use semantic_vocabulary::{MachineId, ScalarTerm};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use crate::source_state::trait_definition::AppliedTraitScope;

#[derive(Debug)]
pub(super) struct ApplicationArguments<'source> {
    pub(super) types: Vec<ValueType>,
    pub(super) references: Option<Vec<crate::source_state::types::TypeReferenceHandle>>,
    pub(super) values: Vec<ScalarTerm>,
    pub(super) callables: Vec<(
        SymbolHandle,
        &'source crate::source_state::expression::StaticMachineArgument,
    )>,
}

impl<'source> Program<'source> {
    /// Authenticate the whole tuple once, then read each argument by its binder kind.
    /// Types select a checking instance; scalar consts become its explicit inputs.
    /// Record consts retain their complete source-only schema account.
    pub(super) fn application_arguments(
        &mut self,
        id: MachineId,
        target: SymbolHandle,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        authored: &[crate::source_state::expression::StaticMachineArgument],
    ) -> Checked<ApplicationArguments<'source>> {
        let unavailable = |reason| super::services::unavailable(id, reason);
        let typed = self.typed;
        let machine = typed
            .machine_by_symbol(target)
            .ok_or_else(|| unavailable("generic application has no actual callee"))?;
        let caller = *self
            .symbols
            .get(id)
            .ok_or_else(|| unavailable("generic application has no source caller"))?;
        let parameters = typed.machine_type_parameters(machine);
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
                .map(|machine| machine.symbol)
                != Some(caller)
                || typed
                    .machine_by_own_or_state_symbol(source.registration_operation)
                    .map(|machine| machine.symbol)
                    != Some(target)
                || !source.selected_machine_arguments.is_empty()
                || !source.input_selections.is_empty()
                || !source.lifetime_bindings.is_empty()
                || source.conformance_selection.is_some()
                || source.callable_contract_source.is_valid()
            {
                return Err(unavailable(
                    "generic application changed its caller or target, or has another static feature",
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
        let mut retained = symbols::SymbolMap::default();
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
        let mut types = Vec::new();
        let mut references = Vec::new();
        let mut values = Vec::new();
        let mut callables = Vec::new();
        for parameter in parameters {
            if let crate::source_state::data::TypeParameterKind::Machine { contract } = &parameter.kind {
                if contract.structural().is_none() {
                    return Err(unavailable(
                        "generic callable lost its complete structural contract",
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
                    references.push(reference);
                    self.value_type(id, reference)?
                };
                types.push(value);
                continue;
            }
            let crate::source_state::data::TypeParameterKind::Const { type_reference } = parameter.kind
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
                let crate::source_state::static_applications::StaticMachineBindingValue::Const(reference) =
                    binding.value
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
            types,
            references: (!inherited).then_some(references),
            values,
            callables,
        })
    }
}
