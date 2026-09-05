use super::*;

impl Builder<'_> {
    pub(super) fn charge(&mut self, count: usize) -> Result<(), Vec<Diagnostic>> {
        self.elements = self
            .elements
            .checked_add(count)
            .filter(|count| *count <= MAX_NODES)
            .ok_or_else(|| rejected("source-edit graph exceeds its finite node budget"))?;
        Ok(())
    }
    pub(super) fn expression(&mut self, handle: ExpressionHandle) -> Result<(), Vec<Diagnostic>> {
        if handle.is_valid()
            && self
                .seen_expressions
                .insert((handle.arena_index(), handle.generation()))
        {
            self.charge(1)?;
            self.pending.push(Pending::Expression(handle));
        }
        Ok(())
    }
    pub(super) fn type_reference(
        &mut self,
        handle: TypeReferenceHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        if handle.is_valid()
            && self
                .seen_types
                .insert((handle.arena_index(), handle.generation()))
        {
            self.charge(1)?;
            self.pending.push(Pending::Type(handle));
        }
        Ok(())
    }
    pub(super) fn symbol(&mut self, handle: SymbolHandle) -> Result<(), Vec<Diagnostic>> {
        if handle.is_valid()
            && self
                .seen_symbols
                .insert((handle.arena_index(), handle.generation()))
        {
            self.charge(1)?;
            self.pending.push(Pending::Symbol(handle));
        }
        Ok(())
    }
}
