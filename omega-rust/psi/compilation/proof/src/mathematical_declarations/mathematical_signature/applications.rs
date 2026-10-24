//! Explicit declaration applications split the authored generic telescope into
//! universe arguments and term arguments. The existing kernel checks both the
//! instantiated constant and the resulting dependent application spine.

use super::{
    Budget, CarrierHead, Context, DEFAULT_CONVERSION_STEPS, Elaborator, Identifier, Level,
    LevelArgument, Signature, SymbolHandle, Term, TermHandle, TypeParameter, TypeParameterKind,
    TypeReferenceNode, infer_sort,
};
use diagnostics::Diagnostic;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

impl Elaborator<'_> {
    /// Form applications demanded outside mathematical declaration bodies.
    /// The same Pi spine, prefix application and capture-avoiding kernel
    /// substitution judge these uses. Exact source carriers additionally keep
    /// fixed integer widths and arithmetic policies distinct before embedding.
    pub(super) fn source_applications(
        &mut self,
    ) -> Result<
        Vec<(
            ExpressionHandle,
            proof_admission::TermHandle,
            Vec<proof_admission::TermHandle>,
            Vec<super::PreparedCapture>,
        )>,
        Vec<Diagnostic>,
    > {
        let owned = validation::mathematical_applications::expression_nodes(self.program);
        let calls = self
            .program
            .expression_table
            .iter_expressions()
            .filter_map(|(expression, node)| {
                let ExpressionNode::Call(call) = node else {
                    return None;
                };
                (self.authored_index.contains_key(&call.target_symbol)
                    && !owned.contains(&expression))
                .then_some((expression, call.clone()))
            })
            .collect::<Vec<_>>();
        let owners = validation::expression_owners::OwnerEnvironmentIndex::new(
            self.program,
            &arena::Arena::default(),
        );
        let mut applications = Vec::with_capacity(calls.len());
        for (expression, call) in calls {
            self.span = Some(self.program.expression_table.source_span(expression));
            let retained_owner =
                typed_trees::references::StaticArgumentCall::Expression(expression, &call)
                    .lexical_owner(self.program);
            self.source_owner = if retained_owner.is_valid() {
                retained_owner
            } else {
                validation::expression_owners::source_declaration_owner(
                    self.program,
                    expression,
                    &owners,
                )
                .unwrap_or_default()
            };
            self.scope.clear();
            self.level_binders.clear();
            self.level_names.clear();
            self.generalized_levels = 0;
            self.captured_expressions.clear();
            self.source_family_values.clear();
            let mut captures = Vec::new();
            let mut pending = self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec();
            let mut seen = Vec::new();
            while let Some(operand) = pending.pop() {
                if !self.program.expression_table.expression_is_valid(operand)
                    || seen.contains(&operand)
                {
                    continue;
                }
                seen.push(operand);
                let node = self.program.expression_table.expression(operand);
                let mathematical = matches!(node, ExpressionNode::Call(call) if self.authored_index.contains_key(&call.target_symbol));
                let declaration_description = matches!(node, ExpressionNode::Name(path)
                    if self.program.symbols.builtin_type_member(path.symbol).is_some()
                        || self.program.symbols.builtin_type_atom(path.symbol).is_some());
                if !mathematical
                    && !declaration_description
                    && let Some(reference) =
                        validation::mathematical_applications::operand_type_reference(
                            self.program,
                            operand,
                        )
                {
                    // Capture the complete evaluated value. Splitting a fixed
                    // Wrapping computation into source leaves would silently
                    // replace its denotation with unbounded arithmetic.
                    let domain = self.elaborate_source_operand_type(reference)?;
                    if let TypeReferenceNode::Generic { base_symbol, .. } =
                        self.program.type_reference_table.type_reference(reference)
                        && self.indexed_families.contains_key(base_symbol)
                    {
                        let value = self.source_family_constructor(operand, reference, 0)?;
                        self.source_family_values.insert(operand, value);
                        continue;
                    }
                    let position = self.scope.len();
                    self.push_scope(None, Identifier::default(), domain);
                    self.captured_expressions.push((operand, position));
                    captures.push(super::PreparedCapture {
                        expression: operand,
                        source_type: reference,
                    });
                } else {
                    validation::expression_traversal::children(self.program, node, |child| {
                        pending.push(child)
                    });
                }
            }
            let term = self.elaborate_expression(expression)?;
            applications.push((
                expression,
                term,
                self.scope.iter().map(|entry| entry.domain).collect(),
                captures,
            ));
        }
        self.scope.clear();
        self.captured_expressions.clear();
        self.source_family_values.clear();
        Ok(applications)
    }

    pub(super) fn elaborate_callee(
        &mut self,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
        name: &Identifier,
        arguments: &[StaticMachineArgument],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        if !arguments.is_empty() {
            if let Some(&index) = self.authored_index.get(&symbol) {
                return self.instantiate_declaration(expression, index, arguments);
            }
            return Err(self.refuse(format!(
                "mathematical callee `{name}` has no declaration telescope for static arguments"
            )));
        }
        let term = if symbol.is_valid() {
            self.named_term(symbol, name)?
        } else if let Some(term) = self.scope_term_for_name(name) {
            term
        } else {
            return Err(self.refuse(format!("unresolved mathematical callee `{name}`")));
        };
        Ok(term)
    }

    fn instantiate_declaration(
        &mut self,
        expression: ExpressionHandle,
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
        let plan = validation::mathematical_applications::source_generic_argument_plan(
            self.program,
            binders,
            arguments,
            &self.fixed_core,
        );
        let Some(plan) = plan else {
            let levels = binders
                .iter()
                .filter(|binder| self.binder_is_level(binder))
                .count();
            return Err(self.refuse(format!(
                "mathematical declaration `{}` requires {} explicit generic arguments or {} with universe arguments omitted, got {}",
                definition.name, binders.len(), binders.len() - levels, arguments.len()
            )));
        };
        let level_binders = plan
            .iter()
            .filter_map(|slot| {
                matches!(
                    slot.kind,
                    validation::mathematical_applications::MathematicalBinderKind::Level
                )
                .then_some(slot.binder)
            })
            .collect::<Vec<_>>();
        let omit_levels = plan.iter().any(|slot| slot.argument.is_none());
        let mut authored_levels = vec![None; level_binders.len()];
        let mut terms = Vec::new();
        let mut term_domains = Vec::new();
        let mut generalized = Vec::new();
        let mut constraints = Vec::new();
        let mut level_position = 0;
        for slot in &plan {
            let binder = slot.binder;
            if self.binder_is_level(binder) {
                if !omit_levels {
                    authored_levels[level_position] =
                        Some(self.static_level(slot.argument.expect("checked generic arity"))?);
                }
                level_position += 1;
                continue;
            }
            let argument = slot.argument.expect("checked generic arity");
            let (term, domain) = if let Some(literal) = &argument.const_literal {
                self.static_integer_literal(binder, argument, literal)?
            } else {
                (self.static_term(argument)?, None)
            };
            if self.binder_claims_generalized_level(binder) {
                generalized.push((term, binder.name.clone()));
            } else if omit_levels
                && let Some(symbol) = self.binder_authored_level(binder)
                && let Some(position) = level_binders
                    .iter()
                    .position(|level| level.symbol == symbol)
            {
                constraints.push((position, term, &binder.name));
            }
            terms.push(term);
            term_domains.push(domain);
        }
        // Elaborate every supplied term before snapshotting the caller context:
        // a later type argument can intern a carrier in the signature prefix.
        let context = (!constraints.is_empty() || !generalized.is_empty())
            .then(|| self.instantiation_context());
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        if let Some(context) = &context {
            for (position, term, name) in constraints {
                if authored_levels[position].is_none() {
                    // One exact constraint selects the instantiation. Retaining
                    // every application lets the kernel check the rest by
                    // conversion, without syntactic equality or cumulativity.
                    authored_levels[position] =
                        Some(self.infer_argument_level(term, name, context, &mut budget)?);
                }
            }
        }
        let mut levels = Vec::with_capacity(authored_levels.len() + generalized.len());
        for (binder, level) in level_binders.iter().zip(authored_levels) {
            let Some(level) = level else {
                return Err(self.refuse(format!(
                    "cannot infer the universe argument of mathematical binder `{}`; supply explicit level arguments",
                    binder.name
                )));
            };
            levels.push(level);
        }
        if let Some(context) = &context {
            for (term, name) in generalized {
                levels.push(self.infer_argument_level(term, &name, context, &mut budget)?);
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
        let callee = self.arena.insert(Term::Constant {
            declaration: position,
            levels,
        });
        let inputs = terms
            .into_iter()
            .zip(term_domains)
            .map(|(term, domain)| super::source_domains::SourceInput {
                // Generic terms are already typed in the exact caller scope.
                // No anonymous ordinary-expression landing is available here.
                expression: ExpressionHandle::default(),
                term,
                domain,
            })
            .collect();
        Ok(self.apply_source_inputs(expression, callee, inputs))
    }

    fn binder_is_level(&self, binder: &TypeParameter) -> bool {
        match binder.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                matches!(self.carrier_head(type_reference), CarrierHead::Level)
            }
            _ => false,
        }
    }

    /// Only a supplied type at the callee's exact authored level constrains
    /// that level. Caller names and positions cannot identify callee binders.
    fn binder_authored_level(&self, binder: &TypeParameter) -> Option<SymbolHandle> {
        let type_reference = match binder.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => type_reference,
            _ => return None,
        };
        let CarrierHead::Type(LevelArgument::Authored(reference)) =
            self.carrier_head(type_reference)
        else {
            return None;
        };
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => Some(*symbol),
            _ => None,
        }
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
                        literal
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
            if let Some(reference) = self
                .program
                .type_reference_table
                .find_named_type_reference(argument.symbol)
            {
                return self.elaborate_type_reference(reference);
            }
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

    fn static_integer_literal(
        &mut self,
        binder: &TypeParameter,
        argument: &StaticMachineArgument,
        literal: &numerics::literals::IntegerLiteral,
    ) -> Result<(TermHandle, Option<TermHandle>), Vec<Diagnostic>> {
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return Err(self.refuse(
                "a generic integer literal cannot discard application or evidence payloads".into(),
            ));
        }
        let (TypeParameterKind::Value { type_reference }
        | TypeParameterKind::Const { type_reference }) = binder.kind
        else {
            return Err(self.refuse("an integer literal requires a declared value carrier".into()));
        };
        let primitive = self
            .program
            .primitive_type_reference(type_reference)
            .filter(|primitive| primitive.is_fixed_integer())
            .ok_or_else(|| {
                self.refuse(
                    "this mathematical generic literal carrier has no supported integer landing"
                        .into(),
                )
            })?;
        let expected = numerics::literals::LandedIntegerType::from_name(primitive.name())
            .ok_or_else(|| {
                self.refuse("mathematical generic literal has no fixed integer carrier".into())
            })?;
        if let Some(landing) = literal.landing()
            && (landing.landed_type != expected
                || landing.domain
                    != self
                        .program
                        .arithmetic_domain_for_type_reference(type_reference))
        {
            return Err(self.refuse(
                "mathematical generic literal has a different landed carrier or arithmetic policy"
                    .into(),
            ));
        }
        if argument.type_reference.is_valid()
            && !self
                .program
                .static_const_literal_from_type_reference(argument.type_reference)
                .is_some_and(|retained| {
                    retained.value_bignum() == literal.value_bignum()
                        && retained.landing() == literal.landing()
                })
        {
            return Err(self.refuse(
                "mathematical generic literal changed its retained source operand".into(),
            ));
        }
        let value = literal.value_bignum().ok_or_else(|| {
            self.refuse("mathematical generic literal has no exact integer value".into())
        })?;
        let (minimum, maximum) = expected.bounds();
        if value < numerics::bignum::BigInt::from_i128(minimum)
            || value > numerics::bignum::BigInt::from_i128(maximum)
        {
            return Err(self.refuse(format!(
                "mathematical generic literal `{}` does not fit `{}`",
                literal,
                primitive.name()
            )));
        }
        let domain = self.elaborate_type_reference(type_reference)?;
        // The anonymous literal acquires this carrier only after its exact
        // source landing succeeds. Named/previously typed values still travel
        // through their own domain; they cannot borrow the receiver's carrier.
        let term = self.literal_term(literal)?;
        Ok((term, Some(domain)))
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

    /// The universe argument a type binder's parameter takes: the
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
                "cannot infer the universe argument of mathematical binder `{binder_name}`: {error:?}"
            ))),
        }
    }
}
