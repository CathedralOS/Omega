//! Rejoin an authored accepted-machine handle to exactly one checked declaration.

use declaration_contracts::trust_declarations::AcceptedMachineDeclaration;
use diagnostics::Diagnostic;

pub(crate) fn accepted_machine<'a>(
    machines: &[&'a AcceptedMachineDeclaration],
    symbol: symbols::SymbolHandle,
) -> Result<&'a AcceptedMachineDeclaration, Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "the granted accepted machine has no valid exact symbol",
        ));
    }
    let key = (symbol.arena_index(), symbol.generation());
    let first = machines.partition_point(|machine| {
        (machine.machine.arena_index(), machine.machine.generation()) < key
    });
    let last = machines.partition_point(|machine| {
        (machine.machine.arena_index(), machine.machine.generation()) <= key
    });
    let machines = &machines[first..last];
    let [machine] = machines else {
        return Err(Diagnostic::error(match machines.len() {
            0 => {
                format!("granted accepted-machine symbol {symbol:?} has no exact typed definition")
            }
            count => format!(
                "granted accepted-machine symbol {symbol:?} has {count} exact typed definitions"
            ),
        }));
    };
    Ok(*machine)
}
