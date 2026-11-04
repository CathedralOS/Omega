//! Keep a chosen helper and its arguments when copying a contract.
//!
//! The original binder still owns the promise that checking must verify. Its
//! selected helper owns a separate call application, including configured
//! arguments and their supplying scope.

mod applications;

use super::CallableApplication;
use crate::{TypedTrees, expression, name::Identifier};
use symbols::{SymbolHandle, SymbolKind};

pub(super) struct AppliedCallTarget {
    parameter: SymbolHandle,
    target: SymbolHandle,
    name: Identifier,
    attached_data: Option<(SymbolHandle, Identifier)>,
    supplied: Option<expression::StaticMachineArgument>,
    provider: Option<crate::static_applications::StaticMachineApplicationHandle>,
}

impl CallableApplication {
    pub(super) fn selected_call_target(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Result<Option<AppliedCallTarget>, &'static str> {
        if !parameter.is_valid()
            || program.symbols.get(parameter).kind != SymbolKind::MachineParameter
        {
            return Ok(None);
        }
        let supplied = self
            .static_arguments
            .iter()
            .rev()
            .find(|(symbol, _)| *symbol == parameter)
            .map(|(_, argument)| argument.clone());
        let provider = self
            .selected_providers
            .iter()
            .find(|(binder, _)| *binder == parameter)
            .map(|(_, provider)| *provider);
        let (selected, name) = if let Some(provider) = provider {
            let record = program.static_machine_application(provider);
            (record.registration_operation, record.target.clone())
        } else if let Some(argument) = &supplied {
            let name = argument
                .path
                .last()
                .cloned()
                .ok_or("selected contract helper lost its diagnostic name")?;
            (argument.symbol, name)
        } else {
            let Some((_, reference)) = self
                .types
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == parameter)
            else {
                return Ok(None);
            };
            let crate::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*reference)
            else {
                return Err("applied contract call needs an exact named callable declaration");
            };
            (*symbol, name.clone())
        };
        if !selected.is_valid()
            || !matches!(
                program.symbols.get(selected).kind,
                SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
            )
        {
            return Err("applied contract call lost its selected callable declaration");
        }
        if selected == parameter
            && supplied
                .as_ref()
                .is_none_or(|argument| argument.application.is_none())
        {
            return Ok(None);
        }
        let machine = program
            .machine_by_symbol(selected)
            .or_else(|| program.machine_holding_state(selected));
        let target = if let Some(machine) = machine
            && machine.symbol == selected
        {
            program
                .machine_states(machine)
                .first()
                .ok_or("applied contract call selected a machine without its entry")?
                .symbol
        } else {
            selected
        };
        let attached_data = match machine {
            Some(machine) if machine.attached_data_symbol.is_valid() => Some((
                machine.attached_data_symbol,
                machine.attached_data.clone().ok_or(
                    "applied contract call lost its attached declaration's diagnostic name",
                )?,
            )),
            _ => None,
        };
        Ok(Some(AppliedCallTarget {
            parameter,
            target,
            name,
            attached_data,
            supplied,
            provider,
        }))
    }

    pub(super) fn apply_call_target(
        &self,
        program: &mut TypedTrees,
        call: &mut expression::TableCallExpression,
        selected: AppliedCallTarget,
    ) -> Result<(), &'static str> {
        if !call.static_machine_parameter.is_valid() {
            call.static_machine_parameter = self.symbol(selected.parameter);
        }
        if let Some(provider) = selected.provider {
            self.retain_prepared_call_application(program, call, provider)?;
        } else if let Some(argument) = &selected.supplied {
            self.retain_selected_call_application(
                program,
                call,
                selected.target,
                &selected.name,
                argument,
            )?;
        }
        call.target_symbol = selected.target;
        call.target = selected.name;
        if let Some((symbol, name)) = selected.attached_data {
            call.receiver = program
                .expression_table
                .insert_tree(&expression::Expression::Name(
                    expression::NamePath::resolved_from_iter([name], symbol, symbol),
                ));
        }
        Ok(())
    }
}
