//! Use the helper selected by a trait in that trait’s contracts.
//!
//! Selecting a named helper changes the call target, not the source binder’s
//! contract or the call’s own arguments. The existing checker still verifies
//! that the helper satisfies the binder’s contract.

use super::CallableApplication;
use crate::{TypedTrees, expression, name::Identifier};
use symbols::{SymbolHandle, SymbolKind};

pub(super) struct AppliedCallTarget {
    parameter: SymbolHandle,
    target: SymbolHandle,
    name: Identifier,
    attached_data: Option<(SymbolHandle, Identifier)>,
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
        // The trait parameter bindings supply named declarations. Executable
        // application bindings retain their separate complete selection and
        // are not projected here.
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
        if !symbol.is_valid()
            || !matches!(
                program.symbols.get(*symbol).kind,
                SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
            )
        {
            return Err("applied contract call lost its named callable declaration");
        }
        if *symbol == parameter {
            return Ok(None);
        }
        let selected = *symbol;
        let name = name.clone();
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
        }))
    }

    pub(super) fn apply_call_target(
        &self,
        program: &mut TypedTrees,
        call: &mut expression::TableCallExpression,
        selected: AppliedCallTarget,
    ) {
        if !call.static_machine_parameter.is_valid() {
            call.static_machine_parameter = self.symbol(selected.parameter);
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
    }
}
