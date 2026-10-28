//! Each declared state's `let` locals, recorded once.
//!
//! Lowering needs a state's local declarations in places no result binding
//! reaches: the live affine owners a selection transports, a scalar local's
//! declared refinement, the `LocalInitializer` ordinal an immutable scalar
//! value is keyed by, and the record referee a silent loan lends. These are
//! questions about the frozen typed program alone, so checking records the
//! answer for every declared state and lowering reads the rows instead of
//! scanning the state's statements again.
//!
//! The table is dense by the state symbol's `Handle::arena_index`, the key
//! every reader already resolves a state through. Index 0 is the invalid symbol, which, like
//! every symbol the walk did not name, declares no locals.

use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// One `let` a state declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedStateLocal {
    /// The declaring statement's position in the state body.
    pub statement_index: u32,
    pub symbol: SymbolHandle,
    pub declared_type: TypeReferenceHandle,
    /// The declaration spells no type; `declared_type` is the inferred one.
    pub type_is_inferred: bool,
    pub is_mutable: bool,
    /// An immutable, initialized primitive local: its value is a scalar SSA
    /// value keyed by its `LocalInitializer` ordinal
    /// (`StateLocalFacts::scalar_locals_before` its statement).
    pub is_scalar_value: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StateLocalFacts {
    /// `spans[index]` is the range of `locals` the state whose symbol has
    /// arena index `index` declares, in statement order.
    spans: Vec<(u32, u32)>,
    locals: Vec<CheckedStateLocal>,
}

impl StateLocalFacts {
    /// Adopt every declared state's locals, each list in statement order.
    pub fn with_states(
        states: impl IntoIterator<Item = (SymbolHandle, Vec<CheckedStateLocal>)>,
    ) -> Self {
        let mut facts = Self::default();
        for (state, locals) in states {
            let index = state.arena_index() as usize;
            if facts.spans.len() <= index {
                facts.spans.resize(index + 1, (0, 0));
            }
            let start = facts.locals.len() as u32;
            facts.locals.extend(locals);
            facts.spans[index] = (start, facts.locals.len() as u32);
        }
        facts
    }

    /// The locals `state` declares, in statement order.
    pub fn locals(&self, state: SymbolHandle) -> &[CheckedStateLocal] {
        let (start, end) = self
            .spans
            .get(state.arena_index() as usize)
            .copied()
            .unwrap_or_default();
        &self.locals[start as usize..end as usize]
    }

    /// The local `state` declares at `statement_index`.
    pub fn at_statement(
        &self,
        state: SymbolHandle,
        statement_index: u32,
    ) -> Option<&CheckedStateLocal> {
        self.locals(state)
            .iter()
            .find(|local| local.statement_index == statement_index)
    }

    /// How many scalar-value locals `state` declares before `statement_index`:
    /// the `LocalInitializer` ordinal of a scalar local declared there.
    pub fn scalar_locals_before(&self, state: SymbolHandle, statement_index: u32) -> u32 {
        self.locals(state)
            .iter()
            .take_while(|local| local.statement_index < statement_index)
            .filter(|local| local.is_scalar_value)
            .count() as u32
    }

    /// The one local of `state` named `symbol`.
    pub fn named(&self, state: SymbolHandle, symbol: SymbolHandle) -> Option<&CheckedStateLocal> {
        let mut named = self
            .locals(state)
            .iter()
            .filter(|local| local.symbol == symbol && symbol.is_valid());
        let local = named.next()?;
        named.next().is_none().then_some(local)
    }
}
