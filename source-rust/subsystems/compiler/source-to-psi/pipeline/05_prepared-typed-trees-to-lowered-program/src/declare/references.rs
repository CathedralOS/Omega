//! Which declarations of a typed program refer to each symbol.
//!
//! A private helper may take its callers' guarantees as its input contract
//! only when no declaration outside the module's lowered machines refers to
//! it (`Program::private_input_contracts`). The closure and each conformance
//! witness ask that question of the same typed program, so the whole-program
//! reference walk runs once per program (`SharedProgram::declaration_references`)
//! and each module checks the recorded rows against its own lowered machines.

use crate::source_state::TypedTrees;
use crate::source_state::references::{TypedReferences, visit_program};
use symbols::SymbolHandle;

/// Every symbol the reference walk reaches, with the declaration whose visit
/// reached it.
///
/// `visit_program` asks `TypedReferences::declaration` before it visits each
/// declaration's references, and no reference is reached outside such a
/// visit. Recording the latest declaration it asked about therefore names
/// the declaration behind every reference. A module's own walk skips the
/// declarations it lowered, so a symbol is referenced from outside that
/// module exactly when one of its rows names a declaration the module did not
/// lower.
pub(crate) struct DeclarationReferences {
    /// (referenced symbol, declaring symbol), sorted by `key` of the
    /// referenced symbol and deduplicated. The declaring symbol is `None` for
    /// a reference reached before the walk asked about any declaration.
    rows: Vec<(SymbolHandle, Option<SymbolHandle>)>,
}

/// A handle as a sort key; handles have no order of their own. Generation
/// comes first so equal slots of independently loaded pages never alias.
fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.generation(), symbol.arena_index())
}

impl DeclarationReferences {
    pub(crate) fn new(typed: &TypedTrees) -> Self {
        let mut recorder = Recorder {
            current: None,
            rows: Vec::new(),
        };
        visit_program(typed, &mut recorder);
        Self::from_rows(recorder.rows)
    }

    fn from_rows(mut rows: Vec<(SymbolHandle, Option<SymbolHandle>)>) -> Self {
        rows.sort_unstable_by_key(|(symbol, declaration)| (key(*symbol), declaration.map(key)));
        rows.dedup_by_key(|(symbol, declaration)| (key(*symbol), declaration.map(key)));
        rows.shrink_to_fit();
        Self { rows }
    }

    /// Whether a declaration that is not among `lowered` refers to `symbol`.
    pub(crate) fn referenced_outside(
        &self,
        symbol: SymbolHandle,
        lowered: &LoweredSymbols,
    ) -> bool {
        let symbol = key(symbol);
        let start = self.rows.partition_point(|(row, _)| key(*row) < symbol);
        self.rows[start..]
            .iter()
            .take_while(|(row, _)| key(*row) == symbol)
            .any(|(_, declaration)| {
                declaration.is_none_or(|declaration| !lowered.contains(declaration))
            })
    }
}

/// The machines one module lowered, the declarations its own walk would skip.
pub(crate) struct LoweredSymbols {
    keys: Vec<(u32, u32)>,
}

impl LoweredSymbols {
    /// `None` when a lowered symbol is not a valid handle.
    pub(crate) fn new(symbols: impl IntoIterator<Item = SymbolHandle>) -> Option<Self> {
        let mut keys = symbols
            .into_iter()
            .map(|symbol| symbol.is_valid().then(|| key(symbol)))
            .collect::<Option<Vec<_>>>()?;
        keys.sort_unstable();
        keys.dedup();
        Some(Self { keys })
    }

    fn contains(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid() && self.keys.binary_search(&key(symbol)).is_ok()
    }
}

/// Visits every declaration and records which one each reference came from.
struct Recorder {
    current: Option<SymbolHandle>,
    rows: Vec<(SymbolHandle, Option<SymbolHandle>)>,
}

impl Extend<SymbolHandle> for Recorder {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        // An invalid handle names no helper, so nothing can ask about it.
        let current = self.current;
        self.rows.extend(
            symbols
                .into_iter()
                .filter(|symbol| symbol.is_valid())
                .map(|symbol| (symbol, current)),
        );
    }
}

impl TypedReferences for Recorder {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.current = Some(symbol);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_counts_as_outside_only_from_a_declaration_the_module_did_not_lower() {
        let helper = SymbolHandle::from_parts(7, 1);
        let caller = SymbolHandle::from_parts(8, 1);
        let other = SymbolHandle::from_parts(9, 1);
        let references = DeclarationReferences::from_rows(vec![
            (helper, Some(caller)),
            (helper, Some(caller)),
            (other, None),
        ]);
        let with_caller = LoweredSymbols::new([helper, caller]).unwrap();
        let without_caller = LoweredSymbols::new([helper]).unwrap();
        assert!(!references.referenced_outside(helper, &with_caller));
        assert!(references.referenced_outside(helper, &without_caller));
        // A reference reached outside any declaration is always outside.
        assert!(references.referenced_outside(other, &with_caller));
        assert!(!references.referenced_outside(caller, &without_caller));
    }

    #[test]
    fn a_foreign_page_with_the_same_slot_cannot_hide_an_outside_caller() {
        let helper = SymbolHandle::from_parts(7, 1);
        let lowered_caller = SymbolHandle::from_parts(8, 1);
        let foreign_caller = SymbolHandle::from_parts(8, 2);
        let references = DeclarationReferences::from_rows(vec![(helper, Some(foreign_caller))]);
        let lowered = LoweredSymbols::new([helper, lowered_caller]).unwrap();
        assert!(references.referenced_outside(helper, &lowered));
        let foreign_helper = SymbolHandle::from_parts(7, 2);
        assert!(!references.referenced_outside(foreign_helper, &lowered));
    }

    #[test]
    fn an_invalid_lowered_symbol_is_refused() {
        assert!(LoweredSymbols::new([SymbolHandle::from_parts(0, 1)]).is_none());
    }
}
