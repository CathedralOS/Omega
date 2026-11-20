use arena::HandleKeyMap;
use symbols::SymbolHandle;

type SymbolPairs = [(SymbolHandle, SymbolHandle)];

#[derive(Debug, Default)]
struct IndexedPairs {
    pairs: Vec<(SymbolHandle, SymbolHandle)>,
    index: HandleKeyMap<SymbolHandle, SymbolHandle>,
}

impl IndexedPairs {
    fn insert(&mut self, source: SymbolHandle, target: SymbolHandle) -> Option<SymbolHandle> {
        if let Some(previous) = self.index.get(&source) {
            return Some(*previous);
        }
        self.index.insert(source, target);
        self.pairs.push((source, target));
        None
    }
}

/// Prepared lookup for the same structural execution/authored association.
/// Copied symbols are a sparse subset of the retained program: index only
/// those pairs, without allocating from its full symbol-number range.
/// Pair order remains explicit; hashing never chooses declaration order.
#[derive(Debug, Default)]
pub struct PreparedSymbolRebinding {
    execution: IndexedPairs,
    authored: IndexedPairs,
}

impl PreparedSymbolRebinding {
    pub fn insert_execution(
        &mut self,
        source: SymbolHandle,
        target: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.execution.insert(source, target)
    }

    pub fn insert_authored(
        &mut self,
        source: SymbolHandle,
        target: SymbolHandle,
    ) -> Option<SymbolHandle> {
        self.authored.insert(source, target)
    }

    pub fn execution_symbol(&self, source: SymbolHandle) -> Option<SymbolHandle> {
        self.execution.index.get(&source).copied()
    }

    pub fn rebinding(&self) -> SymbolRebinding<'_> {
        SymbolRebinding {
            execution: Pairs::Indexed(&self.execution),
            authored: Pairs::Indexed(&self.authored),
        }
    }
}

#[derive(Clone, Copy)]
enum Pairs<'scope> {
    Borrowed(&'scope SymbolPairs),
    Indexed(&'scope IndexedPairs),
}

impl<'scope> Pairs<'scope> {
    fn lookup(self, source: SymbolHandle) -> Option<SymbolHandle> {
        match self {
            Self::Borrowed(pairs) => pairs
                .iter()
                .find_map(|(old, new)| (*old == source).then_some(*new)),
            Self::Indexed(pairs) => pairs.index.get(&source).copied(),
        }
    }

    fn ordered(self) -> &'scope SymbolPairs {
        match self {
            Self::Borrowed(pairs) => pairs,
            Self::Indexed(pairs) => &pairs.pairs,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SymbolRebinding<'scope> {
    execution: Pairs<'scope>,
    authored: Pairs<'scope>,
}

impl<'scope> SymbolRebinding<'scope> {
    pub fn new(execution: &'scope SymbolPairs, authored: &'scope SymbolPairs) -> Self {
        Self {
            execution: Pairs::Borrowed(execution),
            authored: Pairs::Borrowed(authored),
        }
    }

    pub fn lookup(self, source: SymbolHandle) -> Option<SymbolHandle> {
        self.execution.lookup(source)
    }

    pub(crate) fn execution(self) -> Self {
        Self {
            execution: self.execution,
            authored: self.execution,
        }
    }

    pub fn authored(self) -> Self {
        Self {
            execution: self.authored,
            authored: self.authored,
        }
    }
}

impl<'scope> From<&'scope SymbolPairs> for SymbolRebinding<'scope> {
    fn from(symbols: &'scope SymbolPairs) -> Self {
        Self::new(symbols, symbols)
    }
}

impl<'scope> From<&'scope Vec<(SymbolHandle, SymbolHandle)>> for SymbolRebinding<'scope> {
    fn from(symbols: &'scope Vec<(SymbolHandle, SymbolHandle)>) -> Self {
        Self::from(symbols.as_slice())
    }
}

impl<'scope, const N: usize> From<&'scope [(SymbolHandle, SymbolHandle); N]>
    for SymbolRebinding<'scope>
{
    fn from(symbols: &'scope [(SymbolHandle, SymbolHandle); N]) -> Self {
        Self::from(symbols.as_slice())
    }
}

impl std::ops::Deref for SymbolRebinding<'_> {
    type Target = SymbolPairs;
    fn deref(&self) -> &Self::Target {
        self.execution.ordered()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_rebinding_preserves_scope_order_and_first_match() {
        let declaration = SymbolHandle::from_arena_index(7);
        let formal = SymbolHandle::from_arena_index(9);
        let copied_declaration = SymbolHandle::from_arena_index(17);
        let copied_formal = SymbolHandle::from_arena_index(19);
        let mut prepared = PreparedSymbolRebinding::default();
        assert!(
            prepared
                .insert_execution(declaration, copied_declaration)
                .is_none()
        );
        assert!(prepared.insert_execution(formal, copied_formal).is_none());
        assert!(prepared.insert_authored(formal, copied_formal).is_none());
        assert_eq!(
            prepared.insert_execution(declaration, copied_formal),
            Some(copied_declaration)
        );
        let view = prepared.rebinding();
        assert_eq!(
            crate::source_state::remapped(declaration, view),
            copied_declaration
        );
        assert_eq!(
            crate::source_state::remapped(declaration, view.authored()),
            declaration
        );
        assert_eq!(
            crate::source_state::remapped(formal, view.authored()),
            copied_formal
        );
        let external = SymbolHandle::from_arena_index(1_000_000);
        assert_eq!(crate::source_state::remapped(external, view), external);
        assert_eq!(
            view.iter().copied().collect::<Vec<_>>(),
            [(declaration, copied_declaration), (formal, copied_formal)]
        );
    }
}
