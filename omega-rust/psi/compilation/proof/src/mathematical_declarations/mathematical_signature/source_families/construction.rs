//! Source constructors supply actual labels, predicate derivations and indexed
//! children. A receiving application is not evidence that a value inhabits it.

use super::super::Elaborator;
use diagnostics::Diagnostic;
use proof_admission::{Budget, Context, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle};
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Temporary named values are placeholders only while constructing a lambda.
/// Each retained schema abstracts every placeholder, including dependent child
/// domains and contract premises; no source value becomes a signature axiom.
#[derive(Default)]
pub(in super::super) struct SourceConstructorContext {
    pub owner: SymbolHandle,
    pub values: Vec<(SymbolHandle, TermHandle)>,
    pub parameters: Vec<(u32, SymbolHandle, TermHandle)>,
    pub premises: Vec<(TermHandle, TermHandle)>,
}

impl Elaborator<'_> {
    pub(in super::super) fn retain_source_constructor_schema(
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
        let evidence = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                parameter.relevance == language_core::BindingRelevance::Erased
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
                    "open source constructor premise has no mathematical proposition".into(),
                ));
            };
            let closed = self
                .integer_theory
                .as_mut()
                .map(|theory| {
                    theory.closed_predicate(&mut self.arena, &mut self.declarations, domain)
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
            super::terms::abstract_parameters(&mut self.arena, term, ty, &parameters).map_err(
                |error| self.refuse(format!("source constructor abstraction failed: {error}")),
            )?;
        if !super::terms::excludes_parameters(
            &self.arena,
            &self.declarations,
            &[schema, schema_type],
            &temporary_parameters,
        ) {
            return Err(self.refuse(
                "source constructor schema retains a temporary value or premise assumption".into(),
            ));
        }
        proof_admission::check_type(
            &mut self.arena,
            &Context::empty()
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
        self.declarations
            .push(proof_admission::Declaration::definition(
                0,
                schema_type,
                schema,
            ));
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
        else {
            return Err(
                self.refuse("source constructor schema lost its construction occurrence".into())
            );
        };
        let TypeReferenceNode::Generic { base_symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return Err(self.refuse("source constructor schema lost its family application".into()));
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
                    .is_some_and(|(actual, candidate)| actual.symbol == candidate.symbol),
                None => !case.symbol.is_valid(),
            })
            .ok_or_else(|| {
                self.refuse("source constructor schema lost its selected case".into())
            })?;
        let predicate_expressions = selected.predicate_expressions.clone();
        self.prepared.source_constructors.append(
            validation::mathematical_applications::PreparedSourceConstructor {
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
            },
        );
        self.source_constructor_context = SourceConstructorContext::default();
        Ok(())
    }

    /// Select the ordered source telescope actually used by these roots. The
    /// kernel's transitive closure includes declaration bodies and dependent
    /// parameter domains; unused lexical carriers introduce no assumptions.
    fn source_constructor_dependencies(
        &self,
        roots: &[TermHandle],
    ) -> Vec<(u32, SymbolHandle, TermHandle)> {
        let dependencies = proof_admission::judgment_assumption_closure(
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
        self.declarations
            .push(proof_admission::Declaration::assumption(0, domain));
        let term = self.constant(position);
        self.source_constructor_context
            .parameters
            .push((position, symbol, domain));
        if symbol.is_valid() {
            self.source_constructor_context.values.push((symbol, term));
        }
        term
    }

    pub(in super::super) fn source_family_constructor(
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
            self.prepared.source_evidence.append(
                validation::mathematical_applications::PreparedSourceEvidence {
                    expression,
                    source_type: reference,
                    type_identity: self.program.normalized_type_identity(reference),
                    term,
                    ty,
                },
            );
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
                "source constructor evidence exceeds the construction depth bound".into(),
            ));
        }
        if let ExpressionNode::Name(path) = self.program.expression_table.expression(expression) {
            let symbol = path
                .resolved_symbol()
                .ok_or_else(|| self.refuse("source constructor use has no exact binding".into()))?;
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
                            || local.relevance != language_core::BindingRelevance::Erased
                        {
                            return Err(self.refuse("source indexed capture requires retained immutable erased constructor evidence".into()));
                        }
                        if self.program.normalized_type_identity(local.type_reference)
                            != self.program.normalized_type_identity(reference)
                        {
                            return Err(self.refuse(
                                "source indexed capture changed its exact application".into(),
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
                                "source indexed capture has duplicate declaration identity".into(),
                            ));
                        }
                    }
                }
            }
            let (value, actual, declaring_state, declaration_ordinal) =
                initializer.ok_or_else(|| {
                    self.refuse(
                        "source indexed capture has no retained constructor derivation".into(),
                    )
                })?;
            let owners = validation::expression_owners::OwnerEnvironmentIndex::new(
                self.program,
                &arena::Arena::default(),
            );
            let sites = owners.containing_statement_sites(expression);
            if validation::expression_owners::source_declaration_owner(
                self.program,
                expression,
                &owners,
            ) != Some(self.source_owner)
                || sites.is_empty()
                || sites.iter().any(|(machine, state, statement)| {
                    let machine = &self.program.machines()[*machine];
                    machine.symbol != self.source_owner
                        || self.program.machine_states(machine)[*state].symbol != declaring_state
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
            return Err(
                self.refuse("source indexed operand has no constructor derivation yet".into())
            );
        };
        let literal = literal.clone();
        self.span = Some(self.program.expression_table.source_span(expression));
        let TypeReferenceNode::Generic { base_symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return Err(
                self.refuse("source indexed constructor lost its exact family application".into())
            );
        };
        let family = self.source_family(*base_symbol)?;
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
                        .data_constructor_case(reference, literal.type_symbol, case.symbol)
                        .is_some_and(|candidate| candidate.symbol == actual.symbol)
                })
                .ok_or_else(|| {
                    self.refuse("source indexed constructor selected a foreign case".into())
                })?
        } else {
            if family.cases.len() != 1 || family.cases[0].symbol.is_valid() {
                return Err(
                    self.refuse("source indexed constructor omitted its active case".into())
                );
            }
            0
        };
        let selected = &family.cases[case];
        let fields = self
            .program
            .expression_table
            .struct_fields(literal.fields)
            .to_vec();
        if fields.len() != family.common.len() + selected.payload.len() + selected.children.len() {
            return Err(
                self.refuse("source indexed constructor changed its exact field roster".into())
            );
        }
        let mut common = Vec::new();
        let mut payload = Vec::new();
        let mut children = Vec::new();
        for field in &family.common {
            let operand =
                self.source_constructor_operand(expression, reference, &literal, field)?;
            common.push(self.elaborate_expression(operand)?);
        }
        for field in &selected.payload {
            let operand =
                self.source_constructor_operand(expression, reference, &literal, field)?;
            payload.push(self.elaborate_expression(operand)?);
        }
        if selected.children.len() != selected.child_indices.len() {
            return Err(self.refuse(
                "generated source constructor lost its declared child index roster".into(),
            ));
        }
        for (field, &child_index) in selected.children.iter().zip(&selected.child_indices) {
            let operand =
                self.source_constructor_operand(expression, reference, &literal, field)?;
            self.span = Some(self.program.expression_table.source_span(operand));
            let actual = validation::mathematical_applications::operand_type_reference(
                self.program,
                operand,
            )
            .ok_or_else(|| {
                self.refuse("source recursive child has no retained actual application".into())
            })?;
            // Kernel descriptions can be structurally equal for distinct source
            // declarations. Conversion never supplies nominal source custody.
            let mut application = actual;
            let mut actual_family = None;
            for _ in 0..128 {
                match self
                    .program
                    .type_reference_table
                    .type_reference(application)
                {
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
            for &argument in common.iter().chain(payload.iter()) {
                index = self.arena.insert(Term::Apply {
                    function: index,
                    argument,
                });
            }
            let expected = family.description.indexed_w(&mut self.arena, index);
            let context = Context::empty()
                .with_signature(Signature::from_declarations(self.declarations.clone()));
            match proof_admission::check_type(
                &mut self.arena,
                &context,
                child,
                expected,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            ) {
                Ok(()) => children.push(child),
                Err(proof_admission::CoreError::TypeMismatch { .. }) => {
                    return Err(self.refuse(source_mismatch));
                }
                Err(proof_admission::CoreError::StepCeiling) => {
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
        for &argument in common.iter().chain(payload.iter()) {
            predicate = self.arena.insert(Term::Apply {
                function: predicate,
                argument,
            });
        }
        let Some(theory) = &mut self.integer_theory else {
            return Err(
                self.refuse("source constructor predicate has no shared integer vocabulary".into())
            );
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
            self.refuse("source constructor predicate has no checked derivation yet".into())
        })?;
        for (_, argument) in premises {
            proof = self.arena.insert(Term::Apply {
                function: proof,
                argument,
            });
        }
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        proof_admission::check_type(
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
        proof_admission::check_type(
            &mut self.arena,
            &context,
            label,
            family.description.carrier,
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
        let parameters = self.source_constructor_dependencies(&[label, family.description.carrier]);
        let (label_body, label_type) = super::terms::abstract_parameters(
            &mut self.arena,
            label,
            family.description.carrier,
            &parameters,
        )
        .map_err(|error| {
            self.refuse(format!(
                "source constructor label abstraction failed: {error}"
            ))
        })?;
        let label_position = self.declarations.len() as u32;
        self.declarations
            .push(proof_admission::Declaration::definition(
                0, label_type, label_body,
            ));
        let mut label = self.constant(label_position);
        for (position, _, _) in parameters {
            let argument = self.constant(position);
            label = self.arena.insert(Term::Apply {
                function: label,
                argument,
            });
        }
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        let child_domain = self.arena.insert(Term::Apply {
            function: family.description.children,
            argument: label,
        });
        let child_body = match children.as_slice() {
            [child] => *child,
            [] => {
                let p = self.arena.insert(Term::Variable(0));
                let next = self.arena.insert(Term::Apply { function: family.description.next, argument: label });
                let index = self.arena.insert(Term::Apply { function: next, argument: p });
                let result = family.description.indexed_w(&mut self.arena, index);
                let motive = self.arena.insert(Term::Lambda { domain: child_domain, body: result });
                let empty = self.arena.insert(Term::Empty);
                let e = self.arena.insert(Term::Variable(0));
                let boxed = self.arena.insert(Term::BoxIntro { ty: empty, value: e });
                let index = self.arena.insert(Term::Apply { function: next, argument: boxed });
                let result = family.description.indexed_w(&mut self.arena, index);
                let body = self.arena.insert(Term::EmptyElim { ty: result, scrutinee: e });
                let body = self.arena.insert(Term::Lambda { domain: empty, body });
                self.arena.insert(Term::BoxElim { motive, body, scrutinee: p })
            }
            _ => return Err(self.refuse("source constructor child-function production for multiple recursive fields is not implemented yet".into())),
        };
        let children_fn = self.arena.insert(Term::Lambda {
            domain: child_domain,
            body: child_body,
        });
        let next = self.arena.insert(Term::Apply {
            function: family.description.next,
            argument: label,
        });
        let p = self.arena.insert(Term::Variable(0));
        let index = self.arena.insert(Term::Apply {
            function: next,
            argument: p,
        });
        let codomain = family.description.indexed_w(&mut self.arena, index);
        let expected_children = self.arena.insert(Term::Pi {
            domain: child_domain,
            codomain,
        });
        proof_admission::check_type(
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
        let term = family.description.sup(&mut self.arena, label, children_fn);
        let expected = self.elaborate_type_reference(reference)?;
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        proof_admission::check_type(
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
                    "source constructor lost its exact declared field correspondence".into(),
                )
            })?;
        let mut operands = self
            .program
            .expression_table
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
            return Err(
                self.refuse("source indexed constructor duplicated a declared field".into())
            );
        }
        Ok(operand)
    }
}
