//! The machine table's first position for each symbol in one frozen program.

use std::sync::Arc;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

/// Every machine symbol's first position in the machine table, from one pass.
pub(crate) struct MachineIndex(symbols::SymbolKeyMap<symbols::SymbolHandle, u32>);

impl MachineIndex {
    fn of(program: &TypedTrees) -> Self {
        let mut positions = symbols::SymbolKeyMap::default();
        for (position, machine) in program.machines().iter().enumerate() {
            let position = u32::try_from(position).expect("machine position overflow");
            positions.entry(machine.symbol).or_insert(position);
        }
        Self(positions)
    }
}

/// The first machine declaring `symbol`, exactly as a scan of the machine
/// table finds it. A frozen program answers from one index; any other program
/// scans, since a single query costs less than building the index.
pub(crate) fn machine_by_symbol(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&Machine> {
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        let index = memos
            .machines
            .get_or_init(|| Arc::new(MachineIndex::of(program)));
        return index
            .0
            .get(&symbol)
            .map(|&position| &program.machines()[position as usize]);
    }
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
}
