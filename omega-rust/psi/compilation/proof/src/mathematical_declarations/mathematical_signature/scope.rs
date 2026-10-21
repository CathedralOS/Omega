//! Scope and declaration lookup: in-scope symbols, level indices, and
//! the constant/term a declaration or carrier resolves to.

use proof_admission::{Term, TermHandle};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::name::Identifier;

use super::{Elaborator, ScopeEntry};

impl<'a> Elaborator<'a> {
    /// Resolve a symbol in term position: scope first, then earlier
    /// authored declarations, then carrier interning or a kind refusal.
    pub(super) fn named_term(
        &mut self,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if self.program.symbols.builtin_type_member(symbol)
            == Some(symbols::BuiltinTypeMember::AddressBound)
        {
            // Symbolic Int owned by the exact address carrier, not a host-size
            // literal or an authored assumption. Numeric closure belongs to
            // the selected address model; signature formation needs its type.
            let ty = self.integer_constant();
            let position = match self.address_bound {
                Some(position) => position,
                None => {
                    let position = self.push_assumption(ty);
                    self.address_bound = Some(position);
                    position
                }
            };
            return Ok(self.constant(position));
        }
        if let Some(term) = self.scope_term_for_symbol(symbol) {
            return Ok(term);
        }
        if let Some(term) = self.constant_for_declaration(symbol)? {
            return Ok(term);
        }
        if self.program.symbols.builtin_type_atom(symbol).is_some()
            && let Some(reference) = self
                .program
                .type_reference_table
                .find_named_type_reference(symbol)
        {
            return self.elaborate_type_reference(reference);
        }
        if let Some(term) = self.carrier_for_symbol(symbol) {
            return Ok(term);
        }
        Err(self.refuse(format!("`{name}` is not a mathematical term")))
    }

    /// A `Variable` for the scope entry carrying `symbol`, if any —
    /// innermost wins under shadowing.
    pub(super) fn scope_term_for_symbol(&mut self, symbol: SymbolHandle) -> Option<TermHandle> {
        if let Some((_, term)) = self
            .source_constructor_context
            .values
            .iter()
            .find(|(bound, _)| *bound == symbol)
        {
            return Some(*term);
        }
        let position = self
            .scope
            .iter()
            .rposition(|entry| entry.symbol == Some(symbol))?;
        Some(self.scope_variable(position))
    }

    /// A `Variable` for the innermost scope entry named `name`, if any —
    /// how arrow binders and symbol-less callee references resolve.
    pub(super) fn scope_term_for_name(&mut self, name: &Identifier) -> Option<TermHandle> {
        let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
        Some(self.scope_variable(position))
    }

    /// The `Variable` for scope position `position`: entry `k` of `n` is
    /// de Bruijn index `n - 1 - k`.
    pub(super) fn scope_variable(&mut self, position: usize) -> TermHandle {
        let index = self.scope.len() - 1 - position;
        self.arena.insert(Term::Variable(index as u32))
    }

    pub(super) fn push_scope(
        &mut self,
        symbol: Option<SymbolHandle>,
        name: Identifier,
        domain: TermHandle,
    ) {
        self.scope.push(ScopeEntry {
            symbol,
            name,
            domain,
        });
    }

    /// A `Constant` for an elaborated earlier authored declaration, or a
    /// refusal when the reference is self/forward or the declaration is
    /// universe-polymorphic without an explicit application.
    pub(super) fn constant_for_declaration(
        &mut self,
        symbol: SymbolHandle,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        if let Some(&position) = self.authored_positions.get(&symbol) {
            let declaration = &self.declarations[position as usize];
            if declaration.level_arity != 0 {
                return Err(self.refuse(format!(
                    "referencing mathematical declaration `{}` needs {} explicit level arguments",
                    self.program.symbols.name(symbol),
                    declaration.level_arity
                )));
            }
            let term = self.arena.insert(Term::Constant {
                declaration: position,
                levels: Vec::new(),
            });
            return Ok(Some(term));
        }
        if self.authored_index.contains_key(&symbol) {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` references itself or a later declaration; the signature admits strictly earlier ones",
                self.program.symbols.name(symbol)
            )));
        }
        Ok(None)
    }

    /// Intern a resolved non-core symbol as an uninterpreted carrier
    /// (`assumption(0, Type 0)`), reusing its position on repeat use.
    /// Fixed-width integer and `Int`/`UInt` atoms share the bounded
    /// denotation's `Int` carrier; `bool` and `addr` share their own;
    /// other builtin atoms and authored `data` types intern per-symbol.
    /// Symbols of other kinds decline so the caller can refuse.
    pub(super) fn carrier_for_symbol(&mut self, symbol: SymbolHandle) -> Option<TermHandle> {
        if let Some(&position) = self.carriers.get(&symbol) {
            return Some(self.constant(position));
        }
        let position = match self.program.symbols.get(symbol).kind {
            symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => {
                match self.program.symbols.builtin_type_atom(symbol) {
                    Some(
                        BuiltinTypeAtom::I8
                        | BuiltinTypeAtom::I16
                        | BuiltinTypeAtom::I32
                        | BuiltinTypeAtom::I64
                        | BuiltinTypeAtom::U8
                        | BuiltinTypeAtom::U16
                        | BuiltinTypeAtom::U32
                        | BuiltinTypeAtom::U64
                        | BuiltinTypeAtom::Int
                        | BuiltinTypeAtom::UInt,
                    ) => self.integer_carrier(),
                    Some(BuiltinTypeAtom::Bool) => self.boolean_carrier(),
                    Some(BuiltinTypeAtom::Address) => self.address_carrier(),
                    Some(BuiltinTypeAtom::F32) => self.float32_carrier(),
                    Some(BuiltinTypeAtom::F64) => self.float64_carrier(),
                    _ => self.push_type_carrier(),
                }
            }
            _ => return None,
        };
        self.carriers.insert(symbol, position);
        Some(self.constant(position))
    }
}
