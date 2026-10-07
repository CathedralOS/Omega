//! Universe-level and declared-type elaboration: authored level parameters,
//! mathematical types, type references, and named types become binder
//! entries and term domains inside the signature under construction.

use crate::proof::FixedCore;
use proof_admission::{Level, Sort, Term, TermHandle};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::mathematical::{MathematicalType, MathematicalTypeHandle};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Elaborator, ScalarCarrier, ScopeEntry};

impl<'a> Elaborator<'a> {
    /// Elaborate a universe-level argument: an authored `core::Level`
    /// binder resolves to `Level::Parameter` by authored position, and an
    /// integer literal (`core::Strict<0>`) is a closed `Level::Constant`.
    /// Computed level expressions are a later leg.
    pub(super) fn elaborate_level(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<Level, Vec<diagnostics::Diagnostic>> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(self.refuse(
                "a universe-level argument references a type that does not exist".to_owned(),
            ));
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(index) = self.level_index(*symbol, name) {
                    return Ok(Level::Parameter(index));
                }
                if symbol.is_valid() {
                    return Err(self.refuse(format!(
                        "`{name}` is not a `core::Level` binder of this declaration"
                    )));
                }
                // A literal level arrives as an unresolved name — authored
                // identifiers cannot spell digits.
                if let Ok(value) = name.as_str().parse::<u64>() {
                    if value <= u64::from(u32::MAX) {
                        return Ok(Level::Constant(value as u32));
                    }
                    return Err(self.refuse(format!(
                        "universe level literal `{name}` exceeds the kernel's level range"
                    )));
                }
                Err(self.refuse(format!(
                    "unresolved universe level `{name}`; only `core::Level` binders and literals elaborate"
                )))
            }
            TypeReferenceNode::ConstExpression(expression) => {
                match self.program.expression_table.expression(*expression) {
                    ExpressionNode::Integer(literal) => match literal.value_u64() {
                        Some(value) if value <= u64::from(u32::MAX) => {
                            Ok(Level::Constant(value as u32))
                        }
                        _ => Err(self.refuse(format!(
                            "universe level literal `{}` exceeds the kernel's level range",
                            literal.text()
                        ))),
                    },
                    _ => Err(self.refuse(
                        "computed universe levels are a later leg; use a `core::Level` binder or a literal"
                            .to_owned(),
                    )),
                }
            }
            _ => Err(self.refuse(
                "a universe-level argument must name a `core::Level` binder or a literal"
                    .to_owned(),
            )),
        }
    }

    /// Elaborate a `MathematicalType` node: ordinary references delegate,
    /// arrows become `Pi` with the named binder scoping over the codomain,
    /// and applications fold to `Apply` spines over elaborated argument
    /// terms.
    pub(super) fn elaborate_mathematical_type(
        &mut self,
        handle: MathematicalTypeHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.program.mathematical_type(handle) {
            MathematicalType::Ordinary(reference) => self.elaborate_type_reference(*reference),
            MathematicalType::Arrow {
                binder,
                domain,
                codomain,
            } => {
                let domain = self.elaborate_mathematical_type(*domain)?;
                // The kernel's `Pi` codomain lives under the binder whether
                // or not it is named, so an anonymous arrow still pushes a
                // scope entry — an unmatchable (missing) name and no symbol.
                self.scope.push(ScopeEntry {
                    symbol: None,
                    name: binder.clone().unwrap_or_default(),
                    domain,
                });
                let codomain = self.elaborate_mathematical_type(*codomain);
                self.scope.pop();
                Ok(self.arena.insert(Term::Pi {
                    domain,
                    codomain: codomain?,
                }))
            }
            MathematicalType::Application { callee, arguments } => {
                let mut term = self.elaborate_mathematical_type(*callee)?;
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(*arguments)
                    .to_vec()
                {
                    let argument = self.elaborate_expression(argument)?;
                    term = self.arena.insert(Term::Apply {
                        function: term,
                        argument,
                    });
                }
                Ok(term)
            }
        }
    }

    /// Elaborate an ordinary type reference in type position. The fixed
    /// `core::*` vocabulary forms sorts and propositions; resolved
    /// non-core symbols intern or reference carrier assumptions and
    /// earlier declarations; everything else refuses.
    pub(super) fn elaborate_type_reference(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(
                self.refuse("a mathematical type references a type that does not exist".to_owned())
            );
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, name } => self.named_type(*symbol, name),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if !lifetime_arguments.is_empty() {
                    return Err(self.refuse(format!(
                        "mathematical type `{base_name}` carries lifetime arguments the kernel cannot express"
                    )));
                }
                let arguments = self
                    .program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec();
                let base_symbol = *base_symbol;
                let base_name = base_name.clone();
                match self.fixed_core.classify(base_symbol, &base_name) {
                    Some(FixedCore::Type) => match arguments.as_slice() {
                        [universe] => {
                            let level = self.elaborate_level(*universe)?;
                            Ok(self.arena.insert(Term::Sort(Sort::Type(level))))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Type` takes one universe level, not {}",
                            arguments.len()
                        ))),
                    },
                    Some(FixedCore::Strict) => match arguments.as_slice() {
                        [universe] => {
                            let level = self.elaborate_level(*universe)?;
                            Ok(self.arena.insert(Term::Sort(Sort::Strict(level))))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Strict` takes one universe level, not {}",
                            arguments.len()
                        ))),
                    },
                    Some(FixedCore::Squash) => match arguments.as_slice() {
                        [ty] => {
                            let ty = self.elaborate_type_reference(*ty)?;
                            Ok(self.arena.insert(Term::Squash { ty }))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Squash` takes one type, not {}",
                            arguments.len()
                        ))),
                    },
                    Some(FixedCore::Level) => {
                        Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
                    }
                    None => {
                        if base_symbol.is_valid() && self.authored_index.contains_key(&base_symbol)
                        {
                            return Err(self.refuse(format!(
                                "applying mathematical declaration `{base_name}` as a carrier is a later leg"
                            )));
                        }
                        Err(self.refuse(format!(
                            "applied carrier `{base_name}` has no kernel denotation yet"
                        )))
                    }
                }
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::DynamicTrait { .. } => Err(self.refuse(format!(
                "mathematical type `{}` cannot be a borrow, constraint or dynamic trait",
                self.program.display_type_reference(reference)
            ))),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::ConstExpression(_) => Err(self.refuse(format!(
                "mathematical type `{}` has no kernel denotation yet",
                self.program.display_type_reference(reference)
            ))),
            TypeReferenceNode::Unit => Ok(self.carrier_term(ScalarCarrier::Unit)),
        }
    }

    /// Resolve a `Named` reference in type position. Scope entries
    /// (type/subject binders) resolve by symbol; authored declarations
    /// become `Constant`s; other resolved symbols intern as carrier
    /// assumptions or refuse by kind; the fixed `core::*` vocabulary
    /// refuses unapplied.
    pub(super) fn named_type(
        &mut self,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if symbol.is_valid() {
            // Fixed-vocabulary identity wins over every other resolution
            // outcome: scope binders and authored declarations carry their
            // own symbols, so a reference that resolves to a fixed `core::*`
            // declaration is the vocabulary however it was spelled — and a
            // same-spelled foreign symbol is not.
            if let Some(member) = self.fixed_core.classify(symbol, name) {
                return match member {
                    FixedCore::Level => {
                        Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
                    }
                    member => self.unapplied_core_type(member, name),
                };
            }
            if let Some(term) = self.scope_term_for_symbol(symbol) {
                return Ok(term);
            }
            if let Some(term) = self.constant_for_declaration(symbol)? {
                return Ok(term);
            }
            if let Some(term) = self.carrier_for_symbol(symbol) {
                return Ok(term);
            }
            return Err(self.refuse(format!("`{name}` is not a mathematical type")));
        }
        match self.fixed_core.classify(symbol, name) {
            Some(FixedCore::Level) => {
                Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
            }
            Some(member) => self.unapplied_core_type(member, name),
            None => {
                if let Some(term) = self.scope_term_for_name(name) {
                    return Ok(term);
                }
                Err(self.refuse(format!("unresolved mathematical type `{name}`")))
            }
        }
    }

    /// Elaborate a declaration's body: at a `core::Strict` result a
    /// An unapplied fixed-core universe in type position claims the next
    /// generalized level parameter — `x: core::Type` binds a type at a
    /// published universe parameter exactly as a bare `T` binder does.
    /// `core::Squash` needs a type argument, so it cannot generalize and
    /// refuses, as does `core::Level` unapplied (handled by its own arm in
    /// `named_type`).
    pub(super) fn unapplied_core_type(
        &mut self,
        member: FixedCore,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match member {
            FixedCore::Type => {
                let level = self.generalized_level();
                Ok(self.arena.insert(Term::Sort(Sort::Type(level))))
            }
            FixedCore::Strict => {
                let level = self.generalized_level();
                Ok(self.arena.insert(Term::Sort(Sort::Strict(level))))
            }
            _ => Err(self.refuse(format!(
                "`{name}` needs its arguments; it cannot appear unapplied"
            ))),
        }
    }

    /// The next generalized universe parameter — appended after the
    /// authored `core::Level` binders in elaboration order, so the
    /// declaration's published `level_arity` is deterministic.
    pub(super) fn generalized_level(&mut self) -> Level {
        let parameter = (self.level_binders.len() + self.generalized_levels as usize) as u32;
        self.generalized_levels += 1;
        Level::Parameter(parameter)
    }

    /// The level-binder position of `symbol`/`name`, if it names one of
    /// this declaration's `core::Level` binders.
    pub(super) fn level_index(&self, symbol: SymbolHandle, name: &Identifier) -> Option<u32> {
        if symbol.is_valid() {
            return self
                .level_binders
                .iter()
                .position(|binder| *binder == symbol)
                .map(|index| index as u32);
        }
        self.level_names
            .iter()
            .position(|binder| binder == name)
            .map(|index| index as u32)
    }
}
