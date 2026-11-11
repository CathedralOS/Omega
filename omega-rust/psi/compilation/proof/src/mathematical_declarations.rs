/*
    Elaboration of authored mathematical declarations, inductive families,
    propositions, and source evidence into kernel-checked signatures.

    We elaborate source mathematical definitions, W-based inductive families,
    universe levels, and theorem evidence into `CheckedMathematics` and verify
    the resulting signature once in the `proof-admission` mathematical core.
*/

mod declaration_records {
    use crate::typed_expressions::{expression_table, type_reference_table};
    use checked_trees::{
        CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
        CheckedMathematicalDeclaration, CheckedMathematicalParameter,
    };
    use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
    use typed_trees::mathematical::{
        MathematicalBody, MathematicalDefinition, MathematicalType, MathematicalTypeHandle,
    };
    use typed_trees::proposition::ProofSubstitutions;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    use super::{FixedCore, FixedCoreVocabulary};

    pub(crate) fn build_checked_mathematical_declarations(
        program: &dyn ::validation::CheckingInput,
    ) -> Result<Vec<CheckedMathematicalDeclaration>, Vec<diagnostics::Diagnostic>> {
        let vocabulary = FixedCoreVocabulary::resolve(&program.symbols);
        program
            .mathematical_definitions()
            .iter()
            .map(|definition| elaborate_declaration(program, &vocabulary, definition))
            .collect()
    }

    fn elaborate_declaration(
        program: &dyn ::validation::CheckingInput,
        vocabulary: &FixedCoreVocabulary,
        definition: &MathematicalDefinition,
    ) -> Result<CheckedMathematicalDeclaration, Vec<diagnostics::Diagnostic>> {
        let binders = program
            .data_type_parameters
            .span_or_empty(definition.binders)
            .iter()
            .map(|binder| elaborate_binder(program, vocabulary, definition, binder))
            .collect::<Result<_, _>>()?;
        let parameters = program
            .mathematical_parameters(definition.parameters)
            .iter()
            .map(|parameter| {
                Ok(CheckedMathematicalParameter {
                    name: parameter.name.to_string(),
                    relevance: parameter.relevance,
                    type_spelling: mathematical_type_identity(program, parameter.ty)?,
                })
            })
            .collect::<Result<_, Vec<diagnostics::Diagnostic>>>()?;
        let result = mathematical_type_identity(program, definition.result)?;
        let body = match definition.body {
            MathematicalBody::Assumption => CheckedMathematicalBody::Assumption,
            MathematicalBody::Definition(term) => CheckedMathematicalBody::Definition {
                term_spelling: program.render_proof_expression(term, ProofSubstitutions::None),
            },
        };
        Ok(CheckedMathematicalDeclaration {
            symbol: definition.symbol,
            name: definition.name.to_string(),
            is_public: definition.is_public,
            binders,
            parameters,
            result,
            body,
        })
    }

    fn elaborate_binder(
        program: &dyn ::validation::CheckingInput,
        vocabulary: &FixedCoreVocabulary,
        definition: &MathematicalDefinition,
        binder: &TypeParameter,
    ) -> Result<CheckedMathematicalBinder, Vec<diagnostics::Diagnostic>> {
        if binder.bounds != DataProperties::default() {
            return Err(refuse(
                program,
                definition,
                format!(
                    "mathematical binder `{}` cannot carry property bounds",
                    binder.name
                ),
            ));
        }
        let kind = match &binder.kind {
            TypeParameterKind::Type => CheckedMathematicalBinderKind::Type { universe: None },
            TypeParameterKind::Const { type_reference } => CheckedMathematicalBinderKind::Const {
                type_identity: ordinary_type_identity(program, definition, *type_reference)?,
            },
            TypeParameterKind::Value { type_reference } => {
                classify_subject_carrier(program, vocabulary, definition, binder, *type_reference)?
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {
                return Err(refuse(
                    program,
                    definition,
                    format!(
                        "mathematical binder `{}` cannot carry a machine or proposition contract",
                        binder.name
                    ),
                ));
            }
            TypeParameterKind::Implementation => {
                return Err(refuse(
                    program,
                    definition,
                    format!(
                        "mathematical binder `{}` cannot select an implementation; implementation slots belong to trait telescopes",
                        binder.name
                    ),
                ));
            }
        };
        Ok(CheckedMathematicalBinder {
            name: binder.name.to_string(),
            kind,
        })
    }

    fn classify_subject_carrier(
        program: &dyn ::validation::CheckingInput,
        vocabulary: &FixedCoreVocabulary,
        definition: &MathematicalDefinition,
        binder: &TypeParameter,
        carrier: TypeReferenceHandle,
    ) -> Result<CheckedMathematicalBinderKind, Vec<diagnostics::Diagnostic>> {
        if !type_reference_table(program).contains_type_reference(carrier) {
            return Err(refuse(
                program,
                definition,
                format!("mathematical binder `{}` has no carrier type", binder.name),
            ));
        }
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Named { symbol, name } => match vocabulary.classify(*symbol, name) {
                Some(FixedCore::Level) => Ok(CheckedMathematicalBinderKind::Level),
                Some(FixedCore::Type) => Ok(CheckedMathematicalBinderKind::Type { universe: None }),
                _ => Ok(CheckedMathematicalBinderKind::Value {
                    type_identity: ordinary_type_identity(program, definition, carrier)?,
                }),
            },
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => match vocabulary.classify(*base_symbol, base_name) {
                Some(FixedCore::Type) => {
                    let arguments =
                        type_reference_table(program).type_reference_handles(*arguments);
                    match arguments {
                        [universe] => Ok(CheckedMathematicalBinderKind::Type {
                            universe: Some(program.normalized_type_identity_text(*universe)),
                        }),
                        _ => Err(refuse(
                            program,
                            definition,
                            format!(
                                "mathematical binder `{}` applies `core::Type` to {} arguments; it takes one level",
                                binder.name,
                                arguments.len()
                            ),
                        )),
                    }
                }
                Some(FixedCore::Level) => Err(refuse(
                    program,
                    definition,
                    format!(
                        "mathematical binder `{}` applies arguments to `core::Level`; it takes none",
                        binder.name
                    ),
                )),
                _ => Ok(CheckedMathematicalBinderKind::Value {
                    type_identity: ordinary_type_identity(program, definition, carrier)?,
                }),
            },
            _ => Ok(CheckedMathematicalBinderKind::Value {
                type_identity: ordinary_type_identity(program, definition, carrier)?,
            }),
        }
    }

    fn mathematical_type_identity(
        program: &dyn ::validation::CheckingInput,
        handle: MathematicalTypeHandle,
    ) -> Result<String, Vec<diagnostics::Diagnostic>> {
        match program.mathematical_type(handle) {
            MathematicalType::Ordinary(reference) => {
                Ok(program.normalized_type_identity_text(*reference))
            }
            MathematicalType::Arrow {
                binder,
                domain,
                codomain,
            } => {
                let domain_is_arrow = matches!(
                    program.mathematical_type(*domain),
                    MathematicalType::Arrow { .. }
                );
                let domain = mathematical_type_identity(program, *domain)?;
                let domain = if domain_is_arrow {
                    format!("({domain})")
                } else {
                    domain
                };
                let codomain = mathematical_type_identity(program, *codomain)?;
                Ok(match binder {
                    Some(binder) => format!("({binder}: {domain}) -> {codomain}"),
                    None => format!("{domain} -> {codomain}"),
                })
            }
            MathematicalType::Application { callee, arguments } => {
                let callee_identity = mathematical_type_identity(program, *callee)?;
                let callee_identity = match program.mathematical_type(*callee) {
                    MathematicalType::Arrow { .. } => format!("({callee_identity})"),
                    _ => callee_identity,
                };
                let arguments = expression_table(program)
                    .expression_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        program.render_proof_expression(*argument, ProofSubstitutions::None)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("{callee_identity}({arguments})"))
            }
        }
    }

    fn ordinary_type_identity(
        program: &dyn ::validation::CheckingInput,
        definition: &MathematicalDefinition,
        reference: TypeReferenceHandle,
    ) -> Result<typed_trees::type_identity::TypeIdentityId, Vec<diagnostics::Diagnostic>> {
        if !type_reference_table(program).contains_type_reference(reference) {
            return Err(refuse(
                program,
                definition,
                format!(
                    "mathematical declaration `{}` references a type that does not exist",
                    definition.name
                ),
            ));
        }
        let carrier = validation::peel_arithmetic_policy(program, reference);
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::DynamicTrait { .. } => Err(refuse(
                program,
                definition,
                format!(
                    "mathematical type `{}` cannot be a borrow, constraint or dynamic trait",
                    program.display_type_reference(reference)
                ),
            )),
            _ => Ok(program.normalized_type_identity(reference)),
        }
    }

    fn refuse(
        program: &dyn ::validation::CheckingInput,
        definition: &MathematicalDefinition,
        message: String,
    ) -> Vec<diagnostics::Diagnostic> {
        let mut diagnostic = diagnostics::Diagnostic::error(message);
        if let Some(span) = program.symbols.symbol_source_span(definition.symbol) {
            diagnostic = diagnostic.with_source_span(span);
        }
        vec![diagnostic]
    }
}
mod fixed_core_vocabulary {
    pub(crate) use validation::mathematical_applications::{FixedCore, FixedCoreVocabulary};
}
mod mathematical_signature {
    macro_rules! interned_literal {
        ($self:ident . $map:ident, $key:expr, $carrier:expr) => {{
            let key = $key;
            let position = match $self.$map.get(&key) {
                Some(&position) => position,
                None => {
                    let ty = $self.constant($carrier);
                    let position = $self.push_assumption(ty);
                    $self.$map.insert(key, position);
                    position
                }
            };
            $self.constant(position)
        }};
    }

    mod applications {
        use super::{
            Budget, CarrierHead, Context, DEFAULT_CONVERSION_STEPS, Elaborator, Identifier, Level,
            LevelArgument, Signature, SymbolHandle, Term, TermHandle, TypeParameter,
            TypeParameterKind, TypeReferenceNode, infer_sort,
        };
        use crate::typed_expressions::{expression_table, type_reference_table};
        use diagnostics::Diagnostic;
        use numerics::bignum::BigInt;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use validation::expression_owners::{OwnerEnvironmentIndex, source_declaration_owner};
        use validation::expression_traversal::children;
        use validation::mathematical_applications::{
            MathematicalBinderKind, expression_nodes, source_generic_argument_plan,
        };

        pub(super) struct SourceApplication {
            pub(super) expression: ExpressionHandle,
            pub(super) owner: SymbolHandle,
            pub(super) term: TermHandle,
            pub(super) domains: Vec<TermHandle>,
            pub(super) level_arity: u32,
            pub(super) captures: Vec<super::PreparedCapture>,
            pub(super) emissions: std::ops::Range<usize>,
        }

        impl Elaborator<'_> {
            pub(super) fn source_applications(
                &mut self,
            ) -> Result<Vec<SourceApplication>, Vec<Diagnostic>> {
                let candidate_calls = expression_table(self.program)
                    .iter_expressions()
                    .filter_map(|(expression, node)| {
                        let ExpressionNode::Call(call) = node else {
                            return None;
                        };
                        (self.authored_index.contains_key(&call.target_symbol)
                            || validation::mathematical_applications::is_identity_call(
                                self.program,
                                call,
                            ))
                        .then_some((expression, call.clone()))
                    })
                    .collect::<Vec<_>>();
                if candidate_calls.is_empty() {
                    return Ok(Vec::new());
                }
                let owned = expression_nodes(self.program);
                let calls = candidate_calls
                    .into_iter()
                    .filter(|(expression, _)| !owned.contains(expression))
                    .collect::<Vec<_>>();
                if calls.is_empty() {
                    return Ok(Vec::new());
                }
                let mut applications = Vec::with_capacity(calls.len());
                for (expression, call) in calls {
                    self.span = Some(self.program.expression_table.source_span(expression));
                    let retained_owner =
                        typed_trees::references::StaticArgumentCall::Expression(expression, &call)
                            .lexical_owner(self.program);
                    self.source_owner = if retained_owner.is_valid() {
                        retained_owner
                    } else {
                        source_declaration_owner(
                            self.program,
                            expression,
                            self.source_owners.get_or_init(|| {
                                OwnerEnvironmentIndex::for_preparation(self.program)
                            }),
                        )
                        .unwrap_or_default()
                    };
                    self.scope.clear();
                    self.level_binders.clear();
                    self.level_names.clear();
                    self.generalized_levels = 0;
                    self.source_type_levels.clear();
                    self.captured_expressions.clear();
                    self.source_family_values.clear();
                    let mut captures = Vec::new();
                    let argument_types = self
                        .call_returns_type(&call)
                        .then(|| {
                            validation::mathematical_applications::argument_type_references(
                                self.program,
                                &call,
                            )
                        })
                        .flatten();
                    let mut pending = expression_table(self.program)
                        .expression_handles(call.arguments)
                        .iter()
                        .enumerate()
                        .map(|(index, expression)| {
                            (
                                *expression,
                                argument_types
                                    .as_ref()
                                    .and_then(|types| types.get(index).copied().flatten()),
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut seen = Vec::new();
                    while let Some((operand, expected)) = pending.pop() {
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
                        let reference = (!mathematical && !declaration_description)
                            .then(|| self.source_operand_type_reference(operand))
                            .flatten();
                        if let Some(reference) = reference {
                            // Capture the complete evaluated value. Splitting a fixed
                            // Wrapping computation into source leaves would silently
                            // replace its denotation with unbounded arithmetic.
                            let observation_type =
                                self.source_subject_observation_type(reference, expected);
                            let domain = self.elaborate_source_operand_type(observation_type)?;
                            // A relation may name an existing value without
                            // constructing or duplicating its runtime resource. A
                            // mathematical value actual still owes its constructor
                            // evidence; Id formation only borrows the typed subject.
                            if !self.call_returns_type(&call)
                                && let TypeReferenceNode::Generic { base_symbol, .. } =
                                    self.program.type_reference_table.type_reference(reference)
                                && self.indexed_families.contains_key(base_symbol)
                            {
                                let value =
                                    self.source_family_constructor(operand, reference, 0)?;
                                self.source_family_values.insert(operand, value);
                                continue;
                            }
                            let position = self.scope.len();
                            self.push_scope(None, Identifier::default(), domain);
                            self.captured_expressions.push((operand, position));
                            captures.push(super::PreparedCapture {
                                expression: operand,
                                source_type: reference,
                                observation_type,
                            });
                        } else {
                            children(self.program, node, |child| pending.push((child, None)));
                        }
                    }
                    let emission_start = self.identity_emissions.len();
                    let term = self.elaborate_expression(expression)?;
                    applications.push(SourceApplication {
                        expression,
                        owner: self.source_owner,
                        term,
                        domains: self.scope.iter().map(|entry| entry.domain).collect(),
                        level_arity: self.level_binders.len() as u32 + self.generalized_levels,
                        captures,
                        emissions: emission_start..self.identity_emissions.len(),
                    });
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
            ) -> Result<(TermHandle, Vec<super::source_domains::SourceInput>), Vec<Diagnostic>>
            {
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
                Ok((term, Vec::new()))
            }

            fn instantiate_declaration(
                &mut self,
                expression: ExpressionHandle,
                index: usize,
                arguments: &[StaticMachineArgument],
            ) -> Result<(TermHandle, Vec<super::source_domains::SourceInput>), Vec<Diagnostic>>
            {
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
                let plan = source_generic_argument_plan(
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
                        matches!(slot.kind, MathematicalBinderKind::Level).then_some(slot.binder)
                    })
                    .collect::<Vec<_>>();
                let omit_levels = plan.iter().any(|slot| slot.argument.is_none());
                let mut authored_levels = vec![None; level_binders.len()];
                let mut terms = Vec::new();
                let mut term_domains = Vec::new();
                let mut term_requirements = Vec::new();
                let mut generalized = Vec::new();
                let mut constraints = Vec::new();
                let mut level_position = 0;
                for slot in &plan {
                    let binder = slot.binder;
                    if self.binder_is_level(binder) {
                        if !omit_levels {
                            authored_levels[level_position] = Some(
                                self.static_level(slot.argument.expect("checked generic arity"))?,
                            );
                        }
                        level_position += 1;
                        continue;
                    }
                    let argument = slot.argument.expect("checked generic arity");
                    self.result_identity_requirements =
                        super::identity::IdentityRequirements::default();
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
                    if let Term::Variable(_) = self.arena.get(term) {
                        self.retain_identity_parameter_value(term)?;
                    }
                    term_requirements.push(std::mem::take(&mut self.result_identity_requirements));
                    terms.push(term);
                    term_domains.push(domain);
                }
                let context = (!constraints.is_empty() || !generalized.is_empty())
                    .then(|| self.instantiation_context());
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                if let Some(context) = &context {
                    for (position, term, name) in constraints {
                        if authored_levels[position].is_none() {
                            authored_levels[position] = Some(self.infer_argument_level(
                                term,
                                name,
                                context,
                                &mut budget,
                            )?);
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
                        levels.push(self.infer_argument_level(
                            term,
                            &name,
                            context,
                            &mut budget,
                        )?);
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
                let inputs: Vec<_> = terms
                    .into_iter()
                    .zip(term_domains)
                    .zip(term_requirements)
                    .map(|((term, domain), identity_requirements)| {
                        super::source_domains::SourceInput {
                            // Generic terms are already typed in the exact caller scope.
                            // No anonymous ordinary-expression landing is available here.
                            expression: ExpressionHandle::default(),
                            term,
                            domain,
                            identity_requirements,
                        }
                    })
                    .collect();
                let term = self.apply_source_inputs(expression, callee, inputs.clone());
                Ok((term, inputs))
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

            fn static_level(
                &mut self,
                argument: &StaticMachineArgument,
            ) -> Result<Level, Vec<Diagnostic>> {
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
                    "a universe-level argument must name a `core::Level` binder or a literal"
                        .to_owned(),
                ))
            }

            fn static_term(
                &mut self,
                argument: &StaticMachineArgument,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                if let Some(application) = argument.application.as_deref() {
                    if argument.type_reference.is_valid()
                        || argument.evidence_projection.is_some()
                        || argument.const_literal.is_some()
                        || application.member.is_valid()
                        || !application.lifetime_arguments.is_empty()
                    {
                        return Err(self.refuse(
                            "a mathematical type application carries conflicting operand or selection payloads"
                                .to_owned(),
                        ));
                    }
                    // The fixed vocabulary's unresolved fallback expects the full
                    // qualified spelling. A resolved symbol remains authoritative.
                    let name = Identifier::generated(
                        argument
                            .path
                            .iter()
                            .map(Identifier::as_str)
                            .collect::<Vec<_>>()
                            .join("::"),
                    );
                    let Some(sort) = self
                        .fixed_core
                        .classify(argument.symbol, &name)
                        .and_then(|member| member.universe_sort())
                    else {
                        return Err(self.refuse(
                            "a mathematical generic term application has no kernel denotation yet"
                                .to_owned(),
                        ));
                    };
                    let [level] = application.arguments.as_ref() else {
                        return Err(self.refuse(
                            "a mathematical universe application requires one level argument"
                                .to_owned(),
                        ));
                    };
                    let level = self.static_level(level)?;
                    return Ok(self.arena.insert(Term::Sort(sort(level))));
                }
                if argument.evidence_projection.is_some() || argument.const_literal.is_some() {
                    return Err(self.refuse(
                        "a mathematical generic term argument has no kernel denotation yet"
                            .to_owned(),
                    ));
                }
                if argument.type_reference.is_valid() {
                    return self.elaborate_type_reference(argument.type_reference);
                }
                if argument.symbol.is_valid() {
                    if let Some(reference) = type_reference_table(self.program)
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
                        "a generic integer literal cannot discard application or evidence payloads"
                            .into(),
                    ));
                }
                let (TypeParameterKind::Value { type_reference }
                | TypeParameterKind::Const { type_reference }) = binder.kind
                else {
                    return Err(
                        self.refuse("an integer literal requires a declared value carrier".into())
                    );
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
                if value < BigInt::from_i128(minimum) || value > BigInt::from_i128(maximum) {
                    return Err(self.refuse(format!(
                        "mathematical generic literal `{}` does not fit `{}`",
                        literal,
                        primitive.name()
                    )));
                }
                let domain = self.elaborate_type_reference(type_reference)?;
                let term = self.literal_term(literal)?;
                Ok((term, Some(domain)))
            }

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

            pub(super) fn instantiation_context(&self) -> Context {
                let mut context = Context::with_level_arity(
                    self.level_binders.len() as u32 + self.generalized_levels,
                )
                .with_signature(Signature::from_declarations(self.declarations.clone()));
                for entry in &self.scope {
                    context = context.extend(entry.domain);
                }
                context
            }

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
    }
    mod carriers {
        use crate::typed_expressions::expression_table;
        use numerics::bignum::BigInt;
        use numerics::literals::{FloatFormat, LandedIntegerType};

        use proof_admission::{Declaration, Level, Sort, Term, TermHandle};
        use symbols::{BuiltinTypeAtom, BuiltinTypeMember, SymbolHandle};
        use typed_trees::data::DataMember;
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableMemberExpression, TableNamePath, UnaryOperator,
        };
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        use super::{Elaborator, ScalarCarrier};

        impl<'a> Elaborator<'a> {
            pub(super) fn operand_carrier(
                &self,
                handle: ExpressionHandle,
            ) -> Option<ScalarCarrier> {
                match self.program.expression_table.expression(handle) {
                    ExpressionNode::Integer(literal) => {
                        literal.landing().map(|landing| match landing.landed_type {
                            LandedIntegerType::Addr => ScalarCarrier::Address,
                            _ => ScalarCarrier::Integer,
                        })
                    }
                    ExpressionNode::Boolean(_) => Some(ScalarCarrier::Boolean),
                    ExpressionNode::Float(literal) => {
                        literal.landing().map(|format| match format {
                            FloatFormat::F32 => ScalarCarrier::Float32,
                            FloatFormat::F64 => ScalarCarrier::Float64,
                        })
                    }
                    ExpressionNode::Name(path) => self.name_carrier(path),
                    ExpressionNode::Call(call) => self.declaration_result_carrier(
                        validation::call_target_symbol(self.program, call),
                    ),
                    ExpressionNode::Binary(binary) => match binary.operator {
                        op if op.is_arithmetic() => {
                            match (
                                self.operand_carrier(binary.left),
                                self.operand_carrier(binary.right),
                            ) {
                                (Some(left), Some(right)) if left == right => Some(left),
                                (Some(carrier), None) | (None, Some(carrier)) => Some(carrier),
                                (None, None)
                                    if !self.expression_has_float_leaf(handle)
                                        && self
                                            .program
                                            .closed_integer_expression_value(handle)
                                            .is_some() =>
                                {
                                    Some(ScalarCarrier::Integer)
                                }
                                _ => None,
                            }
                        }
                        _ => Some(ScalarCarrier::Boolean),
                    },
                    ExpressionNode::Unary(unary) => Some(match unary.operator {
                        UnaryOperator::LogicalNot => ScalarCarrier::Boolean,
                        UnaryOperator::BitwiseNot => ScalarCarrier::Integer,
                    }),
                    ExpressionNode::Member(member) => self.member_carrier(member),
                    ExpressionNode::Cast(cast) => {
                        self.scalar_carrier_of_type_reference(cast.target_type)
                    }
                    ExpressionNode::ZeroValue(reference) => {
                        self.scalar_carrier_of_type_reference(*reference)
                    }
                    ExpressionNode::StructLiteral(literal) => self
                        .carriers
                        .get(&literal.type_symbol)
                        .map(|&position| self.carrier_class_at(position)),
                    ExpressionNode::Match(matched) => {
                        let mut carrier = None;
                        for arm in expression_table(self.program)
                            .match_arms(matched.arms)
                            .iter()
                        {
                            let arm_carrier = self.operand_carrier(arm.value)?;
                            match carrier {
                                None => carrier = Some(arm_carrier),
                                Some(agreed) if agreed == arm_carrier => {}
                                Some(_) => return None,
                            }
                        }
                        carrier
                    }
                    _ => None,
                }
            }

            pub(super) fn expression_has_float_leaf(&self, handle: ExpressionHandle) -> bool {
                if !handle.is_valid() {
                    return false;
                }
                match self.program.expression_table.expression(handle) {
                    ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => true,
                    ExpressionNode::Binary(binary) => {
                        self.expression_has_float_leaf(binary.left)
                            || self.expression_has_float_leaf(binary.right)
                    }
                    ExpressionNode::Unary(unary) => self.expression_has_float_leaf(unary.operand),
                    _ => false,
                }
            }

            pub(super) fn member_carrier(
                &self,
                member: &TableMemberExpression,
            ) -> Option<ScalarCarrier> {
                if member.member_symbol.is_valid() {
                    let field_type = self
                        .program
                        .any_field_owner(member.member_symbol)
                        .map(|(_, field)| field.type_reference)?;
                    return self.scalar_carrier_of_type_reference(field_type);
                }
                let ScalarCarrier::Carrier(position) = self.operand_carrier(member.receiver)?
                else {
                    return None;
                };
                let symbol = self
                    .carriers
                    .iter()
                    .find(|(_, carrier_position)| **carrier_position == position)
                    .map(|(symbol, _)| *symbol)?;
                let field_type =
                    self.program
                        .data_definition_by_symbol(symbol)
                        .and_then(|data| {
                            self.program
                                .data_members(data)
                                .iter()
                                .find_map(|member_decl| match member_decl {
                                    DataMember::Field(field) if field.name == member.member => {
                                        Some(field.type_reference)
                                    }
                                    DataMember::Variant(variant) => self
                                        .program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|field| field.name == member.member)
                                        .map(|field| field.type_reference),
                                    _ => None,
                                })
                        })?;
                self.scalar_carrier_of_type_reference(field_type)
            }

            pub(super) fn name_carrier(&self, path: &TableNamePath) -> Option<ScalarCarrier> {
                if path.symbol.is_valid() {
                    if let Some(position) = self
                        .scope
                        .iter()
                        .rposition(|entry| entry.symbol == Some(path.symbol))
                    {
                        return self.domain_carrier(position);
                    }
                    return self.declaration_result_carrier(path.symbol);
                }
                let members = expression_table(self.program).name_path_members(path.members);
                let [name] = members else { return None };
                let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
                self.domain_carrier(position)
            }

            pub(super) fn domain_carrier(&self, scope_position: usize) -> Option<ScalarCarrier> {
                match self.arena.get(self.scope[scope_position].domain) {
                    Term::Constant { declaration, .. } => Some(self.carrier_class_at(declaration)),
                    _ => None,
                }
            }

            pub(super) fn declaration_result_carrier(
                &self,
                symbol: SymbolHandle,
            ) -> Option<ScalarCarrier> {
                if self.program.symbols.builtin_type_member(symbol)
                    == Some(BuiltinTypeMember::AddressBound)
                    || self.program.symbols.builtin_function_for_symbol(symbol)
                        == Some(symbols::BuiltinFunction::IntegerEmbed)
                {
                    return Some(ScalarCarrier::Integer);
                }
                if let Some(&position) = self.authored_positions.get(&symbol) {
                    let mut ty = self.declarations[position as usize].ty;
                    while let Term::Pi { codomain, .. } = self.arena.get(ty) {
                        ty = codomain;
                    }
                    return match self.arena.get(ty) {
                        Term::Constant { declaration, .. } => {
                            Some(self.carrier_class_at(declaration))
                        }
                        _ => None,
                    };
                }
                let result = validation::call_target_return_type(self.program, symbol)?;
                self.scalar_carrier_of_type_reference(result)
            }

            pub(super) fn scalar_carrier_of_type_reference(
                &self,
                reference: TypeReferenceHandle,
            ) -> Option<ScalarCarrier> {
                if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    return match primitive {
                        PrimitiveType::Bool => Some(ScalarCarrier::Boolean),
                        PrimitiveType::Addr => Some(ScalarCarrier::Address),
                        PrimitiveType::F32 => Some(ScalarCarrier::Float32),
                        PrimitiveType::F64 => Some(ScalarCarrier::Float64),
                        _ => Some(ScalarCarrier::Integer),
                    };
                }
                if matches!(
                    self.program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Unit
                ) {
                    return Some(ScalarCarrier::Unit);
                }
                if let TypeReferenceNode::Named { symbol, .. } =
                    self.program.type_reference_table.type_reference(reference)
                {
                    return match self.program.symbols.builtin_type_atom(*symbol) {
                        Some(BuiltinTypeAtom::Bool) => Some(ScalarCarrier::Boolean),
                        Some(BuiltinTypeAtom::Address) => Some(ScalarCarrier::Address),
                        Some(BuiltinTypeAtom::F32) => Some(ScalarCarrier::Float32),
                        Some(BuiltinTypeAtom::F64) => Some(ScalarCarrier::Float64),
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
                        ) => Some(ScalarCarrier::Integer),
                        _ => self
                            .carriers
                            .get(symbol)
                            .map(|&position| self.carrier_class_at(position)),
                    };
                }
                None
            }

            pub(super) fn carrier_class_at(&self, position: u32) -> ScalarCarrier {
                if let Some(alias) = self
                    .source_aliases
                    .iter()
                    .find(|alias| alias.declaration == position)
                {
                    return alias.carrier;
                }
                if self.integer == Some(position) {
                    ScalarCarrier::Integer
                } else if self.boolean == Some(position) {
                    ScalarCarrier::Boolean
                } else if self.address == Some(position) {
                    ScalarCarrier::Address
                } else if self.float32 == Some(position) {
                    ScalarCarrier::Float32
                } else if self.float64 == Some(position) {
                    ScalarCarrier::Float64
                } else if self.unit == Some(position) {
                    ScalarCarrier::Unit
                } else {
                    ScalarCarrier::Carrier(position)
                }
            }

            pub(super) fn carrier_name(&self, carrier: ScalarCarrier) -> &'static str {
                match carrier {
                    ScalarCarrier::Integer => "integer",
                    ScalarCarrier::Boolean => "boolean",
                    ScalarCarrier::Address => "address",
                    ScalarCarrier::Float32 => "f32",
                    ScalarCarrier::Float64 => "f64",
                    ScalarCarrier::Unit => "unit",
                    ScalarCarrier::Carrier(..) => "scalar",
                }
            }

            pub(super) fn carrier_position(&mut self, carrier: ScalarCarrier) -> u32 {
                match carrier {
                    ScalarCarrier::Integer => self.integer_carrier(),
                    ScalarCarrier::Boolean => self.boolean_carrier(),
                    ScalarCarrier::Address => self.address_carrier(),
                    ScalarCarrier::Float32 => self.float32_carrier(),
                    ScalarCarrier::Float64 => self.float64_carrier(),
                    ScalarCarrier::Unit => self.unit_carrier(),
                    ScalarCarrier::Carrier(position) => position,
                }
            }

            pub(super) fn carrier_term(&mut self, carrier: ScalarCarrier) -> TermHandle {
                let position = self.carrier_position(carrier);
                self.constant(position)
            }

            pub(super) fn integer_carrier(&mut self) -> u32 {
                if self.integer.is_none() {
                    self.integer = Some(self.push_type_carrier());
                }
                self.integer.unwrap()
            }

            pub(super) fn boolean_carrier(&mut self) -> u32 {
                if self.boolean.is_none() {
                    let ty = self.type_zero();
                    let body = self.arena.insert(Term::Two);
                    let position = self.declarations.len() as u32;
                    self.declarations.push(Declaration::definition(0, ty, body));
                    self.boolean = Some(position);
                }
                self.boolean.unwrap()
            }

            pub(super) fn address_carrier(&mut self) -> u32 {
                if self.address.is_none() {
                    self.address = Some(self.push_type_carrier());
                }
                self.address.unwrap()
            }

            pub(super) fn float32_carrier(&mut self) -> u32 {
                if self.float32.is_none() {
                    self.float32 = Some(self.push_type_carrier());
                }
                self.float32.unwrap()
            }

            pub(super) fn float64_carrier(&mut self) -> u32 {
                if self.float64.is_none() {
                    self.float64 = Some(self.push_type_carrier());
                }
                self.float64.unwrap()
            }

            pub(super) fn unit_carrier(&mut self) -> u32 {
                if self.unit.is_none() {
                    self.unit = Some(self.push_type_carrier());
                }
                self.unit.unwrap()
            }

            pub(super) fn integer_relation(&mut self) -> u32 {
                let int = self.integer_constant();
                let codomain = self.type_zero();
                let inner = self.arena.insert(Term::Pi {
                    domain: int,
                    codomain,
                });
                let ty = self.arena.insert(Term::Pi {
                    domain: int,
                    codomain: inner,
                });
                self.push_assumption(ty)
            }

            pub(super) fn integer_less_than(&mut self) -> u32 {
                if self.integer_less_than.is_none() {
                    self.integer_less_than = Some(self.integer_relation());
                }
                self.integer_less_than.unwrap()
            }

            pub(super) fn integer_less_or_equal(&mut self) -> u32 {
                if self.integer_less_or_equal.is_none() {
                    self.integer_less_or_equal = Some(self.integer_relation());
                }
                self.integer_less_or_equal.unwrap()
            }

            pub(super) fn proposition_negation(&mut self) -> u32 {
                if self.proposition_negation.is_none() {
                    let domain = self.type_zero();
                    let codomain = self.type_zero();
                    let ty = self.arena.insert(Term::Pi { domain, codomain });
                    self.proposition_negation = Some(self.push_assumption(ty));
                }
                self.proposition_negation.unwrap()
            }

            pub(super) fn integer_operation(&mut self) -> u32 {
                let int = self.integer_constant();
                let inner = self.arena.insert(Term::Pi {
                    domain: int,
                    codomain: int,
                });
                let ty = self.arena.insert(Term::Pi {
                    domain: int,
                    codomain: inner,
                });
                self.push_assumption(ty)
            }

            pub(super) fn integer_add(&mut self) -> u32 {
                if self.integer_add.is_none() {
                    self.integer_add = Some(self.integer_operation());
                }
                self.integer_add.unwrap()
            }

            pub(super) fn integer_subtract(&mut self) -> u32 {
                if self.integer_subtract.is_none() {
                    self.integer_subtract = Some(self.integer_operation());
                }
                self.integer_subtract.unwrap()
            }

            pub(super) fn constant(&mut self, position: u32) -> TermHandle {
                self.quantified_constant(position)
            }

            pub(super) fn type_zero(&mut self) -> TermHandle {
                self.arena
                    .insert(Term::Sort(Sort::Type(Level::Constant(0))))
            }

            pub(super) fn push_assumption(&mut self, ty: TermHandle) -> u32 {
                let position = self.declarations.len() as u32;
                self.declarations.push(Declaration::assumption(
                    proof_admission::universe_parameter_arity(&self.arena, &[ty]),
                    ty,
                ));
                position
            }

            pub(super) fn push_type_carrier(&mut self) -> u32 {
                let ty = self.type_zero();
                self.push_assumption(ty)
            }

            pub(super) fn integer_constant(&mut self) -> TermHandle {
                let position = self.integer_carrier();
                self.constant(position)
            }

            pub(super) fn numeric_literal(
                &mut self,
                carrier: u32,
                value: BigInt,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                if self.integer == Some(carrier)
                    && let Some(theory) = &mut self.integer_theory
                {
                    let term = theory
                        .literal(&mut self.arena, &mut self.declarations, value.clone())
                        .map_err(|error| {
                            self.refuse(format!(
                                "source integer literal could not be denoted: {error}"
                            ))
                        })?;
                    let Term::Constant { declaration, .. } = self.arena.get(term) else {
                        return Err(self.refuse(
                            "shared integer literal lost its canonical declaration".into(),
                        ));
                    };
                    self.numeric_literals.insert((carrier, value), declaration);
                    return Ok(term);
                }
                Ok(interned_literal!(
                    self.numeric_literals,
                    (carrier, value),
                    carrier
                ))
            }

            pub(super) fn boolean_literal(&mut self, value: bool) -> TermHandle {
                if let Some(position) = self.boolean_literals[value as usize] {
                    return self.constant(position);
                }
                let carrier = self.boolean_carrier();
                let ty = self.constant(carrier);
                let body = self
                    .arena
                    .insert(if value { Term::TwoOne } else { Term::TwoZero });
                let position = self.declarations.len() as u32;
                self.declarations.push(Declaration::definition(0, ty, body));
                self.boolean_literals[value as usize] = Some(position);
                self.constant(position)
            }
        }
    }
    mod content_values {
        /*
        The closed content algebras have mathematical values, rather than the
        start/end fields used by their source literal formers. Source types and
        captures use the same opaque carrier as canonical content derivations.
        */

        use super::Elaborator;
        use diagnostics::Diagnostic;
        use proof_admission::{
            Budget, DEFAULT_CONVERSION_STEPS, Sort, SourceContentTheory, TermHandle, infer_sort,
        };
        use typed_trees::types::TypeReferenceHandle;

        impl Elaborator<'_> {
            pub(super) fn elaborate_content_carrier(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
                let Some(algebra) =
                    validation::content_algebra_application(self.program, reference)
                else {
                    return Ok(None);
                };
                let parameter = self.elaborate_type_reference(algebra.parameter)?;
                let context = self.instantiation_context();
                let sort = infer_sort(
                    &mut self.arena,
                    &context,
                    parameter,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "canonical content parameter fails formation: {error:?}"
                    ))
                })?;
                let Sort::Type(level) = sort else {
                    return Err(self.refuse(
                        "canonical content requires a relevant coordinate or unit type".to_owned(),
                    ));
                };
                let theory = self.source_content.get_or_insert_with(|| {
                    SourceContentTheory::new(&mut self.arena, &mut self.declarations)
                });
                theory
                    .carrier(
                        &mut self.arena,
                        &mut self.declarations,
                        algebra.identity.canonical_algebra().kind,
                        parameter,
                        level,
                    )
                    .map(Some)
                    .map_err(|error| {
                        self.refuse(format!(
                            "canonical content carrier fails formation: {error}"
                        ))
                    })
            }
        }
    }
    mod dependencies {
        use proof_admission::{Term, TermArena, TermHandle, substitute};

        pub(super) fn free_positions(
            arena: &TermArena,
            term: TermHandle,
            scope: usize,
            positions: &mut Vec<usize>,
        ) {
            fn walk(
                arena: &TermArena,
                term: TermHandle,
                scope: usize,
                depth: u32,
                positions: &mut Vec<usize>,
            ) {
                macro_rules! children { ($($child:ident),+) => {{ $(walk(arena, $child, scope, depth, positions);)+ }}; }
                match arena.get(term) {
                    Term::Variable(index) if index >= depth => {
                        if let Some(position) = scope.checked_sub((index - depth) as usize + 1)
                            && !positions.contains(&position)
                        {
                            positions.push(position);
                        }
                    }
                    Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                        walk(arena, domain, scope, depth, positions);
                        walk(arena, codomain, scope, depth + 1, positions);
                    }
                    Term::Lambda { domain, body } => {
                        walk(arena, domain, scope, depth, positions);
                        walk(arena, body, scope, depth + 1, positions);
                    }
                    Term::Apply { function, argument } => children!(function, argument),
                    Term::Pair { first, second } => children!(first, second),
                    Term::Fst { pair } | Term::Snd { pair } => children!(pair),
                    Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    } => children!(motive, zero_branch, one_branch, scrutinee),
                    Term::Id { ty, left, right } => children!(ty, left, right),
                    Term::Refl { ty, value }
                    | Term::SquashIntro { ty, value }
                    | Term::BoxIntro { ty, value } => children!(ty, value),
                    Term::IdElim {
                        motive,
                        base,
                        endpoint,
                        proof,
                    } => children!(motive, base, endpoint, proof),
                    Term::W { carrier, children } => children!(carrier, children),
                    Term::Sup {
                        carrier,
                        children,
                        label,
                        function,
                    } => children!(carrier, children, label, function),
                    Term::IndW { motive, step, tree } => children!(motive, step, tree),
                    Term::EmptyElim { ty, scrutinee } => children!(ty, scrutinee),
                    Term::Squash { ty } | Term::Box { ty } => children!(ty),
                    Term::SquashElim {
                        proposition,
                        function,
                        scrutinee,
                    } => children!(proposition, function, scrutinee),
                    Term::BoxElim {
                        motive,
                        body,
                        scrutinee,
                    } => children!(motive, body, scrutinee),
                    _ => {}
                }
            }
            walk(arena, term, scope, 0, positions);
        }

        pub(super) fn rebind_domain(
            arena: &mut TermArena,
            domain: TermHandle,
            old_domains: &[TermHandle],
            retained: &[usize],
        ) -> TermHandle {
            let mut body = domain;
            for domain in old_domains.iter().rev() {
                body = arena.insert(Term::Lambda {
                    domain: *domain,
                    body,
                });
            }
            for position in 0..old_domains.len() {
                let argument = retained
                    .iter()
                    .position(|retained| *retained == position)
                    .map(|index| arena.insert(Term::Variable((retained.len() - 1 - index) as u32)))
                    .unwrap_or_else(|| arena.insert(Term::Dummy));
                let Term::Lambda { body: inner, .. } = arena.get(body) else {
                    unreachable!("constructed telescope")
                };
                body = substitute(arena, inner, argument);
            }
            body
        }
    }
    mod identity {
        /*
        Form the sealed relation using one transparent, universe-polymorphic Id.

        Conversion retains this checked declaration's meaning, so a mathematical
        wrapper returns the same typed identity value as a direct application.
        Neither formation nor the declaration body supplies a witness of the Id.
        */

        use super::{Elaborator, Term, TermHandle};
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use proof_admission::{
            Budget, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, infer_sort,
        };
        use symbols::SymbolHandle;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::expression::{ExpressionHandle, TableCallExpression};
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, applied_type_identity,
        };
        use typed_trees::types::TypeReferenceHandle;
        use validation::mathematical_applications::{
            MathematicalBinderKind, PreparedIdentityValue, source_generic_argument_plan,
        };

        impl Elaborator<'_> {
            pub(super) fn elaborate_identity(
                &mut self,
                expression: ExpressionHandle,
                call: &TableCallExpression,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let application =
                    validation::mathematical_applications::identity_application(self.program, call)
                        .map_err(|reason| self.refuse(reason.to_owned()))?;
                let ty = self.elaborate_source_operand_type(application.carrier)?;
                let context = self.instantiation_context();
                let sort = infer_sort(
                    &mut self.arena,
                    &context,
                    ty,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    self.refuse(format!("core::Identity carrier fails formation: {error:?}"))
                })?;
                let Sort::Type(level) = sort else {
                    return Err(
                        self.refuse("core::Identity requires a relevant Type carrier".to_owned())
                    );
                };
                let declaration = self.identity_declaration();
                let mut term = self.arena.insert(Term::Constant {
                    declaration,
                    levels: vec![level],
                });
                term = self.arena.insert(Term::Apply {
                    function: term,
                    argument: ty,
                });
                for subject in application.subjects {
                    let actual = self.source_expression_input(subject)?;
                    let value = actual.term;
                    self.retain_source_result(ty, actual);
                    term = self.arena.insert(Term::Apply {
                        function: term,
                        argument: value,
                    });
                }
                self.record_identity_emission(
                    term,
                    application.carrier,
                    self.source_owner,
                    Vec::new(),
                    Vec::new(),
                )?;
                self.result_identity_requirements = IdentityRequirements {
                    owner: self.source_owner,
                    emissions: vec![self.identity_emissions.len() - 1],
                    applications: Vec::new(),
                };
                self.span = Some(self.program.expression_table.source_span(expression));
                Ok(term)
            }

            fn identity_declaration(&mut self) -> u32 {
                if let Some(declaration) = self.identity_declaration {
                    return declaration;
                }
                let universe = self
                    .arena
                    .insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                let a = self.arena.insert(Term::Variable(0));
                let a_under_left = self.arena.insert(Term::Variable(1));
                let a_under_both = self.arena.insert(Term::Variable(2));
                let right = self.arena.insert(Term::Variable(0));
                let left = self.arena.insert(Term::Variable(1));
                let result = self.arena.insert(Term::Id {
                    ty: a_under_both,
                    left,
                    right,
                });
                let body = self.arena.insert(Term::Lambda {
                    domain: a_under_left,
                    body: result,
                });
                let body = self.arena.insert(Term::Lambda { domain: a, body });
                let body = self.arena.insert(Term::Lambda {
                    domain: universe,
                    body,
                });
                let ty = self.arena.insert(Term::Pi {
                    domain: a_under_left,
                    codomain: universe,
                });
                let ty = self.arena.insert(Term::Pi {
                    domain: a,
                    codomain: ty,
                });
                let ty = self.arena.insert(Term::Pi {
                    domain: universe,
                    codomain: ty,
                });
                let declaration = self.declarations.len() as u32;
                self.declarations.push(Declaration {
                    level_arity: 1,
                    ty,
                    body: Some(body),
                });
                self.identity_declaration = Some(declaration);
                declaration
            }

            pub(super) fn call_returns_type(&self, call: &TableCallExpression) -> bool {
                validation::mathematical_applications::is_identity_call(self.program, call)
                    || self
                        .type_returning_declarations
                        .contains(&call.target_symbol)
            }

            /// Every emission keeps its source type template and exact application
            /// scopes. The calculus value follows the same level and beta substitution
            /// as the authored declaration; no consumer reconstructs a source carrier
            /// from an ambiguous shared kernel type.
            fn record_identity_emission(
                &mut self,
                value: TermHandle,
                source_type: TypeReferenceHandle,
                source_owner: SymbolHandle,
                source_scopes: Vec<IdentitySourceScope>,
                pending: Vec<IdentitySourceParameter>,
            ) -> Result<(), Vec<Diagnostic>> {
                let mut abstraction = value;
                for entry in self.scope.iter().rev() {
                    abstraction = self.arena.insert(Term::Lambda {
                        domain: entry.domain,
                        body: abstraction,
                    });
                }
                let Some(requirements) = self
                    .identity_requirements_by_owner
                    .get_mut(self.source_owner.arena_index() as usize)
                else {
                    return Err(self.refuse("Id emission has no retained source owner".into()));
                };
                if requirements.owner != self.source_owner {
                    *requirements = IdentityRequirements {
                        owner: self.source_owner,
                        ..Default::default()
                    };
                }
                requirements.emissions.push(self.identity_emissions.len());
                self.identity_emissions.push(IdentityEmission {
                    value,
                    abstraction,
                    source_type,
                    source_owner,
                    source_scopes,
                    pending,
                });
                Ok(())
            }

            pub(super) fn transport_identity_requirements(
                &mut self,
                call: &TableCallExpression,
                callee: TermHandle,
                static_inputs: &[super::source_domains::SourceInput],
                inputs: &[super::source_domains::SourceInput],
            ) -> Result<(), Vec<Diagnostic>> {
                let actuals = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                self.transport_identity_value(
                    call.target_symbol,
                    &call.machine_arguments,
                    actuals,
                    callee,
                    static_inputs,
                    inputs,
                )
            }

            pub(super) fn transport_identity_value(
                &mut self,
                target: SymbolHandle,
                static_arguments: &[StaticMachineArgument],
                actuals: &[ExpressionHandle],
                callee: TermHandle,
                static_inputs: &[super::source_domains::SourceInput],
                inputs: &[super::source_domains::SourceInput],
            ) -> Result<(), Vec<Diagnostic>> {
                let emission_start = self.identity_emissions.len();
                let application_start = self.identity_applications.len();
                self.transport_identity_value_requirements(
                    target,
                    static_arguments,
                    actuals,
                    callee,
                    static_inputs,
                    inputs,
                )?;
                self.result_identity_requirements = IdentityRequirements {
                    owner: self.source_owner,
                    emissions: (emission_start..self.identity_emissions.len()).collect(),
                    applications: (application_start..self.identity_applications.len()).collect(),
                };
                Ok(())
            }

            fn transport_identity_value_requirements(
                &mut self,
                target: SymbolHandle,
                static_arguments: &[StaticMachineArgument],
                actuals: &[ExpressionHandle],
                mut callee: TermHandle,
                static_inputs: &[super::source_domains::SourceInput],
                inputs: &[super::source_domains::SourceInput],
            ) -> Result<(), Vec<Diagnostic>> {
                if !self.authored_index.contains_key(&target) {
                    if self.scope.iter().any(|entry| entry.symbol == Some(target)) {
                        let mut value = callee;
                        for input in inputs {
                            value = self.arena.insert(Term::Apply {
                                function: value,
                                argument: input.term,
                            });
                        }
                        self.record_identity_application(IdentityApplication {
                            parameter: target,
                            value,
                            abstraction: value,
                            arguments: inputs.iter().map(|input| input.expression).collect(),
                            source_scopes: Vec::new(),
                            pending: Vec::new(),
                        })?;
                    }
                    return Ok(());
                }
                let requirements = self
                    .identity_requirements_by_owner
                    .get(target.arena_index() as usize)
                    .filter(|requirements| requirements.owner == target)
                    .cloned()
                    .unwrap_or_default();
                if requirements.emissions.is_empty() && requirements.applications.is_empty() {
                    return Ok(());
                }
                let emitted = requirements
                    .emissions
                    .iter()
                    .map(|position| self.identity_emissions[*position].clone())
                    .collect::<Vec<_>>();
                let Some(&index) = self.authored_index.get(&target) else {
                    return Err(
                        self.refuse("Id value lost its mathematical declaration owner".into())
                    );
                };
                let definition = &self.program.mathematical_definitions()[index];
                let binders = self
                    .program
                    .data_type_parameters
                    .span_or_empty(definition.binders);
                let plan = source_generic_argument_plan(
                    self.program,
                    binders,
                    static_arguments,
                    &self.fixed_core,
                );
                let mut arguments = Vec::new();
                let mut static_formals = Vec::new();
                let mut pending_static = Vec::new();
                if let Some(plan) = plan {
                    for slot in plan {
                        if matches!(slot.kind, MathematicalBinderKind::Level) {
                            continue;
                        }
                        static_formals.push(slot.binder.symbol);
                        let actual = slot.argument.ok_or_else(|| {
                            self.refuse("Id application lost a supplied generic operand".into())
                        })?;
                        arguments.push((
                            slot.binder.symbol,
                            IdentitySourceArgument::Static(actual.clone()),
                        ));
                    }
                } else if static_arguments.is_empty()
                    && static_inputs.is_empty()
                    && actuals.is_empty()
                    && inputs.is_empty()
                {
                    // A bare declaration value retains its whole unapplied Pi prefix.
                    // The ordinary application operation supplies these actuals later.
                    for binder in binders {
                        static_formals.push(binder.symbol);
                        pending_static.push(IdentitySourceParameter {
                            declaration: definition.symbol,
                            binder: binder.symbol,
                        });
                    }
                } else {
                    return Err(self
                        .refuse("Id value lost its checked generic application telescope".into()));
                }
                let formals = self.program.mathematical_parameters(definition.parameters);
                arguments.extend(formals.iter().zip(actuals).map(|(formal, actual)| {
                    (formal.symbol, IdentitySourceArgument::Expression(*actual))
                }));
                let scope = IdentitySourceScope {
                    declaration: definition.symbol,
                    arguments,
                };
                let mut operands = Vec::new();
                while let Term::Apply { function, argument } = self.arena.get(callee) {
                    operands.push(argument);
                    callee = function;
                }
                operands.reverse();
                operands.extend(inputs.iter().map(|input| input.term));
                let Term::Constant { levels, .. } = self.arena.get(callee) else {
                    return Err(self.refuse("Id application has no checked constant spine".into()));
                };
                for row in emitted {
                    let value =
                        self.instantiate_identity_requirement(row.abstraction, &levels, &operands)?;
                    self.retain_instantiated_identity(
                        row,
                        value,
                        &scope,
                        formals,
                        actuals,
                        &[],
                        &pending_static,
                    )?;
                }
                let static_arity = operands.len() - inputs.len();
                let applications = requirements
                    .applications
                    .iter()
                    .map(|position| self.identity_applications[*position].clone())
                    .collect::<Vec<_>>();
                for mut application in applications {
                    let position = static_formals
                        .iter()
                        .position(|formal| *formal == application.parameter)
                        .map(|position| (position, static_inputs.get(position)))
                        .or_else(|| {
                            formals
                                .iter()
                                .position(|formal| formal.symbol == application.parameter)
                                .map(|position| (static_arity + position, inputs.get(position)))
                        })
                        .or_else(|| {
                            application
                                .pending
                                .iter()
                                .position(|parameter| parameter.binder == application.parameter)
                                .map(|position| {
                                    (
                                        static_arity + formals.len() + position,
                                        inputs.get(formals.len() + position),
                                    )
                                })
                        });
                    let Some((operand_position, input)) = position else {
                        return Err(
                            self.refuse("Id value dependency lost its exact source formal".into())
                        );
                    };
                    let trailing = actuals
                        .iter()
                        .skip(formals.len())
                        .copied()
                        .collect::<Vec<_>>();
                    self.complete_identity_scopes(
                        &mut application.source_scopes,
                        &application.pending,
                        &trailing,
                    )?;
                    let mut pending = pending_static.clone();
                    pending.extend(
                        formals
                            .iter()
                            .skip(actuals.len())
                            .map(|formal| IdentitySourceParameter {
                                declaration: definition.symbol,
                                binder: formal.symbol,
                            })
                            .collect::<Vec<_>>(),
                    );
                    pending.extend(application.pending.iter().skip(trailing.len()).cloned());
                    application.pending = pending;
                    let Some(input) = input else {
                        // An unapplied function formal stays in the checked closure;
                        // the next prefix application will supply its requirements.
                        let value = self.instantiate_identity_requirement(
                            application.abstraction,
                            &levels,
                            &operands,
                        )?;
                        let mut application = application;
                        application.value = value;
                        application.source_scopes.insert(0, scope.clone());
                        self.record_identity_application(application)?;
                        continue;
                    };
                    let rows = input
                        .identity_requirements
                        .emissions
                        .iter()
                        .map(|position| self.identity_emissions[*position].clone())
                        .collect::<Vec<_>>();
                    for row in rows {
                        let mut kernel_operands = operands.clone();
                        kernel_operands[operand_position] = row.value;
                        let value = self.instantiate_identity_requirement(
                            application.abstraction,
                            &levels,
                            &kernel_operands,
                        )?;
                        let mut row = row;
                        row.source_scopes
                            .splice(0..0, application.source_scopes.clone());
                        self.retain_instantiated_identity(
                            row,
                            value,
                            &scope,
                            formals,
                            actuals,
                            &application.arguments,
                            &pending_static,
                        )?;
                    }
                    let deferred = input
                        .identity_requirements
                        .applications
                        .iter()
                        .map(|position| self.identity_applications[*position].clone())
                        .collect::<Vec<_>>();
                    for mut supplied in deferred {
                        let mut kernel_operands = operands.clone();
                        kernel_operands[operand_position] = supplied.value;
                        let value = self.instantiate_identity_requirement(
                            application.abstraction,
                            &levels,
                            &kernel_operands,
                        )?;
                        let consumed = supplied.pending.len().min(application.arguments.len());
                        self.complete_identity_scopes(
                            &mut supplied.source_scopes,
                            &supplied.pending,
                            &application.arguments[..consumed],
                        )?;
                        supplied.pending.drain(..consumed);
                        supplied
                            .arguments
                            .extend_from_slice(&application.arguments[consumed..]);
                        let mut scopes = Vec::with_capacity(
                            1 + application.source_scopes.len() + supplied.source_scopes.len(),
                        );
                        scopes.push(scope.clone());
                        scopes.extend(application.source_scopes.iter().cloned());
                        scopes.extend(supplied.source_scopes);
                        supplied.source_scopes = scopes;
                        supplied
                            .pending
                            .splice(0..0, application.pending.iter().cloned());
                        supplied.value = value;
                        self.record_identity_application(supplied)?;
                    }
                }
                Ok(())
            }

            pub(super) fn retain_identity_parameter_value(
                &mut self,
                value: TermHandle,
            ) -> Result<(), Vec<Diagnostic>> {
                let Term::Variable(index) = self.arena.get(value) else {
                    return Ok(());
                };
                let Some(parameter) = self
                    .scope
                    .len()
                    .checked_sub(index as usize + 1)
                    .and_then(|position| self.scope[position].symbol)
                else {
                    return Ok(());
                };
                self.transport_identity_value(parameter, &[], &[], value, &[], &[])
            }

            fn instantiate_identity_requirement(
                &mut self,
                abstraction: TermHandle,
                levels: &[Level],
                arguments: &[TermHandle],
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let mut value =
                    proof_admission::instantiate_levels(&mut self.arena, abstraction, levels)
                        .map_err(|level| {
                            self.refuse(format!("Id application lost universe parameter {level}"))
                        })?;
                for argument in arguments {
                    value = self.arena.insert(Term::Apply {
                        function: value,
                        argument: *argument,
                    });
                }
                Ok(value)
            }

            fn retain_instantiated_identity(
                &mut self,
                row: IdentityEmission,
                value: TermHandle,
                scope: &IdentitySourceScope,
                formals: &[typed_trees::mathematical::MathematicalParameter],
                actuals: &[ExpressionHandle],
                function_arguments: &[ExpressionHandle],
                pending_static: &[IdentitySourceParameter],
            ) -> Result<(), Vec<Diagnostic>> {
                let mut scopes = Vec::with_capacity(row.source_scopes.len() + 1);
                scopes.push(scope.clone());
                scopes.extend(row.source_scopes);
                let trailing = actuals
                    .iter()
                    .skip(formals.len())
                    .copied()
                    .collect::<Vec<_>>();
                let supplied = function_arguments
                    .iter()
                    .copied()
                    .chain(trailing)
                    .collect::<Vec<_>>();
                self.complete_identity_scopes(&mut scopes, &row.pending, &supplied)?;
                let mut pending = pending_static.to_vec();
                pending.extend(
                    formals
                        .iter()
                        .skip(actuals.len())
                        .map(|formal| IdentitySourceParameter {
                            declaration: scope.declaration,
                            binder: formal.symbol,
                        })
                        .collect::<Vec<_>>(),
                );
                pending.extend(row.pending.into_iter().skip(supplied.len()));
                self.record_identity_emission(
                    value,
                    row.source_type,
                    row.source_owner,
                    scopes,
                    pending,
                )
            }

            fn complete_identity_scopes(
                &self,
                scopes: &mut [IdentitySourceScope],
                pending: &[IdentitySourceParameter],
                supplied: &[ExpressionHandle],
            ) -> Result<(), Vec<Diagnostic>> {
                for (parameter, actual) in pending.iter().zip(supplied) {
                    let Some(scope) = scopes
                        .iter_mut()
                        .find(|scope| scope.declaration == parameter.declaration)
                    else {
                        return Err(self
                            .refuse("Id prefix application lost its supplying telescope".into()));
                    };
                    scope.arguments.push((
                        parameter.binder,
                        IdentitySourceArgument::Expression(*actual),
                    ));
                }
                Ok(())
            }

            fn record_identity_application(
                &mut self,
                mut application: IdentityApplication,
            ) -> Result<(), Vec<Diagnostic>> {
                application.abstraction = application.value;
                for entry in self.scope.iter().rev() {
                    application.abstraction = self.arena.insert(Term::Lambda {
                        domain: entry.domain,
                        body: application.abstraction,
                    });
                }
                let Some(requirements) = self
                    .identity_requirements_by_owner
                    .get_mut(self.source_owner.arena_index() as usize)
                else {
                    return Err(
                        self.refuse("Id function application has no retained source owner".into())
                    );
                };
                if requirements.owner != self.source_owner {
                    *requirements = IdentityRequirements {
                        owner: self.source_owner,
                        ..Default::default()
                    };
                }
                requirements
                    .applications
                    .push(self.identity_applications.len());
                self.identity_applications.push(application);
                Ok(())
            }

            pub(super) fn prepare_identity_values(
                &mut self,
                signature: &proof_admission::Signature,
                context: &proof_admission::Context,
                retained_constants: &std::collections::HashSet<u32>,
                emissions: &std::ops::Range<usize>,
            ) -> Result<HandleSpan<PreparedIdentityValue>, Vec<Diagnostic>> {
                let mut values = Vec::with_capacity(emissions.len());
                for position in emissions.clone() {
                    let row = &self.identity_emissions[position];
                    let term = super::normalization::normalize(
                        &mut self.arena,
                        signature,
                        row.value,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        &|declaration| retained_constants.contains(&declaration),
                    )
                    .map_err(|error| {
                        self.refuse(format!("Id value substitution failed: {error:?}"))
                    })?;
                    let mut head = term;
                    let mut arity = 0;
                    while let Term::Apply { function, .. } = self.arena.get(head) {
                        arity += 1;
                        head = function;
                    }
                    if arity != 3
                        || !matches!(self.arena.get(head), Term::Constant { declaration, .. } if Some(declaration) == self.identity_declaration)
                    {
                        continue;
                    }
                    if !row.pending.is_empty() {
                        return Err(
                            self.refuse("formed Id retains unapplied source subjects".into())
                        );
                    }
                    let arguments = row
                        .source_scopes
                        .iter()
                        .map(|scope| {
                            scope
                                .arguments
                                .iter()
                                .map(|(binder, actual)| {
                                    (
                                        *binder,
                                        match actual {
                                            IdentitySourceArgument::Static(actual) => {
                                                IndexIdentityArgument::StaticArgument(actual)
                                            }
                                            IdentitySourceArgument::Expression(actual) => {
                                                IndexIdentityArgument::Expression(*actual)
                                            }
                                        },
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let scopes = row
                        .source_scopes
                        .iter()
                        .zip(&arguments)
                        .map(|(scope, arguments)| AppliedTypeIdentityScope {
                            declaration: scope.declaration,
                            arguments,
                            lifetimes: &[],
                        })
                        .collect::<Vec<_>>();
                    let type_identity = applied_type_identity(
                        self.program,
                        row.source_type,
                        row.source_owner,
                        &scopes,
                    )
                    .ok_or_else(|| {
                        self.refuse(
                            "Id application lost its exact source carrier substitution".into(),
                        )
                    })?;
                    let ty = proof_admission::infer_type(
                        &mut self.arena,
                        context,
                        term,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!("emitted Id value fails formation: {error:?}"))
                    })?;
                    values.push(PreparedIdentityValue {
                        term,
                        ty,
                        type_identity,
                    });
                }
                Ok(self.prepared.identity_values.insert_many(values))
            }
        }

        #[derive(Clone)]
        pub(super) struct IdentityEmission {
            value: TermHandle,
            abstraction: TermHandle,
            source_type: TypeReferenceHandle,
            source_owner: SymbolHandle,
            source_scopes: Vec<IdentitySourceScope>,
            pending: Vec<IdentitySourceParameter>,
        }

        #[derive(Clone)]
        struct IdentitySourceScope {
            declaration: SymbolHandle,
            arguments: Vec<(SymbolHandle, IdentitySourceArgument)>,
        }

        #[derive(Clone)]
        enum IdentitySourceArgument {
            Static(StaticMachineArgument),
            Expression(ExpressionHandle),
        }

        #[derive(Clone)]
        struct IdentitySourceParameter {
            declaration: SymbolHandle,
            binder: SymbolHandle,
        }

        /// Requirements are retained on the checked mathematical value's owner. A
        /// function formal records its typed application closure until an actual
        /// supplies the emissions; the same level/beta substitution transports both.
        #[derive(Clone, Default)]
        pub(super) struct IdentityRequirements {
            pub(super) owner: SymbolHandle,
            pub(super) emissions: Vec<usize>,
            pub(super) applications: Vec<usize>,
        }

        #[derive(Clone)]
        pub(super) struct IdentityApplication {
            parameter: SymbolHandle,
            value: TermHandle,
            abstraction: TermHandle,
            arguments: Vec<ExpressionHandle>,
            source_scopes: Vec<IdentitySourceScope>,
            pending: Vec<IdentitySourceParameter>,
        }
    }
    mod level_types {
        use crate::mathematical_declarations::FixedCore;
        use crate::typed_expressions::{expression_table, type_reference_table};
        use proof_admission::{Level, Term, TermHandle};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::mathematical::{MathematicalType, MathematicalTypeHandle};
        use typed_trees::name::Identifier;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        use super::{Elaborator, ScalarCarrier, ScopeEntry};

        impl<'a> Elaborator<'a> {
            pub(super) fn elaborate_level(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<Level, Vec<diagnostics::Diagnostic>> {
                if !type_reference_table(self.program).contains_type_reference(reference) {
                    return Err(self.refuse(
                        "a universe-level argument references a type that does not exist"
                            .to_owned(),
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
                            literal
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

            pub(super) fn elaborate_mathematical_type(
                &mut self,
                handle: MathematicalTypeHandle,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                match self.program.mathematical_type(handle) {
                    MathematicalType::Ordinary(reference) => {
                        self.elaborate_type_reference(*reference)
                    }
                    MathematicalType::Arrow {
                        binder,
                        domain,
                        codomain,
                    } => {
                        let domain = self.elaborate_mathematical_type(*domain)?;
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
                        let callee = self.elaborate_mathematical_type(*callee)?;
                        let mut inputs = Vec::new();
                        for argument in expression_table(self.program)
                            .expression_handles(*arguments)
                            .to_vec()
                        {
                            inputs.push(self.source_expression_input(argument)?);
                        }
                        Ok(self.apply_source_inputs(ExpressionHandle::default(), callee, inputs))
                    }
                }
            }

            pub(super) fn elaborate_type_reference(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                if !type_reference_table(self.program).contains_type_reference(reference) {
                    return Err(self.refuse(
                        "a mathematical type references a type that does not exist".to_owned(),
                    ));
                }
                if let Some(carrier) = self.elaborate_content_carrier(reference)? {
                    return Ok(carrier);
                }
                if let Some(alias) = self.source_type_alias(reference)? {
                    return Ok(alias);
                }
                if let Some(carrier) = self.region_nominal_carrier(reference)? {
                    return Ok(carrier);
                }
                // Arithmetic policy changes the selected source value operation, not
                // the calculus domain. Source application admission and captured type
                // references retain the complete shell; only this carrier projection
                // peels policy-only layers. Other predicates remain unsupported here.
                let reference = validation::peel_arithmetic_policy(self.program, reference);
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
                        let arguments = type_reference_table(self.program)
                            .type_reference_handles(*arguments)
                            .to_vec();
                        let base_symbol = *base_symbol;
                        let base_name = base_name.clone();
                        match self.fixed_core.classify(base_symbol, &base_name) {
                            Some(member @ (FixedCore::Type | FixedCore::Strict)) => {
                                match arguments.as_slice() {
                                    [universe] => {
                                        let level = self.elaborate_level(*universe)?;
                                        Ok(self.arena.insert(Term::Sort(member
                                            .universe_sort()
                                            .unwrap()(
                                            level
                                        ))))
                                    }
                                    _ => Err(self.refuse(format!(
                                        "`{}` takes one universe level, not {}",
                                        member.name(),
                                        arguments.len()
                                    ))),
                                }
                            }
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
                            Some(FixedCore::Identity) => Err(self.refuse(
                                "core::Identity requires its two typed subjects".to_owned(),
                            )),
                            Some(FixedCore::Level) => Err(self.refuse(
                                "`core::Level` is a universe level, not a type".to_owned(),
                            )),
                            None => {
                                if base_symbol.is_valid()
                                    && self.authored_index.contains_key(&base_symbol)
                                {
                                    return Err(self.refuse(format!(
                                "applying mathematical declaration `{base_name}` as a carrier is a later leg"
                            )));
                                }
                                if base_symbol.is_valid()
                                    && self
                                        .program
                                        .unique_data_definition_by_symbol(base_symbol)
                                        .is_some()
                                {
                                    return self.indexed_application(base_symbol, &arguments);
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

            /// A data type with only lifetime parameters has the same opaque proof
            /// representation as an ordinary nominal type. Its key retains the
            /// declaring scope and exact lifetimes. Type/value parameters continue
            /// through the checked indexed-family representation.
            fn region_nominal_carrier(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
                let TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } = self.program.type_reference_table.type_reference(reference)
                else {
                    return Ok(None);
                };
                if lifetime_arguments.is_empty()
                    || !type_reference_table(self.program)
                        .type_reference_handles(*arguments)
                        .is_empty()
                {
                    return Ok(None);
                }
                let Some(data) = self.program.unique_data_definition_by_symbol(*base_symbol) else {
                    return Ok(None);
                };
                if !self.program.data_type_parameters(data).is_empty()
                    || data.lifetime_parameters.len() != lifetime_arguments.len()
                {
                    return Ok(None);
                }
                let identity = typed_trees::type_identity::applied_type_identity(
                    self.program,
                    reference,
                    self.source_owner,
                    &[],
                )
                .ok_or_else(|| {
                    self.refuse("a nominal carrier lost its exact lifetime scope".into())
                })?;
                let position = if let Some(position) = self.region_carriers.get(&identity) {
                    *position
                } else {
                    let position = self.push_type_carrier();
                    self.region_carriers.insert(identity, position);
                    position
                };
                Ok(Some(self.constant(position)))
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
                    if let Some(member) = self.fixed_core.classify(symbol, name) {
                        return match member {
                            FixedCore::Level => Err(self.refuse(
                                "`core::Level` is a universe level, not a type".to_owned(),
                            )),
                            member => self.unapplied_core_type(member, name),
                        };
                    }
                    if let Some(term) = self.scope_term_for_symbol(symbol) {
                        return Ok(term);
                    }
                    if let Some(term) = self.constant_for_declaration(symbol)? {
                        return Ok(term);
                    }
                    if let Some(reference) = self
                        .program
                        .unique_data_definition_by_symbol(symbol)
                        .and_then(|definition| definition.generic_instance)
                    {
                        if matches!(
                            self.program.type_reference_table.type_reference(reference),
                            TypeReferenceNode::Named { symbol: selected, .. } if *selected == symbol
                        ) {
                            return Err(self.refuse(
                                "a specialized mathematical carrier lost its source application"
                                    .into(),
                            ));
                        }
                        return self.elaborate_type_reference(reference);
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

            pub(super) fn unapplied_core_type(
                &mut self,
                member: FixedCore,
                name: &Identifier,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                match member {
                    member @ (FixedCore::Type | FixedCore::Strict) => {
                        let level = self.generalized_level();
                        Ok(self
                            .arena
                            .insert(Term::Sort(member.universe_sort().unwrap()(level))))
                    }
                    _ => Err(self.refuse(format!(
                        "`{name}` needs its arguments; it cannot appear unapplied"
                    ))),
                }
            }

            pub(super) fn generalized_level(&mut self) -> Level {
                let parameter =
                    (self.level_binders.len() + self.generalized_levels as usize) as u32;
                self.generalized_levels += 1;
                Level::Parameter(parameter)
            }

            pub(super) fn level_index(
                &self,
                symbol: SymbolHandle,
                name: &Identifier,
            ) -> Option<u32> {
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
    }
    mod normalization {
        use proof_admission::{
            Budget, CoreError, Signature, Term, TermArena, TermHandle,
            weak_head_normalize_retaining,
        };

        pub(super) fn normalize(
            arena: &mut TermArena,
            signature: &Signature,
            term: TermHandle,
            budget: &mut Budget,
            retain: &impl Fn(u32) -> bool,
        ) -> Result<TermHandle, CoreError> {
            let head = weak_head_normalize_retaining(arena, signature, term, budget, retain)?;
            let original = arena.get(head);
            macro_rules! children {
        ($variant:ident { $($field:ident),+ }) => {
            Term::$variant { $($field: normalize(arena, signature, $field, budget, retain)?),+ }
        };
            }
            let reduced = match original.clone() {
                Term::Pi { domain, codomain } => children!(Pi { domain, codomain }),
                Term::Lambda { domain, body } => children!(Lambda { domain, body }),
                Term::Apply { function, argument } => children!(Apply { function, argument }),
                Term::Sigma { domain, codomain } => children!(Sigma { domain, codomain }),
                Term::Pair { first, second } => children!(Pair { first, second }),
                Term::Fst { pair } => children!(Fst { pair }),
                Term::Snd { pair } => children!(Snd { pair }),
                Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                } => children!(CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee
                }),
                Term::Id { ty, left, right } => children!(Id { ty, left, right }),
                Term::Refl { ty, value } => children!(Refl { ty, value }),
                Term::IdElim {
                    motive,
                    base,
                    endpoint,
                    proof,
                } => children!(IdElim {
                    motive,
                    base,
                    endpoint,
                    proof
                }),
                Term::W { carrier, children } => children!(W { carrier, children }),
                Term::Sup {
                    carrier,
                    children,
                    label,
                    function,
                } => children!(Sup {
                    carrier,
                    children,
                    label,
                    function
                }),
                Term::IndW { motive, step, tree } => children!(IndW { motive, step, tree }),
                Term::EmptyElim { ty, scrutinee } => children!(EmptyElim { ty, scrutinee }),
                Term::Squash { ty } => children!(Squash { ty }),
                Term::SquashIntro { ty, value } => children!(SquashIntro { ty, value }),
                Term::SquashElim {
                    proposition,
                    function,
                    scrutinee,
                } => children!(SquashElim {
                    proposition,
                    function,
                    scrutinee
                }),
                Term::Box { ty } => children!(Box { ty }),
                Term::BoxIntro { ty, value } => children!(BoxIntro { ty, value }),
                Term::BoxElim {
                    motive,
                    body,
                    scrutinee,
                } => children!(BoxElim {
                    motive,
                    body,
                    scrutinee
                }),
                leaf => leaf,
            };
            Ok(if reduced == original {
                head
            } else {
                arena.insert(reduced)
            })
        }
    }
    mod operand_terms {
        use crate::typed_expressions::expression_table;
        use numerics::bignum::BigInt;
        use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral, LandedIntegerType};

        use proof_admission::{Term, TermHandle};
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        };
        use typed_trees::types::TypeReferenceHandle;

        use super::dependencies::{free_positions, rebind_domain};
        use super::{Elaborator, ScalarCarrier};
        use validation::expression_traversal::children;
        use validation::mathematical_applications::{
            PreparedSourceBinding, PreparedSourceBindingKind, integer_embedding_meaning,
            source_operations_have_same_meaning,
        };

        pub(super) struct SourceOperationDeclaration {
            expression: ExpressionHandle,
            owner: symbols::SymbolHandle,
            source_type: TypeReferenceHandle,
            bindings: Vec<PreparedSourceBinding>,
            declaration: u32,
        }

        impl<'a> Elaborator<'a> {
            pub(super) fn scalar_operand_term(
                &mut self,
                handle: ExpressionHandle,
                carrier: ScalarCarrier,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                match self.program.expression_table.expression(handle) {
                    ExpressionNode::Integer(literal) => {
                        let Some(value) = literal.value_bignum() else {
                            return Err(self.refuse(format!(
                                "integer literal `{}` has no exact value",
                                literal
                            )));
                        };
                        let position = self.carrier_position(carrier);
                        self.numeric_literal(position, value)
                    }
                    ExpressionNode::Boolean(value) => {
                        if carrier != ScalarCarrier::Boolean {
                            return Err(self.refuse(format!(
                                "boolean literal is not a `{}` operand",
                                self.carrier_name(carrier)
                            )));
                        }
                        Ok(self.boolean_literal(*value))
                    }
                    ExpressionNode::Float(literal) => {
                        let literal = literal.clone();
                        self.float_operand_term(&literal, carrier)
                    }
                    ExpressionNode::ZeroValue(reference) => {
                        let reference = *reference;
                        match self.scalar_carrier_of_type_reference(reference) {
                            Some(determined) if determined == carrier => self.zero_literal(carrier),
                            _ => Err(self.refuse(format!(
                                "zero value of `{}` is not a `{}` operand",
                                self.program.display_type_reference(reference),
                                self.carrier_name(carrier)
                            ))),
                        }
                    }
                    ExpressionNode::Binary(binary) if binary.operator.is_arithmetic() => {
                        let binary = *binary;
                        match carrier {
                            ScalarCarrier::Integer => self.integer_operation_term(handle, &binary),
                            _ => match self.operand_carrier(handle) {
                                Some(determined) if determined == carrier => self.open_term(handle),
                                determined => {
                                    Err(self.unexpected_carrier_error(determined, handle))
                                }
                            },
                        }
                    }
                    ExpressionNode::Name(..)
                    | ExpressionNode::Call(..)
                    | ExpressionNode::Unary(..)
                    | ExpressionNode::Member(..)
                    | ExpressionNode::Cast(..)
                    | ExpressionNode::StructLiteral(..)
                    | ExpressionNode::Match(..) => match self.operand_carrier(handle) {
                        Some(determined) if determined == carrier => {
                            self.elaborate_expression(handle)
                        }
                        determined => Err(self.unexpected_carrier_error(determined, handle)),
                    },
                    _ => Err(self.unsupported_expression(handle)),
                }
            }

            pub(super) fn integer_operand(
                &mut self,
                handle: ExpressionHandle,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                self.scalar_operand_term(handle, ScalarCarrier::Integer)
            }

            pub(super) fn literal_term(
                &mut self,
                literal: &IntegerLiteral,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let Some(value) = literal.value_bignum() else {
                    return Err(
                        self.refuse(format!("integer literal `{}` has no exact value", literal))
                    );
                };
                let carrier = match literal.landing() {
                    Some(landing) if landing.landed_type == LandedIntegerType::Addr => {
                        self.address_carrier()
                    }
                    _ => self.integer_carrier(),
                };
                self.numeric_literal(carrier, value)
            }

            pub(super) fn float_literal_term(
                &mut self,
                literal: &FloatLiteral,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let Some(landing) = literal.landing() else {
                    return Err(self.refuse(format!(
                        "cannot determine the float carrier of literal `{}`",
                        literal.text()
                    )));
                };
                let carrier = match landing {
                    FloatFormat::F32 => ScalarCarrier::Float32,
                    FloatFormat::F64 => ScalarCarrier::Float64,
                };
                self.float_operand_term(literal, carrier)
            }

            pub(super) fn float_operand_term(
                &mut self,
                literal: &FloatLiteral,
                carrier: ScalarCarrier,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let format = match carrier {
                    ScalarCarrier::Float32 => FloatFormat::F32,
                    ScalarCarrier::Float64 => FloatFormat::F64,
                    _ => {
                        return Err(self.refuse(format!(
                            "float literal is not a `{}` operand",
                            self.carrier_name(carrier)
                        )));
                    }
                };
                if let Some(landing) = literal.landing()
                    && landing != format
                {
                    return Err(self.refuse(format!(
                        "a `{}` float literal is not a `{}` operand",
                        landing.name(),
                        self.carrier_name(carrier)
                    )));
                }
                let bits = match format {
                    FloatFormat::F32 => u64::from(literal.f32_bits()),
                    FloatFormat::F64 => literal.value_f64().to_bits(),
                };
                let position = self.carrier_position(carrier);
                Ok(self.float_literal(position, bits))
            }

            pub(super) fn float_literal(&mut self, carrier: u32, bits: u64) -> TermHandle {
                interned_literal!(self.float_literals, (carrier, bits), carrier)
            }

            pub(super) fn zero_literal(
                &mut self,
                carrier: ScalarCarrier,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                match carrier {
                    ScalarCarrier::Boolean => Ok(self.boolean_literal(false)),
                    ScalarCarrier::Float32 | ScalarCarrier::Float64 => {
                        let position = self.carrier_position(carrier);
                        Ok(self.float_literal(position, 0))
                    }
                    ScalarCarrier::Carrier(..) => Err(self.refuse(
                        "the zero value of a non-numeric carrier has no bounded denotation"
                            .to_owned(),
                    )),
                    _ => {
                        let position = self.carrier_position(carrier);
                        self.numeric_literal(position, BigInt::from_u64(0))
                    }
                }
            }

            pub(super) fn zero_value_term(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                match self.scalar_carrier_of_type_reference(reference) {
                    Some(carrier) => self.zero_literal(carrier),
                    None => Err(self.refuse(format!(
                        "the zero value of `{}` has no scalar denotation",
                        self.program.display_type_reference(reference)
                    ))),
                }
            }

            pub(super) fn integer_operation_term(
                &mut self,
                handle: ExpressionHandle,
                binary: &TableBinaryExpression,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                if !self.expression_has_float_leaf(handle)
                    && let Some(value) = self.program.closed_integer_expression_value(handle)
                {
                    let carrier = self.integer_carrier();
                    return self.numeric_literal(carrier, value);
                }
                if self
                    .source_operand_type_reference(handle)
                    .is_some_and(|reference| {
                        self.program
                            .primitive_type_reference(reference)
                            .is_some_and(|primitive| primitive.is_fixed_integer())
                            && self.program.arithmetic_domain_for_type_reference(reference)
                                != numerics::arithmetic::ArithmeticDomain::Exact
                    })
                {
                    return self.open_term(handle);
                }
                let references = [binary.left, binary.right]
                    .map(|operand| self.source_operand_type_reference(operand));
                let spelling = match binary.operator {
                    BinaryOperator::Add => language_core::OperatorSpelling::Add,
                    BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
                    BinaryOperator::Multiply if self.integer_multiply.is_some() => {
                        language_core::OperatorSpelling::Multiply
                    }
                    _ => return self.open_term(handle),
                };
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    self.source_owner,
                    handle,
                    spelling,
                    &references,
                ) {
                    return self.open_term(handle);
                }
                let position = match binary.operator {
                    BinaryOperator::Add => self.integer_add(),
                    BinaryOperator::Subtract => self.integer_subtract(),
                    BinaryOperator::Multiply if self.integer_multiply.is_some() => {
                        self.integer_multiply.unwrap()
                    }
                    _ => return self.open_term(handle),
                };
                let left = self.integer_operand(binary.left)?;
                let right = self.integer_operand(binary.right)?;
                let operation = self.constant(position);
                let function = self.arena.insert(Term::Apply {
                    function: operation,
                    argument: left,
                });
                Ok(self.arena.insert(Term::Apply {
                    function,
                    argument: right,
                }))
            }

            pub(super) fn open_term(
                &mut self,
                handle: ExpressionHandle,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                if self.integer_theory.is_some()
                    && let ExpressionNode::Call(call) =
                        self.program.expression_table.expression(handle)
                    && self
                        .program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        == Some(symbols::BuiltinFunction::IntegerEmbed)
                {
                    let arguments =
                        expression_table(self.program).expression_handles(call.arguments);
                    if let [argument] = arguments
                        && self.operand_carrier(*argument) == Some(ScalarCarrier::Integer)
                    {
                        return self.integer_operand(*argument);
                    }
                }
                // An opaque operation's declared result can be the first occurrence
                // of a data carrier. Elaborate that exact source domain before asking
                // which carrier it denotes; signature occurrence is not authority.
                let source_type = self.source_operand_type_reference(handle);
                let source_domain = source_type
                    .map(|reference| self.elaborate_source_value_type(reference))
                    .transpose()?;
                if let Some(meaning) = integer_embedding_meaning(self.program, handle) {
                    return self.embedding_term(handle, meaning);
                }
                // An applied data carrier already has its complete source domain.
                // The scalar classifier cannot replace its retained type arguments.
                let result_type = match source_domain {
                    Some(domain) => domain,
                    None => {
                        let carrier = self
                            .operand_carrier(handle)
                            .ok_or_else(|| self.unsupported_expression(handle))?;
                        self.carrier_term(carrier)
                    }
                };
                let mut dependencies = Vec::new();
                free_positions(
                    &self.arena,
                    result_type,
                    self.scope.len(),
                    &mut dependencies,
                );
                let mut pending = vec![handle];
                let mut seen = Vec::new();
                let mut occurrences = Vec::new();
                while let Some(expression) = pending.pop() {
                    if !expression_table(self.program).expression_is_valid(expression)
                        || seen.contains(&expression)
                    {
                        continue;
                    }
                    seen.push(expression);
                    if let Some((_, position)) = self
                        .captured_expressions
                        .iter()
                        .find(|(captured, _)| *captured == expression)
                    {
                        if !dependencies.contains(position) {
                            dependencies.push(*position);
                        }
                        occurrences.push((expression, *position));
                        continue;
                    }
                    let node = self.program.expression_table.expression(expression);
                    if let ExpressionNode::Name(path) = node {
                        let name = expression_table(self.program)
                            .name_path_members(path.members)
                            .last();
                        if let Some(position) = self.scope.iter().rposition(|entry| {
                            if path.symbol.is_valid() {
                                entry.symbol == Some(path.symbol)
                            } else {
                                entry.symbol.is_none()
                                    && name.is_some_and(|name| *name == entry.name)
                            }
                        }) {
                            if !dependencies.contains(&position) {
                                dependencies.push(position);
                            }
                            occurrences.push((expression, position));
                        }
                    }
                    children(self.program, node, |child| pending.push(child));
                }
                let mut index = 0;
                while index < dependencies.len() {
                    let position = dependencies[index];
                    free_positions(
                        &self.arena,
                        self.scope[position].domain,
                        position,
                        &mut dependencies,
                    );
                    index += 1;
                }
                dependencies.sort_unstable();
                let source_type = source_type.unwrap_or_default();
                let mut bindings = Vec::with_capacity(dependencies.len());
                for (index, position) in dependencies.iter().copied().enumerate() {
                    let expressions = occurrences
                        .iter()
                        .filter_map(|(expression, bound)| {
                            (*bound == position).then_some(*expression)
                        })
                        .collect::<Vec<_>>();
                    let symbol = self.scope[position]
                        .symbol
                        .or_else(|| {
                            expressions.iter().find_map(|expression| {
                                let ExpressionNode::Name(path) =
                                    self.program.expression_table.expression(*expression)
                                else {
                                    return None;
                                };
                                path.symbol.is_valid().then_some(path.symbol)
                            })
                        })
                        .unwrap_or_default();
                    let source_type = expressions
                        .iter()
                        .find_map(|expression| self.source_operand_type_reference(*expression))
                        .unwrap_or_default();
                    let old = self.scope[..position]
                        .iter()
                        .map(|entry| entry.domain)
                        .collect::<Vec<_>>();
                    let domain = rebind_domain(
                        &mut self.arena,
                        self.scope[position].domain,
                        &old,
                        &dependencies[..index],
                    );
                    bindings.push(PreparedSourceBinding {
                        source_domain: self.scope[position].domain,
                        domain,
                        kind: if matches!(
                            self.arena.get(self.scope[position].domain),
                            Term::Sort(_)
                        ) {
                            PreparedSourceBindingKind::Type
                        } else {
                            PreparedSourceBindingKind::Value
                        },
                        symbol,
                        source_type,
                        expressions,
                    });
                }
                let position = match self.source_operations.iter().find(|operation| {
                    operation.owner == self.source_owner
                        && operation.source_type == source_type
                        && operation.bindings.len() == bindings.len()
                        && operation
                            .bindings
                            .iter()
                            .zip(&bindings)
                            .all(|(old, new)| old.same_source_binding(new))
                        && (operation.expression == handle
                            || source_operations_have_same_meaning(
                                self.program,
                                self.source_owner,
                                operation.expression,
                                handle,
                            ))
                }) {
                    Some(operation) => operation.declaration,
                    None => {
                        let old_domains = self
                            .scope
                            .iter()
                            .map(|entry| entry.domain)
                            .collect::<Vec<_>>();
                        let mut ty = rebind_domain(
                            &mut self.arena,
                            result_type,
                            &old_domains,
                            &dependencies,
                        );
                        for binding in bindings.iter().rev() {
                            ty = self.arena.insert(Term::Pi {
                                domain: binding.domain,
                                codomain: ty,
                            });
                        }
                        let position = self.push_assumption(ty);
                        self.source_operations.push(SourceOperationDeclaration {
                            expression: handle,
                            owner: self.source_owner,
                            source_type,
                            bindings: bindings.clone(),
                            declaration: position,
                        });
                        self.prepared.constants.append(super::PreparedConstant {
                            declaration: position,
                            meaning: super::PreparedConstantMeaning::SourceOperation {
                                expression: handle,
                                owner: self.source_owner,
                                source_type,
                                bindings,
                            },
                        });
                        position
                    }
                };
                let mut function = self.constant(position);
                for position in dependencies {
                    let argument = self.scope_variable(position);
                    function = self.arena.insert(Term::Apply { function, argument });
                }
                Ok(function)
            }

            fn embedding_term(
                &mut self,
                handle: ExpressionHandle,
                meaning: super::PreparedConstantMeaning,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let (_, argument) = validation::integer_embedding_argument(self.program, handle)
                    .ok_or_else(|| self.unsupported_expression(handle))?;
                let source = self
                    .operand_carrier(argument)
                    .ok_or_else(|| self.unsupported_expression(argument))?;
                let argument = self.scalar_operand_term(argument, source)?;
                if self.integer_theory.is_some() && source == ScalarCarrier::Integer {
                    return Ok(argument);
                }
                let domain = self.carrier_term(source);
                let codomain = self.integer_constant();
                let ty = self.arena.insert(Term::Pi { domain, codomain });
                let position = self.push_assumption(ty);
                self.prepared.constants.append(super::PreparedConstant {
                    declaration: position,
                    meaning,
                });
                let function = self.constant(position);
                Ok(self.arena.insert(Term::Apply { function, argument }))
            }
        }
    }
    mod propositions {
        use proof_admission::{Term, TermHandle, shift};
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
        };

        use super::{Elaborator, ScalarCarrier};
        use numerics::literals::FloatFormat;
        use typed_trees::proposition::ProofSubstitutions;
        use typed_trees::types::PrimitiveType;

        impl<'a> Elaborator<'a> {
            pub(super) fn elaborate_proposition(
                &mut self,
                handle: ExpressionHandle,
            ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
                self.result_identity_requirements =
                    super::identity::IdentityRequirements::default();
                self.elaborate_proposition_term(handle)
            }

            fn elaborate_proposition_term(
                &mut self,
                handle: ExpressionHandle,
            ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
                match self.program.expression_table.expression(handle) {
                    ExpressionNode::Boolean(value) => Ok(Some(self.boolean_proposition(*value))),
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        let operand = unary.operand;
                        self.elaborate_proposition(operand).map(|proposition| {
                            proposition.map(|term| self.negated_proposition(term))
                        })
                    }
                    ExpressionNode::Call(call)
                        if validation::mathematical_applications::is_identity_call(
                            self.program,
                            call,
                        ) =>
                    {
                        let call = call.clone();
                        self.elaborate_identity(handle, &call).map(Some)
                    }
                    ExpressionNode::Name(..)
                    | ExpressionNode::Call(..)
                    | ExpressionNode::Unary(..)
                    | ExpressionNode::Member(..)
                    | ExpressionNode::Cast(..)
                    | ExpressionNode::Indexed(..) => self.subject_proposition(handle),
                    ExpressionNode::Binary(binary) => {
                        let binary = *binary;
                        self.source_expression_domain(binary.left)?;
                        self.source_expression_domain(binary.right)?;
                        match binary.operator {
                            BinaryOperator::Equal => {
                                self.equality_proposition(handle, binary).map(Some)
                            }
                            BinaryOperator::NotEqual => {
                                self.inequality_proposition(handle, binary).map(Some)
                            }
                            BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual => {
                                self.order_proposition(handle, binary).map(Some)
                            }
                            BinaryOperator::And | BinaryOperator::Or => self
                                .connective_proposition(handle, binary.operator)
                                .map(Some),
                            _ => Ok(None),
                        }
                    }
                    _ => self.subject_proposition(handle),
                }
            }

            pub(super) fn subject_proposition(
                &mut self,
                handle: ExpressionHandle,
            ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
                match self.operand_carrier(handle) {
                    Some(ScalarCarrier::Boolean) => {
                        let subject = self.scalar_operand_term(handle, ScalarCarrier::Boolean)?;
                        let ty = self.carrier_term(ScalarCarrier::Boolean);
                        let truth = self.boolean_literal(true);
                        Ok(Some(self.arena.insert(Term::Id {
                            ty,
                            left: subject,
                            right: truth,
                        })))
                    }
                    _ => Ok(None),
                }
            }

            pub(super) fn inequality_proposition(
                &mut self,
                handle: ExpressionHandle,
                binary: TableBinaryExpression,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let carrier = self
                    .operand_carrier(binary.left)
                    .or_else(|| self.operand_carrier(binary.right));
                if let Some(term) = self.float_comparison(handle, binary, carrier)? {
                    return Ok(term);
                }
                let equality = self.equality_proposition(handle, binary)?;
                Ok(self.negated_proposition(equality))
            }

            fn negated_proposition(&mut self, proposition: TermHandle) -> TermHandle {
                let position = self.proposition_negation();
                let function = self.constant(position);
                self.arena.insert(Term::Apply {
                    function,
                    argument: proposition,
                })
            }

            pub(super) fn equality_proposition(
                &mut self,
                handle: ExpressionHandle,
                binary: TableBinaryExpression,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let carrier = self
                    .operand_carrier(binary.left)
                    .or_else(|| self.operand_carrier(binary.right))
                    .ok_or_else(|| {
                        self.refuse(format!(
                            "cannot determine a shared scalar carrier for `{}`",
                            self.program
                                .render_proof_expression(handle, ProofSubstitutions::None)
                        ))
                    })?;
                let references = [binary.left, binary.right]
                    .map(|operand| self.source_operand_type_reference(operand));
                if let Some(term) = self.float_comparison(handle, binary, Some(carrier))? {
                    return Ok(term);
                }
                let identity = !matches!(carrier, ScalarCarrier::Float32 | ScalarCarrier::Float64)
                    && typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.source_owner,
                        handle,
                        language_core::OperatorSpelling::Equal,
                        &references,
                    );
                if !identity {
                    let left = self.open_term(handle)?;
                    let right = self.boolean_literal(true);
                    let ty = self.carrier_term(ScalarCarrier::Boolean);
                    return Ok(self.arena.insert(Term::Id { ty, left, right }));
                }
                let mut left = self.scalar_operand_term(binary.left, carrier)?;
                let mut right = self.scalar_operand_term(binary.right, carrier)?;
                if self.term_order_key(left) > self.term_order_key(right) {
                    std::mem::swap(&mut left, &mut right);
                }
                let ty = self.carrier_term(carrier);
                Ok(self.arena.insert(Term::Id { ty, left, right }))
            }

            pub(super) fn order_proposition(
                &mut self,
                handle: ExpressionHandle,
                binary: TableBinaryExpression,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let carrier = self
                    .operand_carrier(binary.left)
                    .or_else(|| self.operand_carrier(binary.right));
                let references = [binary.left, binary.right]
                    .map(|operand| self.source_operand_type_reference(operand));
                if let Some(term) = self.float_comparison(handle, binary, carrier)? {
                    return Ok(term);
                }
                let spelling = match binary.operator {
                    BinaryOperator::Less => language_core::OperatorSpelling::Less,
                    BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
                    BinaryOperator::Greater => language_core::OperatorSpelling::Greater,
                    BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
                    _ => unreachable!("order relation"),
                };
                if carrier != Some(ScalarCarrier::Integer)
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.source_owner,
                        handle,
                        spelling,
                        &references,
                    )
                {
                    let left = self.open_term(handle)?;
                    let right = self.boolean_literal(true);
                    let ty = self.carrier_term(ScalarCarrier::Boolean);
                    return Ok(self.arena.insert(Term::Id { ty, left, right }));
                }
                let left = self.integer_operand(binary.left)?;
                let right = self.integer_operand(binary.right)?;
                let (strict, left, right) = match binary.operator {
                    BinaryOperator::Less => (true, left, right),
                    BinaryOperator::LessOrEqual => (false, left, right),
                    BinaryOperator::Greater => (true, right, left),
                    _ => (false, right, left),
                };
                let position = if strict {
                    self.integer_less_than()
                } else {
                    self.integer_less_or_equal()
                };
                let relation = self.constant(position);
                Ok(self.arena.apply_binary(relation, left, right))
            }

            fn float_comparison(
                &mut self,
                handle: ExpressionHandle,
                binary: TableBinaryExpression,
                carrier: Option<ScalarCarrier>,
            ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
                let (carrier, format) = match carrier {
                    Some(ScalarCarrier::Float32) => (ScalarCarrier::Float32, FloatFormat::F32),
                    Some(ScalarCarrier::Float64) => (ScalarCarrier::Float64, FloatFormat::F64),
                    _ => return Ok(None),
                };
                let references = [binary.left, binary.right]
                    .map(|operand| self.source_operand_type_reference(operand));
                if !validation::has_builtin_or_sealed_float_comparison_meaning(
                    self.program,
                    self.source_owner,
                    handle,
                    binary.operator,
                    &references,
                ) {
                    return Ok(None);
                }
                let Some(reference) = references.into_iter().flatten().find(|reference| {
                    matches!(
                        (self.program.primitive_type_reference(*reference), format),
                        (Some(PrimitiveType::F32), FloatFormat::F32)
                            | (Some(PrimitiveType::F64), FloatFormat::F64)
                    )
                }) else {
                    return Ok(None);
                };
                let existing =
                    self.prepared
                        .constants
                        .iter()
                        .find_map(|(_, row)| match row.meaning {
                            super::PreparedConstantMeaning::FloatRelation {
                                operator,
                                format: retained,
                                reference: retained_reference,
                            } if operator == binary.operator
                                && retained == format
                                && retained_reference == reference =>
                            {
                                Some(row.declaration)
                            }
                            _ => None,
                        });
                let position = if let Some(position) = existing {
                    position
                } else {
                    let domain = self.carrier_term(carrier);
                    let codomain = self.type_zero();
                    let inner = self.arena.insert(Term::Pi { domain, codomain });
                    let ty = self.arena.insert(Term::Pi {
                        domain,
                        codomain: inner,
                    });
                    let position = self.push_assumption(ty);
                    self.prepared.constants.append(super::PreparedConstant {
                        declaration: position,
                        meaning: super::PreparedConstantMeaning::FloatRelation {
                            operator: binary.operator,
                            format,
                            reference,
                        },
                    });
                    position
                };
                let left = self.scalar_operand_term(binary.left, carrier)?;
                let right = self.scalar_operand_term(binary.right, carrier)?;
                let relation = self.constant(position);
                Ok(Some(self.arena.apply_binary(relation, left, right)))
            }

            pub(super) fn connective_proposition(
                &mut self,
                handle: ExpressionHandle,
                operator: BinaryOperator,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                let mut operands = Vec::new();
                self.proposition_spine(handle, operator, &mut operands);
                let mut terms = Vec::with_capacity(operands.len());
                let mut requirements = super::identity::IdentityRequirements::default();
                for operand in operands {
                    match self.elaborate_proposition(operand)? {
                        Some(term) => {
                            terms.push(term);
                            let value = std::mem::take(&mut self.result_identity_requirements);
                            requirements.emissions.extend(value.emissions);
                            requirements.applications.extend(value.applications);
                        }
                        None => {
                            return Err(self.refuse(format!(
                                "operand `{}` of a proposition connective is not a proposition",
                                self.program
                                    .render_proof_expression(operand, ProofSubstitutions::None)
                            )));
                        }
                    }
                }
                self.result_identity_requirements = requirements;
                terms.sort_by_key(|term| self.term_order_key(*term));
                Ok(match operator {
                    BinaryOperator::And => self.right_nested_sigma(&terms, 0),
                    _ => self.tagged_sum(&terms, 0),
                })
            }

            pub(super) fn proposition_spine(
                &self,
                handle: ExpressionHandle,
                operator: BinaryOperator,
                operands: &mut Vec<ExpressionHandle>,
            ) {
                match self.program.expression_table.expression(handle) {
                    ExpressionNode::Binary(binary) if binary.operator == operator => {
                        self.proposition_spine(binary.left, operator, operands);
                        self.proposition_spine(binary.right, operator, operands);
                    }
                    _ => operands.push(handle),
                }
            }

            pub(super) fn right_nested_sigma(
                &mut self,
                terms: &[TermHandle],
                under: u32,
            ) -> TermHandle {
                let domain = shift(&mut self.arena, terms[0], 0, under);
                let codomain = if terms.len() == 2 {
                    shift(&mut self.arena, terms[1], 0, under + 1)
                } else {
                    self.right_nested_sigma(&terms[1..], under + 1)
                };
                self.arena.insert(Term::Sigma { domain, codomain })
            }

            pub(super) fn tagged_sum(&mut self, terms: &[TermHandle], under: u32) -> TermHandle {
                let two = self.arena.insert(Term::Two);
                let type_zero = self.type_zero();
                let motive = self.arena.insert(Term::Lambda {
                    domain: two,
                    body: type_zero,
                });
                let scrutinee = self.arena.insert(Term::Variable(0));
                let zero_branch = shift(&mut self.arena, terms[0], 0, under + 1);
                let one_branch = if terms.len() == 2 {
                    shift(&mut self.arena, terms[1], 0, under + 1)
                } else {
                    self.tagged_sum(&terms[1..], under + 1)
                };
                let family = self.arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                });
                self.arena.insert(Term::Sigma {
                    domain: two,
                    codomain: family,
                })
            }

            pub(super) fn boolean_proposition(&mut self, value: bool) -> TermHandle {
                let ty = self.arena.insert(Term::Two);
                let zero = self.arena.insert(Term::TwoZero);
                let right = if value {
                    zero
                } else {
                    self.arena.insert(Term::TwoOne)
                };
                self.arena.insert(Term::Id {
                    ty,
                    left: zero,
                    right,
                })
            }
        }
    }
    mod scope {
        use crate::typed_expressions::type_reference_table;
        use proof_admission::{Term, TermHandle};
        use symbols::{BuiltinTypeAtom, BuiltinTypeMember, SymbolHandle};
        use typed_trees::name::Identifier;

        use super::{Elaborator, ScopeEntry};

        impl<'a> Elaborator<'a> {
            pub(super) fn named_term(
                &mut self,
                symbol: SymbolHandle,
                name: &Identifier,
            ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
                if self.program.symbols.builtin_type_member(symbol)
                    == Some(BuiltinTypeMember::AddressBound)
                {
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
                    && let Some(reference) =
                        type_reference_table(self.program).find_named_type_reference(symbol)
                {
                    return self.elaborate_type_reference(reference);
                }
                if let Some(term) = self.carrier_for_symbol(symbol) {
                    return Ok(term);
                }
                let owner = self.program.symbols.display_path(self.source_owner, "::");
                let owner = if owner.is_empty() {
                    "an expression without a retained source owner"
                } else {
                    owner.as_str()
                };
                Err(self.refuse(format!(
                    "`{name}` is not a mathematical term: its exact binding is unavailable in {owner}"
                )))
            }

            pub(super) fn scope_term_for_symbol(
                &mut self,
                symbol: SymbolHandle,
            ) -> Option<TermHandle> {
                if let Some((_, term)) = self
                    .source_constructor_context
                    .values
                    .iter()
                    .find(|(bound, _)| *bound == symbol)
                {
                    return Some(*term);
                }
                let resolved = self
                    .source_index_aliases
                    .iter()
                    .find(|(parameter, _)| *parameter == symbol)
                    .map(|(_, field)| *field)
                    .unwrap_or(symbol);
                let position = self
                    .scope
                    .iter()
                    .rposition(|entry| entry.symbol == Some(resolved))?;
                Some(self.scope_variable(position))
            }

            pub(super) fn scope_term_for_name(&mut self, name: &Identifier) -> Option<TermHandle> {
                let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
                Some(self.scope_variable(position))
            }

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

            pub(super) fn carrier_for_symbol(
                &mut self,
                symbol: SymbolHandle,
            ) -> Option<TermHandle> {
                if self.program.symbols.get(symbol).kind == symbols::SymbolKind::TypeParameter
                    && self
                        .program
                        .data_type_parameter_by_symbol(symbol)
                        .is_some_and(|parameter| {
                            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                        })
                {
                    let level = self.source_type_level(symbol);
                    let position = if let Some(&position) = self.carriers.get(&symbol) {
                        position
                    } else {
                        let ty = self.arena.insert(Term::Sort(proof_admission::Sort::Type(
                            proof_admission::Level::Parameter(0),
                        )));
                        let position = self.declarations.len() as u32;
                        self.declarations
                            .push(proof_admission::Declaration::assumption(1, ty));
                        self.carriers.insert(symbol, position);
                        position
                    };
                    return Some(self.arena.insert(Term::Constant {
                        declaration: position,
                        levels: vec![level],
                    }));
                }
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

            pub(super) fn source_type_level(
                &mut self,
                symbol: SymbolHandle,
            ) -> proof_admission::Level {
                if let Some((_, level)) = self
                    .source_type_levels
                    .iter()
                    .find(|(bound, _)| *bound == symbol)
                {
                    return level.clone();
                }
                let level = self.generalized_level();
                self.source_type_levels.push((symbol, level.clone()));
                level
            }
        }
    }
    mod source_domains {
        use super::{Elaborator, ScalarCarrier};
        use crate::typed_expressions::type_reference_table;
        use diagnostics::Diagnostic;
        use proof_admission::{
            Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
            TermHandle, convertible_retaining, infer_type, substitute,
            weak_head_normalize_retaining,
        };
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::{
            PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };

        pub(super) struct SourceAlias {
            pub reference: TypeReferenceHandle,
            pub declaration: u32,
            pub carrier: ScalarCarrier,
        }

        #[derive(Clone)]
        pub(super) struct SourceInput {
            pub expression: ExpressionHandle,
            pub term: TermHandle,
            pub identity_requirements: super::identity::IdentityRequirements,
            /// A source-typed ordinary value's complete domain. Mathematical function
            /// terms instead use their instantiated calculus telescope directly.
            pub domain: Option<TermHandle>,
        }

        pub(super) struct SourceInvocation {
            pub expression: ExpressionHandle,
            pub span: Option<source::SourceSpan>,
            pub formation: SourceFormation,
            pub domains: Vec<TermHandle>,
            pub level_arity: u32,
        }

        pub(super) enum SourceFormation {
            Application {
                callee: TermHandle,
                inputs: Vec<SourceInput>,
            },
            Result {
                expected: TermHandle,
                actual: SourceInput,
            },
        }

        impl Elaborator<'_> {
            /// Keep the declaration that supplied this value when reading its type.
            /// In particular, a sibling requirement's result is scoped by the same
            /// exact telescope as its operands, before any calculus conversion.
            pub(super) fn source_operand_type_reference(
                &self,
                expression: ExpressionHandle,
            ) -> Option<TypeReferenceHandle> {
                let owners = self.source_owners.get_or_init(|| {
                    validation::expression_owners::OwnerEnvironmentIndex::for_preparation(
                        self.program,
                    )
                });
                if owners.contains_expression(expression) {
                    return validation::expression_owners::checked_expression_type_reference_from_exact_owner(
                        self.program,
                        &arena::Arena::default(),
                        expression,
                        expression,
                        Some(owners),
                    );
                }
                if self.source_owner.is_valid() {
                    validation::mathematical_applications::operand_type_reference_in_declaration(
                        self.program,
                        self.source_owner,
                        expression,
                    )
                } else {
                    validation::mathematical_applications::operand_type_reference(
                        self.program,
                        expression,
                    )
                }
            }

            /// Construct all mathematical Apply spines and retain their source-domain
            /// obligations together. Ordinary calls, generic terms and dependent type
            /// applications differ in argument roles, never in application formation.
            pub(super) fn apply_source_inputs(
                &mut self,
                expression: ExpressionHandle,
                callee: TermHandle,
                inputs: Vec<SourceInput>,
            ) -> TermHandle {
                let mut function = callee;
                for input in &inputs {
                    function = self.arena.insert(Term::Apply {
                        function,
                        argument: input.term,
                    });
                }
                self.source_invocations.push(SourceInvocation {
                    expression,
                    span: if expression.is_valid() {
                        Some(self.program.expression_table.source_span(expression))
                    } else {
                        self.span
                    },
                    formation: SourceFormation::Application { callee, inputs },
                    domains: self.scope.iter().map(|entry| entry.domain).collect(),
                    level_arity: self.level_binders.len() as u32 + self.generalized_levels,
                });
                function
            }

            pub(super) fn source_expression_input(
                &mut self,
                expression: ExpressionHandle,
            ) -> Result<SourceInput, Vec<Diagnostic>> {
                let term = self.elaborate_expression(expression)?;
                let identity_requirements = std::mem::take(&mut self.result_identity_requirements);
                let domain = self.source_expression_domain(expression)?;
                Ok(SourceInput {
                    expression,
                    term,
                    domain,
                    identity_requirements,
                })
            }

            pub(super) fn source_application_input(
                &mut self,
                expression: ExpressionHandle,
                prefix: TermHandle,
            ) -> Result<SourceInput, Vec<Diagnostic>> {
                let typed_trees::expression::ExpressionNode::Float(literal) =
                    self.program.expression_table.expression(expression)
                else {
                    return self.source_expression_input(expression);
                };
                if literal.landing().is_some() {
                    return self.source_expression_input(expression);
                }
                let literal = literal.clone();
                let context = self.instantiation_context();
                let signature = Signature::from_declarations(self.declarations.clone());
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                let function_type = infer_type(&mut self.arena, &context, prefix, &mut budget)
                    .map_err(|error| {
                        self.source_domain_error(
                            expression,
                            format!("cannot determine literal argument domain: {error:?}"),
                        )
                    })?;
                let function_type =
                    self.source_domain_head(&signature, function_type, &mut budget)?;
                let Term::Pi { domain, .. } = self.arena.get(function_type) else {
                    return Err(self.source_domain_error(
                        expression,
                        "literal argument has no function parameter domain".to_owned(),
                    ));
                };
                let domain = self.source_domain_head(&signature, domain, &mut budget)?;
                let carrier = if let Some(reference) = self.source_alias_reference(domain) {
                    self.scalar_carrier_of_type_reference(reference)
                } else if let Term::Constant { declaration, .. } = self.arena.get(domain) {
                    Some(self.carrier_class_at(declaration))
                } else {
                    None
                };
                let Some(carrier @ (ScalarCarrier::Float32 | ScalarCarrier::Float64)) = carrier
                else {
                    return self.source_expression_input(expression);
                };
                let term = self.float_operand_term(&literal, carrier)?;
                Ok(SourceInput {
                    expression,
                    term,
                    domain: None,
                    identity_requirements: super::identity::IdentityRequirements::default(),
                })
            }

            pub(super) fn source_expression_domain(
                &mut self,
                expression: ExpressionHandle,
            ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
                if let Some((_, position)) = self
                    .captured_expressions
                    .iter()
                    .find(|(captured, _)| *captured == expression)
                {
                    let amount = u32::try_from(self.scope.len() - *position).map_err(|_| {
                        self.refuse("source capture exceeds the kernel binding range".to_owned())
                    })?;
                    return Ok(Some(proof_admission::shift(
                        &mut self.arena,
                        self.scope[*position].domain,
                        0,
                        amount,
                    )));
                }
                let node = self.program.expression_table.expression(expression);
                let reference = if matches!(node, typed_trees::expression::ExpressionNode::Call(call)
            if self.authored_index.contains_key(&call.target_symbol))
                    || matches!(node, typed_trees::expression::ExpressionNode::Name(path)
                if self.program.symbols.builtin_type_atom(path.symbol).is_some())
                {
                    None
                } else {
                    self.source_operand_type_reference(expression)
                };
                reference
                    .map(|reference| {
                        self.elaborate_source_value_type(reference)
                            .map_err(|mut diagnostics| {
                                for diagnostic in &mut diagnostics {
                                    diagnostic.source_span =
                                        Some(self.program.expression_table.source_span(expression));
                                }
                                diagnostics
                            })
                    })
                    .transpose()
            }

            pub(super) fn elaborate_source_operand_type(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                if let Some(projected) = self.scalar_operand_carrier(reference)
                    && let Some(alias) = self.source_alias_for_carrier(reference, projected)?
                {
                    return Ok(alias);
                }
                self.elaborate_type_reference(reference)
            }

            pub(super) fn elaborate_source_value_type(
                &mut self,
                declared: TypeReferenceHandle,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let observed =
                    validation::mathematical_applications::scalar_observation_type_reference(
                        self.program,
                        declared,
                    )
                    .unwrap_or(declared);
                self.elaborate_source_operand_type(observed)
            }

            pub(super) fn source_subject_observation_type(
                &self,
                declared: TypeReferenceHandle,
                expected: Option<TypeReferenceHandle>,
            ) -> TypeReferenceHandle {
                if let Some(expected) = expected
                    && let Some(observed) =
                        validation::mathematical_applications::subject_observation_type_reference(
                            self.program,
                            declared,
                            expected,
                        )
                {
                    return observed;
                }
                validation::mathematical_applications::scalar_observation_type_reference(
                    self.program,
                    declared,
                )
                .unwrap_or(declared)
            }

            fn scalar_operand_carrier(
                &self,
                mut reference: TypeReferenceHandle,
            ) -> Option<TypeReferenceHandle> {
                while let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = self.program.type_reference_table.type_reference(reference)
                {
                    if !type_reference_table(self.program)
                        .constraints(*constraints)
                        .iter()
                        .all(|constraint| {
                            matches!(constraint, TypeConstraintNode::ArithmeticDomain(_))
                        })
                    {
                        return None;
                    }
                    reference = *base_type;
                }
                (matches!(
                    self.program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { .. }
                ) && matches!(
                    self.program.primitive_type_reference(reference),
                    Some(
                        PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                            | PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::Addr
                            | PrimitiveType::F32
                            | PrimitiveType::F64
                    )
                ))
                .then_some(reference)
            }

            pub(super) fn retain_source_result(
                &mut self,
                expected: TermHandle,
                actual: SourceInput,
            ) {
                let expression = actual.expression;
                self.source_invocations.push(SourceInvocation {
                    expression,
                    span: Some(self.program.expression_table.source_span(expression)),
                    formation: SourceFormation::Result { expected, actual },
                    domains: self.scope.iter().map(|entry| entry.domain).collect(),
                    level_arity: self.level_binders.len() as u32 + self.generalized_levels,
                });
            }

            pub(super) fn source_type_alias(
                &mut self,
                reference: TypeReferenceHandle,
            ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
                let projected =
                    type_reference_table(self.program).unconstrained_type_reference(reference);
                self.source_alias_for_carrier(reference, projected)
            }

            fn source_alias_for_carrier(
                &mut self,
                reference: TypeReferenceHandle,
                projected: TypeReferenceHandle,
            ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
                if !matches!(
                    self.program.type_reference_table.type_reference(projected),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Unit
                ) {
                    return Ok(None);
                }
                let Some(carrier) = self.scalar_carrier_of_type_reference(projected) else {
                    return Ok(None);
                };
                if matches!(carrier, ScalarCarrier::Carrier(_)) {
                    return Ok(None);
                }
                if let Some(alias) = self
                    .source_aliases
                    .iter()
                    .find(|alias| self.source_references_correspond(reference, alias.reference))
                {
                    return Ok(Some(self.constant(alias.declaration)));
                }
                let body = self.carrier_term(carrier);
                let ty = self
                    .arena
                    .insert(Term::Sort(Sort::Type(Level::Constant(0))));
                let declaration = self.declarations.len() as u32;
                self.declarations.push(Declaration {
                    level_arity: 0,
                    ty,
                    body: Some(body),
                });
                self.source_aliases.push(SourceAlias {
                    reference,
                    declaration,
                    carrier,
                });
                Ok(Some(self.constant(declaration)))
            }

            pub(super) fn check_source_domains(
                &mut self,
                signature: &Signature,
            ) -> Result<(), Vec<Diagnostic>> {
                let invocations = std::mem::take(&mut self.source_invocations);
                for invocation in invocations {
                    self.span = invocation.span;
                    let context = invocation.domains.iter().fold(
                        Context::with_level_arity(invocation.level_arity)
                            .with_signature(signature.clone()),
                        |context, domain| context.extend(*domain),
                    );
                    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                    let (callee, inputs) = match invocation.formation {
                        SourceFormation::Application { callee, inputs } => (callee, inputs),
                        SourceFormation::Result { expected, actual } => {
                            let domain = match actual.domain {
                                Some(domain) => domain,
                                None => {
                                    infer_type(&mut self.arena, &context, actual.term, &mut budget)
                                        .map_err(|error| {
                                            self.source_domain_error(
                                                invocation.expression,
                                                format!("{error:?}"),
                                            )
                                        })?
                                }
                            };
                            if !self.source_domains_correspond(
                                signature,
                                &context,
                                expected,
                                domain,
                                Some(actual.expression),
                                &mut budget,
                            )? {
                                return Err(self.source_domain_error(
                            invocation.expression,
                            "mathematical result does not preserve its declared carrier and policy"
                                .to_owned(),
                        ));
                            }
                            continue;
                        }
                    };
                    let mut function_type = infer_type(
                        &mut self.arena,
                        &context,
                        callee,
                        &mut budget,
                    )
                    .map_err(|error| {
                        self.source_domain_error(invocation.expression, format!("{error:?}"))
                    })?;
                    for input in inputs {
                        let function_head =
                            self.source_domain_head(signature, function_type, &mut budget)?;
                        let Term::Pi { domain, codomain } = self.arena.get(function_head) else {
                            return Err(self.source_domain_error(
                                invocation.expression,
                                "excess arguments do not apply a remaining mathematical function"
                                    .to_owned(),
                            ));
                        };
                        let actual = match input.domain {
                            Some(domain) => domain,
                            None => infer_type(&mut self.arena, &context, input.term, &mut budget)
                                .map_err(|error| {
                                    self.source_domain_error(
                                        invocation.expression,
                                        format!("{error:?}"),
                                    )
                                })?,
                        };
                        if !self.source_domains_correspond(
                            signature,
                            &context,
                            domain,
                            actual,
                            input.expression.is_valid().then_some(input.expression),
                            &mut budget,
                        )? {
                            return Err(self.source_domain_error(
                        invocation.expression,
                        "mathematical argument does not preserve its declared carrier and policy"
                            .to_owned(),
                    ));
                        }
                        function_type = substitute(&mut self.arena, codomain, input.term);
                    }
                }
                Ok(())
            }

            fn source_domain_head(
                &mut self,
                signature: &Signature,
                term: TermHandle,
                budget: &mut Budget,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let aliases = &self.source_aliases;
                let families = &self.indexed_families;
                weak_head_normalize_retaining(
                    &mut self.arena,
                    signature,
                    term,
                    budget,
                    &|declaration| {
                        aliases.iter().any(|alias| alias.declaration == declaration)
                            || families
                                .values()
                                .any(|family| family.declaration == declaration)
                    },
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "mathematical source-domain reduction failed: {error:?}"
                    ))
                })
            }

            pub(super) fn source_domains_correspond(
                &mut self,
                signature: &Signature,
                context: &Context,
                expected: TermHandle,
                actual: TermHandle,
                expression: Option<ExpressionHandle>,
                budget: &mut Budget,
            ) -> Result<bool, Vec<Diagnostic>> {
                let expected = self.source_domain_head(signature, expected, budget)?;
                if let Some(reference) = self.source_alias_reference(expected)
                    && !self.source_requires_exact_qualification(reference)
                    && let Some(expression) = expression
                    && validation::has_anonymous_numeric_results(self.program, expression)
                {
                    return Ok(validation::checked_argument_matches_type_reference(
                        self.program,
                        expression,
                        reference,
                    ));
                }
                let shared_type =
                    infer_type(&mut self.arena, context, expected, budget).map_err(|error| {
                        self.refuse(format!("mathematical source domain has no type: {error:?}"))
                    })?;
                let aliases = &self.source_aliases;
                let families = &self.indexed_families;
                convertible_retaining(
                    &mut self.arena,
                    context,
                    expected,
                    actual,
                    shared_type,
                    budget,
                    &|declaration| {
                        aliases.iter().any(|alias| alias.declaration == declaration)
                            || families
                                .values()
                                .any(|family| family.declaration == declaration)
                    },
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "mathematical source-domain conversion failed: {error:?}"
                    ))
                })
            }

            fn source_requires_exact_qualification(&self, reference: TypeReferenceHandle) -> bool {
                self.scalar_operand_carrier(reference).is_none()
                    && matches!(
                        type_reference_table(self.program).type_reference(
                            validation::peel_arithmetic_policy(self.program, reference)
                        ),
                        TypeReferenceNode::Constrained { .. }
                    )
            }

            fn source_references_correspond(
                &self,
                left: TypeReferenceHandle,
                right: TypeReferenceHandle,
            ) -> bool {
                if self.source_requires_exact_qualification(left)
                    || self.source_requires_exact_qualification(right)
                {
                    return self
                        .program
                        .type_identity(TypeIdentityRequest::ordinary(left))
                        == self
                            .program
                            .type_identity(TypeIdentityRequest::ordinary(right));
                }
                match (
                    self.program.primitive_type_reference(left),
                    self.program.primitive_type_reference(right),
                ) {
                    (Some(left_carrier), Some(right_carrier)) => {
                        left_carrier == right_carrier
                            && self.program.type_reference_table.arithmetic_domain(left)
                                == self.program.type_reference_table.arithmetic_domain(right)
                    }
                    _ => {
                        self.program
                            .type_identity(TypeIdentityRequest::ordinary(left))
                            == self
                                .program
                                .type_identity(TypeIdentityRequest::ordinary(right))
                    }
                }
            }

            fn source_alias_reference(&self, term: TermHandle) -> Option<TypeReferenceHandle> {
                let Term::Constant { declaration, .. } = self.arena.get(term) else {
                    return None;
                };
                self.source_aliases
                    .iter()
                    .find_map(|alias| (alias.declaration == declaration).then_some(alias.reference))
            }

            fn source_domain_error(
                &self,
                expression: ExpressionHandle,
                reason: String,
            ) -> Vec<Diagnostic> {
                if !expression.is_valid() {
                    return self.refuse(format!(
                        "invalid dependent mathematical application: {reason}"
                    ));
                }
                let mut diagnostics = self.refuse(format!(
                    "invalid mathematical application `{}`: {reason}",
                    self.program.render_proof_expression(
                        expression,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                ));
                for diagnostic in &mut diagnostics {
                    diagnostic.source_span =
                        Some(self.program.expression_table.source_span(expression));
                }
                diagnostics
            }
        }
    }
    mod source_evidence {
        use super::{
            Budget, Context, DEFAULT_CONVERSION_STEPS, Elaborator, TermHandle, TypeParameterKind,
            TypeReferenceNode,
        };
        use crate::typed_expressions::{expression_table, statement_table, type_reference_table};
        use diagnostics::Diagnostic;
        use proof_admission::{
            Signature, Sort, Term, check_type, infer_sort, infer_type, judgment_assumption_closure,
            substitute,
        };
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::statement::StatementNode;
        use typed_trees::static_applications::StaticMachineBindingValue;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
        use validation::expression_owners::{OwnerEnvironmentIndex, source_declaration_owner};
        use validation::mathematical_applications::PreparedIndexedTheorem;
        use validation::mathematical_applications::{
            PreparedSourceCarrier, PreparedSourceEvidence,
        };

        impl Elaborator<'_> {
            fn source_carrier_domains(&mut self, ty: TermHandle) -> Vec<TermHandle> {
                let domains = self
                    .scope
                    .iter()
                    .map(|entry| entry.domain)
                    .collect::<Vec<_>>();
                if domains.is_empty() {
                    return domains;
                }
                let context = Context::with_level_arity(
                    self.level_binders.len() as u32 + self.generalized_levels,
                )
                .with_signature(Signature::from_declarations(self.declarations.clone()));
                if infer_sort(
                    &mut self.arena,
                    &context,
                    ty,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .is_ok()
                {
                    Vec::new()
                } else {
                    domains
                }
            }

            fn known_source_family(&self, reference: TypeReferenceHandle) -> bool {
                matches!(self.program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Generic { base_symbol, .. }
                if self.indexed_families.contains_key(base_symbol))
            }

            fn source_carrier_has_runtime_types(&self, reference: TypeReferenceHandle) -> bool {
                let mut pending = vec![reference];
                let mut seen = std::collections::HashSet::new();
                while let Some(reference) = pending.pop() {
                    if !seen.insert(reference) {
                        continue;
                    }
                    match type_reference_table(self.program).type_reference(reference) {
                        TypeReferenceNode::Reference { .. }
                        | TypeReferenceNode::DynamicTrait { .. }
                        | TypeReferenceNode::FixedArray { .. }
                        | TypeReferenceNode::Slice { .. } => return true,
                        TypeReferenceNode::Generic {
                            lifetime_arguments,
                            arguments,
                            ..
                        } => {
                            if !lifetime_arguments.is_empty() {
                                return true;
                            }
                            pending.extend_from_slice(
                                type_reference_table(self.program)
                                    .type_reference_handles(*arguments),
                            );
                        }
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            pending.push(*base_type);
                        }
                        _ => {}
                    }
                }
                false
            }

            pub(super) fn retain_source_evidence_carriers(
                &mut self,
            ) -> Result<(), Vec<Diagnostic>> {
                if self.indexed_families.is_empty() {
                    return Ok(());
                }
                let mut families = self.indexed_families.values().cloned().collect::<Vec<_>>();
                families.sort_by_key(|family| family.symbol.arena_index());
                for family in families {
                    let saved_levels = self.enter_family_levels(&family.parameters);
                    for case in &family.cases {
                        self.scope.clear();
                        self.push_family_type_scope(&family.parameters);
                        for field in family.common.iter().chain(&case.payload) {
                            let domain = self.elaborate_type_reference(field.type_reference)?;
                            self.push_scope(Some(field.symbol), field.name.clone(), domain);
                        }
                        for field in &case.children {
                            let reference = field.type_reference;
                            let ty = self.elaborate_type_reference(reference)?;
                            let domains = self.source_carrier_domains(ty);
                            self.prepared.source_carriers.append(PreparedSourceCarrier {
                                level_arity: proof_admission::universe_parameter_arity(
                                    &self.arena,
                                    &domains
                                        .iter()
                                        .copied()
                                        .chain(std::iter::once(ty))
                                        .collect::<Vec<_>>(),
                                ),
                                source_type: reference,
                                type_identity: self.program.normalized_type_identity(reference),
                                ty,
                                domains,
                            });
                        }
                    }
                    self.restore_source_levels(saved_levels);
                }
                let machines = self.program.machines().to_vec();
                for machine in &machines {
                    self.scope.clear();
                    let states = self
                        .program
                        .machine_states(machine)
                        .iter()
                        .filter_map(|state| {
                            let references = self
                                .program
                                .state_parameters(state)
                                .iter()
                                .map(|parameter| {
                                    (parameter.type_reference, parameter.relevance.is_erased())
                                })
                                .chain(
                                    statement_table(self.program)
                                        .statements(state.statement_nodes)
                                        .iter()
                                        .filter_map(|statement| {
                                            if let StatementNode::LocalData(local) = statement {
                                                Some((
                                                    local.type_reference,
                                                    local.relevance.is_erased(),
                                                ))
                                            } else {
                                                None
                                            }
                                        }),
                                )
                                .collect::<Vec<_>>();
                            references
                                .iter()
                                .any(|(reference, _)| self.known_source_family(*reference))
                                .then_some((state, references))
                        })
                        .collect::<Vec<_>>();
                    if states.is_empty() {
                        continue;
                    }
                    self.source_owner = machine.symbol;
                    self.span = self.program.symbols.symbol_source_span(machine.symbol);
                    let binders = self.program.machine_type_parameters(machine).to_vec();
                    if binders.iter().any(|binder| {
                        !matches!(
                            binder.kind,
                            TypeParameterKind::Type | TypeParameterKind::Value { .. }
                        )
                    }) {
                        continue;
                    }
                    let saved_levels = self.enter_family_levels(&binders);
                    for binder in &binders {
                        let domain = match binder.kind {
                            TypeParameterKind::Type => {
                                let level = self.source_type_level(binder.symbol);
                                self.arena.insert(Term::Sort(Sort::Type(level)))
                            }
                            TypeParameterKind::Value { type_reference } => {
                                self.elaborate_type_reference(type_reference)?
                            }
                            _ => unreachable!(),
                        };
                        self.push_scope(Some(binder.symbol), binder.name.clone(), domain);
                    }
                    let machine_depth = self.scope.len();
                    for (state, references) in states {
                        self.scope.truncate(machine_depth);
                        for parameter in self.program.state_parameters(state) {
                            if self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some_and(|primitive| {
                                    primitive == PrimitiveType::Bool
                                        || primitive.integer_bounds().is_some()
                                })
                            {
                                let domain =
                                    self.elaborate_source_operand_type(parameter.type_reference)?;
                                let domains = self.source_carrier_domains(domain);
                                self.prepared.source_carriers.append(PreparedSourceCarrier {
                                    level_arity: proof_admission::universe_parameter_arity(
                                        &self.arena,
                                        &domains
                                            .iter()
                                            .copied()
                                            .chain(std::iter::once(domain))
                                            .collect::<Vec<_>>(),
                                    ),
                                    source_type: parameter.type_reference,
                                    type_identity: self
                                        .program
                                        .normalized_type_identity(parameter.type_reference),
                                    ty: domain,
                                    domains,
                                });
                                self.push_scope(
                                    Some(parameter.symbol),
                                    parameter.name.clone(),
                                    domain,
                                );
                            }
                        }
                        for (reference, erased) in references {
                            if !self.known_source_family(reference)
                                // A runtime application need not have a mathematical
                                // value domain just because another application of
                                // its generic declaration has one. Erased inputs and
                                // directly demanded mathematical types still check.
                                || (!erased && self.source_carrier_has_runtime_types(reference))
                                || self
                                    .prepared
                                    .source_carriers
                                    .iter()
                                    .any(|(_, carrier)| carrier.source_type == reference)
                            {
                                continue;
                            }
                            let ty = self.elaborate_type_reference(reference)?;
                            let domains = self.source_carrier_domains(ty);
                            self.prepared.source_carriers.append(PreparedSourceCarrier {
                                level_arity: proof_admission::universe_parameter_arity(
                                    &self.arena,
                                    &domains
                                        .iter()
                                        .copied()
                                        .chain(std::iter::once(ty))
                                        .collect::<Vec<_>>(),
                                ),
                                source_type: reference,
                                type_identity: self.program.normalized_type_identity(reference),
                                ty,
                                domains,
                            });
                        }
                        let constructors = statement_table(self.program)
                            .statements(state.statement_nodes)
                            .iter()
                            .filter_map(|statement| match statement {
                                StatementNode::LocalData(local)
                                    if local.relevance
                                        == language_core::BindingRelevance::Erased
                                        && self.known_source_family(local.type_reference)
                                        && matches!(
                                            expression_table(self.program)
                                                .expression(local.initial_value),
                                            ExpressionNode::StructLiteral(_)
                                        ) =>
                                {
                                    Some((local.initial_value, local.type_reference))
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        for (expression, reference) in constructors {
                            if self.scope.is_empty()
                                && !self
                                    .program
                                    .state_parameters(state)
                                    .iter()
                                    .any(|parameter| {
                                        parameter.relevance.is_erased()
                                            && self.known_source_family(parameter.type_reference)
                                    })
                            {
                                self.source_family_constructor(expression, reference, 0)?;
                            } else {
                                self.retain_source_constructor_schema(
                                    expression, reference, machine, state,
                                )?;
                            }
                        }
                    }
                    self.restore_source_levels(saved_levels);
                }
                self.scope.clear();
                Ok(())
            }

            pub(super) fn retain_checked_source_application(
                &mut self,
                expression: ExpressionHandle,
                owner: SymbolHandle,
                term: TermHandle,
                context: &Context,
                signature: &Signature,
            ) -> Result<(), Vec<Diagnostic>> {
                let Some(reference) =
                    validation::mathematical_applications::operand_type_reference_in_declaration(
                        self.program,
                        owner,
                        expression,
                    )
                else {
                    return Ok(());
                };
                if !self.known_source_family(reference) || !self.scope.is_empty() {
                    return Ok(());
                }
                let ty = infer_type(
                    &mut self.arena,
                    context,
                    term,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "source mathematical initializer lost its checked type: {error:?}"
                    ))
                })?;
                let carriers = self
                    .prepared
                    .source_carriers
                    .iter()
                    .filter_map(|(_, carrier)| {
                        (carrier.domains.is_empty() && carrier.level_arity == 0).then_some((
                            carrier.source_type,
                            carrier.type_identity,
                            carrier.ty,
                        ))
                    })
                    .collect::<Vec<_>>();
                let mut selected = None;
                for (source_type, identity, expected) in carriers {
                    if self.source_domains_correspond(
                        signature,
                        context,
                        expected,
                        ty,
                        None,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )? {
                        if selected.is_some_and(|(_, prior)| prior != identity) {
                            return Err(self.refuse(
                                "mathematical initializer has ambiguous source carrier correspondence"
                                    .into(),
                            ));
                        }
                        selected = Some((source_type, identity));
                    }
                }
                let Some((source_type, type_identity)) = selected else {
                    return Err(self.refuse(
                        "mathematical initializer has no exact formed source carrier correspondence"
                            .into(),
                    ));
                };
                self.prepared
                    .source_evidence
                    .append(PreparedSourceEvidence {
                        expression,
                        source_type,
                        type_identity,
                        term,
                        ty,
                    });
                Ok(())
            }
        }

        impl Elaborator<'_> {
            pub(super) fn retain_indexed_contract_theorems(
                &mut self,
            ) -> Result<(), Vec<Diagnostic>> {
                use proof_admission::{Signature, Term, judgment_assumption_closure};
                use typed_trees::domain::ProofFact;
                use typed_trees::signature::SignatureContractKind;
                if self.indexed_families.is_empty() {
                    return Ok(());
                }
                let machines = self.program.machines().to_vec();
                for machine in &machines {
                    let Some(entry) = self.program.machine_states(machine).first() else {
                        continue;
                    };
                    let evidence = self
                        .program
                        .state_parameters(entry)
                        .iter()
                        .filter(|parameter| {
                            parameter.relevance == language_core::BindingRelevance::Erased
                                && self.known_source_family(parameter.type_reference)
                        })
                        .collect::<Vec<_>>();
                    let [evidence] = evidence.as_slice() else {
                        continue;
                    };
                    if !validation::machine_has_unit_result(self.program, machine) {
                        continue;
                    }
                    let reference = evidence.type_reference;
                    let TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } = self.program.type_reference_table.type_reference(reference)
                    else {
                        continue;
                    };
                    let symbol = *base_symbol;
                    let arguments = type_reference_table(self.program)
                        .type_reference_handles(*arguments)
                        .to_vec();
                    let family = self.source_family(symbol)?;
                    let binders = self.program.machine_type_parameters(machine).to_vec();
                    if binders.iter().any(|binder| {
                        !matches!(
                            binder.kind,
                            TypeParameterKind::Type | TypeParameterKind::Value { .. }
                        )
                    }) {
                        continue;
                    }
                    let mut telescope = binders
                        .iter()
                        .map(|binder| {
                            let reference = match binder.kind {
                                TypeParameterKind::Type => None,
                                TypeParameterKind::Value { type_reference } => Some(type_reference),
                                _ => unreachable!(),
                            };
                            (binder.symbol, binder.name.clone(), reference)
                        })
                        .collect::<Vec<_>>();
                    for parameter in self.program.state_parameters(entry) {
                        if parameter.is_mutable
                    || parameter.is_const
                    || telescope
                        .iter()
                        .any(|(symbol, _, _)| *symbol == parameter.symbol)
                    || !arguments.iter().any(|argument| {
                        matches!(
                            self.program.type_reference_table.type_reference(*argument),
                            TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol
                        )
                    })
                    || !self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some_and(|primitive| {
                            primitive == PrimitiveType::Bool || primitive.integer_bounds().is_some()
                        })
                {
                    continue;
                }
                        telescope.push((
                            parameter.symbol,
                            parameter.name.clone(),
                            Some(parameter.type_reference),
                        ));
                    }
                    let specializations = self
                        .program
                        .machine_specializations
                        .iter()
                        .filter(|row| row.instance == machine.symbol)
                        .collect::<Vec<_>>();
                    if let [specialization] = specializations.as_slice()
                        && self.program.executable_application_template(machine.symbol)
                            == Ok(Some(specialization.template))
                        && let Some(template) =
                            self.program.machine_by_symbol(specialization.template)
                    {
                        for parameter in self.program.state_parameters(entry) {
                            if parameter.is_mutable
                        || parameter.is_const
                        || !arguments.iter().any(|argument| {
                            matches!(self.program.type_reference_table.type_reference(*argument),
                                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
                        })
                        || !self.program.primitive_type_reference(parameter.type_reference)
                            .is_some_and(|primitive| {
                                primitive == PrimitiveType::Bool
                                    || primitive.integer_bounds().is_some()
                            })
                    {
                        continue;
                    }
                            let declaration = self.program.symbols.get(parameter.symbol);
                            let Some(binder) = self
                                .program
                                .machine_type_parameters(template)
                                .iter()
                                .find(|binder| binder.symbol == declaration.generated_from)
                            else {
                                continue;
                            };
                            let TypeParameterKind::Value { type_reference } = binder.kind else {
                                continue;
                            };
                            let Some(StaticMachineBindingValue::RuntimeValue {
                                type_reference: realized_carrier,
                                ..
                            }) = self
                                .program
                                .specialization_binding(specialization, binder.symbol)
                            else {
                                continue;
                            };
                            let identity = self.program.normalized_type_identity(type_reference);
                            if declaration.parent != entry.symbol
                        || !identity.is_valid()
                        || identity
                            != self
                                .program
                                .normalized_type_identity(parameter.type_reference)
                        || identity != self.program.normalized_type_identity(*realized_carrier)
                        || arguments.iter().zip(&family.parameters).any(|(argument, index)| {
                            if !matches!(self.program.type_reference_table.type_reference(*argument),
                                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
                            {
                                return false;
                            }
                            let TypeParameterKind::Value { type_reference } = index.kind else {
                                return true;
                            };
                            identity != self.program.normalized_type_identity(type_reference)
                        })
                        || telescope
                            .iter()
                            .any(|(symbol, _, _)| *symbol == parameter.symbol)
                    {
                        continue;
                    }
                            telescope.push((
                                parameter.symbol,
                                parameter.name.clone(),
                                Some(parameter.type_reference),
                            ));
                        }
                    }
                    let outer_types = if family.external_parameter_indices.is_empty() {
                        Vec::new()
                    } else {
                        telescope
                            .iter()
                            .filter(|(_, _, reference)| reference.is_none())
                            .map(|(symbol, _, _)| *symbol)
                            .collect::<Vec<_>>()
                    };
                    if !family.external_parameter_indices.is_empty() {
                        // Uniform declaration Types stay arbitrary outer carriers,
                        // not changing value indices of the induction invariant.
                        telescope.retain(|(_, _, reference)| reference.is_some());
                    }
                    if telescope.len() > family.index_parameter_indices.len() {
                        continue;
                    }
                    let saved_constructor_context =
                        std::mem::take(&mut self.source_constructor_context);
                    let mut outer_parameters = Vec::new();
                    let mut lifted_declarations =
                        super::source_families::ParameterAbstractions::default();
                    let saved_levels = self.enter_family_levels(&binders);
                    for symbol in outer_types {
                        let level = self.source_type_level(symbol);
                        let domain = self.arena.insert(Term::Sort(Sort::Type(level)));
                        let position = self.declarations.len() as u32;
                        self.declarations
                            .push(proof_admission::Declaration::assumption(
                                proof_admission::universe_parameter_arity(&self.arena, &[domain]),
                                domain,
                            ));
                        let term = self.constant(position);
                        self.source_constructor_context.values.push((symbol, term));
                        outer_parameters.push((position, symbol, domain));
                    }
                    self.source_owner = machine.symbol;
                    self.scope.clear();
                    self.span = self.program.symbols.symbol_source_span(machine.symbol);
                    for (symbol, name, type_reference) in &telescope {
                        let domain = if let Some(reference) = type_reference {
                            self.elaborate_type_reference(*reference)?
                        } else {
                            {
                                let level = self.source_type_level(*symbol);
                                self.arena.insert(Term::Sort(Sort::Type(level)))
                            }
                        };
                        self.push_scope(Some(*symbol), name.clone(), domain);
                    }
                    let actuals = arguments
                        .iter()
                        .zip(&family.parameters)
                        .map(|(argument, parameter)| self.index_operand(*argument, parameter, &[]))
                        .collect::<Result<Vec<_>, _>>()?;
                    let family = self.applied_source_family(&family, &actuals)?;
                    let description = family
                        .description_at(&mut self.arena, &actuals)
                        .map_err(|reason| self.refuse(reason.into()))?;
                    let index_actuals = family.index_actuals(&actuals);
                    let mut mapping = Vec::new();
                    for ordinal in 0..telescope.len() {
                        let variable = self.scope_variable(ordinal);
                        let positions = index_actuals
                            .iter()
                            .enumerate()
                            .filter_map(|(position, &actual)| {
                                self.arena
                                    .structurally_equal(variable, actual)
                                    .then_some(position)
                            })
                            .collect::<Vec<_>>();
                        let [position] = positions.as_slice() else {
                            mapping.clear();
                            break;
                        };
                        mapping.push(*position);
                    }
                    if mapping.len() != telescope.len() {
                        self.scope.clear();
                        self.source_constructor_context = saved_constructor_context;
                        self.restore_source_levels(saved_levels);
                        continue;
                    }
                    let expressions = self
                        .program
                        .machine_contracts(machine)
                        .iter()
                        .filter(|contract| matches!(contract.kind, SignatureContractKind::Ensures))
                        .flat_map(|contract| {
                            self.program
                                .proof_facts
                                .span_or_empty(contract.facts)
                                .iter()
                        })
                        .filter_map(|fact| match fact {
                            ProofFact::Expression(expression) => Some(*expression),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    for (ensure_ordinal, expression) in expressions.into_iter().enumerate() {
                        let Some(mut proposition) = self.elaborate_proposition(expression)? else {
                            continue;
                        };
                        for entry in self.scope.iter().rev() {
                            proposition = self.arena.insert(Term::Lambda {
                                domain: entry.domain,
                                body: proposition,
                            });
                        }
                        for &position in &mapping {
                            let mut projection = self.arena.insert(Term::Variable(0));
                            for _ in 0..position {
                                projection = self.arena.insert(Term::Snd { pair: projection });
                            }
                            if position + 1 < family.index_parameter_indices.len() {
                                projection = self.arena.insert(Term::Fst { pair: projection });
                            }
                            let Term::Lambda { body, .. } = self.arena.get(proposition) else {
                                unreachable!("constructed source invariant telescope")
                            };
                            proposition = substitute(&mut self.arena, body, projection);
                        }
                        let invariant = self.arena.insert(Term::Lambda {
                            domain: description.index,
                            body: proposition,
                        });
                        self.scope.clear();
                        let mut theorem =
                            self.source_family_induction(symbol, invariant, &actuals)?;
                        (theorem.theorem, theorem.ty) =
                            super::source_families::abstract_parameter_closure(
                                &mut self.arena,
                                &mut self.declarations,
                                theorem.theorem,
                                theorem.ty,
                                &outer_parameters,
                                &mut lifted_declarations,
                            )
                            .map_err(|reason| self.refuse(reason.into()))?;
                        check_type(
                            &mut self.arena,
                            &Context::with_level_arity(
                                self.level_binders.len() as u32 + self.generalized_levels,
                            )
                            .with_signature(
                                Signature::from_declarations(self.declarations.clone()),
                            ),
                            theorem.theorem,
                            theorem.ty,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        )
                        .map_err(|error| {
                            self.refuse(format!(
                                "abstracted source Type theorem fails kernel checking: {error:?}"
                            ))
                        })?;
                        let signature = Signature::from_declarations(self.declarations.clone());
                        let closure = judgment_assumption_closure(
                            &self.arena,
                            &signature,
                            &[theorem.theorem, theorem.ty],
                        );
                        self.prepared
                            .indexed_theorems
                            .append(PreparedIndexedTheorem {
                                level_arity: self.level_binders.len() as u32
                                    + self.generalized_levels,
                                machine: machine.symbol,
                                expression,
                                ensure_ordinal: ensure_ordinal as u32,
                                evidence_formal: evidence.symbol,
                                source_type: reference,
                                theorem: theorem.theorem,
                                ty: theorem.ty,
                                assumption_closure: closure.into_iter().collect(),
                            });
                        for (symbol, name, type_reference) in &telescope {
                            let domain = if let Some(reference) = type_reference {
                                self.elaborate_type_reference(*reference)?
                            } else {
                                {
                                    let level = self.source_type_level(*symbol);
                                    self.arena.insert(Term::Sort(Sort::Type(level)))
                                }
                            };
                            self.push_scope(Some(*symbol), name.clone(), domain);
                        }
                    }
                    self.source_constructor_context = saved_constructor_context;
                    self.restore_source_levels(saved_levels);
                }
                self.scope.clear();
                Ok(())
            }
        }

        impl Elaborator<'_> {
            pub(super) fn bind_source_evidence_types(
                &mut self,
                signature: &Signature,
            ) -> Result<(), Vec<Diagnostic>> {
                let accepted = self
                    .integer_theory
                    .as_ref()
                    .map(|theory| theory.source_safety_roster())
                    .unwrap_or_default();
                for (_, theorem) in self.prepared.indexed_theorems.iter() {
                    if theorem
                        .assumption_closure
                        .iter()
                        .any(|position| !accepted.contains(position))
                    {
                        return Err(self.refuse("source indexed theorem depends on an assumption outside the audited integer safety roster".into()));
                    }
                }
                self.prepared.safety_assumptions = accepted;
                let rejected_constructor =
                    self.prepared
                        .source_constructors
                        .iter()
                        .find_map(|(_, constructor)| {
                            let closure = judgment_assumption_closure(
                                &self.arena,
                                signature,
                                &[constructor.schema, constructor.ty],
                            );
                            let ungranted = closure
                                .into_iter()
                                .filter(|position| {
                                    !self.prepared.safety_assumptions.contains(position)
                                })
                                .collect::<Vec<_>>();
                            (!ungranted.is_empty()).then_some((constructor.expression, ungranted))
                        });
                if let Some((expression, ungranted)) = rejected_constructor {
                    self.span = Some(self.program.expression_table.source_span(expression));
                    return Err(self.refuse(format!("source constructor schema depends on an assumption outside the audited integer safety roster: {ungranted:?}")));
                }
                let rows = self
                    .prepared
                    .source_evidence
                    .iter()
                    .map(|(handle, row)| (handle, row.type_identity, row.term))
                    .collect::<Vec<_>>();
                for (handle, identity, term) in rows {
                    let Some(carrier) =
                        self.prepared
                            .source_carriers
                            .iter()
                            .find_map(|(_, carrier)| {
                                (identity.is_valid()
                                    && carrier.type_identity == identity
                                    && carrier.domains.is_empty()
                                    && carrier.level_arity == 0)
                                    .then_some(carrier.ty)
                            })
                    else {
                        return Err(self.refuse(
                            "source evidence has no independently formed closed source carrier"
                                .into(),
                        ));
                    };
                    check_type(
                        &mut self.arena,
                        &Context::empty().with_signature(signature.clone()),
                        term,
                        carrier,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "source evidence changed its independently formed carrier: {error:?}"
                        ))
                    })?;
                    let closure =
                        judgment_assumption_closure(&self.arena, signature, &[term, carrier]);
                    if closure
                        .iter()
                        .any(|position| !self.prepared.safety_assumptions.contains(position))
                    {
                        return Err(self.refuse("source indexed initializer depends on an assumption outside the audited integer safety roster".into()));
                    }
                    self.prepared.source_evidence.get_mut(handle).ty = carrier;
                }
                Ok(())
            }
        }

        impl Elaborator<'_> {
            pub(super) fn retain_source_evidence_aliases(&mut self) -> Result<(), Vec<Diagnostic>> {
                if self.prepared.source_evidence.is_empty()
                    && self.prepared.source_constructors.is_empty()
                {
                    return Ok(());
                }
                let owners = OwnerEnvironmentIndex::new(self.program, &arena::Arena::default());
                let mut pending = expression_table(self.program)
                    .iter_expressions()
                    .filter_map(|(expression, node)| {
                        let ExpressionNode::Name(path) = node else {
                            return None;
                        };
                        Some((expression, path.resolved_symbol()?))
                    })
                    .collect::<Vec<_>>();
                for _ in 0..128 {
                    let mut progress = false;
                    pending.retain(|(expression, symbol)| {
                        if self
                            .prepared
                            .source_evidence
                            .iter()
                            .any(|(_, row)| row.expression == *expression)
                            || self
                                .prepared
                                .source_constructors
                                .iter()
                                .any(|(_, row)| row.expression == *expression)
                        {
                            return false;
                        }
                        let locals =
                            self.program
                                .machines()
                                .iter()
                                .flat_map(|machine| {
                                    self.program.machine_states(machine).iter().filter_map(
                                        |state| {
                                            self.program
                                                .state_local_declaration(state, *symbol)
                                                .map(|(ordinal, local)| {
                                                    (machine.symbol, state.symbol, ordinal, local)
                                                })
                                        },
                                    )
                                })
                                .collect::<Vec<_>>();
                        let [(machine, state, ordinal, local)] = locals.as_slice() else {
                            return false;
                        };
                        if source_declaration_owner(self.program, *expression, &owners)
                            != Some(*machine)
                        {
                            return false;
                        }
                        let sites = owners.containing_statement_sites(*expression);
                        if sites.is_empty()
                            || sites.iter().any(|(m, s, statement)| {
                                let owner = &self.program.machines()[*m];
                                owner.symbol != *machine
                                    || self.program.machine_states(owner)[*s].symbol != *state
                                    || *statement <= *ordinal
                            })
                        {
                            return false;
                        }
                        if local.is_mutable
                            || local.relevance != language_core::BindingRelevance::Erased
                        {
                            return false;
                        }
                        let identity = self.program.normalized_type_identity(local.type_reference);
                        if let Some(mut constructor) = self
                            .prepared
                            .source_constructors
                            .iter()
                            .find_map(|(_, row)| {
                                (row.expression == local.initial_value
                                    && row.machine == *machine
                                    && row.state == *state
                                    && identity.is_valid()
                                    && row.type_identity == identity)
                                    .then_some(row.clone())
                            })
                        {
                            constructor.expression = *expression;
                            self.prepared.source_constructors.append(constructor);
                            progress = true;
                            return false;
                        }
                        let Some((term, ty)) =
                            self.prepared.source_evidence.iter().find_map(|(_, row)| {
                                (row.expression == local.initial_value
                                    && identity.is_valid()
                                    && row.type_identity == identity)
                                    .then_some((row.term, row.ty))
                            })
                        else {
                            return true;
                        };
                        self.prepared
                            .source_evidence
                            .append(PreparedSourceEvidence {
                                expression: *expression,
                                source_type: local.type_reference,
                                type_identity: identity,
                                term,
                                ty,
                            });
                        progress = true;
                        false
                    });
                    if !progress {
                        break;
                    }
                }
                Ok(())
            }
        }
    }
    mod source_families {
        /*
        Source descriptions for the checked indexed W scheme. Mathematical
        encodings do not replace source nominal identity, field custody or native
        layout: source-domain aliases keep the declaration head opaque to those
        checks while the kernel checks its defined W body.

        Every constructor label stores its common fields, exact active payload and
        complete predicates. Recursive fields become ordered child positions whose
        next indices are closed functions of that same source telescope. This file
        does not turn legacy constructor-success verdicts into evidence: actual
        constructor applications additionally need their retained derivations.

        Uniform declaration parameters stay outside the W tree. Nonuniform Type
        arguments are genuine family indices and retain their larger index sort.
        The checked description keeps parameter, index and payload roles distinct.
        */

        macro_rules! term {
            (Pi => $domain:expr => $codomain:expr) => {
                Term::Pi {
                    domain: $domain,
                    codomain: $codomain,
                }
            };
            (Sigma => $domain:expr => $codomain:expr) => {
                Term::Sigma {
                    domain: $domain,
                    codomain: $codomain,
                }
            };
            (Lambda => $domain:expr => $body:expr) => {
                Term::Lambda {
                    domain: $domain,
                    body: $body,
                }
            };
            (Apply => $function:expr => $argument:expr) => {
                Term::Apply {
                    function: $function,
                    argument: $argument,
                }
            };
            (Constant => $declaration:expr => $levels:expr) => {
                Term::Constant {
                    declaration: $declaration,
                    levels: $levels,
                }
            };
            (Pair => $first:expr => $second:expr) => {
                Term::Pair {
                    first: $first,
                    second: $second,
                }
            };
            (Refl => $ty:expr => $value:expr) => {
                Term::Refl {
                    ty: $ty,
                    value: $value,
                }
            };
        }

        mod parameter_closure {
            /*
            Generated theorem definitions bind their source parameters so temporary
            assumptions cannot escape. Parameterized copies preserve the immutable
            signature prefix and reuse shared definitions; dependent assumptions remain
            ungranted.
            */

            use proof_admission::{Declaration, Term, TermArena, TermHandle};
            use symbols::SymbolHandle;

            /// One immutable source telescope's copies in its generated declaration
            /// fragment. Coordinates before this base cannot depend on the telescope.
            #[derive(Default)]
            pub(in super::super) struct ParameterAbstractions {
                parameters: Vec<(u32, TermHandle)>,
                base: usize,
                copies: Vec<Option<u32>>,
            }
            impl ParameterAbstractions {
                fn get(&self, position: u32) -> Option<u32> {
                    (position as usize)
                        .checked_sub(self.base)
                        .and_then(|position| self.copies.get(position))
                        .copied()
                        .flatten()
                }
            }

            /// Abstract temporary parameter assumptions without capturing existing binders.
            pub(super) fn abstract_parameters(
                arena: &mut TermArena,
                proof: TermHandle,
                ty: TermHandle,
                parameters: &[(u32, SymbolHandle, TermHandle)],
            ) -> Result<(TermHandle, TermHandle), &'static str> {
                abstract_parameters_with_declarations(
                    arena,
                    proof,
                    ty,
                    parameters,
                    &ParameterAbstractions::default(),
                )
            }

            fn abstract_parameters_with_declarations(
                arena: &mut TermArena,
                proof: TermHandle,
                ty: TermHandle,
                parameters: &[(u32, SymbolHandle, TermHandle)],
                lifted: &ParameterAbstractions,
            ) -> Result<(TermHandle, TermHandle), &'static str> {
                fn go(
                    arena: &mut TermArena,
                    term: TermHandle,
                    parameters: &[(u32, SymbolHandle, TermHandle)],
                    lifted: &ParameterAbstractions,
                    depth: u32,
                ) -> Result<TermHandle, &'static str> {
                    macro_rules! at {
                        ($value:expr) => {
                            go(arena, $value, parameters, lifted, depth)?
                        };
                    }
                    macro_rules! under {
                        ($value:expr) => {
                            go(arena, $value, parameters, lifted, depth + 1)?
                        };
                    }
                    let node = match arena.get(term) {
                        Term::Constant {
                            declaration,
                            levels,
                        } => {
                            if let Some(ordinal) = parameters
                                .iter()
                                .position(|(position, _, _)| *position == declaration)
                            {
                                let arity = proof_admission::universe_parameter_arity(
                                    arena,
                                    &[parameters[ordinal].2],
                                );
                                let expected = (0..arity)
                                    .map(proof_admission::Level::Parameter)
                                    .collect::<Vec<_>>();
                                if levels != expected {
                                    return Err(
                                        "source value placeholder changed its exact universe application",
                                    );
                                }
                                Term::Variable(depth + (parameters.len() - ordinal - 1) as u32)
                            } else if let Some(replacement) = lifted.get(declaration) {
                                let mut term = arena.insert(Term::Constant {
                                    declaration: replacement,
                                    levels,
                                });
                                for ordinal in 0..parameters.len() {
                                    let argument = arena.insert(Term::Variable(
                                        depth + (parameters.len() - ordinal - 1) as u32,
                                    ));
                                    term = arena.insert(Term::Apply {
                                        function: term,
                                        argument,
                                    });
                                }
                                return Ok(term);
                            } else {
                                return Ok(term);
                            }
                        }
                        Term::Variable(index) => {
                            if index >= depth {
                                return Err(
                                    "source case proof contains an unbound value before abstraction",
                                );
                            }
                            return Ok(term);
                        }
                        Term::Pi { domain, codomain } => {
                            term!(Pi => at!(domain) => under!(codomain))
                        }
                        Term::Lambda { domain, body } => {
                            term!(Lambda => at!(domain) => under!(body))
                        }
                        Term::Sigma { domain, codomain } => {
                            term!(Sigma => at!(domain) => under!(codomain))
                        }
                        Term::Apply { function, argument } => {
                            term!(Apply => at!(function) => at!(argument))
                        }
                        Term::Pair { first, second } => term!(Pair => at!(first) => at!(second)),
                        Term::Fst { pair } => Term::Fst { pair: at!(pair) },
                        Term::Snd { pair } => Term::Snd { pair: at!(pair) },
                        Term::Id { ty, left, right } => Term::Id {
                            ty: at!(ty),
                            left: at!(left),
                            right: at!(right),
                        },
                        Term::Refl { ty, value } => term!(Refl => at!(ty) => at!(value)),
                        Term::IdElim {
                            motive,
                            base,
                            endpoint,
                            proof,
                        } => Term::IdElim {
                            motive: at!(motive),
                            base: at!(base),
                            endpoint: at!(endpoint),
                            proof: at!(proof),
                        },
                        Term::CaseTwo {
                            motive,
                            zero_branch,
                            one_branch,
                            scrutinee,
                        } => Term::CaseTwo {
                            motive: at!(motive),
                            zero_branch: at!(zero_branch),
                            one_branch: at!(one_branch),
                            scrutinee: at!(scrutinee),
                        },
                        Term::Box { ty } => Term::Box { ty: at!(ty) },
                        Term::BoxIntro { ty, value } => Term::BoxIntro {
                            ty: at!(ty),
                            value: at!(value),
                        },
                        Term::BoxElim {
                            motive,
                            body,
                            scrutinee,
                        } => Term::BoxElim {
                            motive: at!(motive),
                            body: at!(body),
                            scrutinee: at!(scrutinee),
                        },
                        Term::EmptyElim { ty, scrutinee } => Term::EmptyElim {
                            ty: at!(ty),
                            scrutinee: at!(scrutinee),
                        },
                        Term::W { carrier, children } => Term::W {
                            carrier: at!(carrier),
                            children: at!(children),
                        },
                        Term::Sup {
                            carrier,
                            children,
                            label,
                            function,
                        } => Term::Sup {
                            carrier: at!(carrier),
                            children: at!(children),
                            label: at!(label),
                            function: at!(function),
                        },
                        Term::IndW { motive, step, tree } => Term::IndW {
                            motive: at!(motive),
                            step: at!(step),
                            tree: at!(tree),
                        },
                        Term::Squash { ty } => Term::Squash { ty: at!(ty) },
                        Term::SquashIntro { ty, value } => Term::SquashIntro {
                            ty: at!(ty),
                            value: at!(value),
                        },
                        Term::SquashElim {
                            proposition,
                            function,
                            scrutinee,
                        } => Term::SquashElim {
                            proposition: at!(proposition),
                            function: at!(function),
                            scrutinee: at!(scrutinee),
                        },
                        Term::Dummy => return Err("source case proof contains a dummy term"),
                        Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne | Term::Empty => {
                            return Ok(term);
                        }
                    };
                    Ok(arena.insert(node))
                }
                let mut body = go(arena, proof, parameters, lifted, 0)?;
                let mut statement = go(arena, ty, parameters, lifted, 0)?;
                for (ordinal, &(_, _, domain)) in parameters.iter().enumerate().rev() {
                    // A child/premise domain may depend on earlier value parameters. Its
                    // binder is formed under that prefix, not under the full body scope.
                    let domain = go(arena, domain, &parameters[..ordinal], lifted, 0)?;
                    body = arena.insert(Term::Lambda { domain, body });
                    statement = arena.insert(term!(Pi => domain => statement));
                }
                Ok((body, statement))
            }

            pub(super) fn excludes_parameters(
                arena: &TermArena,
                declarations: &[Declaration],
                roots: &[TermHandle],
                parameters: &[(u32, SymbolHandle, TermHandle)],
            ) -> bool {
                let mut seen_terms = Vec::new();
                let Some(references) = collect_constant_references(arena, roots, &mut seen_terms)
                else {
                    return false;
                };
                let mut pending = references.into_iter().collect::<Vec<_>>();
                let mut seen = Vec::<u64>::new();
                while let Some(position) = pending.pop() {
                    if parameters
                        .iter()
                        .any(|(parameter, _, _)| *parameter == position)
                    {
                        return false;
                    }
                    let Some(declaration) = declarations.get(position as usize) else {
                        return false;
                    };
                    let word = position as usize / 64;
                    if word >= seen.len() {
                        seen.resize(word + 1, 0);
                    }
                    let bit = 1u64 << (position % 64);
                    if seen[word] & bit != 0 {
                        continue;
                    }
                    seen[word] |= bit;
                    let roots = std::iter::once(declaration.ty)
                        .chain(declaration.body)
                        .collect::<Vec<_>>();
                    let Some(references) =
                        collect_constant_references(arena, &roots, &mut seen_terms)
                    else {
                        return false;
                    };
                    pending.extend(references);
                }
                true
            }

            /// Close an outer source telescope through shared generated definitions.
            /// Dependent definitions are copied once as parameterized definitions; existing
            /// declarations/caches remain immutable and no assumption gains authority.
            pub(in super::super) fn abstract_parameter_closure(
                arena: &mut TermArena,
                declarations: &mut Vec<Declaration>,
                proof: TermHandle,
                ty: TermHandle,
                parameters: &[(u32, SymbolHandle, TermHandle)],
                lifted: &mut ParameterAbstractions,
            ) -> Result<(TermHandle, TermHandle), &'static str> {
                if parameters.is_empty() {
                    return Ok((proof, ty));
                }
                let original_len = declarations.len();
                let first = parameters
                    .iter()
                    .map(|(position, _, _)| *position as usize)
                    .min()
                    .ok_or("source telescope is empty")?;
                let parameter_identities = parameters
                    .iter()
                    .map(|(position, _, domain)| (*position, *domain))
                    .collect::<Vec<_>>();
                if lifted.parameters.is_empty() {
                    lifted.parameters = parameter_identities;
                    lifted.base = first;
                } else if lifted.parameters != parameter_identities {
                    return Err("source theorem changed its outer telescope");
                }
                lifted.copies.resize(original_len - first, None);
                let mut edges = vec![None; original_len - first];
                let mut pending = constant_references(arena, &[proof, ty])
                    .ok_or("source theorem has a dummy term")?
                    .into_iter()
                    .collect::<Vec<_>>();
                while let Some(position) = pending.pop() {
                    let position = position as usize;
                    if position >= original_len {
                        return Err("source theorem has an unformed declaration");
                    }
                    if position <= first
                        || lifted.get(position as u32).is_some()
                        || edges[position - first].is_some()
                    {
                        continue;
                    }
                    let declaration = &declarations[position];
                    let roots = std::iter::once(declaration.ty)
                        .chain(declaration.body)
                        .collect::<Vec<_>>();
                    let references = constant_references(arena, &roots)
                        .ok_or("source theorem declaration has a dummy term")?;
                    if references
                        .iter()
                        .any(|&reference| reference as usize >= position)
                    {
                        return Err("source theorem changed its declaration prefix");
                    }
                    pending.extend(references.iter().copied());
                    edges[position - first] = Some(references);
                }
                for position in first + 1..original_len {
                    let Some(references) = &edges[position - first] else {
                        continue;
                    };
                    if !references.iter().any(|reference| {
                        parameters
                            .iter()
                            .any(|(parameter, _, _)| parameter == reference)
                            || lifted.get(*reference).is_some()
                    }) {
                        continue;
                    }
                    let declaration = declarations[position].clone();
                    let Some(body) = declaration.body else {
                        return Err("source theorem retains an unabstracted dependent assumption");
                    };
                    let (body, ty) = abstract_parameters_with_declarations(
                        arena,
                        body,
                        declaration.ty,
                        parameters,
                        lifted,
                    )?;
                    let replacement = declarations.len() as u32;
                    declarations.push(Declaration::definition(declaration.level_arity, ty, body));
                    lifted.copies[position - first] = Some(replacement);
                }
                let closed =
                    abstract_parameters_with_declarations(arena, proof, ty, parameters, lifted)?;
                if !excludes_parameters(arena, declarations, &[closed.0, closed.1], parameters) {
                    return Err(
                        "source theorem retains an outer parameter assumption after abstraction",
                    );
                }
                Ok(closed)
            }

            fn constant_references(arena: &TermArena, roots: &[TermHandle]) -> Option<Vec<u32>> {
                collect_constant_references(arena, roots, &mut Vec::new())
            }

            fn collect_constant_references(
                arena: &TermArena,
                roots: &[TermHandle],
                seen_terms: &mut Vec<u64>,
            ) -> Option<Vec<u32>> {
                let mut pending = roots.to_vec();
                let mut constants = Vec::new();
                while let Some(term) = pending.pop() {
                    // A stale sibling must not hide behind the current slot's visited bit.
                    let node = arena.get(term);
                    if matches!(node, Term::Dummy) {
                        return None;
                    }
                    let position = term.arena_index() as usize;
                    let word = position / 64;
                    if word >= seen_terms.len() {
                        seen_terms.resize(word + 1, 0);
                    }
                    let bit = 1u64 << (position % 64);
                    if seen_terms[word] & bit != 0 {
                        continue;
                    }
                    seen_terms[word] |= bit;
                    match node {
                        Term::Constant { declaration, .. } => {
                            constants.push(declaration);
                        }
                        Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                            pending.extend([domain, codomain])
                        }
                        Term::Lambda { domain, body } => pending.extend([domain, body]),
                        Term::Apply { function, argument } => pending.extend([function, argument]),
                        Term::Pair { first, second } => pending.extend([first, second]),
                        Term::Fst { pair } | Term::Snd { pair } => pending.push(pair),
                        Term::CaseTwo {
                            motive,
                            zero_branch,
                            one_branch,
                            scrutinee,
                        } => pending.extend([motive, zero_branch, one_branch, scrutinee]),
                        Term::Id { ty, left, right } => pending.extend([ty, left, right]),
                        Term::Refl { ty, value }
                        | Term::SquashIntro { ty, value }
                        | Term::BoxIntro { ty, value } => pending.extend([ty, value]),
                        Term::IdElim {
                            motive,
                            base,
                            endpoint,
                            proof,
                        } => pending.extend([motive, base, endpoint, proof]),
                        Term::W { carrier, children } => pending.extend([carrier, children]),
                        Term::Sup {
                            carrier,
                            children,
                            label,
                            function,
                        } => pending.extend([carrier, children, label, function]),
                        Term::IndW { motive, step, tree } => pending.extend([motive, step, tree]),
                        Term::EmptyElim { ty, scrutinee } => pending.extend([ty, scrutinee]),
                        Term::Squash { ty } | Term::Box { ty } => pending.push(ty),
                        Term::SquashElim {
                            proposition,
                            function,
                            scrutinee,
                        } => pending.extend([proposition, function, scrutinee]),
                        Term::BoxElim {
                            motive,
                            body,
                            scrutinee,
                        } => pending.extend([motive, body, scrutinee]),
                        Term::Dummy => return None,
                        Term::Variable(_)
                        | Term::Sort(_)
                        | Term::Two
                        | Term::TwoZero
                        | Term::TwoOne
                        | Term::Empty => {}
                    }
                }
                Some(constants)
            }

            #[cfg(test)]
            mod outer_type_closure_tests {
                use super::*;
                use proof_admission::{
                    Budget, Context, Level, Sort, check_signature, check_type,
                    judgment_assumption_closure,
                };

                #[test]
                fn a_stale_term_cannot_hide_behind_a_visited_slot() {
                    let mut arena = TermArena::new();
                    let current = arena.insert(Term::Two);
                    let stale =
                        TermHandle::from_parts(current.arena_index(), current.generation() + 1);
                    // Stack order visits the current handle before its stale sibling.
                    assert!(constant_references(&arena, &[stale, current]).is_none());
                }

                #[test]
                fn closes_shared_type_dependent_definitions_without_granting_an_assumption() {
                    let mut arena = TermArena::new();
                    let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                    let parameter = SymbolHandle::from_arena_index(1);
                    let mut declarations = vec![Declaration::assumption(0, sort)];
                    let carrier = arena.insert(Term::Constant {
                        declaration: 0,
                        levels: vec![],
                    });
                    let value = arena.insert(Term::Variable(0));
                    let relation = arena.insert(Term::Id {
                        ty: carrier,
                        left: value,
                        right: value,
                    });
                    let ty = arena.insert(Term::Pi {
                        domain: carrier,
                        codomain: relation,
                    });
                    let reflexive = arena.insert(Term::Refl { ty: carrier, value });
                    let body = arena.insert(Term::Lambda {
                        domain: carrier,
                        body: reflexive,
                    });
                    declarations.push(Declaration::definition(0, ty, body));
                    let shared = arena.insert(Term::Constant {
                        declaration: 1,
                        levels: vec![],
                    });
                    declarations.push(Declaration::definition(0, ty, shared));
                    let proof = arena.insert(Term::Constant {
                        declaration: 2,
                        levels: vec![],
                    });
                    check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
                    let parameters = [(0, parameter, sort)];
                    let mut lifted = ParameterAbstractions::default();
                    let (closed, statement) = abstract_parameter_closure(
                        &mut arena,
                        &mut declarations,
                        proof,
                        ty,
                        &parameters,
                        &mut lifted,
                    )
                    .unwrap();
                    assert!(excludes_parameters(
                        &arena,
                        &declarations,
                        &[closed, statement],
                        &parameters
                    ));
                    let signature =
                        check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
                    check_type(
                        &mut arena,
                        &Context::empty().with_signature(signature.clone()),
                        closed,
                        statement,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert!(
                        judgment_assumption_closure(&arena, &signature, &[closed, statement])
                            .is_empty()
                    );
                    let count = declarations.len();
                    let _ = abstract_parameter_closure(
                        &mut arena,
                        &mut declarations,
                        proof,
                        ty,
                        &parameters,
                        &mut lifted,
                    )
                    .unwrap();
                    assert_eq!(
                        declarations.len(),
                        count,
                        "shared dependent definitions are lifted once"
                    );

                    // An arbitrary inhabitant cannot become a polymorphic theorem.
                    let assumed = declarations.len() as u32;
                    declarations.push(Declaration::assumption(0, carrier));
                    let assumed = arena.insert(Term::Constant {
                        declaration: assumed,
                        levels: vec![],
                    });
                    assert!(
                        abstract_parameter_closure(
                            &mut arena,
                            &mut declarations,
                            assumed,
                            carrier,
                            &parameters,
                            &mut lifted
                        )
                        .is_err()
                    );
                }
            }
        }
        pub(super) use parameter_closure::{ParameterAbstractions, abstract_parameter_closure};
        mod construction {
            use super::super::Elaborator;
            use super::parameter_closure::{abstract_parameters, excludes_parameters};
            use super::terms::{constructor_children, lift_payload};
            use crate::typed_expressions::{expression_table, type_reference_table};
            use diagnostics::Diagnostic;
            use language_core::BindingRelevance;
            use proof_admission::{
                Budget, Context, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle,
            };
            use proof_admission::{
                CoreError, Declaration, check_type, judgment_assumption_closure,
            };
            use symbols::SymbolHandle;
            use typed_trees::data::DataField;
            use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
            use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
            use validation::expression_owners::{OwnerEnvironmentIndex, source_declaration_owner};
            use validation::mathematical_applications::{
                PreparedSourceConstructor, PreparedSourceEvidence,
            };

            #[derive(Default)]
            pub(crate) struct SourceConstructorContext {
                pub owner: SymbolHandle,
                pub values: Vec<(SymbolHandle, TermHandle)>,
                pub parameters: Vec<(u32, SymbolHandle, TermHandle)>,
                pub premises: Vec<(TermHandle, TermHandle)>,
            }

            impl Elaborator<'_> {
                pub(crate) fn retain_source_constructor_schema(
                    &mut self,
                    expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                    machine: &typed_trees::machine::Machine,
                    state: &typed_trees::state::State,
                ) -> Result<(), Vec<Diagnostic>> {
                    use typed_trees::signature::SignatureContractKind;
                    self.source_constructor_context = SourceConstructorContext {
                        owner: machine.symbol,
                        ..SourceConstructorContext::default()
                    };
                    let entries = self
                        .scope
                        .iter()
                        .filter_map(|entry| entry.symbol.map(|symbol| (symbol, entry.domain)))
                        .collect::<Vec<_>>();
                    for (symbol, domain) in entries {
                        if self
                            .program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| parameter.symbol == symbol && parameter.is_mutable)
                        {
                            continue;
                        }
                        self.source_constructor_parameter(symbol, domain);
                    }
                    let typed_values = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_mutable
                    && matches!(
                        self.program.type_reference_table.type_reference(parameter.type_reference),
                        TypeReferenceNode::Named { symbol, .. }
                            if self.program.machine_type_parameters(machine).iter().any(|binder| {
                                binder.symbol == *symbol
                                    && matches!(binder.kind, typed_trees::data::TypeParameterKind::Type)
                            })
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
                    for parameter in typed_values {
                        let TypeReferenceNode::Named { symbol, .. } =
                            type_reference_table(self.program)
                                .type_reference(parameter.type_reference)
                        else {
                            unreachable!("selected exact Type-formal domain")
                        };
                        if !self
                            .source_constructor_context
                            .values
                            .iter()
                            .any(|(bound, _)| bound == symbol)
                        {
                            return Err(self.refuse(
                                "source constructor value lost its exact Type-formal scope".into(),
                            ));
                        }
                        let domain = self.elaborate_type_reference(parameter.type_reference)?;
                        self.source_constructor_parameter(parameter.symbol, domain);
                    }
                    let evidence = self
                        .program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| {
                            parameter.relevance == BindingRelevance::Erased
                                && self.known_source_constructor_family(parameter.type_reference)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    for parameter in evidence {
                        if parameter.is_mutable {
                            continue;
                        }
                        let domain = self.elaborate_type_reference(parameter.type_reference)?;
                        self.source_constructor_parameter(parameter.symbol, domain);
                    }
                    let is_entry = self
                        .program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == state.symbol);
                    let predicates = self
                        .program
                        .machine_contracts(machine)
                        .iter()
                        .filter(|_| is_entry)
                        .chain(self.program.state_contracts(state))
                        .filter(|contract| contract.kind == SignatureContractKind::Requires)
                        .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
                        .filter_map(|fact| fact.as_expression().copied())
                        .collect::<Vec<_>>();
                    for predicate in predicates {
                        let Some(domain) = self.elaborate_proposition(predicate)? else {
                            return Err(self.refuse(
                                "open source constructor premise has no mathematical proposition"
                                    .into(),
                            ));
                        };
                        let closed = self
                            .integer_theory
                            .as_mut()
                            .map(|theory| {
                                theory.closed_predicate(
                                    &mut self.arena,
                                    &mut self.declarations,
                                    domain,
                                )
                            })
                            .transpose()
                            .map_err(|error| {
                                self.refuse(format!(
                                    "source constructor premise derivation failed: {error}"
                                ))
                            })?
                            .flatten();
                        let proof = closed.unwrap_or_else(|| {
                            self.source_constructor_parameter(SymbolHandle::invalid(), domain)
                        });
                        self.source_constructor_context
                            .premises
                            .push((domain, proof));
                    }
                    let term = self.source_family_constructor_term(expression, reference, 0)?;
                    let ty = self.elaborate_type_reference(reference)?;
                    let temporary_parameters = self.source_constructor_context.parameters.clone();
                    let parameters = self.source_constructor_dependencies(&[term, ty]);
                    let (schema, schema_type) =
                        abstract_parameters(&mut self.arena, term, ty, &parameters).map_err(
                            |error| {
                                self.refuse(format!(
                                    "source constructor abstraction failed: {error}"
                                ))
                            },
                        )?;
                    if !excludes_parameters(
                        &self.arena,
                        &self.declarations,
                        &[schema, schema_type],
                        &temporary_parameters,
                    ) {
                        return Err(self.refuse(
                            "source constructor schema retains a temporary value or premise assumption"
                                .into(),
                        ));
                    }
                    check_type(
                        &mut self.arena,
                        &Context::with_level_arity(
                            self.level_binders.len() as u32 + self.generalized_levels,
                        )
                        .with_signature(Signature::from_declarations(self.declarations.clone())),
                        schema,
                        schema_type,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "source constructor schema fails kernel checking: {error:?}"
                        ))
                    })?;
                    self.declarations.push(Declaration::definition(
                        proof_admission::universe_parameter_arity(
                            &self.arena,
                            &[schema_type, schema],
                        ),
                        schema_type,
                        schema,
                    ));
                    let ExpressionNode::StructLiteral(literal) =
                        self.program.expression_table.expression(expression)
                    else {
                        return Err(self.refuse(
                            "source constructor schema lost its construction occurrence".into(),
                        ));
                    };
                    let TypeReferenceNode::Generic { base_symbol, .. } =
                        self.program.type_reference_table.type_reference(reference)
                    else {
                        return Err(self.refuse(
                            "source constructor schema lost its family application".into(),
                        ));
                    };
                    let family = self.indexed_families.get(base_symbol).ok_or_else(|| {
                        self.refuse("source constructor schema lost its checked family".into())
                    })?;
                    let selected = family
                        .cases
                        .iter()
                        .find(|case| match literal.case_symbol {
                            Some(actual) => self
                                .program
                                .data_constructor_case(reference, literal.type_symbol, actual)
                                .zip(self.program.data_constructor_case(
                                    reference,
                                    literal.type_symbol,
                                    case.symbol,
                                ))
                                .is_some_and(|(actual, candidate)| {
                                    actual.symbol == candidate.symbol
                                }),
                            None => !case.symbol.is_valid(),
                        })
                        .ok_or_else(|| {
                            self.refuse("source constructor schema lost its selected case".into())
                        })?;
                    let predicate_expressions = selected.predicate_expressions.clone();
                    self.prepared
                        .source_constructors
                        .append(PreparedSourceConstructor {
                            level_arity: proof_admission::universe_parameter_arity(
                                &self.arena,
                                &[schema_type, schema],
                            ),
                            expression,
                            constructor: expression,
                            source_type: reference,
                            type_identity: self.program.normalized_type_identity(reference),
                            machine: machine.symbol,
                            state: state.symbol,
                            schema,
                            ty: schema_type,
                            parameters: parameters.iter().map(|(_, symbol, _)| *symbol).collect(),
                            predicate_expressions,
                        });
                    self.source_constructor_context = SourceConstructorContext::default();
                    Ok(())
                }

                fn source_constructor_dependencies(
                    &self,
                    roots: &[TermHandle],
                ) -> Vec<(u32, SymbolHandle, TermHandle)> {
                    let dependencies = judgment_assumption_closure(
                        &self.arena,
                        &Signature::from_declarations(self.declarations.clone()),
                        roots,
                    );
                    self.source_constructor_context
                        .parameters
                        .iter()
                        .copied()
                        .filter(|(position, _, _)| dependencies.contains(position))
                        .collect()
                }

                fn known_source_constructor_family(&self, reference: TypeReferenceHandle) -> bool {
                    matches!(self.program.type_reference_table.type_reference(reference), TypeReferenceNode::Generic { base_symbol, .. }
            if self.indexed_families.contains_key(base_symbol))
                }

                fn source_constructor_parameter(
                    &mut self,
                    symbol: SymbolHandle,
                    domain: TermHandle,
                ) -> TermHandle {
                    let position = self.declarations.len() as u32;
                    self.declarations.push(Declaration::assumption(
                        proof_admission::universe_parameter_arity(&self.arena, &[domain]),
                        domain,
                    ));
                    let term = self.constant(position);
                    self.source_constructor_context
                        .parameters
                        .push((position, symbol, domain));
                    if symbol.is_valid() {
                        self.source_constructor_context.values.push((symbol, term));
                    }
                    term
                }

                pub(crate) fn source_family_constructor(
                    &mut self,
                    expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                    depth: usize,
                ) -> Result<TermHandle, Vec<Diagnostic>> {
                    let term = self.source_family_constructor_term(expression, reference, depth)?;
                    let ty = self.elaborate_type_reference(reference)?;
                    if !self.source_constructor_context.owner.is_valid()
                        && !self
                            .prepared
                            .source_evidence
                            .iter()
                            .any(|(_, row)| row.expression == expression)
                    {
                        self.prepared
                            .source_evidence
                            .append(PreparedSourceEvidence {
                                expression,
                                source_type: reference,
                                type_identity: self.program.normalized_type_identity(reference),
                                term,
                                ty,
                            });
                    }
                    Ok(term)
                }

                fn source_family_constructor_term(
                    &mut self,
                    expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                    depth: usize,
                ) -> Result<TermHandle, Vec<Diagnostic>> {
                    if depth > 128 {
                        return Err(self.refuse(
                            "source constructor evidence exceeds the construction depth bound"
                                .into(),
                        ));
                    }
                    if let ExpressionNode::Name(path) =
                        self.program.expression_table.expression(expression)
                    {
                        let symbol = path.resolved_symbol().ok_or_else(|| {
                            self.refuse("source constructor use has no exact binding".into())
                        })?;
                        if let Some((_, term)) = self
                            .source_constructor_context
                            .values
                            .iter()
                            .find(|(bound, _)| *bound == symbol)
                        {
                            return Ok(*term);
                        }
                        let mut initializer = None;
                        for machine in self
                            .program
                            .machines()
                            .iter()
                            .filter(|machine| machine.symbol == self.source_owner)
                        {
                            for state in self.program.machine_states(machine) {
                                if let Some((ordinal, local)) =
                                    self.program.state_local_declaration(state, symbol)
                                {
                                    if local.is_mutable
                                        || local.relevance != BindingRelevance::Erased
                                    {
                                        return Err(self.refuse("source indexed capture requires retained immutable erased constructor evidence".into()));
                                    }
                                    if self.program.normalized_type_identity(local.type_reference)
                                        != self.program.normalized_type_identity(reference)
                                    {
                                        return Err(self.refuse(
                                            "source indexed capture changed its exact application"
                                                .into(),
                                        ));
                                    }
                                    if initializer
                                        .replace((
                                            local.initial_value,
                                            local.type_reference,
                                            state.symbol,
                                            ordinal,
                                        ))
                                        .is_some()
                                    {
                                        return Err(self.refuse(
                                            "source indexed capture has duplicate declaration identity"
                                                .into(),
                                        ));
                                    }
                                }
                            }
                        }
                        let (value, actual, declaring_state, declaration_ordinal) = initializer
                            .ok_or_else(|| {
                                self.refuse(
                                    "source indexed capture has no retained constructor derivation"
                                        .into(),
                                )
                            })?;
                        let owners =
                            OwnerEnvironmentIndex::new(self.program, &arena::Arena::default());
                        let sites = owners.containing_statement_sites(expression);
                        if source_declaration_owner(self.program, expression, &owners)
                            != Some(self.source_owner)
                            || sites.is_empty()
                            || sites.iter().any(|(machine, state, statement)| {
                                let machine = &self.program.machines()[*machine];
                                machine.symbol != self.source_owner
                                    || self.program.machine_states(machine)[*state].symbol
                                        != declaring_state
                                    || *statement <= declaration_ordinal
                            })
                        {
                            return Err(self.refuse(
                                "source indexed capture changed its declaration owner or lexical frontier"
                                    .into(),
                            ));
                        }
                        return self.source_family_constructor(value, actual, depth + 1);
                    }
                    let ExpressionNode::StructLiteral(literal) =
                        self.program.expression_table.expression(expression)
                    else {
                        return Err(self.refuse(
                            "source indexed operand has no constructor derivation yet".into(),
                        ));
                    };
                    let literal = literal.clone();
                    self.span = Some(self.program.expression_table.source_span(expression));
                    let TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } = self.program.type_reference_table.type_reference(reference)
                    else {
                        return Err(self.refuse(
                            "source indexed constructor lost its exact family application".into(),
                        ));
                    };
                    let arguments = type_reference_table(self.program)
                        .type_reference_handles(*arguments)
                        .to_vec();
                    let family = self.source_family(*base_symbol)?;
                    let actuals = family
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, argument)| self.index_operand(*argument, parameter, &[]))
                        .collect::<Result<Vec<_>, _>>()?;
                    let family = self.applied_source_family(&family, &actuals)?;
                    let description = family
                        .description_at(&mut self.arena, &actuals)
                        .map_err(|reason| self.refuse(reason.into()))?;
                    let type_actuals = family
                        .parameters
                        .iter()
                        .zip(&actuals)
                        .filter_map(|(parameter, &actual)| {
                            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                                .then_some(actual)
                        })
                        .collect::<Vec<_>>();
                    let stored_type_count = family
                        .index_parameter_indices
                        .iter()
                        .filter(|&&position| {
                            matches!(
                                family.parameters[position].kind,
                                typed_trees::data::TypeParameterKind::Type
                            )
                        })
                        .count();
                    let case = if let Some(selected) = literal.case_symbol {
                        let actual = self
                            .program
                            .data_constructor_case(reference, literal.type_symbol, selected)
                            .ok_or_else(|| {
                                self.refuse(
                                    "source indexed constructor lost its active case correspondence".into(),
                                )
                            })?;
                        family
                            .cases
                            .iter()
                            .position(|case| {
                                self.program
                                    .data_constructor_case(
                                        reference,
                                        literal.type_symbol,
                                        case.symbol,
                                    )
                                    .is_some_and(|candidate| candidate.symbol == actual.symbol)
                            })
                            .ok_or_else(|| {
                                self.refuse(
                                    "source indexed constructor selected a foreign case".into(),
                                )
                            })?
                    } else {
                        if family.cases.len() != 1 || family.cases[0].symbol.is_valid() {
                            return Err(self.refuse(
                                "source indexed constructor omitted its active case".into(),
                            ));
                        }
                        0
                    };
                    let selected = &family.cases[case];
                    let fields = expression_table(self.program)
                        .struct_fields(literal.fields)
                        .to_vec();
                    if fields.len()
                        != family.common.len() + selected.payload.len() + selected.children.len()
                    {
                        return Err(self.refuse(
                            "source indexed constructor changed its exact field roster".into(),
                        ));
                    }
                    let mut common = family
                        .index_parameter_indices
                        .iter()
                        .filter_map(|&position| {
                            matches!(
                                family.parameters[position].kind,
                                typed_trees::data::TypeParameterKind::Type
                            )
                            .then_some(actuals[position])
                        })
                        .collect::<Vec<_>>();
                    let mut payload = Vec::new();
                    let mut children = Vec::new();
                    for field in &family.common {
                        let operand = self
                            .source_constructor_operand(expression, reference, &literal, field)?;
                        common.push(self.elaborate_expression(operand)?);
                    }
                    for field in &selected.payload {
                        let operand = self
                            .source_constructor_operand(expression, reference, &literal, field)?;
                        payload.push(self.elaborate_expression(operand)?);
                    }
                    if selected.children.len() != selected.child_indices.len() {
                        return Err(self.refuse(
                            "generated source constructor lost its declared child index roster"
                                .into(),
                        ));
                    }
                    for (field, &child_index) in
                        selected.children.iter().zip(&selected.child_indices)
                    {
                        let operand = self
                            .source_constructor_operand(expression, reference, &literal, field)?;
                        self.span = Some(self.program.expression_table.source_span(operand));
                        let actual =
                            self.source_operand_type_reference(operand).ok_or_else(|| {
                                self.refuse(
                                    "source recursive child has no retained actual application"
                                        .into(),
                                )
                            })?;
                        let mut application = actual;
                        let mut actual_family = None;
                        for _ in 0..128 {
                            match type_reference_table(self.program).type_reference(application) {
                                TypeReferenceNode::Generic { base_symbol, .. } => {
                                    actual_family = Some(*base_symbol);
                                    break;
                                }
                                TypeReferenceNode::Named { symbol, .. } => {
                                    let Some(origin) = self
                                        .program
                                        .unique_data_definition_by_symbol(*symbol)
                                        .and_then(|definition| definition.generic_instance)
                                    else {
                                        break;
                                    };
                                    application = origin;
                                }
                                _ => break,
                            }
                        }
                        let source_mismatch = format!(
                            "source recursive child `{}` does not inhabit its declared indexed application `{}`; actual application is `{}`",
                            field.name,
                            self.program.display_type_reference(field.type_reference),
                            self.program.display_type_reference(actual),
                        );
                        if actual_family != Some(family.symbol) {
                            return Err(self.refuse(source_mismatch));
                        }
                        let child = self.source_family_constructor(operand, actual, depth + 1)?;
                        self.span = Some(self.program.expression_table.source_span(operand));
                        let mut index = child_index;
                        for &argument in type_actuals
                            .iter()
                            .chain(common.iter().skip(stored_type_count))
                            .chain(payload.iter())
                        {
                            index = self.arena.insert(Term::Apply {
                                function: index,
                                argument,
                            });
                        }
                        let expected = description.indexed_w(&mut self.arena, index);
                        let context = Context::with_level_arity(
                            self.level_binders.len() as u32 + self.generalized_levels,
                        )
                        .with_signature(Signature::from_declarations(self.declarations.clone()));
                        match check_type(
                            &mut self.arena,
                            &context,
                            child,
                            expected,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        ) {
                            Ok(()) => children.push(child),
                            Err(CoreError::TypeMismatch { .. }) => {
                                return Err(self.refuse(source_mismatch));
                            }
                            Err(CoreError::StepCeiling) => {
                                return Err(self.refuse(format!(
                        "source recursive child `{}` indexed application conversion exceeds the supported checking budget", field.name
                    )));
                            }
                            Err(error) => {
                                return Err(self.refuse(format!(
                        "generated source child evidence for `{}` fails kernel checking: {error:?}",
                        field.name
                    )));
                            }
                        }
                    }
                    self.span = Some(self.program.expression_table.source_span(expression));
                    let mut predicate = selected.constraints;
                    for &argument in type_actuals
                        .iter()
                        .chain(common.iter().skip(stored_type_count))
                        .chain(payload.iter())
                    {
                        predicate = self.arena.insert(Term::Apply {
                            function: predicate,
                            argument,
                        });
                    }
                    let Some(theory) = &mut self.integer_theory else {
                        return Err(self.refuse(
                            "source constructor predicate has no shared integer vocabulary".into(),
                        ));
                    };
                    let premises = self.source_constructor_context.premises.clone();
                    let mut proof = if premises.is_empty() {
                        theory.closed_predicate(&mut self.arena, &mut self.declarations, predicate)
                    } else {
                        theory.predicate_from_assumptions(
                            &mut self.arena,
                            &mut self.declarations,
                            predicate,
                            &premises
                                .iter()
                                .map(|(domain, _)| *domain)
                                .collect::<Vec<_>>(),
                        )
                    }
                    .map_err(|error| {
                        self.refuse(format!(
                            "source constructor predicate derivation failed: {error}"
                        ))
                    })?
                    .ok_or_else(|| {
                        self.refuse(
                            "source constructor predicate has no checked derivation yet".into(),
                        )
                    })?;
                    for (_, argument) in premises {
                        proof = self.arena.insert(Term::Apply {
                            function: proof,
                            argument,
                        });
                    }
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(Signature::from_declarations(self.declarations.clone()));
                    check_type(
                        &mut self.arena,
                        &context,
                        proof,
                        predicate,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "generated source constructor predicate proof fails kernel checking: {error:?}"
                        ))
                    })?;
                    let mut label = proof;
                    for value in payload.into_iter().rev() {
                        label = self.arena.insert(Term::Pair {
                            first: value,
                            second: label,
                        });
                    }
                    if !family.payload_lifts.is_empty() {
                        let mut lift_domains = Vec::new();
                        for &function in &family.payload_lifts {
                            let mut domain = function;
                            for &actual in type_actuals
                                .iter()
                                .chain(common.iter().skip(stored_type_count))
                            {
                                domain = self.arena.insert(Term::Apply {
                                    function: domain,
                                    argument: actual,
                                });
                            }
                            lift_domains.push(domain);
                        }
                        label = lift_payload(&mut self.arena, &lift_domains, label)
                            .map_err(|reason| self.refuse(reason.into()))?;
                    }
                    if case + 1 < family.cases.len() {
                        let tag = self.arena.insert(Term::TwoZero);
                        label = self.arena.insert(Term::Pair {
                            first: tag,
                            second: label,
                        });
                    }
                    for _ in 0..case {
                        let tag = self.arena.insert(Term::TwoOne);
                        label = self.arena.insert(Term::Pair {
                            first: tag,
                            second: label,
                        });
                    }
                    for value in common.into_iter().rev() {
                        label = self.arena.insert(Term::Pair {
                            first: value,
                            second: label,
                        });
                    }
                    check_type(
                        &mut self.arena,
                        &context,
                        label,
                        description.carrier,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "generated source constructor label fails kernel checking: {error:?}"
                        ))
                    })?;
                    // Projections must infer the declaration's dependent label telescope,
                    // not the nondependent tuple type inferred for a raw pair literal.
                    // Package the checked label as an ordinary transparent definition.
                    let parameters =
                        self.source_constructor_dependencies(&[label, description.carrier]);
                    let (label_body, label_type) = abstract_parameters(
                        &mut self.arena,
                        label,
                        description.carrier,
                        &parameters,
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "source constructor label abstraction failed: {error}"
                        ))
                    })?;
                    let label_position = self.declarations.len() as u32;
                    self.declarations.push(Declaration::definition(
                        proof_admission::universe_parameter_arity(
                            &self.arena,
                            &[label_type, label_body],
                        ),
                        label_type,
                        label_body,
                    ));
                    let mut label = self.constant(label_position);
                    for (position, _, _) in parameters {
                        let argument = self.constant(position);
                        label = self.arena.insert(Term::Apply {
                            function: label,
                            argument,
                        });
                    }
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(Signature::from_declarations(self.declarations.clone()));
                    let child_domain = self.arena.insert(Term::Apply {
                        function: description.children,
                        argument: label,
                    });
                    let children_fn =
                        constructor_children(&mut self.arena, &description, label, &children)
                            .map_err(|error| {
                                self.refuse(format!(
                                    "source constructor child-function production failed: {error}"
                                ))
                            })?;
                    let next = self.arena.insert(Term::Apply {
                        function: description.next,
                        argument: label,
                    });
                    let p = self.arena.insert(Term::Variable(0));
                    let index = self.arena.insert(Term::Apply {
                        function: next,
                        argument: p,
                    });
                    let codomain = description.indexed_w(&mut self.arena, index);
                    let expected_children = self.arena.insert(Term::Pi {
                        domain: child_domain,
                        codomain,
                    });
                    check_type(
                        &mut self.arena,
                        &context,
                        children_fn,
                        expected_children,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "generated source constructor child function fails kernel checking: {error:?}"
                        ))
                    })?;
                    let term = description.sup(&mut self.arena, label, children_fn);
                    let expected = self.elaborate_type_reference(reference)?;
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(Signature::from_declarations(self.declarations.clone()));
                    check_type(
                        &mut self.arena,
                        &context,
                        term,
                        expected,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "generated source constructor evidence fails kernel checking: {error:?}"
                        ))
                    })?;
                    Ok(term)
                }

                fn source_constructor_operand(
                    &self,
                    _expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                    literal: &TableStructLiteral,
                    field: &DataField,
                ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
                    let selected = self
                        .program
                        .data_constructor_field(reference, literal.type_symbol, field.symbol)
                        .ok_or_else(|| {
                            self.refuse(
                                "source constructor lost its exact declared field correspondence"
                                    .into(),
                            )
                        })?;
                    let mut operands = expression_table(self.program)
                        .struct_fields(literal.fields)
                        .iter()
                        .filter_map(|operand| {
                            let actual = self.program.data_constructor_field(
                                reference,
                                literal.type_symbol,
                                operand.field_symbol,
                            )?;
                            (actual.symbol == selected.symbol).then_some(operand.value)
                        });
                    let operand = operands.next().ok_or_else(|| {
                        self.refuse(format!(
                            "source indexed constructor omitted field `{}`",
                            field.name
                        ))
                    })?;
                    if operands.next().is_some() {
                        return Err(self.refuse(
                            "source indexed constructor duplicated a declared field".into(),
                        ));
                    }
                    Ok(operand)
                }
            }
        }
        pub(super) use construction::SourceConstructorContext;
        mod induction {
            use super::super::Elaborator;
            use super::terms;
            use diagnostics::Diagnostic;
            use proof_admission::{
                Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Signature, Sort, Term,
                TermHandle,
            };
            use symbols::SymbolHandle;
            use typed_trees::data::TypeParameterKind;

            #[derive(Clone)]
            pub(crate) struct SourceInduction {
                pub theorem: TermHandle,
                pub ty: TermHandle,
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[test]
                fn checked_induction_reuse_retains_the_exact_signature_judgment() {
                    let mut arena = proof_admission::TermArena::new();
                    let two = arena.insert(Term::Two);
                    let zero = arena.insert(Term::TwoZero);
                    let one = arena.insert(Term::TwoOne);
                    let b = arena.insert(Term::Constant {
                        declaration: 0,
                        levels: vec![],
                    });
                    let statement = arena.insert(Term::Id {
                        ty: two,
                        left: b,
                        right: zero,
                    });
                    let proof = arena.insert(Term::Refl { ty: two, value: b });
                    let declarations = vec![Declaration::definition(0, two, zero)];
                    let check = |arena: &mut proof_admission::TermArena,
                                 declarations: &[Declaration],
                                 ty| {
                        let signature = proof_admission::check_signature(
                            arena,
                            declarations,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        )
                        .unwrap();
                        proof_admission::check_type(
                            arena,
                            &Context::empty().with_signature(signature),
                            proof,
                            ty,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        )
                    };
                    check(&mut arena, &declarations, statement).unwrap();
                    let cached = CheckedSourceInduction {
                        raw_invariant: statement,
                        result: SourceInduction {
                            theorem: proof,
                            ty: statement,
                        },
                        declarations: declarations.clone(),
                        parameter_actuals: Vec::new(),
                    };
                    let same_statement = arena.insert(Term::Id {
                        ty: two,
                        left: b,
                        right: zero,
                    });
                    assert!(cached.reusable(&arena, &declarations, same_statement, &[]));
                    let mut extended = declarations.clone();
                    extended.push(Declaration::definition(0, two, one));
                    assert!(cached.reusable(&arena, &extended, same_statement, &[]));
                    check(&mut arena, &extended, statement).unwrap();

                    // The raw handles stay identical, but changing b's transparent body
                    // makes the formerly checked proof invalid in a still-valid signature.
                    let changed = vec![Declaration::definition(0, two, one)];
                    assert!(!cached.reusable(&arena, &changed, statement, &[]));
                    assert!(check(&mut arena, &changed, statement).is_err());

                    let changed_statement = arena.insert(Term::Id {
                        ty: two,
                        left: b,
                        right: one,
                    });
                    assert!(!cached.reusable(&arena, &declarations, changed_statement, &[]));
                    assert!(check(&mut arena, &declarations, changed_statement).is_err());
                }
            }

            #[derive(Clone)]
            pub(crate) struct CheckedSourceInduction {
                raw_invariant: TermHandle,
                result: SourceInduction,
                declarations: Vec<Declaration>,
                parameter_actuals: Vec<TermHandle>,
            }

            impl CheckedSourceInduction {
                fn reusable(
                    &self,
                    arena: &proof_admission::TermArena,
                    declarations: &[Declaration],
                    raw_invariant: TermHandle,
                    parameter_actuals: &[TermHandle],
                ) -> bool {
                    declarations.starts_with(&self.declarations)
                        && self.parameter_actuals.len() == parameter_actuals.len()
                        && self
                            .parameter_actuals
                            .iter()
                            .zip(parameter_actuals)
                            .all(|(&left, &right)| arena.structurally_equal(left, right))
                        && arena.structurally_equal(self.raw_invariant, raw_invariant)
                }
            }

            fn term_mentions_variable(
                arena: &proof_admission::TermArena,
                term: TermHandle,
            ) -> bool {
                match arena.get(term) {
                    Term::Variable(_) => true,
                    Term::Constant { .. }
                    | Term::Sort(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Empty
                    | Term::Dummy => false,
                    Term::Apply { function, argument } => {
                        term_mentions_variable(arena, function)
                            || term_mentions_variable(arena, argument)
                    }
                    Term::Pair { first, second } => {
                        term_mentions_variable(arena, first)
                            || term_mentions_variable(arena, second)
                    }
                    Term::Lambda { domain, body }
                    | Term::Pi {
                        domain,
                        codomain: body,
                    } => {
                        term_mentions_variable(arena, domain) || term_mentions_variable(arena, body)
                    }
                    _ => true,
                }
            }

            impl Elaborator<'_> {
                pub(crate) fn source_family_induction(
                    &mut self,
                    symbol: SymbolHandle,
                    invariant: TermHandle,
                    index_actuals: &[TermHandle],
                ) -> Result<SourceInduction, Vec<Diagnostic>> {
                    let template = self.source_family(symbol)?;
                    let family = self.applied_source_family(&template, index_actuals)?;
                    let description = family
                        .description_at(&mut self.arena, index_actuals)
                        .map_err(|reason| self.refuse(reason.into()))?;
                    let external_actuals = family
                        .external_parameter_indices
                        .iter()
                        .map(|&position| {
                            (family.parameters[position].symbol, index_actuals[position])
                        })
                        .collect::<Vec<_>>();
                    let parameter_actuals = external_actuals
                        .iter()
                        .map(|(_, actual)| *actual)
                        .collect::<Vec<_>>();
                    let all_parameter_actuals = index_actuals;
                    let index_actuals = family.index_actuals(index_actuals);
                    let raw_invariant = invariant;
                    // The family model is immutable after its first construction. Raw
                    // structural identity retains every constant and universe argument;
                    // exact prefix equality additionally forbids reusing a derivation
                    // after changing any referenced declaration's statement or body.
                    // Appending declarations cannot change this checked judgment. Each
                    // source occurrence still retains its own expression, carrier and
                    // receiver-policy association at the caller.
                    if let Some(previous) = family.checked_inductions.iter().find(|previous| {
                        previous.reusable(
                            &self.arena,
                            &self.declarations,
                            raw_invariant,
                            &parameter_actuals,
                        )
                    }) {
                        return Ok(previous.result.clone());
                    }
                    let result_level = terms::result_sort_level(
                        &mut self.arena,
                        &self.declarations,
                        invariant,
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .map_err(|reason| self.refuse(reason))?;
                    let sort = self
                        .arena
                        .insert(Term::Sort(Sort::Type(result_level.clone())));
                    let invariant_type = self.arena.insert(Term::Pi {
                        domain: description.index,
                        codomain: sort,
                    });
                    self.check_induction_term(invariant, invariant_type, "index invariant")?;
                    let invariant = terms::intern_invariant(
                        &mut self.arena,
                        &mut self.declarations,
                        &description,
                        invariant,
                        result_level,
                    );
                    let signature = Signature::from_declarations(self.declarations.clone());
                    let index = self.arena.insert(Term::Variable(0));
                    let applied = self.arena.insert(Term::Apply {
                        function: invariant,
                        argument: index,
                    });
                    let goal = proof_admission::weak_head_normalize(
                        &mut self.arena,
                        &signature,
                        applied,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "source invariant normalization is unsupported: {error:?}"
                        ))
                    })?;
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(signature);
                    if proof_admission::infer_sort(
                        &mut self.arena,
                        &context,
                        goal,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .is_ok()
                    {
                        // A closed specialization is this family's inductive theorem at
                        // those indices. Instantiating it keeps the distributive law the
                        // induction used. A fresh numeral equation would name a different
                        // bodyless assumption and drop that law.
                        if let Some(result) = self.instantiated_open_induction(
                            &family,
                            &index_actuals,
                            &parameter_actuals,
                        )? {
                            self.record_source_induction(
                                symbol,
                                raw_invariant,
                                &result,
                                &parameter_actuals,
                            );
                            return Ok(result);
                        }
                        let proof = if let Some(theory) = self.integer_theory.as_mut() {
                            theory
                                .closed_predicate(&mut self.arena, &mut self.declarations, goal)
                                .map_err(|error| {
                                    self.refuse(format!(
                                        "closed source invariant derivation is unsupported: {error:?}"
                                    ))
                                })?
                        } else {
                            None
                        };
                        if let Some(proof) = proof {
                            let (theorem, ty) = terms::constant_invariant_theorem(
                                &mut self.arena,
                                &description,
                                invariant,
                                proof,
                            );
                            self.check_induction_term(
                                theorem,
                                ty,
                                "closed source invariant theorem",
                            )?;
                            let result = SourceInduction { theorem, ty };
                            self.record_source_induction(
                                symbol,
                                raw_invariant,
                                &result,
                                &parameter_actuals,
                            );
                            return Ok(result);
                        }
                    }
                    let mut lemmas = Vec::new();
                    let mut schemes = Vec::new();
                    let common_types = family
                        .common
                        .iter()
                        .map(|field| {
                            self.source_field_function(
                                field,
                                &family.parameters,
                                &family.type_levels,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut common = family
                        .common
                        .iter()
                        .zip(&common_types)
                        .map(|(field, ty)| (field.symbol, *ty))
                        .collect::<Vec<_>>();
                    let data = self
                        .program
                        .unique_data_definition_by_symbol(symbol)
                        .expect("retained source family declaration");
                    let type_symbols = family
                        .parameters
                        .iter()
                        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>();
                    let mut external_slots = vec![false; family.parameters.len()];
                    for &position in &family.external_parameter_indices {
                        external_slots[position] = true;
                    }
                    let stored_type_slots = family
                        .parameters
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| matches!(parameter.kind, TypeParameterKind::Type))
                        .enumerate()
                        .filter_map(|(type_slot, (position, _))| {
                            (!external_slots[position]).then_some(type_slot)
                        })
                        .collect::<Vec<_>>();
                    let stored_type_symbols = stored_type_slots
                        .iter()
                        .map(|&slot| type_symbols[slot])
                        .collect::<Vec<_>>();
                    common.splice(
                        0..0,
                        stored_type_symbols.iter().map(|symbol| {
                            let ordinal = type_symbols
                                .iter()
                                .position(|bound| bound == symbol)
                                .expect("retained Type binder");
                            (
                                *symbol,
                                self.arena.insert(Term::Sort(Sort::Type(
                                    family.type_levels[ordinal].clone(),
                                ))),
                            )
                        }),
                    );
                    let mut index_fields = self
                        .program
                        .data_value_parameter_fields(data)
                        .map(|(_, field)| field.symbol)
                        .collect::<Vec<_>>();
                    index_fields.splice(0..0, stored_type_symbols.iter().copied());
                    for case in &family.cases {
                        let payload = case
                            .payload
                            .iter()
                            .map(|field| {
                                self.source_field_function(
                                    field,
                                    &family.parameters,
                                    &family.type_levels,
                                )
                                .map(|ty| (field.symbol, ty))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let fields = common
                            .iter()
                            .chain(payload.iter())
                            .copied()
                            .collect::<Vec<_>>();
                        let mut values = Vec::new();
                        let mut parameters = Vec::new();
                        let mut type_actuals = family
                            .parameters
                            .iter()
                            .enumerate()
                            .filter_map(|(position, parameter)| {
                                matches!(parameter.kind, TypeParameterKind::Type)
                                    .then_some(all_parameter_actuals[position])
                            })
                            .collect::<Vec<_>>();
                        for (field_ordinal, &(field, mut domain)) in fields.iter().enumerate() {
                            let type_slot = stored_type_slots.get(field_ordinal).copied();
                            if type_slot.is_none() {
                                for actual in &type_actuals {
                                    domain = self.arena.insert(Term::Apply {
                                        function: domain,
                                        argument: *actual,
                                    });
                                }
                            }
                            let position = self.declarations.len() as u32;
                            self.declarations.push(Declaration {
                                level_arity: proof_admission::universe_parameter_arity(
                                    &self.arena,
                                    &[domain],
                                ),
                                ty: domain,
                                body: None,
                            });
                            parameters.push((position, field, domain));
                            let value = self.constant(position);
                            if let Some(type_slot) = type_slot {
                                type_actuals[type_slot] = value;
                            }
                            values.push(value);
                        }
                        let mut predicate = case.constraints;
                        for &value in type_actuals
                            .iter()
                            .chain(values.iter().skip(stored_type_symbols.len()))
                        {
                            predicate = self.arena.insert(Term::Apply {
                                function: predicate,
                                argument: value,
                            });
                        }
                        let index_values = index_fields
                            .iter()
                            .map(|field| {
                                values[fields
                                    .iter()
                                    .position(|(candidate, _)| candidate == field)
                                    .expect("exact source descriptor coordinate")]
                            })
                            .collect();
                        let index = self.index_tuple(index_values)?;
                        let goal = self.arena.insert(Term::Apply {
                            function: invariant,
                            argument: index,
                        });
                        let mut assumptions = vec![predicate];
                        for &child in &case.child_indices {
                            let mut index = child;
                            for &value in type_actuals
                                .iter()
                                .chain(values.iter().skip(stored_type_symbols.len()))
                            {
                                index = self.arena.insert(Term::Apply {
                                    function: index,
                                    argument: value,
                                });
                            }
                            assumptions.push(self.arena.insert(Term::Apply {
                                function: invariant,
                                argument: index,
                            }));
                        }
                        let Some(theory) = self.integer_theory.as_mut() else {
                            return Err(self.refuse(
                                "source induction requires a shared integer vocabulary".into(),
                            ));
                        };
                        let proof = theory.predicate_from_assumptions(&mut self.arena, &mut self.declarations, goal, &assumptions)
                .map_err(|error| self.refuse(format!("source induction case arithmetic is unsupported: {error:?}")))?
                .ok_or_else(|| self.refuse("source induction case has no checked derivation from its exact constraints and child hypotheses".into()))?;
                        let mut ty = goal;
                        for &domain in assumptions.iter().rev() {
                            ty = proof_admission::shift(&mut self.arena, ty, 0, 1);
                            ty = self.arena.insert(Term::Pi {
                                domain,
                                codomain: ty,
                            });
                        }
                        self.check_induction_term(proof, ty, "case predicate derivation")?;
                        let (lemma, lemma_type) = super::parameter_closure::abstract_parameters(
                            &mut self.arena,
                            proof,
                            ty,
                            &parameters,
                        )
                        .map_err(|error| self.refuse(error.into()))?;
                        if !super::parameter_closure::excludes_parameters(
                            &self.arena,
                            &self.declarations,
                            &[lemma, lemma_type],
                            &parameters,
                        ) {
                            return Err(self.refuse(
                    "source induction case retains an arbitrary value assumption after abstraction"
                        .into(),
                ));
                        }
                        self.check_induction_term(lemma, lemma_type, "abstracted source case")?;
                        let position = self.declarations.len() as u32;
                        self.declarations.push(Declaration::definition(
                            proof_admission::universe_parameter_arity(
                                &self.arena,
                                &[lemma_type, lemma],
                            ),
                            lemma_type,
                            lemma,
                        ));
                        lemmas.push(self.constant(position));
                        schemes.push(terms::Case {
                            payload,
                            constraints: case.constraints,
                            child_indices: case.child_indices.clone(),
                        });
                    }
                    let (theorem, ty, step, step_type) = terms::induction(
                        &mut self.arena,
                        &description,
                        &common,
                        &index_fields,
                        &schemes,
                        invariant,
                        &lemmas,
                        &mut self.declarations,
                        &type_symbols,
                        &external_actuals,
                        &family.payload_lifts,
                    )
                    .map_err(|error| self.refuse(error))?;
                    self.check_induction_term(step, step_type, "indexed-family induction step")?;
                    self.check_induction_term(theorem, ty, "derived indexed-family theorem")?;
                    let result = SourceInduction { theorem, ty };
                    self.record_source_induction(
                        symbol,
                        raw_invariant,
                        &result,
                        &parameter_actuals,
                    );
                    Ok(result)
                }

                fn instantiated_open_induction(
                    &mut self,
                    family: &super::SourceFamily,
                    index_actuals: &[TermHandle],
                    parameter_actuals: &[TermHandle],
                ) -> Result<Option<SourceInduction>, Vec<Diagnostic>> {
                    if index_actuals.is_empty()
                        || index_actuals.len() != family.index_parameter_indices.len()
                    {
                        return Ok(None);
                    }
                    let signature = Signature::from_declarations(self.declarations.clone());
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(signature.clone());
                    if index_actuals
                        .iter()
                        .any(|actual| term_mentions_variable(&self.arena, *actual))
                    {
                        return Ok(None);
                    }
                    let open = {
                        let Some(matched) = family.checked_inductions.iter().find(|previous| {
                            if previous.parameter_actuals.len() != parameter_actuals.len()
                                || !previous
                                    .parameter_actuals
                                    .iter()
                                    .zip(parameter_actuals)
                                    .all(|(&left, &right)| {
                                        self.arena.structurally_equal(left, right)
                                    })
                            {
                                return false;
                            }
                            let index = self.arena.insert(Term::Variable(0));
                            let applied = self.arena.insert(Term::Apply {
                                function: previous.raw_invariant,
                                argument: index,
                            });
                            let Ok(goal) = proof_admission::weak_head_normalize(
                                &mut self.arena,
                                &signature,
                                applied,
                                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                            ) else {
                                return false;
                            };
                            proof_admission::infer_sort(
                                &mut self.arena,
                                &context,
                                goal,
                                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                            )
                            .is_err()
                        }) else {
                            return Ok(None);
                        };
                        matched.result.clone()
                    };
                    let tuple = self.index_tuple(index_actuals.to_vec())?;
                    let theorem = self.arena.insert(Term::Apply {
                        function: open.theorem,
                        argument: tuple,
                    });
                    let ty = proof_admission::infer_type(
                        &mut self.arena,
                        &context,
                        theorem,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "instantiated source induction has no type: {error:?}"
                        ))
                    })?;
                    self.check_induction_term(theorem, ty, "instantiated source induction")?;
                    Ok(Some(SourceInduction { theorem, ty }))
                }

                fn record_source_induction(
                    &mut self,
                    symbol: SymbolHandle,
                    raw_invariant: TermHandle,
                    result: &SourceInduction,
                    parameter_actuals: &[TermHandle],
                ) {
                    self.indexed_families
                        .get_mut(&symbol)
                        .expect("retained source family")
                        .checked_inductions
                        .push(CheckedSourceInduction {
                            raw_invariant,
                            result: result.clone(),
                            declarations: self.declarations.clone(),
                            parameter_actuals: parameter_actuals.to_vec(),
                        });
                }

                fn check_induction_term(
                    &mut self,
                    term: TermHandle,
                    ty: TermHandle,
                    role: &str,
                ) -> Result<(), Vec<Diagnostic>> {
                    let context = Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(Signature::from_declarations(self.declarations.clone()));
                    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                    let result = proof_admission::check_type(
                        &mut self.arena,
                        &context,
                        term,
                        ty,
                        &mut budget,
                    );
                    result.map_err(|error| {
                        self.refuse(format!("generated {role} fails kernel checking: {error:?}"))
                    })
                }
            }
        }
        mod terms {
            /*
            Untrusted source-family description production. All generated applications
            are ordinary terms checked against indexed_scheme; no admission follows
            from this builder or from a source constructor's Boolean verdict.

            Each source common/payload binder keeps its exact symbol key. Constraint
            and child-index functions are closed over that ordered source telescope,
            so moving them under tag/payload eliminators cannot capture a source name.
            Finite child positions use the profile's boxed strict empty/unit types.

            To combine payloads at different levels, let A : Type u and define
            P(A) = (a : A) -> squash(Id A a a) : Strict u. Its canonical proof is
            a -> sq(refl A a), so U(A) = Box P(A) : Type u is inhabited even when
            A is empty. Box elimination reduces identity with the canonical box to
            equality of strict proofs, which holds by strict proof irrelevance.
            Thus x -> (canonical, x) and second projection preserve a payload in
            Sigma(_: U(A)). B : Type max(u, v), for B : Type v. The reverse composite
            is propositionally equal by this contraction; no conversion rule changes.

            For a multi-case family, A is one shared product of its complete unpadded
            branch types, closed over the common source telescope. Its level is exactly
            their maximum, including case constraints, without assuming any branch is
            inhabited. One U(A) component raises each branch to that same level. Common
            fields and indices keep their separately inferred levels. Single-case data
            needs no padding. A Type0 Box or data with an unused high-level parameter
            therefore stays Type0.
            */

            #[cfg(test)]
            use super::parameter_closure::{abstract_parameters, excludes_parameters};
            use proof_admission::{
                Declaration, IndexedFamily, Level, Sort, Term, TermArena, TermHandle, shift,
                substitute,
            };
            use symbols::SymbolHandle;

            #[derive(Clone, Copy, PartialEq, Eq)]
            pub(super) enum Key {
                Source(SymbolHandle),
                Internal(usize),
            }

            #[derive(Clone)]
            enum S {
                Closed(TermHandle),
                SourceDomain(TermHandle, Vec<S>),
                Var(Key),
                Type,
                Universe(Level),
                Squashed(Box<S>),
                SquashValue(Box<S>, Box<S>),
                Two,
                Zero,
                One,
                Empty,
                Pi(Key, Box<S>, Box<S>),
                Lam(Key, Box<S>, Box<S>),
                Sigma(Key, Box<S>, Box<S>),
                App(Box<S>, Box<S>),
                Pair(Box<S>, Box<S>),
                Fst(Box<S>),
                Snd(Box<S>),
                Case(Box<S>, Box<S>, Box<S>, Box<S>),
                Boxed(Box<S>),
                BoxIntro(Box<S>, Box<S>),
                BoxElim(Box<S>, Box<S>, Box<S>),
                EmptyElim(Box<S>, Box<S>),
                Identity(Box<S>, Box<S>, Box<S>),
                Reflexivity(Box<S>, Box<S>),
                IdentityElimination(Box<S>, Box<S>, Box<S>, Box<S>),
            }
            fn app(f: S, x: S) -> S {
                S::App(Box::new(f), Box::new(x))
            }
            fn pair(a: S, b: S) -> S {
                S::Pair(Box::new(a), Box::new(b))
            }
            fn fst(x: S) -> S {
                S::Fst(Box::new(x))
            }
            fn snd(x: S) -> S {
                S::Snd(Box::new(x))
            }
            fn lam(k: Key, a: S, b: S) -> S {
                S::Lam(k, Box::new(a), Box::new(b))
            }
            fn pi(k: Key, a: S, b: S) -> S {
                S::Pi(k, Box::new(a), Box::new(b))
            }
            fn sigma(k: Key, a: S, b: S) -> S {
                S::Sigma(k, Box::new(a), Box::new(b))
            }
            fn choose(m: S, z: S, o: S, t: S) -> S {
                S::Case(Box::new(m), Box::new(z), Box::new(o), Box::new(t))
            }

            impl S {
                fn term(
                    &self,
                    arena: &mut TermArena,
                    scope: &mut Vec<Key>,
                ) -> Result<TermHandle, &'static str> {
                    let node = match self {
                        Self::Closed(t) => return Ok(*t),
                        Self::SourceDomain(function, parameters) => {
                            let mut result = *function;
                            for parameter in parameters {
                                let actual = parameter.term(arena, scope)?;
                                let Term::Lambda { body, .. } = arena.get(result) else {
                                    return Err(
                                        "source payload domain lost its generated Type telescope",
                                    );
                                };
                                result = substitute(arena, body, actual);
                            }
                            return Ok(result);
                        }
                        Self::Var(k) => Term::Variable(
                            scope
                                .iter()
                                .rposition(|v| v == k)
                                .map(|p| (scope.len() - 1 - p) as u32)
                                .ok_or("generated source-family binder escaped its telescope")?,
                        ),
                        Self::Type => Term::Sort(Sort::Type(Level::Constant(0))),
                        Self::Universe(level) => Term::Sort(Sort::Type(level.clone())),
                        Self::Squashed(ty) => Term::Squash {
                            ty: ty.term(arena, scope)?,
                        },
                        Self::SquashValue(ty, value) => Term::SquashIntro {
                            ty: ty.term(arena, scope)?,
                            value: value.term(arena, scope)?,
                        },
                        Self::Two => Term::Two,
                        Self::Zero => Term::TwoZero,
                        Self::One => Term::TwoOne,
                        Self::Empty => Term::Empty,
                        Self::Pi(k, a, b) | Self::Lam(k, a, b) | Self::Sigma(k, a, b) => {
                            let domain = a.term(arena, scope)?;
                            scope.push(*k);
                            let body = b.term(arena, scope);
                            scope.pop();
                            let body = body?;
                            match self {
                                Self::Pi(..) => term!(Pi => domain => body),
                                Self::Lam(..) => Term::Lambda { domain, body },
                                _ => term!(Sigma => domain => body),
                            }
                        }
                        Self::App(f, x) => {
                            term!(Apply => f.term(arena, scope)? => x.term(arena, scope)?)
                        }
                        Self::Pair(a, b) => {
                            term!(Pair => a.term(arena, scope)? => b.term(arena, scope)?)
                        }
                        Self::Fst(a) => Term::Fst {
                            pair: a.term(arena, scope)?,
                        },
                        Self::Snd(a) => Term::Snd {
                            pair: a.term(arena, scope)?,
                        },
                        Self::Case(m, z, o, t) => Term::CaseTwo {
                            motive: m.term(arena, scope)?,
                            zero_branch: z.term(arena, scope)?,
                            one_branch: o.term(arena, scope)?,
                            scrutinee: t.term(arena, scope)?,
                        },
                        Self::Boxed(a) => Term::Box {
                            ty: a.term(arena, scope)?,
                        },
                        Self::BoxIntro(a, x) => Term::BoxIntro {
                            ty: a.term(arena, scope)?,
                            value: x.term(arena, scope)?,
                        },
                        Self::BoxElim(m, f, x) => Term::BoxElim {
                            motive: m.term(arena, scope)?,
                            body: f.term(arena, scope)?,
                            scrutinee: x.term(arena, scope)?,
                        },
                        Self::EmptyElim(a, x) => Term::EmptyElim {
                            ty: a.term(arena, scope)?,
                            scrutinee: x.term(arena, scope)?,
                        },
                        Self::Identity(ty, left, right) => Term::Id {
                            ty: ty.term(arena, scope)?,
                            left: left.term(arena, scope)?,
                            right: right.term(arena, scope)?,
                        },
                        Self::Reflexivity(ty, value) => Term::Refl {
                            ty: ty.term(arena, scope)?,
                            value: value.term(arena, scope)?,
                        },
                        Self::IdentityElimination(motive, base, endpoint, proof) => Term::IdElim {
                            motive: motive.term(arena, scope)?,
                            base: base.term(arena, scope)?,
                            endpoint: endpoint.term(arena, scope)?,
                            proof: proof.term(arena, scope)?,
                        },
                    };
                    Ok(arena.insert(node))
                }
            }

            pub(super) struct Case {
                pub payload: Vec<(SymbolHandle, TermHandle)>,
                pub constraints: TermHandle,
                pub child_indices: Vec<TermHandle>,
            }

            pub(super) fn result_sort_level(
                arena: &mut TermArena,
                declarations: &[Declaration],
                function: TermHandle,
                level_arity: u32,
            ) -> Result<Level, String> {
                let context = proof_admission::Context::with_level_arity(level_arity)
                    .with_signature(proof_admission::Signature::from_declarations(
                        declarations.to_vec(),
                    ));
                result_sort_level_in(arena, &context, function)
            }

            fn result_sort_level_in(
                arena: &mut TermArena,
                context: &proof_admission::Context,
                function: TermHandle,
            ) -> Result<Level, String> {
                let mut ty = proof_admission::infer_type(
                    arena,
                    context,
                    function,
                    &mut proof_admission::Budget::new(proof_admission::DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    format!("source descriptor domain fails universe inference: {error:?}")
                })?;
                while let Term::Pi { codomain, .. } = arena.get(ty) {
                    ty = codomain;
                }
                match arena.get(ty) {
                    Term::Sort(sort) => Ok(sort.level()),
                    _ => Err("source descriptor domain does not return a universe".to_owned()),
                }
            }

            fn payload_sort_level(
                arena: &mut TermArena,
                declarations: &[Declaration],
                cases: &[Case],
                level_arity: u32,
            ) -> Result<Level, String> {
                let context = proof_admission::Context::with_level_arity(level_arity)
                    .with_signature(proof_admission::Signature::from_declarations(
                        declarations.to_vec(),
                    ));
                let mut level = Level::Constant(0);
                for case in cases {
                    level = level.maximum(result_sort_level_in(arena, &context, case.constraints)?);
                    for (_, domain) in &case.payload {
                        level = level.maximum(result_sort_level_in(arena, &context, *domain)?);
                    }
                }
                Ok(level)
            }

            /// The constructor uses the same contractible padding as the descriptor.
            /// Dropping these first components recovers the exact original payload.
            pub(super) fn lift_payload(
                arena: &mut TermArena,
                domains: &[TermHandle],
                mut payload: TermHandle,
            ) -> Result<TermHandle, &'static str> {
                let mut terms = SourceTerms::default();
                for &domain in domains.iter().rev() {
                    let (_, witness) = terms.level_unit(S::Closed(domain));
                    let witness = witness.term(arena, &mut Vec::new())?;
                    payload = arena.insert(Term::Pair {
                        first: witness,
                        second: payload,
                    });
                }
                Ok(payload)
            }

            #[derive(Clone, Copy)]
            enum ResultKind {
                Positions,
                Next,
            }

            #[derive(Default)]
            struct SourceTerms {
                next: usize,
            }
            impl SourceTerms {
                fn fresh(&mut self) -> Key {
                    let k = Key::Internal(self.next);
                    self.next += 1;
                    k
                }
                fn empty(&mut self) -> S {
                    S::Boxed(Box::new(S::Empty))
                }
                fn unit_strict(&mut self) -> S {
                    let k = self.fresh();
                    pi(k, S::Empty, S::Empty)
                }
                fn unit(&mut self) -> S {
                    let u = self.unit_strict();
                    S::Boxed(Box::new(u))
                }
                fn unit_value(&mut self) -> S {
                    let domain = self.unit_strict();
                    let value = self.fresh();
                    S::BoxIntro(
                        Box::new(domain),
                        Box::new(lam(value, S::Empty, S::Var(value))),
                    )
                }
                fn type_motive(&mut self) -> S {
                    self.type_motive_at(Level::Constant(0))
                }
                fn type_motive_at(&mut self, level: Level) -> S {
                    let k = self.fresh();
                    lam(k, S::Two, S::Universe(level))
                }
                fn level_unit(&mut self, carrier: S) -> (S, S) {
                    // This boxed strict function is inhabited for every A : Type u,
                    // including empty A, and lives at exactly Type u. Pairing it with a
                    // lower payload is an explicit lift; projecting the payload is its
                    // inverse up to strict proof irrelevance. No universe is resized.
                    let x = self.fresh();
                    let identity = S::Identity(
                        Box::new(carrier.clone()),
                        Box::new(S::Var(x)),
                        Box::new(S::Var(x)),
                    );
                    let proposition =
                        pi(x, carrier.clone(), S::Squashed(Box::new(identity.clone())));
                    let reflexivity =
                        S::Reflexivity(Box::new(carrier.clone()), Box::new(S::Var(x)));
                    let proof = lam(
                        x,
                        carrier,
                        S::SquashValue(Box::new(identity), Box::new(reflexivity)),
                    );
                    (
                        S::Boxed(Box::new(proposition.clone())),
                        S::BoxIntro(Box::new(proposition), Box::new(proof)),
                    )
                }
                fn positions(&mut self, count: usize) -> S {
                    match count {
                        0 => self.empty(),
                        1 => self.unit(),
                        _ => {
                            let tag = self.fresh();
                            let motive = self.type_motive();
                            let first = self.unit();
                            let rest = self.positions(count - 1);
                            sigma(tag, S::Two, choose(motive, first, rest, S::Var(tag)))
                        }
                    }
                }
                fn select(&mut self, values: &[S], position: S, result_family: S) -> S {
                    let Some((first, rest_values)) = values.split_first() else {
                        let boxed = self.fresh();
                        let impossible = self.fresh();
                        let motive = lam(
                            boxed,
                            self.empty(),
                            app(result_family.clone(), S::Var(boxed)),
                        );
                        let unboxed_position =
                            S::BoxIntro(Box::new(S::Empty), Box::new(S::Var(impossible)));
                        let result = app(result_family, unboxed_position);
                        let body = lam(
                            impossible,
                            S::Empty,
                            S::EmptyElim(Box::new(result), Box::new(S::Var(impossible))),
                        );
                        return S::BoxElim(Box::new(motive), Box::new(body), Box::new(position));
                    };
                    if rest_values.is_empty() {
                        return first.clone();
                    }
                    let tag = self.fresh();
                    let payload = self.fresh();
                    let type_motive = self.type_motive();
                    let unit = self.unit();
                    let rest = self.positions(rest_values.len());
                    let at = choose(type_motive, unit.clone(), rest.clone(), S::Var(tag));
                    let selected_type =
                        app(result_family.clone(), pair(S::Var(tag), S::Var(payload)));
                    let motive = lam(tag, S::Two, pi(payload, at, selected_type));
                    let singleton = self.fresh();
                    let zero = lam(singleton, unit, first.clone());
                    let tail = self.fresh();
                    let nested_position = self.fresh();
                    let nested_family = lam(
                        nested_position,
                        rest.clone(),
                        app(result_family, pair(S::One, S::Var(nested_position))),
                    );
                    let nested = self.select(rest_values, S::Var(tail), nested_family);
                    let one = lam(tail, rest, nested);
                    app(
                        choose(motive, zero, one, fst(position.clone())),
                        snd(position),
                    )
                }
            }

            struct Builder<'a> {
                terms: SourceTerms,
                common: &'a [(SymbolHandle, TermHandle)],
                index_fields: &'a [SymbolHandle],
                cases: &'a [Case],
                type_parameters: &'a [SymbolHandle],
                external_actuals: &'a [(SymbolHandle, TermHandle)],
                payload_level: Level,
                payload_lifts: &'a [TermHandle],
            }
            impl Builder<'_> {
                fn type_actual(&self, symbol: SymbolHandle) -> S {
                    self.external_actuals
                        .iter()
                        .find(|(parameter, _)| *parameter == symbol)
                        .map(|(_, actual)| S::Closed(*actual))
                        .unwrap_or(S::Var(Key::Source(symbol)))
                }
                fn domain(&self, symbol: SymbolHandle, ty: TermHandle) -> S {
                    if self.type_parameters.is_empty() || self.type_parameters.contains(&symbol) {
                        S::Closed(ty)
                    } else {
                        S::SourceDomain(
                            ty,
                            self.type_parameters
                                .iter()
                                .map(|symbol| self.type_actual(*symbol))
                                .collect(),
                        )
                    }
                }
                fn index_type(&mut self) -> Result<S, &'static str> {
                    let mut fields = Vec::new();
                    for symbol in self.index_fields {
                        let (_, ty) = self
                            .common
                            .iter()
                            .find(|(field, _)| field == symbol)
                            .ok_or("source index lost its exact descriptor field")?;
                        fields.push((*symbol, *ty));
                    }
                    let Some((last_symbol, last)) = fields.pop() else {
                        return Ok(self.terms.unit());
                    };
                    let mut body = self.domain(last_symbol, last);
                    for (symbol, ty) in fields.into_iter().rev() {
                        body = sigma(Key::Source(symbol), self.domain(symbol, ty), body);
                    }
                    Ok(body)
                }
                fn constraint_application(&self, case: usize) -> S {
                    let mut term = S::Closed(self.cases[case].constraints);
                    for symbol in self.type_parameters {
                        term = app(term, self.type_actual(*symbol));
                    }
                    for (symbol, _) in self
                        .common
                        .iter()
                        .filter(|(symbol, _)| !self.type_parameters.contains(symbol))
                        .chain(self.cases[case].payload.iter())
                    {
                        term = app(term, S::Var(Key::Source(*symbol)));
                    }
                    term
                }
                fn lift_count(&self) -> usize {
                    self.payload_lifts.len()
                }
                fn unlift_payload(&self, mut payload: S) -> S {
                    for _ in 0..self.lift_count() {
                        payload = snd(payload);
                    }
                    payload
                }
                fn raw_payload_type(&self, case: usize) -> S {
                    let mut body = self.constraint_application(case);
                    for (symbol, ty) in self.cases[case].payload.iter().rev() {
                        body = sigma(Key::Source(*symbol), self.domain(*symbol, *ty), body);
                    }
                    body
                }
                fn common_application(&self, mut function: S) -> S {
                    for symbol in self.type_parameters {
                        function = app(function, self.type_actual(*symbol));
                    }
                    for (symbol, _) in self
                        .common
                        .iter()
                        .filter(|(symbol, _)| !self.type_parameters.contains(symbol))
                    {
                        function = app(function, S::Var(Key::Source(*symbol)));
                    }
                    function
                }
                fn payload_type(&mut self, case: usize) -> S {
                    let mut body = self.raw_payload_type(case);
                    for &function in self.payload_lifts.iter().rev() {
                        let carrier = self.common_application(S::Closed(function));
                        let (unit, _) = self.terms.level_unit(carrier);
                        let binder = self.terms.fresh();
                        body = sigma(binder, unit, body);
                    }
                    body
                }
                fn payload_type_functions(
                    &self,
                    arena: &mut TermArena,
                    type_domains: &[TermHandle],
                ) -> Result<Vec<TermHandle>, &'static str> {
                    if type_domains.len() != self.type_parameters.len() {
                        return Err("source payload lift lost its exact Type telescope");
                    }
                    (0..self.cases.len())
                        .map(|case| {
                            let mut body = self.raw_payload_type(case);
                            for &(symbol, domain) in self
                                .common
                                .iter()
                                .rev()
                                .filter(|(symbol, _)| !self.type_parameters.contains(symbol))
                            {
                                body = lam(Key::Source(symbol), self.domain(symbol, domain), body);
                            }
                            for (&symbol, &domain) in
                                self.type_parameters.iter().zip(type_domains).rev()
                            {
                                body = lam(Key::Source(symbol), S::Closed(domain), body);
                            }
                            body.term(arena, &mut Vec::new())
                        })
                        .collect()
                }
                fn shared_payload_type_function(
                    &mut self,
                    arena: &mut TermArena,
                    type_domains: &[TermHandle],
                ) -> Result<TermHandle, &'static str> {
                    let Some(last) = self.cases.len().checked_sub(1) else {
                        return Err("source payload lift has no constructor cases");
                    };
                    let mut body = self.raw_payload_type(last);
                    for case in (0..last).rev() {
                        let binder = self.terms.fresh();
                        body = sigma(binder, self.raw_payload_type(case), body);
                    }
                    for &(symbol, domain) in self
                        .common
                        .iter()
                        .rev()
                        .filter(|(symbol, _)| !self.type_parameters.contains(symbol))
                    {
                        body = lam(Key::Source(symbol), self.domain(symbol, domain), body);
                    }
                    for (&symbol, &domain) in self.type_parameters.iter().zip(type_domains).rev() {
                        body = lam(Key::Source(symbol), S::Closed(domain), body);
                    }
                    body.term(arena, &mut Vec::new())
                }
                fn sum_type(&mut self, start: usize) -> S {
                    if start == self.cases.len() {
                        return self.terms.empty();
                    }
                    if start + 1 == self.cases.len() {
                        return self.payload_type(start);
                    }
                    let tag = self.terms.fresh();
                    let motive = self.terms.type_motive_at(self.payload_level.clone());
                    let zero = self.payload_type(start);
                    let one = self.sum_type(start + 1);
                    sigma(tag, S::Two, choose(motive, zero, one, S::Var(tag)))
                }
                fn carrier(&mut self) -> S {
                    let mut body = self.sum_type(0);
                    for (symbol, ty) in self.common.iter().rev() {
                        body = sigma(Key::Source(*symbol), self.domain(*symbol, *ty), body);
                    }
                    body
                }
                fn projected(mut value: S, ordinal: usize) -> S {
                    for _ in 0..ordinal {
                        value = snd(value);
                    }
                    fst(value)
                }
                fn child_index(&self, case: usize, child: usize, payload: S) -> S {
                    let mut term = S::Closed(self.cases[case].child_indices[child]);
                    for symbol in self.type_parameters {
                        term = app(term, self.type_actual(*symbol));
                    }
                    for (symbol, _) in self
                        .common
                        .iter()
                        .filter(|(symbol, _)| !self.type_parameters.contains(symbol))
                    {
                        term = app(term, S::Var(Key::Source(*symbol)));
                    }
                    for ordinal in 0..self.cases[case].payload.len() {
                        term = app(term, Self::projected(payload.clone(), ordinal));
                    }
                    term
                }
                fn case_body(
                    &mut self,
                    case: usize,
                    payload: S,
                    kind: ResultKind,
                ) -> Result<S, &'static str> {
                    let payload = self.unlift_payload(payload);
                    let count = self.cases[case].child_indices.len();
                    let positions = self.terms.positions(count);
                    if matches!(kind, ResultKind::Positions) {
                        return Ok(positions);
                    }
                    let index = self.index_type()?;
                    let p = self.terms.fresh();
                    let values = (0..count)
                        .map(|child| self.child_index(case, child, payload.clone()))
                        .collect::<Vec<_>>();
                    let at = self.terms.fresh();
                    let result_family = lam(at, positions.clone(), index);
                    let body = self.terms.select(&values, S::Var(p), result_family);
                    Ok(lam(p, positions, body))
                }
                fn result_type(
                    &mut self,
                    start: usize,
                    value: S,
                    kind: ResultKind,
                ) -> Result<S, &'static str> {
                    if matches!(kind, ResultKind::Positions) {
                        return Ok(S::Type);
                    }
                    let positions = self.eliminate(start, value, ResultKind::Positions)?;
                    let p = self.terms.fresh();
                    Ok(pi(p, positions, self.index_type()?))
                }
                fn eliminate(
                    &mut self,
                    start: usize,
                    value: S,
                    kind: ResultKind,
                ) -> Result<S, &'static str> {
                    if start == self.cases.len() {
                        return Err("empty source constructor roster has no eliminator branch");
                    }
                    if start + 1 == self.cases.len() {
                        return self.case_body(start, value, kind);
                    }
                    let tag = self.terms.fresh();
                    let payload = self.terms.fresh();
                    let tm = self.terms.type_motive_at(self.payload_level.clone());
                    let zero_type = self.payload_type(start);
                    let one_type = self.sum_type(start + 1);
                    let payload_type = choose(tm, zero_type.clone(), one_type.clone(), S::Var(tag));
                    let result =
                        self.result_type(start, pair(S::Var(tag), S::Var(payload)), kind)?;
                    let motive = lam(tag, S::Two, pi(payload, payload_type, result));
                    let z = self.terms.fresh();
                    let zero_body = self.case_body(start, S::Var(z), kind)?;
                    let zero = lam(z, zero_type, zero_body);
                    let o = self.terms.fresh();
                    let one_body = self.eliminate(start + 1, S::Var(o), kind)?;
                    let one = lam(o, one_type, one_body);
                    Ok(app(
                        choose(motive, zero, one, fst(value.clone())),
                        snd(value),
                    ))
                }
                fn unpack(&mut self, label: S, kind: ResultKind) -> Result<S, &'static str> {
                    let tail = self.terms.fresh();
                    let sum_type = self.sum_type(0);
                    let body = self.eliminate(0, S::Var(tail), kind)?;
                    let mut function = lam(tail, sum_type, body);
                    for (symbol, ty) in self.common.iter().rev() {
                        function = lam(Key::Source(*symbol), self.domain(*symbol, *ty), function);
                    }
                    let mut tail_value = label.clone();
                    for _ in self.common {
                        function = app(function, fst(tail_value.clone()));
                        tail_value = snd(tail_value);
                    }
                    Ok(app(function, tail_value))
                }
                fn out(&mut self, label: S) -> Result<S, &'static str> {
                    let mut indices = Vec::new();
                    for field in self.index_fields {
                        let ordinal = self
                            .common
                            .iter()
                            .position(|(symbol, _)| symbol == field)
                            .ok_or("source index descriptor is outside its owner")?;
                        indices.push(Self::projected(label.clone(), ordinal));
                    }
                    let Some(mut body) = indices.pop() else {
                        return Ok(self.terms.unit_value());
                    };
                    for index in indices.into_iter().rev() {
                        body = pair(index, body);
                    }
                    Ok(body)
                }
            }

            pub(super) fn constructor_children(
                arena: &mut TermArena,
                description: &IndexedFamily,
                label: TermHandle,
                children: &[TermHandle],
            ) -> Result<TermHandle, &'static str> {
                let child_domain = arena.insert(term!(Apply => description.children => label));
                let position = arena.insert(Term::Variable(0));
                let next = arena.insert(term!(Apply => description.next => label));
                let child_index = arena.insert(term!(Apply => next => position));
                let child_type = description.indexed_w(arena, child_index);
                let result_family = arena.insert(term!(Lambda => child_domain => child_type));
                let mut terms = SourceTerms::default();
                let position = terms.fresh();
                let values = children.iter().copied().map(S::Closed).collect::<Vec<_>>();
                let body = terms.select(&values, S::Var(position), S::Closed(result_family));
                lam(position, S::Closed(child_domain), body).term(arena, &mut Vec::new())
            }

            pub(super) struct SourceDescription {
                pub family: IndexedFamily,
                /// One shared product of complete case types, abstracted over ordered
                /// Type parameters and common values. All constructor and eliminator
                /// paths use this same definition as the universe lift's carrier.
                pub payload_lifts: Vec<TermHandle>,
            }

            /// Each multi-case payload is explicitly lifted by the same contractible
            /// witnesses. The shared sort is computed from represented fields and their
            /// constraints, not from the unused generic telescope. A single-case record
            /// needs no padding, and a Type0 Box therefore stays at Type0.
            ///
            /// Closed producer terms. The caller must check the resulting ordinary scheme
            /// application and retain source correspondence before admitting any use.
            pub(super) fn description(
                arena: &mut TermArena,
                common: &[(SymbolHandle, TermHandle)],
                index_fields: &[SymbolHandle],
                cases: &[Case],
                type_parameters: &[SymbolHandle],
                external_parameters: &[SymbolHandle],
                declarations: &mut Vec<Declaration>,
                type_domains: &[TermHandle],
                level_arity: u32,
            ) -> Result<SourceDescription, String> {
                let mut b = Builder {
                    terms: SourceTerms::default(),
                    common,
                    index_fields,
                    cases,
                    type_parameters,
                    external_actuals: &[],
                    payload_level: Level::Constant(0),
                    payload_lifts: &[],
                };
                // Close every entire branch type over the shared telescope. A constraint
                // can live above all case-local fields, so field domains alone cannot
                // supply the required lift. No inhabitant of another case is assumed.
                let functions = b.payload_type_functions(arena, type_domains)?;
                let context = proof_admission::Context::with_level_arity(level_arity)
                    .with_signature(proof_admission::Signature::from_declarations(
                        declarations.to_vec(),
                    ));
                let mut payload_level = Level::Constant(0);
                for &function in &functions {
                    payload_level =
                        payload_level.maximum(result_sort_level_in(arena, &context, function)?);
                }
                let payload_lifts = if cases.len() > 1 && payload_level != Level::Constant(0) {
                    // A single shared product has the maximum of all complete branch
                    // sorts. Unit(product) needs no branch inhabitant and adds only one
                    // component per alternative, rather than a quadratic padding roster.
                    let body = b.shared_payload_type_function(arena, type_domains)?;
                    let ty = proof_admission::infer_type(
                        arena,
                        &context,
                        body,
                        &mut proof_admission::Budget::new(
                            proof_admission::DEFAULT_CONVERSION_STEPS,
                        ),
                    )
                    .map_err(|error| {
                        format!("shared source payload carrier fails formation: {error:?}")
                    })?;
                    let declaration = declarations.len() as u32;
                    declarations.push(Declaration::definition(level_arity, ty, body));
                    vec![arena.insert(Term::Constant {
                        declaration,
                        levels: (0..level_arity).map(Level::Parameter).collect(),
                    })]
                } else {
                    Vec::new()
                };
                b.payload_level = payload_level.clone();
                b.payload_lifts = &payload_lifts;
                let carrier = b.carrier();
                let index = b.index_type()?;
                let a = b.terms.fresh();
                let children_body = b.unpack(S::Var(a), ResultKind::Positions)?;
                let children = lam(a, carrier.clone(), children_body);
                let a = b.terms.fresh();
                let out_body = b.out(S::Var(a))?;
                let out = lam(a, carrier.clone(), out_body);
                let a = b.terms.fresh();
                let next_body = b.unpack(S::Var(a), ResultKind::Next)?;
                let next = lam(a, carrier.clone(), next_body);
                let context = proof_admission::Context::with_level_arity(level_arity)
                    .with_signature(proof_admission::Signature::from_declarations(
                        declarations.to_vec(),
                    ));
                let mut index_level = Level::Constant(0);
                let mut carrier_level = payload_level;
                for (symbol, domain) in common {
                    let level = result_sort_level_in(arena, &context, *domain)?;
                    carrier_level = carrier_level.maximum(level.clone());
                    if index_fields.contains(symbol) {
                        index_level = index_level.maximum(level);
                    }
                }
                let mut scope = external_parameters
                    .iter()
                    .copied()
                    .map(Key::Source)
                    .collect::<Vec<_>>();
                Ok(SourceDescription {
                    family: IndexedFamily {
                        levels: [index_level, carrier_level, Level::Constant(0)],
                        index: index.term(arena, &mut scope)?,
                        carrier: carrier.term(arena, &mut scope)?,
                        children: children.term(arena, &mut scope)?,
                        out: out.term(arena, &mut scope)?,
                        next: next.term(arena, &mut scope)?,
                    },
                    payload_lifts,
                })
            }

            /// Share a description under its declaration parameters. Type parameters
            /// outside the W label stay ordinary Pi/Lambda parameters of all five fields.
            /// Only the remaining true indices and stored payloads contribute to W levels.
            pub(super) fn intern_description(
                arena: &mut TermArena,
                declarations: &mut Vec<Declaration>,
                mut family: IndexedFamily,
                parameter_domains: &[TermHandle],
                level_arity: u32,
            ) -> Result<IndexedFamily, &'static str> {
                fn retain(
                    arena: &mut TermArena,
                    declarations: &mut Vec<Declaration>,
                    mut ty: TermHandle,
                    mut body: TermHandle,
                    parameter_domains: &[TermHandle],
                    level_arity: u32,
                ) -> (TermHandle, TermHandle) {
                    let parameter_count = parameter_domains.len();
                    for &parameter_sort in parameter_domains.iter().rev() {
                        body = arena.insert(Term::Lambda {
                            domain: parameter_sort,
                            body,
                        });
                        ty = arena.insert(Term::Pi {
                            domain: parameter_sort,
                            codomain: ty,
                        });
                    }
                    let declaration = declarations.len() as u32;
                    declarations.push(Declaration::definition(level_arity, ty, body));
                    let constant = arena.insert(
                        term!(Constant => declaration => (0..level_arity).map(Level::Parameter).collect()),
                    );
                    let mut applied = constant;
                    for ordinal in 0..parameter_count {
                        let argument =
                            arena.insert(Term::Variable((parameter_count - ordinal - 1) as u32));
                        applied = arena.insert(Term::Apply {
                            function: applied,
                            argument,
                        });
                    }
                    (constant, applied)
                }
                let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                let index_sort = arena.insert(Term::Sort(Sort::Type(family.levels[0].clone())));
                let carrier_sort = arena.insert(Term::Sort(Sort::Type(family.levels[1].clone())));
                let (index, applied_index) = retain(
                    arena,
                    declarations,
                    index_sort,
                    family.index,
                    parameter_domains,
                    level_arity,
                );
                let (carrier, applied_carrier) = retain(
                    arena,
                    declarations,
                    carrier_sort,
                    family.carrier,
                    parameter_domains,
                    level_arity,
                );
                let ty = arena.insert(term!(Pi => applied_carrier => sort));
                let (children, applied_children) = retain(
                    arena,
                    declarations,
                    ty,
                    family.children,
                    parameter_domains,
                    level_arity,
                );
                let index_under_label = shift(arena, applied_index, 0, 1);
                let ty = arena.insert(term!(Pi => applied_carrier => index_under_label));
                let (out, _) = retain(
                    arena,
                    declarations,
                    ty,
                    family.out,
                    parameter_domains,
                    level_arity,
                );
                let a = arena.insert(Term::Variable(0));
                let children_under_label = shift(arena, applied_children, 0, 1);
                let domain = arena.insert(term!(Apply => children_under_label => a));
                let index_under_position = shift(arena, applied_index, 0, 2);
                let codomain = arena.insert(term!(Pi => domain => index_under_position));
                let ty = arena.insert(term!(Pi => applied_carrier => codomain));
                let (next, _) = retain(
                    arena,
                    declarations,
                    ty,
                    family.next,
                    parameter_domains,
                    level_arity,
                );
                family.index = index;
                family.carrier = carrier;
                family.children = children;
                family.out = out;
                family.next = next;
                Ok(family)
            }

            pub(super) fn apply_description(
                arena: &mut TermArena,
                template: &IndexedFamily,
                parameters: &[TermHandle],
            ) -> IndexedFamily {
                fn apply(
                    arena: &mut TermArena,
                    mut function: TermHandle,
                    parameters: &[TermHandle],
                ) -> TermHandle {
                    for &argument in parameters {
                        function = arena.insert(Term::Apply { function, argument });
                    }
                    function
                }
                IndexedFamily {
                    levels: template.levels.clone(),
                    index: apply(arena, template.index, parameters),
                    carrier: apply(arena, template.carrier, parameters),
                    children: apply(arena, template.children, parameters),
                    out: apply(arena, template.out, parameters),
                    next: apply(arena, template.next, parameters),
                }
            }

            struct InductionBuilder<'a, 'b> {
                b: Builder<'a>,
                family: &'b IndexedFamily,
                iw: TermHandle,
                label_pack: TermHandle,
                logical_result: TermHandle,
                reconstruction_equality: TermHandle,
                invariant: TermHandle,
                lemmas: &'b [TermHandle],
            }
            impl InductionBuilder<'_, '_> {
                fn reconstructed_label(&mut self, label: S) -> S {
                    let mut function = S::Closed(self.label_pack);
                    let mut tail = label;
                    for _ in self.b.common {
                        function = app(function, fst(tail.clone()));
                        tail = snd(tail);
                    }
                    if self.b.cases.len() > 1 {
                        tail = pair(fst(tail.clone()), snd(tail));
                    }
                    app(function, tail)
                }
                fn label(&mut self, start: usize, mut value: S) -> S {
                    for _ in 0..start {
                        value = pair(S::One, value);
                    }
                    let mut function = S::Closed(self.label_pack);
                    for (symbol, _) in self.b.common {
                        function = app(function, S::Var(Key::Source(*symbol)));
                    }
                    app(function, value)
                }
                fn result_type(&mut self, start: usize, value: S) -> S {
                    let label = self.label(start, value);
                    app(S::Closed(self.logical_result), label)
                }
                fn type_at_label(&mut self, label: S) -> S {
                    let positions = app(S::Closed(self.family.children), label.clone());
                    let p = self.b.terms.fresh();
                    let next = app(app(S::Closed(self.family.next), label.clone()), S::Var(p));
                    let children = pi(p, positions, app(S::Closed(self.iw), next));
                    let g = self.b.terms.fresh();
                    pi(g, children, app(S::Closed(self.logical_result), label))
                }
                fn unit_value(&mut self) -> S {
                    let x = self.b.terms.fresh();
                    let ty = self.b.terms.unit_strict();
                    S::BoxIntro(Box::new(ty), Box::new(lam(x, S::Empty, S::Var(x))))
                }
                fn position(&mut self, count: usize, ordinal: usize) -> S {
                    if count == 1 {
                        return self.unit_value();
                    }
                    if ordinal == 0 {
                        let unit = self.unit_value();
                        return pair(S::Zero, unit);
                    }
                    let tail = self.position(count - 1, ordinal - 1);
                    pair(S::One, tail)
                }
                fn case_body(&mut self, case: usize, payload: S) -> S {
                    let sum_value = if case + 1 == self.b.cases.len() {
                        payload.clone()
                    } else {
                        pair(S::Zero, payload.clone())
                    };
                    let label = self.label(case, sum_value);
                    let positions = app(S::Closed(self.family.children), label.clone());
                    let p = self.b.terms.fresh();
                    let next = app(app(S::Closed(self.family.next), label), S::Var(p));
                    let hypotheses = pi(p, positions, app(S::Closed(self.invariant), next));
                    let ih = self.b.terms.fresh();
                    let mut body = S::Closed(self.lemmas[case]);
                    for (symbol, _) in self.b.common {
                        body = app(body, S::Var(Key::Source(*symbol)));
                    }
                    let mut predicate = self.b.unlift_payload(payload.clone());
                    for _ in &self.b.cases[case].payload {
                        body = app(body, fst(predicate.clone()));
                        predicate = snd(predicate);
                    }
                    body = app(body, predicate);
                    let count = self.b.cases[case].child_indices.len();
                    for child in 0..count {
                        let position = self.position(count, child);
                        body = app(body, app(S::Var(ih), position));
                    }
                    lam(ih, hypotheses, body)
                }
                fn eliminate(&mut self, start: usize, value: S) -> Result<S, &'static str> {
                    if start >= self.b.cases.len() {
                        return Err(
                            "source induction cannot eliminate an empty constructor roster",
                        );
                    }
                    if start + 1 == self.b.cases.len() {
                        return Ok(self.case_body(start, value));
                    }
                    let tag = self.b.terms.fresh();
                    let payload = self.b.terms.fresh();
                    let tm = self.b.terms.type_motive_at(self.b.payload_level.clone());
                    let zero_type = self.b.payload_type(start);
                    let one_type = self.b.sum_type(start + 1);
                    let payload_type = choose(tm, zero_type.clone(), one_type.clone(), S::Var(tag));
                    let result = self.result_type(start, pair(S::Var(tag), S::Var(payload)));
                    let motive = lam(tag, S::Two, pi(payload, payload_type, result));
                    let z = self.b.terms.fresh();
                    let zero_body = self.case_body(start, S::Var(z));
                    let zero = lam(z, zero_type, zero_body);
                    let o = self.b.terms.fresh();
                    let one_body = self.eliminate(start + 1, S::Var(o))?;
                    let one = lam(o, one_type, one_body);
                    Ok(app(
                        choose(motive, zero, one, fst(value.clone())),
                        snd(value),
                    ))
                }
                fn step(&mut self) -> Result<S, &'static str> {
                    let a = self.b.terms.fresh();
                    let tail = self.b.terms.fresh();
                    let sum_type = self.b.sum_type(0);
                    let body = self.eliminate(0, S::Var(tail))?;
                    let mut function = lam(tail, sum_type, body);
                    for (symbol, ty) in self.b.common.iter().rev() {
                        function = lam(Key::Source(*symbol), self.b.domain(*symbol, *ty), function);
                    }
                    let mut value = S::Var(a);
                    for _ in self.b.common {
                        function = app(function, fst(value.clone()));
                        value = snd(value);
                    }
                    let p = self.b.terms.fresh();
                    let positions = app(S::Closed(self.family.children), S::Var(a));
                    let next = app(app(S::Closed(self.family.next), S::Var(a)), S::Var(p));
                    let children = pi(p, positions, app(S::Closed(self.iw), next));
                    let g = self.b.terms.fresh();
                    let endpoint = self.b.terms.fresh();
                    let equality = self.b.terms.fresh();
                    let reconstructed = self.reconstructed_label(S::Var(a));
                    let equality_domain = S::Identity(
                        Box::new(S::Closed(self.family.carrier)),
                        Box::new(reconstructed),
                        Box::new(S::Var(endpoint)),
                    );
                    let motive = lam(
                        endpoint,
                        S::Closed(self.family.carrier),
                        lam(
                            equality,
                            equality_domain,
                            app(S::Closed(self.logical_result), S::Var(endpoint)),
                        ),
                    );
                    let transported = S::IdentityElimination(
                        Box::new(motive),
                        Box::new(app(function, value)),
                        Box::new(S::Var(a)),
                        Box::new(app(S::Closed(self.reconstruction_equality), S::Var(a))),
                    );
                    Ok(lam(
                        a,
                        S::Closed(self.family.carrier),
                        lam(g, children, transported),
                    ))
                }
            }

            pub(super) fn intern_invariant(
                arena: &mut TermArena,
                declarations: &mut Vec<Declaration>,
                family: &IndexedFamily,
                invariant: TermHandle,
                result_level: Level,
            ) -> TermHandle {
                let sort = arena.insert(Term::Sort(Sort::Type(result_level)));
                let ty = arena.insert(term!(Pi => family.index => sort));
                let declaration = declarations.len() as u32;
                declarations.push(Declaration::definition(
                    proof_admission::universe_parameter_arity(arena, &[ty, invariant]),
                    ty,
                    invariant,
                ));
                arena.insert(term!(Constant => declaration => (0..declarations[declaration as usize].level_arity).map(Level::Parameter).collect()))
            }

            pub(super) fn constant_invariant_theorem(
                arena: &mut TermArena,
                family: &IndexedFamily,
                invariant: TermHandle,
                proof: TermHandle,
            ) -> (TermHandle, TermHandle) {
                let i = arena.insert(Term::Variable(0));
                let evidence_type = family.indexed_w(arena, i);
                let closed = shift(arena, proof, 0, 2);
                let body = arena.insert(term!(Lambda => evidence_type => closed));
                let theorem = arena.insert(term!(Lambda => family.index => body));
                let i = arena.insert(Term::Variable(1));
                let proposition = arena.insert(term!(Apply => invariant => i));
                let result = arena.insert(term!(Pi => evidence_type => proposition));
                let ty = arena.insert(term!(Pi => family.index => result));
                (theorem, ty)
            }

            pub(super) fn induction(
                arena: &mut TermArena,
                family: &IndexedFamily,
                common: &[(SymbolHandle, TermHandle)],
                index_fields: &[SymbolHandle],
                cases: &[Case],
                invariant: TermHandle,
                lemmas: &[TermHandle],
                declarations: &mut Vec<Declaration>,
                type_parameters: &[SymbolHandle],
                external_actuals: &[(SymbolHandle, TermHandle)],
                payload_lifts: &[TermHandle],
            ) -> Result<(TermHandle, TermHandle, TermHandle, TermHandle), String> {
                let level_arity = proof_admission::universe_parameter_arity(
                    arena,
                    &[family.index, family.carrier, invariant],
                );
                let payload_level = payload_sort_level(arena, declarations, cases, level_arity)?;
                let result_level = result_sort_level(arena, declarations, invariant, level_arity)?;
                if cases.len() != lemmas.len() {
                    return Err(
                        "source induction lemma roster differs from its exact cases".to_owned()
                    );
                }
                fn retain(
                    arena: &mut TermArena,
                    declarations: &mut Vec<Declaration>,
                    ty: TermHandle,
                    body: TermHandle,
                ) -> TermHandle {
                    let declaration = declarations.len() as u32;
                    declarations.push(Declaration::definition(
                        proof_admission::universe_parameter_arity(arena, &[ty, body]),
                        ty,
                        body,
                    ));
                    arena.insert(term!(Constant => declaration => (0..declarations[declaration as usize].level_arity).map(Level::Parameter).collect()))
                }
                let w_sort = arena.insert(Term::Sort(Sort::Type(
                    family
                        .levels
                        .iter()
                        .cloned()
                        .reduce(|left, right| left.maximum(right))
                        .expect("three W levels"),
                )));
                let function_type = arena.insert(term!(Pi => family.index => w_sort));
                let i = arena.insert(Term::Variable(0));
                let iw_body = family.indexed_w(arena, i);
                let iw_function = arena.insert(term!(Lambda => family.index => iw_body));
                let iw = retain(arena, declarations, function_type, iw_function);
                let iw_body = arena.insert(term!(Apply => iw => i));
                let mut builder = Builder {
                    terms: SourceTerms::default(),
                    common,
                    index_fields,
                    cases,
                    type_parameters,
                    external_actuals,
                    payload_level: payload_level.clone(),
                    payload_lifts,
                };
                let tail = builder.terms.fresh();
                let sum_type = builder.sum_type(0);
                let mut raw_label = S::Var(tail);
                for (symbol, _) in common.iter().rev() {
                    raw_label = pair(S::Var(Key::Source(*symbol)), raw_label);
                }
                let mut pack = lam(tail, sum_type.clone(), raw_label);
                let mut pack_type = pi(tail, sum_type, S::Closed(family.carrier));
                for (symbol, ty) in common.iter().rev() {
                    pack = lam(Key::Source(*symbol), builder.domain(*symbol, *ty), pack);
                    pack_type = pi(
                        Key::Source(*symbol),
                        builder.domain(*symbol, *ty),
                        pack_type,
                    );
                }
                let pack = pack.term(arena, &mut Vec::new())?;
                let pack_type = pack_type.term(arena, &mut Vec::new())?;
                let label_pack = retain(arena, declarations, pack_type, pack);
                let a = arena.insert(Term::Variable(0));
                let positions = arena.insert(term!(Apply => family.children => a));
                let next = arena.insert(term!(Apply => family.next => a));
                let position = arena.insert(Term::Variable(0));
                let next = shift(arena, next, 0, 1);
                let next = arena.insert(term!(Apply => next => position));
                let hypothesis = arena.insert(term!(Apply => invariant => next));
                let hypotheses = arena.insert(term!(Pi => positions => hypothesis));
                let out = arena.insert(term!(Apply => family.out => a));
                let goal = arena.insert(term!(Apply => invariant => out));
                let goal = shift(arena, goal, 0, 1);
                let result = arena.insert(term!(Pi => hypotheses => goal));
                let result = arena.insert(term!(Lambda => family.carrier => result));
                let proposition_sort = arena.insert(Term::Sort(Sort::Type(result_level.clone())));
                let result_type = arena.insert(term!(Pi => family.carrier => proposition_sort));
                let logical_result = retain(arena, declarations, result_type, result);
                let mut producer = InductionBuilder {
                    b: Builder {
                        terms: SourceTerms::default(),
                        common,
                        index_fields,
                        cases,
                        type_parameters,
                        external_actuals,
                        payload_level,
                        payload_lifts,
                    },
                    family,
                    iw,
                    label_pack,
                    logical_result,
                    reconstruction_equality: TermHandle::default(),
                    invariant,
                    lemmas,
                };
                let label = producer.b.terms.fresh();
                let reconstructed = producer.reconstructed_label(S::Var(label));
                let equality_type = pi(
                    label,
                    S::Closed(family.carrier),
                    S::Identity(
                        Box::new(S::Closed(family.carrier)),
                        Box::new(reconstructed.clone()),
                        Box::new(S::Var(label)),
                    ),
                )
                .term(arena, &mut Vec::new())?;
                let equality_body = lam(
                    label,
                    S::Closed(family.carrier),
                    S::Reflexivity(Box::new(S::Closed(family.carrier)), Box::new(reconstructed)),
                )
                .term(arena, &mut Vec::new())?;
                producer.reconstruction_equality =
                    retain(arena, declarations, equality_type, equality_body);
                let step_body = producer.step()?.term(arena, &mut Vec::new())?;
                let a = producer.b.terms.fresh();
                let result_type = producer.type_at_label(S::Var(a));
                let step_type =
                    pi(a, S::Closed(family.carrier), result_type).term(arena, &mut Vec::new())?;
                let declaration = declarations.len() as u32;
                declarations.push(Declaration::definition(
                    proof_admission::universe_parameter_arity(arena, &[step_type, step_body]),
                    step_type,
                    step_body,
                ));
                let step = arena.insert(term!(Constant => declaration => (0..declarations[declaration as usize].level_arity).map(Level::Parameter).collect()));
                let i = arena.insert(Term::Variable(1));
                let proposition = arena.insert(term!(Apply => invariant => i));
                let motive_body = arena.insert(term!(Lambda => iw_body => proposition));
                let motive = arena.insert(term!(Lambda => family.index => motive_body));
                let tree = arena.insert(Term::Variable(0));
                let body = family.ind(arena, result_level, motive, step, i, tree);
                let body = arena.insert(term!(Lambda => iw_body => body));
                let theorem = arena.insert(term!(Lambda => family.index => body));
                let result = arena.insert(term!(Pi => iw_body => proposition));
                let ty = arena.insert(term!(Pi => family.index => result));
                Ok((theorem, ty, step_body, step_type))
            }

            #[cfg(test)]
            mod induction_tests {
                use super::*;
                use proof_admission::{Budget, Context, DEFAULT_CONVERSION_STEPS, Signature};
                use proof_admission::{check_signature, check_type, indexed_scheme, infer_type};

                #[test]
                fn explicit_payload_lift_preserves_its_value_at_the_required_universe() {
                    let mut arena = TermArena::new();
                    let higher = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                    let two = arena.insert(Term::Two);
                    let zero = arena.insert(Term::TwoZero);
                    let mut terms = SourceTerms::default();
                    let (unit, witness) = terms.level_unit(S::Closed(higher));
                    let unit = unit.term(&mut arena, &mut Vec::new()).unwrap();
                    let witness = witness.term(&mut arena, &mut Vec::new()).unwrap();
                    let context = Context::with_level_arity(1);
                    check_type(&mut arena, &context, witness, unit, &mut Budget::default())
                        .unwrap();
                    // Every boxed inhabitant contracts to the canonical witness. The
                    // eliminator enters the strict proof binder, where irrelevance makes
                    // its boxed value agree with the canonical proof; no Box eta axiom is
                    // needed to prove this relevant identity.
                    let Term::Box { ty: proposition } = arena.get(unit) else {
                        unreachable!()
                    };
                    let boxed = arena.insert(Term::Variable(0));
                    let conclusion = arena.insert(Term::Id {
                        ty: unit,
                        left: witness,
                        right: boxed,
                    });
                    let motive = arena.insert(Term::Lambda {
                        domain: unit,
                        body: conclusion,
                    });
                    let proof = arena.insert(Term::Variable(0));
                    let boxed_proof = arena.insert(Term::BoxIntro {
                        ty: proposition,
                        value: proof,
                    });
                    let reflexive = arena.insert(Term::Refl {
                        ty: unit,
                        value: boxed_proof,
                    });
                    let branch = arena.insert(Term::Lambda {
                        domain: proposition,
                        body: reflexive,
                    });
                    let contraction = arena.insert(Term::BoxElim {
                        motive,
                        body: branch,
                        scrutinee: boxed,
                    });
                    check_type(
                        &mut arena,
                        &context.extend(unit),
                        contraction,
                        conclusion,
                        &mut Budget::default(),
                    )
                    .unwrap();

                    let unit_sort = arena.insert(Term::Sort(Sort::Type(
                        Level::Parameter(0).successor().unwrap(),
                    )));
                    check_type(
                        &mut arena,
                        &context,
                        unit,
                        unit_sort,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert!(
                        check_type(&mut arena, &context, unit, higher, &mut Budget::default())
                            .is_err()
                    );
                    let lifted = lift_payload(&mut arena, &[higher], zero).unwrap();
                    let expected = arena.insert(Term::Sigma {
                        domain: unit,
                        codomain: two,
                    });
                    check_type(
                        &mut arena,
                        &context,
                        lifted,
                        expected,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    let recovered = arena.insert(Term::Snd { pair: lifted });
                    assert!(
                        proof_admission::convertible(
                            &mut arena,
                            &context,
                            recovered,
                            zero,
                            two,
                            &mut Budget::default()
                        )
                        .unwrap()
                    );
                    // No implicit resizing: this lifted value is not an ordinary Two.
                    assert!(
                        check_type(&mut arena, &context, lifted, two, &mut Budget::default())
                            .is_err()
                    );
                    assert!(
                        check_type(
                            &mut arena,
                            &Context::empty(),
                            witness,
                            unit,
                            &mut Budget::default()
                        )
                        .is_err()
                    );
                }

                #[test]
                fn shared_high_universe_constraint_is_lifted_without_assuming_its_truth() {
                    let mut arena = TermArena::new();
                    let mut declarations = indexed_scheme(&mut arena);
                    let a_symbol = SymbolHandle::from_arena_index(71);
                    let x_symbol = SymbolHandle::from_arena_index(72);
                    let y_symbol = SymbolHandle::from_arena_index(73);
                    let a = Key::Source(a_symbol);
                    let x = Key::Source(x_symbol);
                    let y = Key::Source(y_symbol);
                    let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                    let field = lam(a, S::Closed(sort), S::Var(a))
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let mut terms = SourceTerms::default();
                    let unit = terms.unit().term(&mut arena, &mut Vec::new()).unwrap();
                    let unit_value = terms
                        .unit_value()
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let high = S::Identity(
                        Box::new(S::Var(a)),
                        Box::new(S::Var(x)),
                        Box::new(S::Var(y)),
                    );
                    let close = |body: S| {
                        lam(
                            a,
                            S::Closed(sort),
                            lam(x, S::Var(a), lam(y, S::Var(a), body)),
                        )
                    };
                    let high = close(high).term(&mut arena, &mut Vec::new()).unwrap();
                    let low = close(S::Closed(unit))
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let cases = [
                        Case {
                            payload: Vec::new(),
                            constraints: high,
                            child_indices: Vec::new(),
                        },
                        Case {
                            payload: Vec::new(),
                            constraints: low,
                            child_indices: Vec::new(),
                        },
                    ];
                    let described = description(
                        &mut arena,
                        &[(x_symbol, field), (y_symbol, field)],
                        &[],
                        &cases,
                        &[a_symbol],
                        &[a_symbol],
                        &mut declarations,
                        &[sort],
                        1,
                    )
                    .unwrap();
                    assert_eq!(described.payload_lifts.len(), 1);
                    let family = intern_description(
                        &mut arena,
                        &mut declarations,
                        described.family,
                        &[sort],
                        1,
                    )
                    .unwrap();
                    let signature =
                        check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
                    let two = arena.insert(Term::Two);
                    let zero = arena.insert(Term::TwoZero);
                    let one = arena.insert(Term::TwoOne);
                    let mut carrier = proof_admission::instantiate_levels(
                        &mut arena,
                        family.carrier,
                        &[Level::Constant(0)],
                    )
                    .unwrap();
                    carrier = arena.insert(Term::Apply {
                        function: carrier,
                        argument: two,
                    });
                    let mut lift_domains = Vec::new();
                    for function in described.payload_lifts {
                        let mut domain = proof_admission::instantiate_levels(
                            &mut arena,
                            function,
                            &[Level::Constant(0)],
                        )
                        .unwrap();
                        for argument in [two, zero, one] {
                            domain = arena.insert(Term::Apply {
                                function: domain,
                                argument,
                            });
                        }
                        lift_domains.push(domain);
                    }
                    let lifted = lift_payload(&mut arena, &lift_domains, unit_value).unwrap();
                    let mut label = arena.insert(Term::Pair {
                        first: one,
                        second: lifted,
                    });
                    label = arena.insert(Term::Pair {
                        first: one,
                        second: label,
                    });
                    label = arena.insert(Term::Pair {
                        first: zero,
                        second: label,
                    });
                    check_type(
                        &mut arena,
                        &Context::empty().with_signature(signature.clone()),
                        label,
                        carrier,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    // The other branch's Id Two zero one is uninhabited. Its lift needs
                    // only the TYPE, and cannot turn the low branch into that proof.
                    assert!(
                        check_type(
                            &mut arena,
                            &Context::empty().with_signature(signature),
                            unit_value,
                            lift_domains[0],
                            &mut Budget::default()
                        )
                        .is_err()
                    );

                    // An unused high-universe argument is outside the represented sum.
                    let phantom = lam(a, S::Closed(sort), S::Closed(unit))
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let phantom_cases = [
                        Case {
                            payload: Vec::new(),
                            constraints: phantom,
                            child_indices: Vec::new(),
                        },
                        Case {
                            payload: Vec::new(),
                            constraints: phantom,
                            child_indices: Vec::new(),
                        },
                    ];
                    let phantom_description = description(
                        &mut arena,
                        &[],
                        &[],
                        &phantom_cases,
                        &[a_symbol],
                        &[a_symbol],
                        &mut declarations,
                        &[sort],
                        1,
                    )
                    .unwrap();
                    assert!(phantom_description.payload_lifts.is_empty());
                    assert_eq!(
                        phantom_description.family.levels,
                        [Level::Constant(0), Level::Constant(0), Level::Constant(0)]
                    );
                }

                #[test]
                fn constructor_abstraction_binds_dependent_child_and_premise_domains() {
                    let mut arena = TermArena::new();
                    let two = arena.insert(Term::Two);
                    let value = arena.insert(term!(Constant => 0 => vec![]));
                    let predicate = arena.insert(Term::Id {
                        ty: two,
                        left: value,
                        right: value,
                    });
                    let evidence = arena.insert(term!(Constant => 1 => vec![]));
                    let declarations = vec![
                        Declaration::assumption(0, two),
                        Declaration::assumption(0, predicate),
                    ];
                    let parameters = [
                        (0, SymbolHandle::from_arena_index(31), two),
                        (1, SymbolHandle::from_arena_index(32), predicate),
                    ];
                    let (proof, statement) =
                        abstract_parameters(&mut arena, evidence, predicate, &parameters).unwrap();
                    assert!(excludes_parameters(
                        &arena,
                        &declarations,
                        &[proof, statement],
                        &parameters
                    ));
                    let context =
                        Context::empty().with_signature(Signature::from_declarations(declarations));
                    check_type(
                        &mut arena,
                        &context,
                        proof,
                        statement,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();

                    // The returned proof is parametric: changing the supplied child's
                    // endpoint cannot establish a different receiving equality.
                    let zero = arena.insert(Term::TwoZero);
                    let one = arena.insert(Term::TwoOne);
                    let supplied = arena.insert(term!(Apply => proof => zero));
                    let reflexivity = arena.insert(term!(Refl => two => zero));
                    let supplied = arena.insert(term!(Apply => supplied => reflexivity));
                    let false_result = arena.insert(Term::Id {
                        ty: two,
                        left: zero,
                        right: one,
                    });
                    assert!(
                        check_type(
                            &mut arena,
                            &context,
                            supplied,
                            false_result,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                        )
                        .is_err()
                    );
                }

                #[test]
                fn lifted_constructor_projection_and_induction_compute_at_a_generalized_level() {
                    let mut arena = TermArena::new();
                    let mut declarations = indexed_scheme(&mut arena);
                    let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                    let carrier_position = declarations.len() as u32;
                    declarations.push(Declaration::assumption(1, sort));
                    let carrier = arena.insert(Term::Constant {
                        declaration: carrier_position,
                        levels: vec![Level::Parameter(0)],
                    });
                    let two = arena.insert(Term::Two);
                    let zero = arena.insert(Term::TwoZero);
                    let one = arena.insert(Term::TwoOne);
                    let mut terms = SourceTerms::default();
                    let unit = terms.unit().term(&mut arena, &mut Vec::new()).unwrap();
                    let unit_value = terms
                        .unit_value()
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let fields = [81, 82].map(SymbolHandle::from_arena_index);
                    let cases = [(fields[0], carrier), (fields[1], two)]
                        .into_iter()
                        .map(|(field, domain)| Case {
                            payload: vec![(field, domain)],
                            constraints: lam(
                                Key::Source(field),
                                S::Closed(domain),
                                S::Closed(unit),
                            )
                            .term(&mut arena, &mut Vec::new())
                            .unwrap(),
                            child_indices: vec![],
                        })
                        .collect::<Vec<_>>();
                    let described = description(
                        &mut arena,
                        &[],
                        &[],
                        &cases,
                        &[],
                        &[],
                        &mut declarations,
                        &[],
                        1,
                    )
                    .unwrap();
                    assert_eq!(described.payload_lifts.len(), 1);
                    assert_eq!(described.family.levels[1].parameter_arity(), 1);
                    let family =
                        intern_description(&mut arena, &mut declarations, described.family, &[], 1)
                            .unwrap();

                    // Build the low branch in the same high-level descriptor. The extra
                    // component is generated from the retained plan, without an A value.
                    let payload = arena.insert(Term::Pair {
                        first: one,
                        second: unit_value,
                    });
                    let payload =
                        lift_payload(&mut arena, &described.payload_lifts, payload).unwrap();
                    let label = arena.insert(Term::Pair {
                        first: one,
                        second: payload,
                    });
                    let projected = arena.insert(Term::Snd { pair: label });
                    let projected = arena.insert(Term::Snd { pair: projected });
                    let projected = arena.insert(Term::Fst { pair: projected });
                    let children = constructor_children(&mut arena, &family, label, &[]).unwrap();
                    let constructor = family.sup(&mut arena, label, children);
                    let index = arena.insert(Term::Apply {
                        function: family.out,
                        argument: label,
                    });
                    let constructor_type = family.indexed_w(&mut arena, index);

                    // The result itself lives at the generalized level. Returning the
                    // low branch's actual field makes dropping the padding incorrectly
                    // observable in the derived computation, rather than just formation.
                    let (result_unit, result_witness) = terms.level_unit(S::Closed(carrier));
                    let result_unit = result_unit.term(&mut arena, &mut Vec::new()).unwrap();
                    let result_witness = result_witness.term(&mut arena, &mut Vec::new()).unwrap();
                    let result_type = arena.insert(Term::Sigma {
                        domain: result_unit,
                        codomain: two,
                    });
                    let invariant = arena.insert(Term::Lambda {
                        domain: family.index,
                        body: result_type,
                    });
                    let invariant = intern_invariant(
                        &mut arena,
                        &mut declarations,
                        &family,
                        invariant,
                        Level::Parameter(0),
                    );
                    let mut lemmas = Vec::new();
                    for (ordinal, case) in cases.iter().enumerate() {
                        let (field, domain) = case.payload[0];
                        let value = if ordinal == 0 {
                            S::Closed(zero)
                        } else {
                            S::Var(Key::Source(field))
                        };
                        let predicate = terms.fresh();
                        let body = lam(
                            Key::Source(field),
                            S::Closed(domain),
                            lam(
                                predicate,
                                S::Closed(unit),
                                pair(S::Closed(result_witness), value),
                            ),
                        )
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                        let context = Context::with_level_arity(1)
                            .with_signature(Signature::from_declarations(declarations.clone()));
                        let ty =
                            infer_type(&mut arena, &context, body, &mut Budget::default()).unwrap();
                        let declaration = declarations.len() as u32;
                        declarations.push(Declaration::definition(1, ty, body));
                        lemmas.push(arena.insert(Term::Constant {
                            declaration,
                            levels: vec![Level::Parameter(0)],
                        }));
                    }
                    let (theorem, theorem_type, step, step_type) = induction(
                        &mut arena,
                        &family,
                        &[],
                        &[],
                        &cases,
                        invariant,
                        &lemmas,
                        &mut declarations,
                        &[],
                        &[],
                        &described.payload_lifts,
                    )
                    .unwrap();
                    let signature =
                        check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
                    let context = Context::with_level_arity(1).with_signature(signature.clone());
                    for (term, ty) in [
                        (family.carrier, sort),
                        (label, family.carrier),
                        (constructor, constructor_type),
                        (step, step_type),
                        (theorem, theorem_type),
                    ] {
                        check_type(&mut arena, &context, term, ty, &mut Budget::default()).unwrap();
                    }
                    assert!(
                        proof_admission::convertible(
                            &mut arena,
                            &context,
                            projected,
                            one,
                            two,
                            &mut Budget::default()
                        )
                        .unwrap()
                    );
                    let at_index = arena.insert(Term::Apply {
                        function: theorem,
                        argument: index,
                    });
                    let computed = arena.insert(Term::Apply {
                        function: at_index,
                        argument: constructor,
                    });
                    let expected = arena.insert(Term::Pair {
                        first: result_witness,
                        second: one,
                    });
                    let wrong_result = arena.insert(Term::Pair {
                        first: result_witness,
                        second: zero,
                    });
                    check_type(
                        &mut arena,
                        &context,
                        computed,
                        result_type,
                        &mut Budget::default(),
                    )
                    .unwrap();
                    assert!(
                        proof_admission::convertible(
                            &mut arena,
                            &context,
                            computed,
                            expected,
                            result_type,
                            &mut Budget::default()
                        )
                        .unwrap()
                    );
                    assert!(
                        !proof_admission::convertible(
                            &mut arena,
                            &context,
                            computed,
                            wrong_result,
                            result_type,
                            &mut Budget::default()
                        )
                        .unwrap()
                    );
                    assert!(
                        check_type(
                            &mut arena,
                            &Context::empty().with_signature(signature),
                            theorem,
                            theorem_type,
                            &mut Budget::default(),
                        )
                        .is_err()
                    );
                }

                #[test]
                fn source_case_abstraction_preserves_bound_proof_variables_and_dependency_closure()
                {
                    let mut arena = TermArena::new();
                    let two = arena.insert(Term::Two);
                    let mut declarations = vec![Declaration {
                        level_arity: 0,
                        ty: two,
                        body: None,
                    }];
                    let value = arena.insert(term!(Constant => 0 => vec![]));
                    let predicate = arena.insert(Term::Id {
                        ty: two,
                        left: value,
                        right: value,
                    });
                    let bound = arena.insert(Term::Variable(0));
                    let proof = arena.insert(term!(Lambda => predicate => bound));
                    let ty = arena.insert(term!(Pi => predicate => predicate));
                    let parameters = [(0, SymbolHandle::from_arena_index(31), two)];
                    assert!(!excludes_parameters(
                        &arena,
                        &declarations,
                        &[proof, ty],
                        &parameters
                    ));
                    let (proof, ty) =
                        abstract_parameters(&mut arena, proof, ty, &parameters).unwrap();
                    assert!(excludes_parameters(
                        &arena,
                        &declarations,
                        &[proof, ty],
                        &parameters
                    ));
                    let context = Context::empty()
                        .with_signature(Signature::from_declarations(declarations.clone()));
                    check_type(
                        &mut arena,
                        &context,
                        proof,
                        ty,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();
                    declarations.push(Declaration {
                        level_arity: 0,
                        ty: two,
                        body: Some(value),
                    });
                    let hidden = arena.insert(term!(Constant => 1 => vec![]));
                    assert!(!excludes_parameters(
                        &arena,
                        &declarations,
                        &[hidden],
                        &parameters
                    ));
                }

                #[test]
                fn generated_source_induction_checks_multiple_cases_and_child_positions() {
                    let mut arena = TermArena::new();
                    let mut declarations = indexed_scheme(&mut arena);
                    let two = arena.insert(Term::Two);
                    let field = SymbolHandle::from_arena_index(2);
                    let x = Key::Source(field);
                    let empty = arena.insert(Term::Empty);
                    let strict_unit = arena.insert(term!(Pi => empty => empty));
                    let unit = arena.insert(Term::Box { ty: strict_unit });
                    let constraints = lam(x, S::Closed(two), S::Closed(unit))
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let child_index = lam(x, S::Closed(two), S::Var(x))
                        .term(&mut arena, &mut Vec::new())
                        .unwrap();
                    let cases = [0usize, 1]
                        .into_iter()
                        .map(|count| Case {
                            payload: vec![],
                            constraints,
                            child_indices: vec![child_index; count],
                        })
                        .collect::<Vec<_>>();
                    let family = description(
                        &mut arena,
                        &[(field, two)],
                        &[field],
                        &cases,
                        &[],
                        &[],
                        &mut declarations,
                        &[],
                        0,
                    )
                    .unwrap()
                    .family;
                    let family =
                        intern_description(&mut arena, &mut declarations, family, &[], 0).unwrap();
                    let index = arena.insert(Term::Variable(0));
                    let reflexive = arena.insert(Term::Id {
                        ty: two,
                        left: index,
                        right: index,
                    });
                    let invariant = arena.insert(term!(Lambda => two => reflexive));
                    let invariant = intern_invariant(
                        &mut arena,
                        &mut declarations,
                        &family,
                        invariant,
                        Level::Constant(0),
                    );
                    let mut lemmas = Vec::new();
                    for case in &cases {
                        let constant =
                            arena.insert(Term::Variable((case.child_indices.len() + 1) as u32));
                        let mut body = arena.insert(term!(Refl => two => constant));
                        for ordinal in (0..case.child_indices.len()).rev() {
                            let value = arena.insert(Term::Variable((ordinal + 1) as u32));
                            let domain = arena.insert(Term::Id {
                                ty: two,
                                left: value,
                                right: value,
                            });
                            body = arena.insert(Term::Lambda { domain, body });
                        }
                        body = arena.insert(Term::Lambda { domain: unit, body });
                        body = arena.insert(Term::Lambda { domain: two, body });
                        let context = Context::empty()
                            .with_signature(Signature::from_declarations(declarations.clone()));
                        let ty = infer_type(
                            &mut arena,
                            &context,
                            body,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                        )
                        .unwrap();
                        let declaration = declarations.len() as u32;
                        declarations.push(Declaration::definition(0, ty, body));
                        lemmas.push(arena.insert(term!(Constant => declaration => vec![])));
                    }
                    let (theorem, ty, step, step_type) = induction(
                        &mut arena,
                        &family,
                        &[(field, two)],
                        &[field],
                        &cases,
                        invariant,
                        &lemmas,
                        &mut declarations,
                        &[],
                        &[],
                        &[],
                    )
                    .unwrap();
                    let context = Context::empty()
                        .with_signature(Signature::from_declarations(declarations.clone()));
                    check_type(
                        &mut arena,
                        &context,
                        step,
                        step_type,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();
                    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                    check_signature(&mut arena, &declarations, &mut budget).unwrap();
                    let context =
                        Context::empty().with_signature(Signature::from_declarations(declarations));
                    check_type(
                        &mut arena,
                        &context,
                        theorem,
                        ty,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();
                    let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                    assert!(
                        check_type(
                            &mut arena,
                            &context,
                            theorem,
                            sort,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                        )
                        .is_err()
                    );
                }
            }

            /// Check actual dependency closure rather than the ambient signature roster:
            /// a temporary value assumption may remain stored, but no theorem declaration
            /// or type can reach it through a transparent definition or another constant.
            #[cfg(test)]
            mod indexed_vector_budget_tests {
                use super::*;
                use numerics::bignum::BigInt;
                use proof_admission::{
                    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Signature,
                    SourceIntegerTheory,
                };
                use proof_admission::{
                    check_signature, check_type, convertible, indexed_scheme, infer_type,
                    judgment_assumption_closure,
                };

                fn apply(
                    arena: &mut TermArena,
                    function: TermHandle,
                    argument: TermHandle,
                ) -> TermHandle {
                    arena.insert(Term::Apply { function, argument })
                }
                fn binary(
                    arena: &mut TermArena,
                    operation: TermHandle,
                    left: TermHandle,
                    right: TermHandle,
                ) -> TermHandle {
                    let function = apply(arena, operation, left);
                    apply(arena, function, right)
                }
                fn equality(
                    arena: &mut TermArena,
                    integer: TermHandle,
                    left: TermHandle,
                    right: TermHandle,
                ) -> TermHandle {
                    arena.insert(Term::Id {
                        ty: integer,
                        left,
                        right,
                    })
                }
                fn and(arena: &mut TermArena, left: TermHandle, right: TermHandle) -> TermHandle {
                    let codomain = shift(arena, right, 0, 1);
                    arena.insert(term!(Sigma => left => codomain))
                }
                fn close(
                    arena: &mut TermArena,
                    integer: TermHandle,
                    count: usize,
                    mut body: TermHandle,
                ) -> TermHandle {
                    for _ in 0..count {
                        body = arena.insert(term!(Lambda => integer => body));
                    }
                    body
                }
                fn close_source_telescope(
                    arena: &mut TermArena,
                    declarations: &[Declaration],
                    source: TermHandle,
                    kernel: TermHandle,
                    count: usize,
                    body: TermHandle,
                ) -> TermHandle {
                    let raw = close(arena, source, count, body);
                    let compact = close(arena, kernel, count, body);
                    let context = Context::empty()
                        .with_signature(Signature::from_declarations(declarations.to_vec()));
                    // For child-index functions the result is I rather than Type0;
                    // conversion is still typed at the independently inferred Pi below.
                    let inferred = infer_type(
                        arena,
                        &context,
                        compact,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();
                    let raw_type = infer_type(
                        arena,
                        &context,
                        raw,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap();
                    assert_eq!(
                        judgment_assumption_closure(arena, context.signature(), &[raw, raw_type]),
                        judgment_assumption_closure(
                            arena,
                            context.signature(),
                            &[compact, inferred]
                        ),
                        "generated primitive-domain compaction must preserve the full judgment assumption closure",
                    );
                    assert!(
                        convertible(
                            arena,
                            &context,
                            raw,
                            compact,
                            inferred,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                        )
                        .unwrap()
                    );
                    compact
                }
                fn checked(
                    arena: &mut TermArena,
                    declarations: &[Declaration],
                    term: TermHandle,
                    ty: TermHandle,
                    role: &str,
                ) {
                    let context = Context::empty()
                        .with_signature(Signature::from_declarations(declarations.to_vec()));
                    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                    let result = check_type(arena, &context, term, ty, &mut budget);
                    eprintln!(
                        "indexed vector {role}: {} kernel steps",
                        DEFAULT_CONVERSION_STEPS - budget.remaining()
                    );
                    result.unwrap_or_else(|error| panic!("indexed vector {role}: {error:?}"));
                }
                fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
                    arena.insert(term!(Constant => declaration => vec![]))
                }

                /// Exact structural/arithmetic obligation of the frozen source vector:
                /// indices (Length,Total), Cons payload (previous,previous_total,head),
                /// complete constructor constraints, and the one child induction premise.
                /// Source declaration correspondence is separately exercised by the CLI.
                #[test]
                fn indexed_vector_integer_pair_step_and_theorem_fit_admission_budget() {
                    vector_budget(false);
                }
                #[test]
                fn indexed_vector_source_alias_and_embeddings_fit_admission_budget() {
                    vector_budget(true);
                }
                fn vector_budget(source_like: bool) {
                    let mut arena = TermArena::new();
                    let mut declarations = indexed_scheme(&mut arena);
                    let mut theory =
                        SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
                    let vocabulary = theory.vocabulary();
                    let integer = constant(&mut arena, vocabulary.carrier);
                    let scalar = if source_like {
                        let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                        let alias = declarations.len() as u32;
                        declarations.push(Declaration::definition(0, sort, integer));
                        constant(&mut arena, alias)
                    } else {
                        integer
                    };
                    let mut embedding_declaration = None;
                    let mut embed = |arena: &mut TermArena,
                                     declarations: &mut Vec<Declaration>,
                                     value: TermHandle| {
                        if !source_like {
                            return value;
                        }
                        let ty = arena.insert(term!(Pi => integer => integer));
                        let variable = arena.insert(Term::Variable(0));
                        let body = arena.insert(term!(Lambda => integer => variable));
                        let declaration = *embedding_declaration.get_or_insert_with(|| {
                            let declaration = declarations.len() as u32;
                            declarations.push(Declaration::definition(0, ty, body));
                            declaration
                        });
                        let function = constant(arena, declaration);
                        let raw = apply(arena, function, value);
                        let mut context = Context::empty()
                            .with_signature(Signature::from_declarations(declarations.clone()));
                        for _ in 0..5 {
                            context = context.extend(scalar);
                        }
                        assert!(
                            convertible(
                                arena,
                                &context,
                                raw,
                                value,
                                integer,
                                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                            )
                            .unwrap()
                        );
                        // Independently selected fixed integer embedding has this
                        // denotation; the raw identity is retained above only to
                        // check the representation change by kernel conversion.
                        value
                    };
                    let add = constant(&mut arena, vocabulary.add);
                    let multiply = constant(&mut arena, vocabulary.multiply);
                    let less_or_equal = constant(&mut arena, vocabulary.less_or_equal);
                    let zero = theory
                        .literal(&mut arena, &mut declarations, BigInt::from_i128(0))
                        .unwrap();
                    let one = theory
                        .literal(&mut arena, &mut declarations, BigInt::from_i128(1))
                        .unwrap();
                    let ten = theory
                        .literal(&mut arena, &mut declarations, BigInt::from_i128(10))
                        .unwrap();
                    let length_symbol = SymbolHandle::from_arena_index(10);
                    let total_symbol = SymbolHandle::from_arena_index(11);
                    let payload_symbols = [12, 13, 14].map(SymbolHandle::from_arena_index);
                    let length = arena.insert(Term::Variable(1));
                    let total = arena.insert(Term::Variable(0));
                    let length_zero = equality(&mut arena, integer, length, zero);
                    let total_zero = equality(&mut arena, integer, total, zero);
                    let nil_predicate = and(&mut arena, length_zero, total_zero);
                    let nil_constraints = close_source_telescope(
                        &mut arena,
                        &declarations,
                        scalar,
                        integer,
                        2,
                        nil_predicate,
                    );
                    let length = arena.insert(Term::Variable(4));
                    let total = arena.insert(Term::Variable(3));
                    let previous = arena.insert(Term::Variable(2));
                    let previous_total = arena.insert(Term::Variable(1));
                    let head = arena.insert(Term::Variable(0));
                    let head_bound = binary(&mut arena, less_or_equal, head, ten);
                    let previous = embed(&mut arena, &mut declarations, previous);
                    let embedded_length = embed(&mut arena, &mut declarations, length);
                    let successor = binary(&mut arena, add, previous, one);
                    let length_equation = equality(&mut arena, integer, successor, embedded_length);
                    let embedded_previous_total =
                        embed(&mut arena, &mut declarations, previous_total);
                    let embedded_head = embed(&mut arena, &mut declarations, head);
                    let embedded_total = embed(&mut arena, &mut declarations, total);
                    let sum = binary(&mut arena, add, embedded_previous_total, embedded_head);
                    let total_equation = equality(&mut arena, integer, sum, embedded_total);
                    let equations = and(&mut arena, length_equation, total_equation);
                    let cons_predicate = and(&mut arena, head_bound, equations);
                    let cons_constraints = close_source_telescope(
                        &mut arena,
                        &declarations,
                        scalar,
                        integer,
                        5,
                        cons_predicate,
                    );
                    let previous = arena.insert(Term::Variable(2));
                    let index = arena.insert(term!(Pair => previous => previous_total));
                    let child_index = close_source_telescope(
                        &mut arena,
                        &declarations,
                        scalar,
                        integer,
                        5,
                        index,
                    );
                    let cases = vec![
                        Case {
                            payload: vec![],
                            constraints: nil_constraints,
                            child_indices: vec![],
                        },
                        Case {
                            payload: payload_symbols
                                .iter()
                                .map(|&symbol| (symbol, integer))
                                .collect(),
                            constraints: cons_constraints,
                            child_indices: vec![child_index],
                        },
                    ];
                    // The internal W uses the fixed primitive carrier; exact authored
                    // alias domains remain in the constraint/invariant functions above.
                    let common = [(length_symbol, integer), (total_symbol, integer)];
                    let indices = [length_symbol, total_symbol];
                    let family = description(
                        &mut arena,
                        &common,
                        &indices,
                        &cases,
                        &[],
                        &[],
                        &mut declarations,
                        &[],
                        0,
                    )
                    .unwrap()
                    .family;
                    let family =
                        intern_description(&mut arena, &mut declarations, family, &[], 0).unwrap();
                    let i = arena.insert(Term::Variable(0));
                    let length = arena.insert(Term::Fst { pair: i });
                    let total = arena.insert(Term::Snd { pair: i });
                    let (authored_length, authored_total) = if source_like {
                        (
                            arena.insert(Term::Variable(1)),
                            arena.insert(Term::Variable(0)),
                        )
                    } else {
                        (length, total)
                    };
                    let authored_length = embed(&mut arena, &mut declarations, authored_length);
                    let authored_total = embed(&mut arena, &mut declarations, authored_total);
                    let bound = binary(&mut arena, multiply, ten, authored_length);
                    let goal = binary(&mut arena, less_or_equal, authored_total, bound);
                    let goal = if source_like {
                        let authored = close(&mut arena, scalar, 2, goal);
                        let applied = apply(&mut arena, authored, length);
                        let raw_goal = apply(&mut arena, applied, total);
                        // Reduce only the binder/application scaffolding just built by
                        // the source invariant producer. No authored source function or
                        // predicate declaration is normalized or discarded.
                        let Term::Lambda { body, .. } = arena.get(authored) else {
                            unreachable!()
                        };
                        let remaining = substitute(&mut arena, body, length);
                        let Term::Lambda { body, .. } = arena.get(remaining) else {
                            unreachable!()
                        };
                        let normalized = substitute(&mut arena, body, total);
                        let context = Context::empty()
                            .with_signature(Signature::from_declarations(declarations.clone()))
                            .extend(family.index);
                        let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
                        assert!(
                            convertible(
                                &mut arena,
                                &context,
                                raw_goal,
                                normalized,
                                sort,
                                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                            )
                            .unwrap()
                        );
                        normalized
                    } else {
                        goal
                    };
                    let invariant = arena.insert(term!(Lambda => family.index => goal));
                    let invariant = intern_invariant(
                        &mut arena,
                        &mut declarations,
                        &family,
                        invariant,
                        Level::Constant(0),
                    );
                    let mut lemmas = Vec::new();
                    for (ordinal, case) in cases.iter().enumerate() {
                        let fields = common
                            .iter()
                            .chain(&case.payload)
                            .copied()
                            .collect::<Vec<_>>();
                        let mut parameters = Vec::new();
                        let mut values = Vec::new();
                        for &(symbol, domain) in &fields {
                            let declaration = declarations.len() as u32;
                            declarations.push(Declaration {
                                level_arity: 0,
                                ty: domain,
                                body: None,
                            });
                            parameters.push((declaration, symbol, domain));
                            values.push(constant(&mut arena, declaration));
                        }
                        let mut predicate = case.constraints;
                        for &value in &values {
                            predicate = apply(&mut arena, predicate, value);
                        }
                        let index = arena.insert(term!(Pair => values[0] => values[1]));
                        let goal = apply(&mut arena, invariant, index);
                        let mut assumptions = vec![predicate];
                        for &function in &case.child_indices {
                            let mut index = function;
                            for &value in &values {
                                index = apply(&mut arena, index, value);
                            }
                            assumptions.push(apply(&mut arena, invariant, index));
                        }
                        let proof = theory
                            .predicate_from_assumptions(
                                &mut arena,
                                &mut declarations,
                                goal,
                                &assumptions,
                            )
                            .unwrap()
                            .unwrap();
                        let mut ty = goal;
                        for &domain in assumptions.iter().rev() {
                            let codomain = shift(&mut arena, ty, 0, 1);
                            ty = arena.insert(Term::Pi { domain, codomain });
                        }
                        let (proof, ty) =
                            abstract_parameters(&mut arena, proof, ty, &parameters).unwrap();
                        assert!(excludes_parameters(
                            &arena,
                            &declarations,
                            &[proof, ty],
                            &parameters
                        ));
                        checked(
                            &mut arena,
                            &declarations,
                            proof,
                            ty,
                            &format!("case {ordinal}"),
                        );
                        let declaration = declarations.len() as u32;
                        declarations.push(Declaration::definition(0, ty, proof));
                        lemmas.push(constant(&mut arena, declaration));
                    }
                    let (theorem, ty, step, step_type) = induction(
                        &mut arena,
                        &family,
                        &common,
                        &indices,
                        &cases,
                        invariant,
                        &lemmas,
                        &mut declarations,
                        &[],
                        &[],
                        &[],
                    )
                    .unwrap();
                    checked(&mut arena, &declarations, step, step_type, "step");
                    checked(&mut arena, &declarations, theorem, ty, "theorem");
                    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                    let result = check_signature(&mut arena, &declarations, &mut budget);
                    eprintln!(
                        "indexed vector signature: {} kernel steps",
                        DEFAULT_CONVERSION_STEPS - budget.remaining()
                    );
                    result.unwrap();
                    // Closed source specializations use direct arithmetic evidence instead
                    // of replaying the generic induction step for every generated clone.
                    let eighty = theory
                        .literal(&mut arena, &mut declarations, BigInt::from_i128(80))
                        .unwrap();
                    let closed_goal = binary(&mut arena, less_or_equal, ten, eighty);
                    let closed_proof = theory
                        .closed_predicate(&mut arena, &mut declarations, closed_goal)
                        .unwrap()
                        .unwrap();
                    let closed_invariant =
                        arena.insert(term!(Lambda => family.index => closed_goal));
                    let (theorem, ty) = constant_invariant_theorem(
                        &mut arena,
                        &family,
                        closed_invariant,
                        closed_proof,
                    );
                    checked(
                        &mut arena,
                        &declarations,
                        theorem,
                        ty,
                        "closed specialization",
                    );
                    let false_goal = binary(&mut arena, less_or_equal, eighty, ten);
                    let false_invariant = arena.insert(term!(Lambda => family.index => false_goal));
                    let (_, false_type) = constant_invariant_theorem(
                        &mut arena,
                        &family,
                        false_invariant,
                        closed_proof,
                    );
                    let context =
                        Context::empty().with_signature(Signature::from_declarations(declarations));
                    assert!(
                        check_type(
                            &mut arena,
                            &context,
                            theorem,
                            false_type,
                            &mut Budget::new(DEFAULT_CONVERSION_STEPS)
                        )
                        .is_err()
                    );
                }
            }
        }

        use super::{Elaborator, ScalarCarrier};
        use crate::typed_expressions::type_reference_table;
        use diagnostics::Diagnostic;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use numerics::bignum::BigInt;
        use proof_admission::{
            Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
            Signature, Sort, Term, TermHandle,
        };
        use symbols::SymbolHandle;
        use typed_trees::data::{
            DataDefinition, DataField, DataMember, TypeParameter, TypeParameterKind,
        };
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        use validation::checked_argument_matches_type_reference;

        pub(super) fn needs_scheme(program: &dyn validation::CheckingInput) -> bool {
            fn reference(
                program: &dyn validation::CheckingInput,
                handle: TypeReferenceHandle,
            ) -> bool {
                match program.type_reference_table.type_reference(handle) {
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => {
                        program
                            .unique_data_definition_by_symbol(*base_symbol)
                            .is_some_and(|data| {
                                let parameters = program.data_type_parameters(data);
                                !parameters.is_empty()
                                    && parameters.iter().all(|p| {
                                        matches!(
                                            p.kind,
                                            TypeParameterKind::Type
                                                | TypeParameterKind::Value { .. }
                                        )
                                    })
                            })
                            || type_reference_table(program)
                                .type_reference_handles(*arguments)
                                .iter()
                                .any(|argument| reference(program, *argument))
                    }
                    TypeReferenceNode::Reference { referee, .. } => reference(program, *referee),
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        reference(program, *base_type)
                    }
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        reference(program, *element_type)
                    }
                    _ => false,
                }
            }
            fn mathematical(
                program: &dyn validation::CheckingInput,
                handle: typed_trees::mathematical::MathematicalTypeHandle,
            ) -> bool {
                use typed_trees::mathematical::MathematicalType;
                match program.mathematical_type(handle) {
                    MathematicalType::Ordinary(handle) => reference(program, *handle),
                    MathematicalType::Arrow {
                        domain, codomain, ..
                    } => mathematical(program, *domain) || mathematical(program, *codomain),
                    MathematicalType::Application { callee, .. } => mathematical(program, *callee),
                }
            }
            program.expression_table.iter_expressions().any(|(_, node)| {
                matches!(node, typed_trees::expression::ExpressionNode::Call(call)
                    if validation::mathematical_applications::is_mathematical_call(program, call)
                        && call.machine_arguments.iter().any(|argument|
                            argument.type_reference.is_valid() && reference(program, argument.type_reference)))
            })
                || !erased_family_demands(program).is_empty()
                || program.mathematical_definitions().iter().any(|definition| {
                    mathematical(program, definition.result)
                        || program
                            .mathematical_parameters(definition.parameters)
                            .iter()
                            .any(|parameter| mathematical(program, parameter.ty))
                        || program
                            .data_type_parameters
                            .span_or_empty(definition.binders)
                            .iter()
                            .any(|parameter| match parameter.kind {
                                TypeParameterKind::Value { type_reference }
                                | TypeParameterKind::Const { type_reference } => {
                                    reference(program, type_reference)
                                }
                                _ => false,
                            })
                })
                || validation::mathematical_applications::expression_nodes(program)
                    .iter()
                    .any(|expression| {
                        validation::mathematical_applications::operand_type_reference(program, *expression)
                            .is_some_and(|handle| reference(program, handle))
                    })
        }

        fn erased_family_demands(program: &dyn validation::CheckingInput) -> Vec<SymbolHandle> {
            fn demand(
                program: &dyn validation::CheckingInput,
                reference: TypeReferenceHandle,
                result: &mut Vec<SymbolHandle>,
            ) {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => {
                        if program
                            .unique_data_definition_by_symbol(*base_symbol)
                            .is_some_and(|data| {
                                let parameters = program.data_type_parameters(data);
                                !parameters.is_empty()
                                    && parameters.iter().all(|parameter| {
                                        matches!(
                                            parameter.kind,
                                            TypeParameterKind::Type
                                                | TypeParameterKind::Value { .. }
                                        )
                                    })
                                    && program.data_members(data).iter().any(|member| {
                                        let fields = match member {
                                            DataMember::Field(field) => std::slice::from_ref(field),
                                            DataMember::Variant(case) => {
                                                program.data_payload_fields(case)
                                            }
                                        };
                                        fields.iter().any(|field| {
                                            declaration_contains_family(
                                                program,
                                                data.symbol,
                                                field.type_reference,
                                                &mut Vec::new(),
                                                &mut Vec::new(),
                                            )
                                        })
                                    })
                            })
                            && !result.contains(base_symbol)
                        {
                            result.push(*base_symbol);
                        }
                        for &argument in
                            type_reference_table(program).type_reference_handles(*arguments)
                        {
                            demand(program, argument, result);
                        }
                    }
                    TypeReferenceNode::Reference { referee, .. } => {
                        demand(program, *referee, result)
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        demand(program, *base_type, result)
                    }
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        demand(program, *element_type, result)
                    }
                    _ => {}
                }
            }
            let mut result = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for parameter in program.state_parameters(state) {
                        if parameter.relevance == language_core::BindingRelevance::Erased {
                            demand(program, parameter.type_reference, &mut result);
                        }
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let typed_trees::statement::StatementNode::LocalData(local) = statement
                            && local.relevance == language_core::BindingRelevance::Erased
                        {
                            demand(program, local.type_reference, &mut result);
                        }
                    }
                }
            }
            for data in program.data_definitions() {
                for member in program.data_members(data) {
                    let fields = match member {
                        DataMember::Field(field) => std::slice::from_ref(field),
                        DataMember::Variant(case) => program.data_payload_fields(case),
                    };
                    for field in fields {
                        if field.relevance == language_core::BindingRelevance::Erased {
                            demand(program, field.type_reference, &mut result);
                        }
                    }
                }
            }
            result
        }

        fn declaration_contains_family(
            program: &dyn validation::CheckingInput,
            owner: SymbolHandle,
            reference: TypeReferenceHandle,
            references: &mut Vec<TypeReferenceHandle>,
            declarations: &mut Vec<SymbolHandle>,
        ) -> bool {
            if references.contains(&reference) {
                return false;
            }
            references.push(reference);
            let table = &program.type_reference_table;
            let symbol = match table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                }
                | TypeReferenceNode::DynamicTrait {
                    symbol: base_symbol,
                    arguments,
                    ..
                } => {
                    if *base_symbol == owner
                        || table
                            .type_reference_handles(*arguments)
                            .iter()
                            .any(|argument| {
                                declaration_contains_family(
                                    program,
                                    owner,
                                    *argument,
                                    references,
                                    declarations,
                                )
                            })
                    {
                        return true;
                    }
                    Some(*base_symbol)
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    return declaration_contains_family(
                        program,
                        owner,
                        *referee,
                        references,
                        declarations,
                    );
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    return declaration_contains_family(
                        program,
                        owner,
                        *base_type,
                        references,
                        declarations,
                    );
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    return declaration_contains_family(
                        program,
                        owner,
                        *element_type,
                        references,
                        declarations,
                    );
                }
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => None,
            };
            let Some(symbol) = symbol else {
                return false;
            };
            if symbol == owner {
                return true;
            }
            if declarations.contains(&symbol) {
                return false;
            }
            declarations.push(symbol);
            let Some(data) = program.unique_data_definition_by_symbol(symbol) else {
                return false;
            };
            if let Some(quotient) = &data.quotient
                && declaration_contains_family(
                    program,
                    owner,
                    quotient.carrier,
                    references,
                    declarations,
                )
            {
                return true;
            }
            program
                .data_members(data)
                .iter()
                .any(|member| match member {
                    DataMember::Field(field) => declaration_contains_family(
                        program,
                        owner,
                        field.type_reference,
                        references,
                        declarations,
                    ),
                    DataMember::Variant(case) => {
                        program.data_payload_fields(case).iter().any(|field| {
                            declaration_contains_family(
                                program,
                                owner,
                                field.type_reference,
                                references,
                                declarations,
                            )
                        })
                    }
                })
        }

        #[derive(Clone)]
        pub(super) struct SourceFamily {
            pub declaration: u32,
            pub symbol: SymbolHandle,
            pub parameters: Vec<TypeParameter>,
            pub type_levels: Vec<Level>,
            /// Shared complete-payload product over Type parameters and common values.
            pub payload_lifts: Vec<TermHandle>,
            pub common: Vec<DataField>,
            pub cases: Vec<SourceCase>,
            description_template: IndexedFamily,
            pub external_parameter_indices: Vec<usize>,
            pub index_parameter_indices: Vec<usize>,
            pub checked_inductions: Vec<induction::CheckedSourceInduction>,
        }
        #[derive(Clone)]
        pub(super) struct SourceCase {
            pub symbol: SymbolHandle,
            pub payload: Vec<DataField>,
            pub children: Vec<DataField>,
            pub constraints: TermHandle,
            pub predicate_expressions: Vec<typed_trees::expression::ExpressionHandle>,
            pub child_indices: Vec<TermHandle>,
        }

        impl SourceFamily {
            pub(super) fn at_levels(
                &self,
                arena: &mut proof_admission::TermArena,
                levels: &[Level],
            ) -> Result<Self, &'static str> {
                if levels.len() != self.type_levels.len() {
                    return Err("source description lost its exact universe telescope");
                }
                let mut result = self.clone();
                result.type_levels = levels.to_vec();
                let instantiate = |arena: &mut proof_admission::TermArena, term| {
                    proof_admission::instantiate_levels(arena, term, levels)
                        .map_err(|_| "source description has an unbound universe parameter")
                };
                result.description_template = IndexedFamily {
                    levels: [
                        self.description_template.levels[0]
                            .instantiate(levels)
                            .map_err(|_| "source index universe is unbound")?,
                        self.description_template.levels[1]
                            .instantiate(levels)
                            .map_err(|_| "source label universe is unbound")?,
                        self.description_template.levels[2]
                            .instantiate(levels)
                            .map_err(|_| "source position universe is unbound")?,
                    ],
                    index: instantiate(arena, self.description_template.index)?,
                    carrier: instantiate(arena, self.description_template.carrier)?,
                    children: instantiate(arena, self.description_template.children)?,
                    out: instantiate(arena, self.description_template.out)?,
                    next: instantiate(arena, self.description_template.next)?,
                };
                for domain in &mut result.payload_lifts {
                    *domain = instantiate(arena, *domain)?;
                }
                for case in &mut result.cases {
                    case.constraints = instantiate(arena, case.constraints)?;
                    for child in &mut case.child_indices {
                        *child = instantiate(arena, *child)?;
                    }
                }
                Ok(result)
            }
            pub(super) fn description_at(
                &self,
                arena: &mut proof_admission::TermArena,
                actuals: &[TermHandle],
            ) -> Result<IndexedFamily, &'static str> {
                if actuals.len() != self.parameters.len() {
                    return Err("source description lost its exact parameter telescope");
                }
                let parameters = self
                    .external_parameter_indices
                    .iter()
                    .map(|&position| actuals[position])
                    .collect::<Vec<_>>();
                Ok(terms::apply_description(
                    arena,
                    &self.description_template,
                    &parameters,
                ))
            }
            pub(super) fn index_actuals(&self, actuals: &[TermHandle]) -> Vec<TermHandle> {
                self.index_parameter_indices
                    .iter()
                    .map(|&position| actuals[position])
                    .collect()
            }
        }

        impl Elaborator<'_> {
            pub(super) fn applied_source_family(
                &mut self,
                family: &SourceFamily,
                actuals: &[TermHandle],
            ) -> Result<SourceFamily, Vec<Diagnostic>> {
                let levels = self.family_actual_levels(&family.parameters, actuals)?;
                family
                    .at_levels(&mut self.arena, &levels)
                    .map_err(|reason| self.refuse(reason.into()))
            }
            /// Seed source evidence demanded without any mathematical declaration or
            /// application. Each model still passes the same exact declaration/W checks.
            pub(super) fn demand_source_family_declarations(
                &mut self,
            ) -> Result<(), Vec<Diagnostic>> {
                for symbol in erased_family_demands(self.program) {
                    self.source_family(symbol)?;
                }
                Ok(())
            }

            pub(super) fn indexed_application(
                &mut self,
                symbol: SymbolHandle,
                arguments: &[TypeReferenceHandle],
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                if self.indexed_family_stack.contains(&symbol) {
                    return Ok(self
                        .arena
                        .insert(Term::Sort(Sort::Type(Level::Constant(0)))));
                }
                let family = self.source_family(symbol)?;
                if family.parameters.len() != arguments.len() {
                    return Err(
                        self.refuse("source indexed application changed its index arity".into())
                    );
                }
                let actuals = family
                    .parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| self.index_operand(*argument, parameter, &[]))
                    .collect::<Result<Vec<_>, _>>()?;
                let levels = self.family_actual_levels(&family.parameters, &actuals)?;
                let mut term = self.arena.insert(Term::Constant {
                    declaration: family.declaration,
                    levels,
                });
                for operand in actuals {
                    term = self.arena.insert(Term::Apply {
                        function: term,
                        argument: operand,
                    });
                }
                Ok(term)
            }

            pub(crate) fn source_family(
                &mut self,
                symbol: SymbolHandle,
            ) -> Result<SourceFamily, Vec<Diagnostic>> {
                if let Some(family) = self.indexed_families.get(&symbol) {
                    return Ok(family.clone());
                }
                if self.indexed_family_stack.contains(&symbol) {
                    return Err(self.refuse(
                        "nested source-family recursion has no checked encoding yet".into(),
                    ));
                }
                let Some(data) = self
                    .program
                    .unique_data_definition_by_symbol(symbol)
                    .cloned()
                else {
                    return Err(self.refuse(
                        "indexed carrier does not select an exact source data declaration".into(),
                    ));
                };
                let parameters = self.program.data_type_parameters(&data).to_vec();
                if parameters.is_empty()
                    || parameters.iter().any(|p| {
                        !matches!(
                            p.kind,
                            TypeParameterKind::Type | TypeParameterKind::Value { .. }
                        )
                    })
                {
                    return Err(self.refuse("source W descriptions currently require a Type/Value telescope; const/machine telescope denotation remains unsupported".into()));
                }
                if data.quotient.is_some()
                    || data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
                    || !data.lifetime_parameters.is_empty()
                {
                    return Err(self.refuse("quotient, opaque or region-bearing source indexed carriers require their own checked description".into()));
                }
                let saved_scope = std::mem::take(&mut self.scope);
                let saved_owner = self.source_owner;
                let saved_span = self.span;
                self.source_owner = symbol;
                self.span = self.program.symbols.symbol_source_span(symbol);
                self.indexed_family_stack.push(symbol);
                let saved_levels = self.enter_family_levels(&parameters);
                let result = self.build_source_family(&data, parameters);
                self.restore_source_levels(saved_levels);
                self.indexed_family_stack.pop();
                self.scope = saved_scope;
                self.source_owner = saved_owner;
                self.span = saved_span;
                let family = result?;
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                proof_admission::check_signature(&mut self.arena, &self.declarations, &mut budget)
                    .map_err(|error| {
                        self.refuse(format!(
                    "generated source indexed-family description fails kernel checking: {error:?}"
                ))
                    })?;
                self.indexed_families.insert(symbol, family.clone());
                Ok(family)
            }

            fn build_source_family(
                &mut self,
                data: &DataDefinition,
                parameters: Vec<TypeParameter>,
            ) -> Result<SourceFamily, Vec<Diagnostic>> {
                let type_levels = parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .map(|parameter| self.source_type_level(parameter.symbol))
                    .collect::<Vec<_>>();
                let mut common = Vec::new();
                let mut common_children = Vec::new();
                let mut variants = Vec::new();
                for member in self.program.data_members(data).to_vec() {
                    match member {
                        DataMember::Field(field) => {
                            if self
                                .direct_child(data.symbol, field.type_reference)?
                                .is_some()
                            {
                                common_children.push(field)
                            } else {
                                common.push(field)
                            }
                        }
                        DataMember::Variant(case) => variants.push(case),
                    }
                }
                let mut uniform = parameters
                    .iter()
                    .map(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .collect::<Vec<_>>();
                for field in common_children.iter().chain(
                    variants
                        .iter()
                        .flat_map(|case| self.program.data_payload_fields(case)),
                ) {
                    if let Some(arguments) = self.direct_child(data.symbol, field.type_reference)? {
                        if arguments.len() != parameters.len() {
                            return Err(self.refuse(
                                "source recursive child changed the family telescope".into(),
                            ));
                        }
                        for (ordinal, (argument, parameter)) in
                            arguments.iter().zip(&parameters).enumerate()
                        {
                            uniform[ordinal] &= matches!(self.program.type_reference_table.type_reference(*argument), TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol);
                        }
                    }
                }
                if parameters.iter().enumerate().any(|(position, parameter)| {
                    matches!(parameter.kind, TypeParameterKind::Type) && !uniform[position]
                }) {
                    // A changing Type child remains the existing fully indexed family.
                    // Do not erase its index or impose a new source uniformity rule.
                    uniform.fill(false);
                }
                let external_parameter_indices = uniform
                    .iter()
                    .enumerate()
                    .filter_map(|(position, uniform)| uniform.then_some(position))
                    .collect::<Vec<_>>();
                let index_parameter_indices = parameters
                    .iter()
                    .enumerate()
                    .filter_map(|(position, parameter)| {
                        (matches!(parameter.kind, TypeParameterKind::Type) && !uniform[position])
                            .then_some(position)
                    })
                    .chain(
                        parameters
                            .iter()
                            .enumerate()
                            .filter_map(|(position, parameter)| {
                                matches!(parameter.kind, TypeParameterKind::Value { .. })
                                    .then_some(position)
                            }),
                    )
                    .collect::<Vec<_>>();
                let index_fields = self
                    .program
                    .data_value_parameter_fields(data)
                    .map(|(parameter, field)| (parameter.symbol, field.symbol))
                    .collect::<Vec<_>>();
                self.source_index_aliases.clone_from(&index_fields);
                if index_fields.len()
                    != parameters
                        .iter()
                        .filter(|parameter| {
                            matches!(parameter.kind, TypeParameterKind::Value { .. })
                        })
                        .count()
                {
                    return Err(self.refuse(
                        "source indexed telescope lost its exact descriptor fields".into(),
                    ));
                }
                let mut source_cases = Vec::new();
                let mut schemes = Vec::new();
                let rosters = if variants.is_empty() {
                    vec![(SymbolHandle::invalid(), Vec::new(), data.requires_facts)]
                } else {
                    variants
                        .iter()
                        .map(|case| {
                            (
                                case.symbol,
                                self.program.data_payload_fields(case).to_vec(),
                                case.requires_facts,
                            )
                        })
                        .collect()
                };
                for (case_symbol, fields, case_facts) in rosters {
                    self.scope.clear();
                    self.push_family_type_scope(&parameters);
                    for field in &common {
                        let domain = self.elaborate_type_reference(field.type_reference)?;
                        self.push_scope(Some(field.symbol), field.name.clone(), domain);
                    }
                    let mut payload = Vec::new();
                    let mut children = common_children.clone();
                    let mut payload_types = Vec::new();
                    for field in fields {
                        if self
                            .direct_child(data.symbol, field.type_reference)?
                            .is_some()
                        {
                            children.push(field);
                            continue;
                        }
                        let domain = self.closed_field_domain(&field)?;
                        self.push_scope(Some(field.symbol), field.name.clone(), domain);
                        payload_types.push((
                            field.symbol,
                            self.source_field_function(&field, &parameters, &type_levels)?,
                        ));
                        payload.push(field);
                    }
                    let mut predicate_expressions = if case_symbol.is_valid() {
                        validation::case_predicate_expressions(self.program, data, case_symbol)
                    } else {
                        Vec::new()
                    };
                    if !case_symbol.is_valid() {
                        for fact in self.program.proof_facts.span_or_empty(data.requires_facts) {
                            let Some(expression) = fact.as_expression().copied() else {
                                return Err(self.refuse(
                            "source indexed constraint has no relevant proposition denotation yet"
                                .into(),
                        ));
                            };
                            predicate_expressions.push(expression);
                        }
                    }
                    for fact in self.program.proof_facts.span_or_empty(case_facts) {
                        let Some(expression) = fact.as_expression().copied() else {
                            return Err(self.refuse(
                                "source indexed constraint has no relevant proposition denotation yet"
                                    .into(),
                            ));
                        };
                        predicate_expressions.push(expression);
                    }
                    let mut propositions = Vec::new();
                    for expression in predicate_expressions.iter().copied() {
                        let proposition =
                            self.elaborate_proposition(expression)?.ok_or_else(|| {
                                self.refuse(
                                "source indexed constraint has no checked proposition denotation"
                                    .into(),
                            )
                            })?;
                        propositions.push(proposition);
                    }
                    let proposition = if let Some(mut body) = propositions.pop() {
                        for preceding in propositions.into_iter().rev() {
                            let codomain = proof_admission::shift(&mut self.arena, body, 0, 1);
                            body = self.arena.insert(Term::Sigma {
                                domain: preceding,
                                codomain,
                            });
                        }
                        body
                    } else {
                        self.relevant_unit()
                    };
                    let constraints = self.close_family_telescope(proposition);
                    let mut child_indices = Vec::new();
                    for child in &children {
                        let arguments = self
                            .direct_child(data.symbol, child.type_reference)?
                            .ok_or_else(|| {
                                self.refuse(
                                    "source recursive field lost its exact family application"
                                        .into(),
                                )
                            })?;
                        if arguments.len() != parameters.len() {
                            return Err(self.refuse(
                                "source recursive child changed the family index arity".into(),
                            ));
                        }
                        let mut indices = Vec::new();
                        for &position in &index_parameter_indices {
                            indices.push(self.index_operand(
                                arguments[position],
                                &parameters[position],
                                &index_fields,
                            )?);
                        }
                        let index = self.index_tuple(indices)?;
                        child_indices.push(self.close_family_telescope(index));
                    }
                    schemes.push(terms::Case {
                        payload: payload_types,
                        constraints,
                        child_indices: child_indices.clone(),
                    });
                    source_cases.push(SourceCase {
                        symbol: case_symbol,
                        payload,
                        children,
                        constraints,
                        predicate_expressions,
                        child_indices,
                    });
                }
                self.scope.clear();
                let mut common_pairs = common
                    .iter()
                    .map(|field| {
                        self.source_field_function(field, &parameters, &type_levels)
                            .map(|ty| (field.symbol, ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let type_symbols = parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>();
                let external_symbols = external_parameter_indices
                    .iter()
                    .map(|&position| parameters[position].symbol)
                    .collect::<Vec<_>>();
                let stored_types = type_symbols
                    .iter()
                    .copied()
                    .filter(|symbol| !external_symbols.contains(symbol))
                    .collect::<Vec<_>>();
                let stored_domains = stored_types
                    .iter()
                    .map(|symbol| {
                        let ordinal = type_symbols
                            .iter()
                            .position(|bound| bound == symbol)
                            .expect("retained Type binder");
                        (
                            *symbol,
                            self.arena
                                .insert(Term::Sort(Sort::Type(type_levels[ordinal].clone()))),
                        )
                    })
                    .collect::<Vec<_>>();
                common_pairs.splice(0..0, stored_domains);
                let mut index_symbols = index_fields
                    .iter()
                    .map(|(_, field)| *field)
                    .collect::<Vec<_>>();
                index_symbols.splice(0..0, stored_types.iter().copied());
                let external_domains = external_parameter_indices
                    .iter()
                    .map(|&position| {
                        let ordinal = type_symbols
                            .iter()
                            .position(|symbol| *symbol == parameters[position].symbol)
                            .expect("external Type parameter");
                        self.arena
                            .insert(Term::Sort(Sort::Type(type_levels[ordinal].clone())))
                    })
                    .collect::<Vec<_>>();
                let type_domains = type_levels
                    .iter()
                    .cloned()
                    .map(|level| self.arena.insert(Term::Sort(Sort::Type(level))))
                    .collect::<Vec<_>>();
                let description = terms::description(
                    &mut self.arena,
                    &common_pairs,
                    &index_symbols,
                    &schemes,
                    &type_symbols,
                    &external_symbols,
                    &mut self.declarations,
                    &type_domains,
                    type_levels.len() as u32,
                )
                .map_err(|error| self.refuse(error))?;
                let payload_lifts = description.payload_lifts;
                let description = terms::intern_description(
                    &mut self.arena,
                    &mut self.declarations,
                    description.family,
                    &external_domains,
                    type_levels.len() as u32,
                )
                .map_err(|error| self.refuse(error.into()))?;
                self.scope.clear();
                let mut domains = Vec::new();
                for parameter in &parameters {
                    let domain = match parameter.kind {
                        TypeParameterKind::Type => {
                            let level = self.source_type_level(parameter.symbol);
                            self.arena.insert(Term::Sort(Sort::Type(level)))
                        }
                        TypeParameterKind::Value { type_reference } => {
                            self.elaborate_type_reference(type_reference)?
                        }
                        _ => unreachable!(),
                    };
                    self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
                    domains.push(domain);
                }
                let actuals = (0..parameters.len())
                    .map(|ordinal| self.scope_variable(ordinal))
                    .collect::<Vec<_>>();
                let external_actuals = external_parameter_indices
                    .iter()
                    .map(|&position| actuals[position])
                    .collect::<Vec<_>>();
                let applied =
                    terms::apply_description(&mut self.arena, &description, &external_actuals);
                let indices = index_parameter_indices
                    .iter()
                    .map(|&position| actuals[position])
                    .collect();
                let index = self.index_tuple(indices)?;
                let mut body = applied.indexed_w(&mut self.arena, index);
                let mut ty = self.arena.insert(Term::Sort(Sort::Type(
                    applied.levels[0]
                        .clone()
                        .maximum(applied.levels[1].clone())
                        .maximum(applied.levels[2].clone()),
                )));
                for domain in domains.into_iter().rev() {
                    body = self.arena.insert(Term::Lambda { domain, body });
                    ty = self.arena.insert(Term::Pi {
                        domain,
                        codomain: ty,
                    });
                }
                let declaration = self.declarations.len() as u32;
                self.declarations.push(Declaration {
                    level_arity: type_levels.len() as u32,
                    ty,
                    body: Some(body),
                });
                self.source_index_aliases.clear();
                Ok(SourceFamily {
                    declaration,
                    symbol: data.symbol,
                    parameters,
                    type_levels,
                    payload_lifts,
                    common,
                    cases: source_cases,
                    description_template: description,
                    external_parameter_indices,
                    index_parameter_indices,
                    checked_inductions: Vec::new(),
                })
            }

            fn direct_child(
                &self,
                owner: SymbolHandle,
                reference: TypeReferenceHandle,
            ) -> Result<Option<Vec<TypeReferenceHandle>>, Vec<Diagnostic>> {
                match self.program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Generic {
                        base_symbol,
                        lifetime_arguments,
                        arguments,
                        ..
                    } if *base_symbol == owner => {
                        if !lifetime_arguments.is_empty() {
                            return Err(self.refuse(
                                "recursive indexed fields cannot discard region arguments".into(),
                            ));
                        }
                        Ok(Some(
                            type_reference_table(self.program)
                                .type_reference_handles(*arguments)
                                .to_vec(),
                        ))
                    }
                    _ => {
                        if self.contains_family(owner, reference, &mut Vec::new(), &mut Vec::new())
                        {
                            return Err(self.refuse(
                                "nested or mutual source-family recursion has no checked encoding yet"
                                    .into(),
                            ));
                        }
                        Ok(None)
                    }
                }
            }

            fn contains_family(
                &self,
                owner: SymbolHandle,
                reference: TypeReferenceHandle,
                references: &mut Vec<TypeReferenceHandle>,
                declarations: &mut Vec<SymbolHandle>,
            ) -> bool {
                declaration_contains_family(
                    self.program,
                    owner,
                    reference,
                    references,
                    declarations,
                )
            }
            pub(super) fn push_family_type_scope(&mut self, parameters: &[TypeParameter]) {
                let levels = parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .map(|parameter| self.source_type_level(parameter.symbol))
                    .collect::<Vec<_>>();
                self.push_family_type_scope_at(parameters, &levels);
            }

            pub(super) fn push_family_type_scope_at(
                &mut self,
                parameters: &[TypeParameter],
                levels: &[Level],
            ) {
                for (parameter, level) in parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .zip(levels)
                {
                    let level = level.clone();
                    let domain = self.arena.insert(Term::Sort(Sort::Type(level)));
                    self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
                }
            }

            fn source_field_function(
                &mut self,
                field: &DataField,
                parameters: &[TypeParameter],
                levels: &[Level],
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let saved = std::mem::take(&mut self.scope);
                self.push_family_type_scope_at(parameters, levels);
                let result = self
                    .kernel_field_domain(field)
                    .map(|body| self.close_family_telescope(body));
                self.scope = saved;
                result
            }

            fn closed_field_domain(
                &mut self,
                field: &DataField,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let domain = self.elaborate_type_reference(field.type_reference)?;
                let context = self.scope.iter().fold(
                    proof_admission::Context::with_level_arity(
                        self.level_binders.len() as u32 + self.generalized_levels,
                    )
                    .with_signature(Signature::from_declarations(self.declarations.clone())),
                    |context, entry| context.extend(entry.domain),
                );
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                proof_admission::infer_sort(&mut self.arena,&context,domain,&mut budget).map_err(|_|self.refuse(format!("indexed payload `{}` has an open dependent carrier whose encoding is unsupported",field.name)))?;
                Ok(domain)
            }
            fn kernel_field_domain(
                &mut self,
                field: &DataField,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let domain = self.closed_field_domain(field)?;
                Ok(self.kernel_primitive_domain(domain))
            }
            fn kernel_primitive_domain(&mut self, domain: TermHandle) -> TermHandle {
                let Term::Constant {
                    declaration,
                    levels,
                } = self.arena.get(domain)
                else {
                    return domain;
                };
                if !levels.is_empty() {
                    return domain;
                }
                let carrier = self
                    .source_aliases
                    .iter()
                    .find(|alias| alias.declaration == declaration)
                    .map(|alias| alias.carrier);
                carrier
                    .map(|carrier| self.carrier_term(carrier))
                    .unwrap_or(domain)
            }
            fn close_family_telescope(&mut self, mut body: TermHandle) -> TermHandle {
                let domains = self
                    .scope
                    .iter()
                    .map(|entry| entry.domain)
                    .collect::<Vec<_>>();
                for domain in domains.into_iter().rev() {
                    let domain = self.kernel_primitive_domain(domain);
                    body = self.arena.insert(Term::Lambda { domain, body });
                }
                body
            }
            fn relevant_unit(&mut self) -> TermHandle {
                let empty = self.arena.insert(Term::Empty);
                let unit = self.arena.insert(Term::Pi {
                    domain: empty,
                    codomain: empty,
                });
                self.arena.insert(Term::Box { ty: unit })
            }
            fn index_tuple(
                &mut self,
                mut indices: Vec<TermHandle>,
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                let Some(mut tuple) = indices.pop() else {
                    let empty = self.arena.insert(Term::Empty);
                    let domain = self.arena.insert(Term::Pi {
                        domain: empty,
                        codomain: empty,
                    });
                    let variable = self.arena.insert(Term::Variable(0));
                    let value = self.arena.insert(Term::Lambda {
                        domain: empty,
                        body: variable,
                    });
                    return Ok(self.arena.insert(Term::BoxIntro { ty: domain, value }));
                };
                for index in indices.into_iter().rev() {
                    tuple = self.arena.insert(Term::Pair {
                        first: index,
                        second: tuple,
                    });
                }
                Ok(tuple)
            }
            pub(crate) fn index_operand(
                &mut self,
                reference: TypeReferenceHandle,
                parameter: &TypeParameter,
                aliases: &[(SymbolHandle, SymbolHandle)],
            ) -> Result<TermHandle, Vec<Diagnostic>> {
                if matches!(parameter.kind, TypeParameterKind::Type) {
                    return self.elaborate_type_reference(reference);
                }
                let TypeParameterKind::Value {
                    type_reference: carrier,
                } = parameter.kind
                else {
                    unreachable!("family producer retains only Value indices")
                };
                let node = type_reference_table(self.program)
                    .type_reference(reference)
                    .clone();
                match node {
                    TypeReferenceNode::Named { symbol, name } if symbol.is_valid() => {
                        let canonical = aliases
                            .iter()
                            .find(|(source, _)| *source == symbol)
                            .map(|(_, field)| *field)
                            .unwrap_or(symbol);
                        let term = self.named_term(canonical, &name)?;
                        let expected = self.elaborate_type_reference(carrier)?;
                        let signature = Signature::from_declarations(self.declarations.clone());
                        let context = self.scope.iter().fold(
                            Context::with_level_arity(
                                self.level_binders.len() as u32 + self.generalized_levels,
                            )
                            .with_signature(signature.clone()),
                            |context, entry| context.extend(entry.domain),
                        );
                        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                        let actual = proof_admission::infer_type(
                            &mut self.arena,
                            &context,
                            term,
                            &mut budget,
                        )
                        .map_err(|error| {
                            self.refuse(format!("source index has no checked domain: {error:?}"))
                        })?;
                        if !self.source_domains_correspond(
                            &signature,
                            &context,
                            expected,
                            actual,
                            None,
                            &mut budget,
                        )? {
                            return Err(self.refuse(format!(
                                "source index `{name}` has a different declared carrier"
                            )));
                        }
                        Ok(term)
                    }
                    TypeReferenceNode::Named { symbol: _, name } => {
                        validation::validate_closed_const_argument(
                            self.program,
                            "source indexed family",
                            parameter,
                            reference,
                            &[],
                            &[],
                        )?;
                        let value = BigInt::from_decimal_str(name.as_str())
                            .or_else(|| {
                                match CanonicalConstValue::from_atom(name.as_str())?
                                    .decode_encoding()?
                                {
                                    DecodedCanonicalConstValue::Integer { value, .. } => {
                                        Some(BigInt::from_i128(value))
                                    }
                                    _ => None,
                                }
                            })
                            .ok_or_else(|| {
                                self.refuse(format!(
                                    "source index `{name}` has no checked scalar denotation"
                                ))
                            })?;
                        if self.scalar_carrier_of_type_reference(carrier)
                            != Some(ScalarCarrier::Integer)
                        {
                            return Err(self.refuse(
                                "integer index does not inhabit the declared source carrier".into(),
                            ));
                        }
                        let integer = self.integer_carrier();
                        self.numeric_literal(integer, value)
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        if !checked_argument_matches_type_reference(
                            self.program,
                            expression,
                            carrier,
                        ) {
                            return Err(self.refuse(
                                "source index expression does not inhabit its declared carrier"
                                    .into(),
                            ));
                        }
                        self.elaborate_expression(expression)
                    }
                    _ => Err(self.refuse(
                        "source index operand is not a bound value or closed scalar".into(),
                    )),
                }
            }
        }
    }
    mod source_levels {
        /*
        Source types keep one universe per binder, including when a data description
        is prepared inside another declaration. A family template has its own level
        telescope; applying it substitutes the actual type levels before any field,
        constructor or theorem consumes the description.
        */

        use super::{Elaborator, Level, Term, TermHandle};
        use symbols::SymbolHandle;
        use typed_trees::{
            data::{TypeParameter, TypeParameterKind},
            name::Identifier,
        };

        pub(super) struct SourceLevelScope {
            binders: Vec<SymbolHandle>,
            names: Vec<Identifier>,
            generalized: u32,
            types: Vec<(SymbolHandle, Level)>,
        }

        impl Elaborator<'_> {
            pub(super) fn enter_family_levels(
                &mut self,
                parameters: &[TypeParameter],
            ) -> SourceLevelScope {
                let saved = SourceLevelScope {
                    binders: std::mem::take(&mut self.level_binders),
                    names: std::mem::take(&mut self.level_names),
                    generalized: std::mem::take(&mut self.generalized_levels),
                    types: std::mem::take(&mut self.source_type_levels),
                };
                for parameter in parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                {
                    self.source_type_level(parameter.symbol);
                }
                saved
            }

            pub(super) fn restore_source_levels(&mut self, saved: SourceLevelScope) {
                self.level_binders = saved.binders;
                self.level_names = saved.names;
                self.generalized_levels = saved.generalized;
                self.source_type_levels = saved.types;
            }

            pub(super) fn family_actual_levels(
                &mut self,
                parameters: &[TypeParameter],
                actuals: &[TermHandle],
            ) -> Result<Vec<Level>, Vec<diagnostics::Diagnostic>> {
                if parameters.len() != actuals.len() {
                    return Err(
                        self.refuse("source family lost its exact level/type telescope".into())
                    );
                }
                let context = self.instantiation_context();
                let mut levels = Vec::new();
                for (parameter, actual) in parameters.iter().zip(actuals) {
                    if matches!(parameter.kind, TypeParameterKind::Type) {
                        let sort = proof_admission::infer_sort(
                            &mut self.arena,
                            &context,
                            *actual,
                            &mut proof_admission::Budget::new(
                                proof_admission::DEFAULT_CONVERSION_STEPS,
                            ),
                        )
                        .map_err(|error| {
                            self.refuse(format!(
                                "source family type argument fails universe formation: {error:?}"
                            ))
                        })?;
                        let proof_admission::Sort::Type(level) = sort else {
                            return Err(self.refuse(
                                "source family Type argument belongs to the strict layer".into(),
                            ));
                        };
                        levels.push(level);
                    }
                }
                Ok(levels)
            }

            pub(super) fn quantified_constant(&mut self, position: u32) -> TermHandle {
                let arity = self.declarations[position as usize].level_arity;
                self.arena.insert(Term::Constant {
                    declaration: position,
                    levels: (0..arity).map(Level::Parameter).collect(),
                })
            }
        }
    }
    mod term_order {
        use super::Elaborator;
        use proof_admission::{Level, Term, TermHandle};

        impl Elaborator<'_> {
            pub(super) fn term_order_key(&self, term: TermHandle) -> Vec<u8> {
                let mut key = Vec::new();
                self.write_term_order_key(term, &mut key);
                key
            }

            pub(super) fn write_term_order_key(&self, term: TermHandle, out: &mut Vec<u8>) {
                match self.arena.get(term) {
                    Term::Dummy => out.push(0),
                    Term::Variable(index) => {
                        out.push(1);
                        out.extend_from_slice(&index.to_le_bytes());
                    }
                    Term::Sort(sort) => {
                        out.push(2);
                        out.push(sort.is_strict() as u8);
                        self.write_level_key(sort.level(), out);
                    }
                    Term::Constant {
                        declaration,
                        levels,
                    } => {
                        out.push(3);
                        out.extend_from_slice(&declaration.to_le_bytes());
                        for level in levels {
                            self.write_level_key(level, out);
                        }
                    }
                    Term::Pi { domain, codomain } => self.write_pair_key(4, domain, codomain, out),
                    Term::Lambda { domain, body } => self.write_pair_key(5, domain, body, out),
                    Term::Apply { function, argument } => {
                        self.write_pair_key(6, function, argument, out)
                    }
                    Term::Sigma { domain, codomain } => {
                        self.write_pair_key(7, domain, codomain, out)
                    }
                    Term::Pair { first, second } => self.write_pair_key(8, first, second, out),
                    Term::Fst { pair } => {
                        out.push(9);
                        self.write_term_order_key(pair, out);
                    }
                    Term::Snd { pair } => {
                        out.push(10);
                        self.write_term_order_key(pair, out);
                    }
                    Term::Two => out.push(11),
                    Term::TwoZero => out.push(12),
                    Term::TwoOne => out.push(13),
                    Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    } => {
                        out.push(14);
                        self.write_term_order_key(motive, out);
                        self.write_term_order_key(zero_branch, out);
                        self.write_term_order_key(one_branch, out);
                        self.write_term_order_key(scrutinee, out);
                    }
                    Term::Id { ty, left, right } => {
                        out.push(15);
                        self.write_term_order_key(ty, out);
                        self.write_term_order_key(left, out);
                        self.write_term_order_key(right, out);
                    }
                    Term::Refl { ty, value } => self.write_pair_key(16, ty, value, out),
                    Term::IdElim {
                        motive,
                        base,
                        endpoint,
                        proof,
                    } => {
                        out.push(17);
                        self.write_term_order_key(motive, out);
                        self.write_term_order_key(base, out);
                        self.write_term_order_key(endpoint, out);
                        self.write_term_order_key(proof, out);
                    }
                    Term::W { carrier, children } => {
                        self.write_pair_key(18, carrier, children, out)
                    }
                    Term::Sup {
                        carrier,
                        children,
                        label,
                        function,
                    } => {
                        out.push(19);
                        self.write_term_order_key(carrier, out);
                        self.write_term_order_key(children, out);
                        self.write_term_order_key(label, out);
                        self.write_term_order_key(function, out);
                    }
                    Term::IndW { motive, step, tree } => {
                        out.push(20);
                        self.write_term_order_key(motive, out);
                        self.write_term_order_key(step, out);
                        self.write_term_order_key(tree, out);
                    }
                    Term::Empty => out.push(21),
                    Term::EmptyElim { ty, scrutinee } => {
                        self.write_pair_key(22, ty, scrutinee, out)
                    }
                    Term::Squash { ty } => {
                        out.push(23);
                        self.write_term_order_key(ty, out);
                    }
                    Term::SquashIntro { ty, value } => self.write_pair_key(24, ty, value, out),
                    Term::SquashElim {
                        proposition,
                        function,
                        scrutinee,
                    } => {
                        out.push(25);
                        self.write_term_order_key(proposition, out);
                        self.write_term_order_key(function, out);
                        self.write_term_order_key(scrutinee, out);
                    }
                    Term::Box { ty } => {
                        out.push(26);
                        self.write_term_order_key(ty, out);
                    }
                    Term::BoxIntro { ty, value } => self.write_pair_key(27, ty, value, out),
                    Term::BoxElim {
                        motive,
                        body,
                        scrutinee,
                    } => {
                        out.push(28);
                        self.write_term_order_key(motive, out);
                        self.write_term_order_key(body, out);
                        self.write_term_order_key(scrutinee, out);
                    }
                }
            }

            pub(super) fn write_pair_key(
                &self,
                tag: u8,
                first: TermHandle,
                second: TermHandle,
                out: &mut Vec<u8>,
            ) {
                out.push(tag);
                self.write_term_order_key(first, out);
                self.write_term_order_key(second, out);
            }

            pub(super) fn write_level_key(&self, level: Level, out: &mut Vec<u8>) {
                let mut pending = vec![&level];
                while let Some(level) = pending.pop() {
                    match level {
                        Level::Constant(value) => {
                            out.push(0);
                            out.extend_from_slice(&value.to_le_bytes());
                        }
                        Level::Parameter(index) => {
                            out.push(1);
                            out.extend_from_slice(&index.to_le_bytes());
                        }
                        Level::Successor(inner) => {
                            out.push(2);
                            pending.push(inner);
                        }
                        Level::Maximum(left, right) => {
                            out.push(3);
                            pending.push(right);
                            pending.push(left);
                        }
                    }
                }
            }
        }
    }

    use crate::typed_expressions::expression_table;
    use std::collections::{BTreeMap, HashMap, HashSet};

    use numerics::bignum::BigInt;
    use validation::mathematical_applications::{
        PreparedApplication, PreparedCapture, PreparedConstant, PreparedConstantMeaning,
        PreparedMathematicalApplications,
    };

    use super::FixedCoreVocabulary;
    use proof_admission::{
        Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
        TermArena, TermHandle, check_signature_with_attribution, infer_sort,
    };
    use source::SourceSpan;
    use symbols::SymbolHandle;
    use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::mathematical::{MathematicalBody, MathematicalDefinition};
    use typed_trees::name::Identifier;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct CheckedMathematicalSignature {
        arena: TermArena,
        signature: Signature,
        authored: Vec<u32>,
        prepared: PreparedMathematicalApplications,
    }

    impl CheckedMathematicalSignature {
        pub(crate) fn into_checked_evidence(
            self,
        ) -> (
            checked_trees::CheckedMathematicalSignature,
            PreparedMathematicalApplications,
        ) {
            let signature = checked_trees::CheckedMathematicalSignature {
                arena: self.arena,
                declarations: self.signature.declarations().to_vec(),
                authored: self.authored,
            };
            (signature, self.prepared)
        }
    }

    struct ScopeEntry {
        symbol: Option<SymbolHandle>,
        name: Identifier,
        domain: TermHandle,
    }

    struct Elaborator<'a> {
        program: &'a dyn ::validation::CheckingInput,
        arena: TermArena,
        declarations: Vec<Declaration>,
        /// Canonical content types are opaque algebra values, never the source
        /// literal former's start/end record representation.
        source_content: Option<proof_admission::SourceContentTheory>,
        /// Carrier symbol -> signature position (a `Type 0` assumption).
        carriers: HashMap<SymbolHandle, u32>,
        /// Region-only nominal types retain their exact scoped lifetime identity.
        region_carriers: HashMap<typed_trees::type_identity::NormalizedTypeIdentity, u32>,
        /// `Int : Type 0` — the shared mathematical-integer carrier every
        /// fixed-width integer carrier denotes into: `i8`–`u64`, `Int` and
        /// `UInt` all name it, exactly as the bounded denotation's scalar
        /// terms embed.
        integer: Option<u32>,
        boolean: Option<u32>,
        address: Option<u32>,
        float32: Option<u32>,
        float64: Option<u32>,
        unit: Option<u32>,
        integer_less_than: Option<u32>,
        integer_less_or_equal: Option<u32>,
        integer_add: Option<u32>,
        integer_subtract: Option<u32>,
        proposition_negation: Option<u32>,
        source_operations: Vec<operand_terms::SourceOperationDeclaration>,

        numeric_literals: BTreeMap<(u32, BigInt), u32>,
        boolean_literals: [Option<u32>; 2],
        float_literals: BTreeMap<(u32, u64), u32>,
        authored_positions: HashMap<SymbolHandle, u32>,
        authored_index: HashMap<SymbolHandle, usize>,
        authored: Vec<u32>,
        level_binders: Vec<SymbolHandle>,
        level_names: Vec<Identifier>,
        generalized_levels: u32,
        /// Ordinary source binders reuse one inferred universe in this telescope.
        source_type_levels: Vec<(SymbolHandle, Level)>,
        /// Term scope for the declaration under elaboration: ordered
        /// telescope entries plus in-flight arrow binders.
        scope: Vec<ScopeEntry>,
        fixed_core: FixedCoreVocabulary,
        address_bound: Option<u32>,
        span: Option<SourceSpan>,
        source_owner: SymbolHandle,
        source_owners: std::sync::OnceLock<validation::expression_owners::OwnerEnvironmentIndex>,
        captured_expressions: Vec<(ExpressionHandle, usize)>,
        source_family_values: HashMap<ExpressionHandle, TermHandle>,
        source_constructor_context: source_families::SourceConstructorContext,
        source_aliases: Vec<source_domains::SourceAlias>,
        source_invocations: Vec<source_domains::SourceInvocation>,
        indexed_families: HashMap<SymbolHandle, source_families::SourceFamily>,
        indexed_family_stack: Vec<SymbolHandle>,
        source_index_aliases: Vec<(SymbolHandle, SymbolHandle)>,
        integer_theory: Option<proof_admission::SourceIntegerTheory>,
        integer_multiply: Option<u32>,
        prepared: PreparedMathematicalApplications,
        identity_declaration: Option<u32>,
        type_returning_declarations: Vec<SymbolHandle>,
        identity_emissions: Vec<identity::IdentityEmission>,
        identity_applications: Vec<identity::IdentityApplication>,
        result_identity_requirements: identity::IdentityRequirements,
        identity_requirements_by_owner: Vec<identity::IdentityRequirements>,
    }

    pub(crate) fn check_mathematical_signature(
        program: &dyn ::validation::CheckingInput,
    ) -> Result<CheckedMathematicalSignature, Vec<diagnostics::Diagnostic>> {
        let mut arena = TermArena::new();
        let needs_scheme = source_families::needs_scheme(program);
        let declarations = if needs_scheme {
            proof_admission::indexed_scheme(&mut arena)
        } else {
            Vec::new()
        };
        let mut elaborator = Elaborator {
            program,
            arena,
            declarations,
            source_content: None,
            carriers: HashMap::new(),
            region_carriers: HashMap::new(),
            integer: None,
            boolean: None,
            address: None,
            float32: None,
            float64: None,
            unit: None,
            integer_less_than: None,
            integer_less_or_equal: None,
            integer_add: None,
            integer_subtract: None,
            proposition_negation: None,
            numeric_literals: BTreeMap::new(),
            source_operations: Vec::new(),
            boolean_literals: [None, None],
            float_literals: BTreeMap::new(),
            authored_positions: HashMap::new(),
            authored_index: program
                .mathematical_definitions()
                .iter()
                .enumerate()
                .map(|(index, definition)| (definition.symbol, index))
                .collect(),
            authored: Vec::new(),
            level_binders: Vec::new(),
            level_names: Vec::new(),
            generalized_levels: 0,
            source_type_levels: Vec::new(),
            scope: Vec::new(),
            fixed_core: FixedCoreVocabulary::resolve(&program.symbols),
            address_bound: None,
            span: None,
            source_owner: SymbolHandle::default(),
            source_owners: std::sync::OnceLock::new(),
            captured_expressions: Vec::new(),
            source_family_values: HashMap::new(),
            source_constructor_context: source_families::SourceConstructorContext::default(),
            source_aliases: Vec::new(),
            source_invocations: Vec::new(),
            indexed_families: HashMap::new(),
            indexed_family_stack: Vec::new(),
            source_index_aliases: Vec::new(),
            integer_theory: None,
            integer_multiply: None,
            prepared: PreparedMathematicalApplications::default(),
            identity_declaration: None,
            type_returning_declarations: Vec::new(),
            identity_emissions: Vec::new(),
            identity_applications: Vec::new(),
            result_identity_requirements: identity::IdentityRequirements::default(),
            identity_requirements_by_owner: vec![
                identity::IdentityRequirements::default();
                program.symbols.symbols().len()
            ],
        };
        if needs_scheme {
            let theory = proof_admission::SourceIntegerTheory::new(
                &mut elaborator.arena,
                &mut elaborator.declarations,
            )
            .map_err(|error| {
                elaborator.refuse(format!(
                    "source integer vocabulary could not be formed: {error}"
                ))
            })?;
            let vocabulary = theory.vocabulary();
            elaborator.integer = Some(vocabulary.carrier);
            elaborator.integer_less_than = Some(vocabulary.less_than);
            elaborator.integer_less_or_equal = Some(vocabulary.less_or_equal);
            elaborator.integer_add = Some(vocabulary.add);
            elaborator.integer_subtract = Some(vocabulary.subtract);
            elaborator.integer_multiply = Some(vocabulary.multiply);
            elaborator.integer_theory = Some(theory);
        }
        for definition in program.mathematical_definitions() {
            elaborator.elaborate_declaration(definition)?;
        }
        elaborator.check()
    }

    impl<'a> Elaborator<'a> {
        fn elaborate_declaration(
            &mut self,
            definition: &MathematicalDefinition,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            self.source_owner = definition.symbol;
            self.span = self.program.symbols.symbol_source_span(definition.symbol);
            self.level_binders.clear();
            self.level_names.clear();
            self.generalized_levels = 0;
            self.source_type_levels.clear();
            self.scope.clear();

            let binders = self
                .program
                .data_type_parameters
                .span_or_empty(definition.binders);
            let plans = binders
                .iter()
                .map(|binder| self.plan_binder(binder))
                .collect::<Result<Vec<_>, _>>()?;
            for (binder, plan) in binders.iter().zip(plans) {
                match plan {
                    BinderPlan::Level => {}
                    BinderPlan::Type(universe) => {
                        let level = match universe {
                            LevelArgument::Authored(reference) => {
                                self.elaborate_level(reference)?
                            }
                            LevelArgument::Generalized => self.generalized_level(),
                        };
                        let domain = self.arena.insert(Term::Sort(Sort::Type(level)));
                        self.push_scope(Some(binder.symbol), binder.name.clone(), domain);
                    }
                    BinderPlan::Term(carrier) => {
                        let domain = self.elaborate_type_reference(carrier)?;
                        self.push_scope(Some(binder.symbol), binder.name.clone(), domain);
                    }
                }
            }
            for parameter in self.program.mathematical_parameters(definition.parameters) {
                let domain = self.elaborate_mathematical_type(parameter.ty)?;
                self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
            }

            let result = self.elaborate_mathematical_type(definition.result)?;
            let strict_result = matches!(self.arena.get(result), Term::Sort(Sort::Strict(_)));
            if matches!(self.arena.get(result), Term::Sort(_)) {
                self.type_returning_declarations.push(definition.symbol);
            }
            let mut ty = result;
            for entry in self.scope.iter().rev() {
                ty = self.arena.insert(Term::Pi {
                    domain: entry.domain,
                    codomain: ty,
                });
            }
            let body = match &definition.body {
                MathematicalBody::Assumption => None,
                MathematicalBody::Definition(term) => {
                    let actual = self.elaborate_body(*term, strict_result)?;
                    let mut body = actual.term;
                    let mut requirements = actual.identity_requirements.clone();
                    requirements.owner = definition.symbol;
                    self.identity_requirements_by_owner[definition.symbol.arena_index() as usize] =
                        requirements;
                    self.retain_source_result(result, actual);
                    for entry in self.scope.iter().rev() {
                        body = self.arena.insert(Term::Lambda {
                            domain: entry.domain,
                            body,
                        });
                    }
                    Some(body)
                }
            };
            let level_arity = self.level_binders.len() as u32 + self.generalized_levels;
            let position = self.declarations.len() as u32;
            self.declarations.push(Declaration {
                level_arity,
                ty,
                body,
            });
            self.authored_positions.insert(definition.symbol, position);
            self.authored.push(position);
            Ok(())
        }

        fn plan_binder(
            &mut self,
            binder: &TypeParameter,
        ) -> Result<BinderPlan, Vec<diagnostics::Diagnostic>> {
            if binder.bounds != DataProperties::default() {
                return Err(self.refuse(format!(
                    "mathematical binder `{}` cannot carry property bounds",
                    binder.name
                )));
            }
            match &binder.kind {
            TypeParameterKind::Type => Ok(BinderPlan::Type(LevelArgument::Generalized)),
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                match self.carrier_head(*type_reference) {
                    CarrierHead::Level => {
                        self.level_binders.push(binder.symbol);
                        self.level_names.push(binder.name.clone());
                        Ok(BinderPlan::Level)
                    }
                    CarrierHead::MalformedLevel => Err(self.refuse(format!(
                        "mathematical binder `{}` applies arguments to `core::Level`; it takes none",
                        binder.name
                    ))),
                    CarrierHead::Type(universe) => Ok(BinderPlan::Type(universe)),
                    CarrierHead::MalformedType(argument_count) => Err(self.refuse(format!(
                        "mathematical binder `{}` applies `core::Type` to {} arguments; it takes one level",
                        binder.name, argument_count
                    ))),
                    CarrierHead::Term => Ok(BinderPlan::Term(*type_reference)),
                }
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => Err(self
                .refuse(format!(
                    "mathematical binder `{}` cannot carry a machine or proposition contract",
                    binder.name
                ))),
            TypeParameterKind::Implementation => Err(self.refuse(format!(
                "mathematical binder `{}` cannot select an implementation; implementation slots belong to trait telescopes",
                binder.name
            ))),
        }
        }

        fn carrier_head(&self, carrier: TypeReferenceHandle) -> CarrierHead {
            use validation::mathematical_applications::MathematicalCarrierHead;
            match validation::mathematical_applications::mathematical_carrier_head(
                self.program,
                carrier,
                &self.fixed_core,
            ) {
                MathematicalCarrierHead::Level => CarrierHead::Level,
                MathematicalCarrierHead::Type(universe) => {
                    CarrierHead::Type(if universe.is_valid() {
                        LevelArgument::Authored(universe)
                    } else {
                        LevelArgument::Generalized
                    })
                }
                MathematicalCarrierHead::MalformedLevel => CarrierHead::MalformedLevel,
                MathematicalCarrierHead::MalformedType(count) => CarrierHead::MalformedType(count),
                MathematicalCarrierHead::Term => CarrierHead::Term,
            }
        }

        fn elaborate_body(
            &mut self,
            term: ExpressionHandle,
            strict_result: bool,
        ) -> Result<source_domains::SourceInput, Vec<diagnostics::Diagnostic>> {
            // Boolean and logical source expressions can denote strict predicates.
            // Mathematical terms retain their own sort; an expected Strict result
            // cannot request a squash of an already Type-valued body.
            if strict_result
                && self.operand_carrier(term) == Some(ScalarCarrier::Boolean)
                && let Some(proposition) = self.elaborate_proposition(term)?
            {
                return Ok(source_domains::SourceInput {
                    expression: term,
                    term: self.arena.insert(Term::Squash { ty: proposition }),
                    domain: None,
                    identity_requirements: std::mem::take(&mut self.result_identity_requirements),
                });
            }
            self.source_expression_input(term)
        }

        fn elaborate_expression(
            &mut self,
            handle: ExpressionHandle,
        ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
            self.result_identity_requirements = identity::IdentityRequirements::default();
            let term = self.elaborate_expression_term(handle)?;
            if !matches!(
                self.program.expression_table.expression(handle),
                ExpressionNode::Name(..) | ExpressionNode::Call(..) | ExpressionNode::Binary(..)
            ) {
                self.result_identity_requirements = identity::IdentityRequirements::default();
            }
            Ok(term)
        }

        fn elaborate_expression_term(
            &mut self,
            handle: ExpressionHandle,
        ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
            if let Some(term) = self.source_family_values.get(&handle) {
                return Ok(*term);
            }
            if let Some((_, position)) = self
                .captured_expressions
                .iter()
                .find(|(expression, _)| *expression == handle)
            {
                return Ok(self.scope_variable(*position));
            }
            match self.program.expression_table.expression(handle) {
                ExpressionNode::Name(path) => {
                    let symbol = path.symbol;
                    let members = expression_table(self.program)
                        .name_path_members(path.members)
                        .to_vec();
                    if symbol.is_valid() {
                        let name = members.last().cloned().unwrap_or_default();
                        let term = self.named_term(symbol, &name)?;
                        self.transport_identity_value(symbol, &[], &[], term, &[], &[])?;
                        return Ok(term);
                    }
                    match members.as_slice() {
                        [name] => {
                            let name = name.clone();
                            if let Some(term) = self.scope_term_for_name(&name) {
                                Ok(term)
                            } else {
                                Err(self.refuse(format!("unresolved mathematical name `{name}`")))
                            }
                        }
                        _ => Err(self.refuse("an unresolved mathematical name path".to_owned())),
                    }
                }
                ExpressionNode::Call(call) => {
                    let call = call.clone();
                    if validation::mathematical_applications::is_identity_call(self.program, &call)
                    {
                        return self.elaborate_identity(handle, &call);
                    }
                    let target_symbol = validation::call_target_symbol(self.program, &call);
                    if target_symbol.is_valid()
                        && !self.authored_index.contains_key(&target_symbol)
                        && !self
                            .scope
                            .iter()
                            .any(|entry| entry.symbol == Some(target_symbol))
                    {
                        if call.static_machine_parameter.is_valid()
                            || !call.selects_only_nominal_route()
                            || !call.evidence_arguments.is_empty()
                            || call.operational_acknowledgement.acknowledges_suspend
                            || call.operational_acknowledgement.acknowledges_block
                        {
                            return Err(self.refuse(format!(
                            "call `{}` carries machine, evidence, quotient or operational payload the kernel cannot express",
                            call.target
                        )));
                        }
                        return self.open_term(handle);
                    }
                    if call.receiver.is_valid() {
                        return Err(self.refuse(format!(
                            "member call `{}` has no mathematical denotation yet",
                            call.target
                        )));
                    }
                    if call.static_machine_parameter.is_valid()
                        || !call.selects_only_nominal_route()
                        || !call.evidence_arguments.is_empty()
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block
                    {
                        return Err(self.refuse(format!(
                        "call `{}` carries machine, evidence, quotient or operational payload the kernel cannot express",
                        call.target
                    )));
                    }
                    let target = call.target.clone();
                    let static_arguments = call.machine_arguments.clone();
                    let (callee, static_inputs) =
                        self.elaborate_callee(handle, target_symbol, &target, &static_arguments)?;
                    let mut inputs = Vec::new();
                    let mut prefix = callee;
                    for argument in expression_table(self.program)
                        .expression_handles(call.arguments)
                        .to_vec()
                    {
                        let input = self.source_application_input(argument, prefix)?;
                        prefix = self.arena.insert(Term::Apply {
                            function: prefix,
                            argument: input.term,
                        });
                        inputs.push(input);
                    }
                    self.transport_identity_requirements(&call, callee, &static_inputs, &inputs)?;
                    Ok(self.apply_source_inputs(handle, callee, inputs))
                }
                ExpressionNode::Integer(literal) => {
                    let literal = literal.clone();
                    self.literal_term(&literal)
                }
                ExpressionNode::Float(literal) => {
                    let literal = literal.clone();
                    self.float_literal_term(&literal)
                }
                ExpressionNode::Boolean(value) => Ok(self.boolean_literal(*value)),
                ExpressionNode::Binary(binary) => {
                    let binary = *binary;
                    match binary.operator {
                        op if op.is_arithmetic() => match self.operand_carrier(handle) {
                            Some(ScalarCarrier::Integer) => {
                                self.integer_operation_term(handle, &binary)
                            }
                            _ => self.open_term(handle),
                        },
                        _ => match self.elaborate_proposition(handle)? {
                            Some(proposition) => Ok(proposition),
                            None => self.open_term(handle),
                        },
                    }
                }
                ExpressionNode::ZeroValue(reference) => self.zero_value_term(*reference),
                ExpressionNode::Member(member) => {
                    let member_symbol = member.member_symbol;
                    if let Some(term) = self.scope_term_for_symbol(member_symbol) {
                        return Ok(term);
                    }
                    self.open_term(handle)
                }
                _ => self.open_term(handle),
            }
        }

        fn unexpected_carrier_error(
            &self,
            determined: Option<ScalarCarrier>,
            handle: ExpressionHandle,
        ) -> Vec<diagnostics::Diagnostic> {
            match determined {
                Some(..) => self.refuse(format!(
                    "operand `{}` inhabits a different scalar carrier",
                    self.program.render_proof_expression(
                        handle,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                )),
                None => self.refuse(format!(
                    "cannot determine the scalar carrier of operand `{}`",
                    self.program.render_proof_expression(
                        handle,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                )),
            }
        }

        fn unsupported_expression(&self, handle: ExpressionHandle) -> Vec<diagnostics::Diagnostic> {
            self.refuse(format!(
                "expression `{}` has no kernel denotation yet",
                self.program.render_proof_expression(
                    handle,
                    typed_trees::proposition::ProofSubstitutions::None
                )
            ))
        }

        fn check(mut self) -> Result<CheckedMathematicalSignature, Vec<diagnostics::Diagnostic>> {
            self.demand_source_family_declarations()?;
            let applications = self.source_applications()?;
            self.retain_source_evidence_carriers()?;
            self.retain_indexed_contract_theorems()?;
            let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
            let signature =
                check_signature_with_attribution(&mut self.arena, &self.declarations, &mut budget)
                    .map_err(|failure| {
                        let authored = self
                            .authored
                            .iter()
                            .position(|position| *position as usize >= failure.declaration);
                        if let Some(index) = authored {
                            let definition = &self.program.mathematical_definitions()[index];
                            self.span = self.program.symbols.symbol_source_span(definition.symbol);
                            self.refuse(format!(
                                "mathematical declaration `{}` fails kernel checking: {:?}",
                                definition.name, failure.error
                            ))
                        } else {
                            self.span = None;
                            self.refuse(format!(
                                "mathematical application signature fails checking: {:?}",
                                failure.error
                            ))
                        }
                    })?;
            self.check_source_domains(&signature)?;
            for (_, carrier) in self.prepared.source_carriers.iter() {
                let context = carrier.domains.iter().copied().fold(
                    Context::with_level_arity(carrier.level_arity)
                        .with_signature(signature.clone()),
                    |context, domain| context.extend(domain),
                );
                infer_sort(
                    &mut self.arena,
                    &context,
                    carrier.ty,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "source evidence carrier fails formation: {error:?}"
                    ))
                })?;
            }
            let retained_constants = self.retained_application_constants();
            for application in applications {
                let applications::SourceApplication {
                    expression,
                    owner,
                    term,
                    domains,
                    captures,
                    emissions,
                    level_arity,
                } = application;
                let mut context =
                    Context::with_level_arity(level_arity).with_signature(signature.clone());
                for domain in &domains {
                    infer_sort(
                        &mut self.arena,
                        &context,
                        *domain,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .map_err(|error| {
                        self.refuse(format!(
                            "mathematical source subject fails formation: {error:?}"
                        ))
                    })?;
                    context = context.extend(*domain);
                }
                let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
                let ty = proof_admission::infer_type(&mut self.arena, &context, term, &mut budget)
                    .map_err(|error| {
                        self.refuse(format!(
                            "mathematical application `{}` fails checking: {error:?}",
                            self.program.render_proof_expression(
                                expression,
                                typed_trees::proposition::ProofSubstitutions::None
                            )
                        ))
                    })?;
                if captures.is_empty() {
                    self.retain_checked_source_application(
                        expression, owner, term, &context, &signature,
                    )?;
                }
                let term = normalization::normalize(
                    &mut self.arena,
                    &signature,
                    term,
                    &mut budget,
                    &|declaration| retained_constants.contains(&declaration),
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "mathematical application reduction failed: {error:?}"
                    ))
                })?;
                let identity_values = self.prepare_identity_values(
                    &signature,
                    &context,
                    &retained_constants,
                    &emissions,
                )?;
                let captures = self.prepared.captures.insert_many(captures);
                self.prepared.applications.append(PreparedApplication {
                    level_arity,
                    expression,
                    owner,
                    term,
                    ty,
                    domains,
                    captures,
                    identity_values,
                });
            }
            self.retain_source_evidence_aliases()?;
            self.bind_source_evidence_types(&signature)?;
            self.retain_constant_meanings();
            Ok(CheckedMathematicalSignature {
                arena: self.arena,
                signature,
                authored: self.authored,
                prepared: self.prepared,
            })
        }

        fn retained_application_constants(&self) -> HashSet<u32> {
            self.numeric_literals
                .values()
                .copied()
                .chain(self.prepared.constants.iter().filter_map(|(_, row)| {
                    matches!(
                        row.meaning,
                        PreparedConstantMeaning::IntegerEmbedding { .. }
                    )
                    .then_some(row.declaration)
                }))
                .chain(self.identity_declaration)
                .chain(self.source_aliases.iter().map(|alias| alias.declaration))
                .chain(
                    self.indexed_families
                        .values()
                        .map(|family| family.declaration),
                )
                .collect()
        }

        fn retain_constant_meanings(&mut self) {
            let mut retain = |declaration, meaning| {
                self.prepared.constants.append(PreparedConstant {
                    declaration,
                    meaning,
                });
            };
            if let Some(declaration) = self.identity_declaration {
                retain(declaration, PreparedConstantMeaning::Identity);
            }
            for ((_, value), position) in &self.numeric_literals {
                retain(*position, PreparedConstantMeaning::Integer(value.clone()));
            }
            for ((carrier, bits), position) in &self.float_literals {
                let format = if self.float32 == Some(*carrier) {
                    numerics::literals::FloatFormat::F32
                } else if self.float64 == Some(*carrier) {
                    numerics::literals::FloatFormat::F64
                } else {
                    continue;
                };
                retain(
                    *position,
                    PreparedConstantMeaning::FloatLiteral {
                        format,
                        bits: *bits,
                    },
                );
            }
            for (value, position) in self.boolean_literals.iter().enumerate() {
                if let Some(position) = position {
                    retain(*position, PreparedConstantMeaning::Boolean(value != 0));
                }
            }
            if let Some(position) = self.integer {
                retain(position, PreparedConstantMeaning::IntegerCarrier);
            }
            if let Some(position) = self.boolean {
                retain(position, PreparedConstantMeaning::BooleanCarrier);
            }
            if let Some(position) = self.address {
                retain(position, PreparedConstantMeaning::AddressCarrier);
            }
            if let Some(position) = self.address_bound
                && let Some(symbol) = self
                    .program
                    .symbols
                    .builtin_type_member_symbol(symbols::BuiltinTypeMember::AddressBound)
            {
                retain(position, PreparedConstantMeaning::AddressBound(symbol));
            }
            if let Some(position) = self.integer_add {
                retain(
                    position,
                    PreparedConstantMeaning::IntegerOperation(BinaryOperator::Add),
                );
            }
            if let Some(position) = self.integer_subtract {
                retain(
                    position,
                    PreparedConstantMeaning::IntegerOperation(BinaryOperator::Subtract),
                );
            }
            if let Some(position) = self.integer_multiply {
                retain(
                    position,
                    PreparedConstantMeaning::IntegerOperation(BinaryOperator::Multiply),
                );
            }
            if let Some(position) = self.integer_less_than {
                retain(
                    position,
                    PreparedConstantMeaning::IntegerRelation(BinaryOperator::Less),
                );
            }
            if let Some(position) = self.integer_less_or_equal {
                retain(
                    position,
                    PreparedConstantMeaning::IntegerRelation(BinaryOperator::LessOrEqual),
                );
            }
            if let Some(position) = self.proposition_negation {
                retain(position, PreparedConstantMeaning::Negation);
            }
        }

        fn refuse(&self, message: String) -> Vec<diagnostics::Diagnostic> {
            let mut diagnostic = diagnostics::Diagnostic::error(message);
            if let Some(span) = self.span {
                diagnostic = diagnostic.with_source_span(span);
            }
            vec![diagnostic]
        }
    }

    enum CarrierHead {
        Level,
        MalformedLevel,
        Type(LevelArgument),
        MalformedType(usize),
        Term,
    }

    enum LevelArgument {
        Authored(TypeReferenceHandle),
        Generalized,
    }

    enum BinderPlan {
        Level,
        Type(LevelArgument),
        Term(TypeReferenceHandle),
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ScalarCarrier {
        Integer,
        Boolean,
        Address,
        Float32,
        Float64,
        Unit,
        Carrier(u32),
    }

    pub(super) fn has_source_evidence_demand(program: &dyn validation::CheckingInput) -> bool {
        source_families::needs_scheme(program)
            || program
                .expression_table
                .iter_expressions()
                .any(|(_, node)| {
                    matches!(node, ExpressionNode::Call(call)
                    if validation::mathematical_applications::is_identity_call(program, call))
                })
    }
}

use declaration_records::build_checked_mathematical_declarations;
use fixed_core_vocabulary::{FixedCore, FixedCoreVocabulary};
use mathematical_signature::check_mathematical_signature;

#[derive(Default)]
pub struct CheckedMathematics {
    pub declarations: Vec<checked_trees::CheckedMathematicalDeclaration>,
    pub signature: Option<checked_trees::CheckedMathematicalSignature>,
    pub applications: validation::mathematical_applications::PreparedMathematicalApplications,
}

pub fn check_mathematical_declarations(
    program: &dyn validation::CheckingInput,
) -> Result<CheckedMathematics, Vec<diagnostics::Diagnostic>> {
    if program.mathematical_definitions().is_empty()
        && !mathematical_signature::has_source_evidence_demand(program)
    {
        return Ok(CheckedMathematics::default());
    }
    let declarations = build_checked_mathematical_declarations(program)?;
    let (signature, applications) = check_mathematical_signature(program)?.into_checked_evidence();
    Ok(CheckedMathematics {
        declarations,
        signature: Some(signature),
        applications,
    })
}

impl CheckedMathematics {
    pub fn prepared_view(
        &self,
    ) -> Option<validation::mathematical_applications::PreparedMathematicalView<'_>> {
        Some(
            validation::mathematical_applications::PreparedMathematicalView {
                arena: &self.signature.as_ref()?.arena,
                declarations: &self.signature.as_ref()?.declarations,
                applications: &self.applications,
            },
        )
    }
}
