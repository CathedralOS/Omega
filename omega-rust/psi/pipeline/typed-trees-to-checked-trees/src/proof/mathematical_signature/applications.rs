//! Explicit declaration applications split the authored generic telescope into
//! universe arguments and term arguments. The existing kernel checks both the
//! instantiated constant and the resulting dependent application spine.

use super::{
    Budget, CarrierHead, Context, DEFAULT_CONVERSION_STEPS, Elaborator, Identifier, Level,
    LevelArgument, Signature, SymbolHandle, Term, TermHandle, TypeParameter, TypeParameterKind,
    infer_sort,
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
        let mut generalized = Vec::new();
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
                continue;
            }
            let term = self.static_term(argument)?;
            if self.binder_claims_generalized_level(binder) {
                generalized.push((term, binder.name.clone()));
            }
            terms.push(term);
        }
        if !generalized.is_empty() {
            let context = self.instantiation_context();
            let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
            for (term, name) in generalized {
                levels.push(self.infer_argument_level(term, &name, &context, &mut budget)?);
            }
        }
        let level_arity = self.declarations[position as usize].level_arity;
        if levels.len() != level_arity as usize {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` resolved {} level arguments for an arity of {level_arity}",
                definition.name,
                levels.len()
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

    /// Whether this binder claimed a generalized universe parameter during
    /// elaboration — bare `T` and unapplied `core::Type` carriers. Those
    /// parameters publish after the authored `core::Level` binders in claim
    /// order, so inferred levels append after the authored ones in walk
    /// order here.
    fn binder_claims_generalized_level(&self, binder: &TypeParameter) -> bool {
        match &binder.kind {
            TypeParameterKind::Type => true,
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => matches!(
                self.carrier_head(*type_reference),
                CarrierHead::Type(LevelArgument::Generalized)
            ),
            _ => false,
        }
    }

    /// The caller's judgment scope at the application site: its own level
    /// arity, the signature prefix elaborated so far, and the in-flight
    /// telescope scope the argument terms were built under.
    fn instantiation_context(&self) -> Context {
        let mut context =
            Context::with_level_arity(self.level_binders.len() as u32 + self.generalized_levels)
                .with_signature(Signature::from_declarations(self.declarations.clone()));
        for entry in &self.scope {
            context = context.extend(entry.domain);
        }
        context
    }

    /// The universe argument a generalized binder's parameter takes: the
    /// supplied type argument's own inferred sort. `infer_sort` computes
    /// the level at which the argument is a type, which is exactly the
    /// level the binder's `Sort::Type` domain instantiates to — and the
    /// kernel re-decides the instantiated constant, so a non-type argument
    /// or a wrong layer still fails there.
    fn infer_argument_level(
        &mut self,
        argument: TermHandle,
        binder_name: &Identifier,
        context: &Context,
        budget: &mut Budget,
    ) -> Result<Level, Vec<Diagnostic>> {
        match infer_sort(&mut self.arena, context, argument, budget) {
            Ok(sort) => Ok(sort.level()),
            Err(error) => Err(self.refuse(format!(
                "cannot infer the generalized universe argument of mathematical binder `{binder_name}`: {error:?}"
            ))),
        }
    }
}
