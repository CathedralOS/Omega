//! Rejoin resolver-seeded attached field aliases to their exact declarations.

use crate::capture::contracts::facts::ContractProjectionContext;
use checked_trees::ContractProofFactOwner;
use compiler::CheckedCompilation;
use symbols::{SymbolHandle, SymbolKind};

#[cfg(test)]
mod tests;

pub(super) fn attached_field(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expected: SymbolHandle,
    selected: SymbolHandle,
) -> bool {
    let machine_symbol = match context.owner {
        ContractProofFactOwner::Machine { machine_symbol }
        | ContractProofFactOwner::MachineState { machine_symbol, .. } => machine_symbol,
        _ => return false,
    };
    let symbols = &compilation.symbols;
    if symbols.get(expected).kind != SymbolKind::Field
        || symbols.get(selected).kind != SymbolKind::Field
        || symbols.get(selected).parent != machine_symbol
    {
        return false;
    }
    let mut machines = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == machine_symbol);
    let Some(machine) = machines.next() else {
        return false;
    };
    if machines.next().is_some()
        || !machine.attached_data_symbol.is_valid()
        || machine.attached_data_symbol != symbols.get(expected).parent
    {
        return false;
    }
    let mut declarations = compilation
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    if declarations.next().is_some() {
        return false;
    }
    if compilation.data_members(declaration).iter().filter(|member| {
        matches!(member, typed_trees::data::DataMember::Field(field) if field.symbol == expected)
    }).count() != 1 { return false; }
    let (Some(expected_span), Some(selected_span)) = (
        symbols.symbol_source_span(expected),
        symbols.symbol_source_span(selected),
    ) else {
        return false;
    };
    expected_span.span.start < expected_span.span.end
        && expected_span == selected_span
        && symbols.same_symbol_source_package(expected, selected)
}
