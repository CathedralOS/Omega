//! Exact source carrier and initializer custody for checked mathematical values.
//! These rows describe formation and typed terms; they do not grant contract
//! authority or classify an executable machine as proof-only.

use super::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Elaborator, TermHandle, TypeParameterKind,
    TypeReferenceNode,
};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;
use validation::mathematical_applications::{PreparedSourceCarrier, PreparedSourceEvidence};

impl Elaborator<'_> {
    /// Ambient binders do not necessarily occur in a source carrier. Retain
    /// an empty context only after the ordinary kernel forms this exact type
    /// without them. Open Type/value coordinates keep their entire telescope;
    /// no receiving policy or assumption grant follows from this check.
    /// Both callers precede constructor retention; every preceding successful
    /// constructor restores its placeholder context before this lookup.
    fn source_carrier_domains(&mut self, ty: TermHandle) -> Vec<TermHandle> {
        let domains = self
            .scope
            .iter()
            .map(|entry| entry.domain)
            .collect::<Vec<_>>();
        if domains.is_empty() {
            return domains;
        }
        let context = Context::empty().with_signature(
            proof_admission::Signature::from_declarations(self.declarations.clone()),
        );
        if proof_admission::infer_sort(
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

    pub(super) fn retain_source_evidence_carriers(&mut self) -> Result<(), Vec<Diagnostic>> {
        if self.indexed_families.is_empty() {
            return Ok(());
        }
        let mut families = self.indexed_families.values().cloned().collect::<Vec<_>>();
        families.sort_by_key(|family| family.symbol.arena_index());
        for family in families {
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
                        source_type: reference,
                        type_identity: self.program.normalized_type_identity(reference),
                        ty,
                        domains,
                    });
                }
            }
        }
        let machines = self.program.machines().to_vec();
        for machine in &machines {
            self.scope.clear();
            // Runtime states without source-family occurrences need no calculus
            // telescope. In particular, unrelated prepared scalar temporaries
            // must not become mathematical declaration admission requests.
            let states = self
                .program
                .machine_states(machine)
                .iter()
                .filter_map(|state| {
                    let references = self
                        .program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.type_reference)
                        .chain(
                            self.program
                                .statement_table
                                .statements(state.statement_nodes)
                                .iter()
                                .filter_map(|statement| {
                                    if let typed_trees::statement::StatementNode::LocalData(local) =
                                        statement
                                    {
                                        Some(local.type_reference)
                                    } else {
                                        None
                                    }
                                }),
                        )
                        .collect::<Vec<_>>();
                    references
                        .iter()
                        .any(|reference| self.known_source_family(*reference))
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
            for binder in &binders {
                let domain =
                    match binder.kind {
                        TypeParameterKind::Type => self.arena.insert(proof_admission::Term::Sort(
                            proof_admission::Sort::Type(proof_admission::Level::Constant(0)),
                        )),
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
                            primitive == typed_trees::types::PrimitiveType::Bool
                                || primitive.integer_bounds().is_some()
                        })
                    {
                        let domain =
                            self.elaborate_source_operand_type(parameter.type_reference)?;
                        // A mixed constructor application needs an independent
                        // denotation for each primitive argument, not an assumed
                        // scalar-to-mathematical cast at the wire boundary.
                        let domains = self.source_carrier_domains(domain);
                        self.prepared.source_carriers.append(PreparedSourceCarrier {
                            source_type: parameter.type_reference,
                            type_identity: self
                                .program
                                .normalized_type_identity(parameter.type_reference),
                            ty: domain,
                            domains,
                        });
                        self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
                    }
                }
                for reference in references {
                    if !self.known_source_family(reference)
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
                        source_type: reference,
                        type_identity: self.program.normalized_type_identity(reference),
                        ty,
                        domains,
                    });
                }
                // Constructors used only in erased locals still demand their
                // own predicate proof, even without a mathematical call.
                let constructors = self
                    .program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| match statement {
                        typed_trees::statement::StatementNode::LocalData(local)
                            if local.relevance == language_core::BindingRelevance::Erased
                                && self.known_source_family(local.type_reference)
                                && matches!(
                                    self.program
                                        .expression_table
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
        }
        self.scope.clear();
        Ok(())
    }

    pub(super) fn retain_checked_source_application(
        &mut self,
        expression: ExpressionHandle,
        term: TermHandle,
        context: &Context,
        signature: &proof_admission::Signature,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(reference) =
            validation::mathematical_applications::operand_type_reference(self.program, expression)
        else {
            return Ok(());
        };
        if !self.known_source_family(reference) || !self.scope.is_empty() {
            return Ok(());
        }
        let ty = proof_admission::infer_type(
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
        // The ordinary result reference is a template, not an instantiated
        // source application. Match the independently inferred Pi result to
        // formed source carriers while retaining nominal family heads.
        let carriers = self
            .prepared
            .source_carriers
            .iter()
            .filter_map(|(_, carrier)| {
                carrier.domains.is_empty().then_some((
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
                "mathematical initializer has no exact formed source carrier correspondence".into(),
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
    pub(super) fn retain_indexed_contract_theorems(&mut self) -> Result<(), Vec<Diagnostic>> {
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
            // Indexed logical citations have no executable result. A runtime
            // function's result contract belongs to normal body/return checking;
            // treating its `result` as a family invariant invents an unrelated
            // induction goal before the body can establish that value.
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
            let arguments = self
                .program
                .type_reference_table
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
            // A runtime Value instance realizes its original scalar binders as
            // ordinary entry parameters. Reconstruct only that authenticated
            // telescope, and only when the exact parameter is a family index.
            // Other formals never become mathematical assumptions by scope.
            let specializations = self
                .program
                .machine_specializations
                .iter()
                .filter(|row| row.instance == machine.symbol)
                .collect::<Vec<_>>();
            if let [specialization] = specializations.as_slice()
                && self.program.executable_application_template(machine.symbol)
                    == Ok(Some(specialization.template))
                && let Some(template) = self.program.machine_by_symbol(specialization.template)
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
                                primitive == typed_trees::types::PrimitiveType::Bool
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
                    let Some(
                        typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
                            type_reference: realized_carrier,
                            ..
                        },
                    ) = self
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
            if telescope.len() > family.parameters.len() {
                continue;
            }
            self.source_owner = machine.symbol;
            self.scope.clear();
            self.span = self.program.symbols.symbol_source_span(machine.symbol);
            for (symbol, name, type_reference) in &telescope {
                let domain = if let Some(reference) = type_reference {
                    self.elaborate_type_reference(*reference)?
                } else {
                    self.arena
                        .insert(proof_admission::Term::Sort(proof_admission::Sort::Type(
                            proof_admission::Level::Constant(0),
                        )))
                };
                self.push_scope(Some(*symbol), name.clone(), domain);
            }
            let actuals = arguments
                .iter()
                .zip(&family.parameters)
                .map(|(argument, parameter)| self.index_operand(*argument, parameter, &[]))
                .collect::<Result<Vec<_>, _>>()?;
            let mut mapping = Vec::new();
            for ordinal in 0..telescope.len() {
                let variable = self.scope_variable(ordinal);
                let positions = actuals
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
                    if position + 1 < family.parameters.len() {
                        projection = self.arena.insert(Term::Fst { pair: projection });
                    }
                    // Discharge only the telescope we just constructed. Its
                    // arguments are projections of the bound index tuple,
                    // so substitution removes administrative beta redexes
                    // without unfolding an authored function or discarding
                    // an assumption-bearing actual. Every constant in the
                    // authored proposition remains in the resulting body;
                    // the family index retains the exact source domains.
                    let Term::Lambda { body, .. } = self.arena.get(proposition) else {
                        unreachable!("constructed source invariant telescope")
                    };
                    proposition = proof_admission::substitute(&mut self.arena, body, projection);
                }
                let invariant = self.arena.insert(Term::Lambda {
                    domain: family.description.index,
                    body: proposition,
                });
                self.scope.clear();
                let theorem = self.source_family_induction(symbol, invariant)?;
                let signature = Signature::from_declarations(self.declarations.clone());
                let closure = judgment_assumption_closure(
                    &self.arena,
                    &signature,
                    &[theorem.theorem, theorem.ty],
                );
                self.prepared.indexed_theorems.append(
                    validation::mathematical_applications::PreparedIndexedTheorem {
                        machine: machine.symbol,
                        expression,
                        ensure_ordinal: ensure_ordinal as u32,
                        evidence_formal: evidence.symbol,
                        source_type: reference,
                        theorem: theorem.theorem,
                        ty: theorem.ty,
                        assumption_closure: closure.into_iter().collect(),
                    },
                );
                // Restore the exact scalar telescope for another authored clause.
                for (symbol, name, type_reference) in &telescope {
                    let domain = if let Some(reference) = type_reference {
                        self.elaborate_type_reference(*reference)?
                    } else {
                        self.arena
                            .insert(proof_admission::Term::Sort(proof_admission::Sort::Type(
                                proof_admission::Level::Constant(0),
                            )))
                    };
                    self.push_scope(Some(*symbol), name.clone(), domain);
                }
            }
        }
        self.scope.clear();
        Ok(())
    }
}

impl Elaborator<'_> {
    pub(super) fn bind_source_evidence_types(
        &mut self,
        signature: &proof_admission::Signature,
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
                    let closure = proof_admission::judgment_assumption_closure(
                        &self.arena,
                        signature,
                        &[constructor.schema, constructor.ty],
                    );
                    let ungranted = closure
                        .into_iter()
                        .filter(|position| !self.prepared.safety_assumptions.contains(position))
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
            let Some(carrier) = self
                .prepared
                .source_carriers
                .iter()
                .find_map(|(_, carrier)| {
                    (identity.is_valid()
                        && carrier.type_identity == identity
                        && carrier.domains.is_empty())
                    .then_some(carrier.ty)
                })
            else {
                return Err(self.refuse(
                    "source evidence has no independently formed closed source carrier".into(),
                ));
            };
            proof_admission::check_type(
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
            let closure = proof_admission::judgment_assumption_closure(
                &self.arena,
                signature,
                &[term, carrier],
            );
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
    /// Retain each exact immutable erased local use from its checked initializer.
    /// A use cannot acquire evidence merely from its receiving destination.
    pub(super) fn retain_source_evidence_aliases(&mut self) -> Result<(), Vec<Diagnostic>> {
        if self.prepared.source_evidence.is_empty()
            && self.prepared.source_constructors.is_empty()
        {
            return Ok(());
        }
        let owners = validation::expression_owners::OwnerEnvironmentIndex::new(
            self.program,
            &arena::Arena::default(),
        );
        let mut pending = self
            .program
            .expression_table
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
                let locals = self
                    .program
                    .machines()
                    .iter()
                    .flat_map(|machine| {
                        self.program
                            .machine_states(machine)
                            .iter()
                            .filter_map(|state| {
                                self.program.state_local_declaration(state, *symbol).map(
                                    |(ordinal, local)| {
                                        (machine.symbol, state.symbol, ordinal, local)
                                    },
                                )
                            })
                    })
                    .collect::<Vec<_>>();
                let [(machine, state, ordinal, local)] = locals.as_slice() else {
                    return false;
                };
                if validation::expression_owners::source_declaration_owner(
                    self.program,
                    *expression,
                    &owners,
                ) != Some(*machine)
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
                if local.is_mutable || local.relevance != language_core::BindingRelevance::Erased {
                    return false;
                }
                let identity = self.program.normalized_type_identity(local.type_reference);
                if let Some(mut constructor) =
                    self.prepared
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
                let Some((term, ty)) = self.prepared.source_evidence.iter().find_map(|(_, row)| {
                    (row.expression == local.initial_value
                        && identity.is_valid()
                        && row.type_identity == identity)
                        .then_some((row.term, row.ty))
                }) else {
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
