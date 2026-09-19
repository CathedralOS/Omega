//! Explicit declaration applications split the authored generic telescope into
//! universe arguments and term arguments. The existing kernel checks both the
//! instantiated constant and the resulting dependent application spine.

use super::{
    CarrierHead, Elaborator, Identifier, Level, SymbolHandle, Term, TermHandle, TypeParameterKind,
};
use diagnostics::Diagnostic;
use typed_trees::expression::StaticMachineArgument;

impl Elaborator<'_> {
    pub(super) fn elaborate_callee(
        &mut self,
        symbol: SymbolHandle,
        name: &Identifier,
        arguments: &[StaticMachineArgument],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        if !arguments.is_empty() {
            if let Some(&index) = self.authored_index.get(&symbol) {
                return self.instantiate_declaration(index, arguments);
            }
            return Err(self.refuse(format!(
                "mathematical callee `{name}` has no declaration telescope for static arguments"
            )));
        }
        if symbol.is_valid() {
            self.named_term(symbol, name)
        } else if let Some(term) = self.scope_term_for_name(name) {
            Ok(term)
        } else {
            Err(self.refuse(format!("unresolved mathematical callee `{name}`")))
        }
    }

    fn instantiate_declaration(
        &mut self,
        index: usize,
        arguments: &[StaticMachineArgument],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let definition = &self.program.mathematical_definitions()[index];
        let Some(&position) = self.authored_positions.get(&definition.symbol) else {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` references itself or a later declaration; the signature admits strictly earlier ones",
                definition.name
            )));
        };
        let binders = self
            .program
            .data_type_parameters
            .span_or_empty(definition.binders);
        if arguments.len() != binders.len() {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` requires {} explicit generic arguments, got {}",
                definition.name,
                binders.len(),
                arguments.len()
            )));
        }
        let mut levels = Vec::new();
        let mut terms = Vec::new();
        for (binder, argument) in binders.iter().zip(arguments) {
            let is_level = match binder.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    matches!(self.carrier_head(type_reference), CarrierHead::Level)
                }
                _ => false,
            };
            if is_level {
                levels.push(self.static_level(argument)?);
            } else {
                terms.push(self.static_term(argument)?);
            }
        }
        let level_arity = self.declarations[position as usize].level_arity;
        if levels.len() != level_arity as usize {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` needs {level_arity} level arguments; implicit generalized level instantiation is not supported",
                definition.name
            )));
        }
        let mut function = self.arena.insert(Term::Constant {
            declaration: position,
            levels,
        });
        for argument in terms {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    fn static_level(&mut self, argument: &StaticMachineArgument) -> Result<Level, Vec<Diagnostic>> {
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return Err(self.refuse(
                "a universe-level argument must name a `core::Level` binder or a literal"
                    .to_owned(),
            ));
        }
        if argument.type_reference.is_valid() {
            return self.elaborate_level(argument.type_reference);
        }
        if let Some(literal) = &argument.const_literal {
            return literal
                .value_u64()
                .and_then(|value| u32::try_from(value).ok())
                .map(Level::Constant)
                .ok_or_else(|| {
                    self.refuse(format!(
                        "universe level literal `{}` exceeds the kernel's level range",
                        literal.text()
                    ))
                });
        }
        if let [name] = argument.path.as_ref()
            && let Some(index) = self.level_index(argument.symbol, name)
        {
            return Ok(Level::Parameter(index));
        }
        Err(self.refuse(
            "a universe-level argument must name a `core::Level` binder or a literal".to_owned(),
        ))
    }

    fn static_term(
        &mut self,
        argument: &StaticMachineArgument,
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        if argument.application.is_some()
            || argument.evidence_projection.is_some()
            || argument.const_literal.is_some()
        {
            return Err(self.refuse(
                "a mathematical generic term argument has no kernel denotation yet".to_owned(),
            ));
        }
        if argument.type_reference.is_valid() {
            return self.elaborate_type_reference(argument.type_reference);
        }
        if argument.symbol.is_valid() {
            let name = argument.path.last().cloned().unwrap_or_default();
            return self.named_term(argument.symbol, &name);
        }
        if let [name] = argument.path.as_ref()
            && let Some(term) = self.scope_term_for_name(name)
        {
            return Ok(term);
        }
        Err(self.refuse("unresolved mathematical generic term argument".to_owned()))
    }
}
