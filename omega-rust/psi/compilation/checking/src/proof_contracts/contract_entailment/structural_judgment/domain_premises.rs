//! Predicate consequences of independently established incoming values.
//!
//! State parameters and Requires describe this invocation's admitted inputs;
//! a completed call's declared result describes that exact returned value.
//! A local annotation or destination type is never an incoming premise. Bind
//! each domain subject to its captured value before retaining its predicates,
//! so later storage writes cannot reinterpret them over replacement contents.

use super::{StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    /// A successful nominal guard establishes only the selected case's facts.
    /// Its fresh common/payload terms identify this observed value, so later
    /// state substitution transports the predicate without rereading storage.
    pub(in crate::proof_contracts::contract_entailment) fn intake_selected_case_predicates(
        &mut self,
        definition: &typed_trees::data::DataDefinition,
        variant: &typed_trees::data::DataVariant,
        value: &StructuralTerm,
        subject_type: TypeReferenceHandle,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        environment: &[(String, StructuralTerm)],
    ) {
        let StructuralTerm::Constructor { fields, .. } = value else {
            return;
        };
        let program = self.program;
        // Active-case predicates belong to this exact application, including
        // its enclosing Value indices. The constructor's fresh descriptor
        // fields must denote those indices, not unrelated fresh integers.
        if program
            .data_constructor_case(subject_type, definition.symbol, variant.symbol)
            .is_none()
        {
            return;
        }
        let Some(indices) =
            self.selected_case_indices(definition, subject_type, machine, state, environment)
        else {
            return;
        };
        let declared = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .chain(program.data_payload_fields(variant));
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        for (binder, field, actual) in &indices {
            parameters.push(StateParameter {
                symbol: binder.symbol,
                name: format!(
                    "\0case_index_{}_{}",
                    binder.symbol.arena_index(),
                    binder.symbol.generation()
                )
                .as_str()
                .into(),
                type_reference: field.type_reference,
                ..Default::default()
            });
            bindings.push((parameters.last().unwrap().name.to_string(), actual.clone()));
        }
        for field in declared {
            let mut values = fields
                .iter()
                .filter(|(name, _)| name == field.name.as_str());
            let Some((_, value)) = values.next() else {
                return;
            };
            if values.next().is_some() {
                return;
            }
            if let Some((_, _, actual)) = indices
                .iter()
                .find(|(_, descriptor, _)| descriptor.symbol == field.symbol)
            {
                self.intake_equation(value.clone(), actual.clone(), 0);
            }
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: format!(
                    "\0case_field_{}_{}",
                    field.symbol.arena_index(),
                    field.symbol.generation()
                )
                .as_str()
                .into(),
                type_reference: field.type_reference,
                ..Default::default()
            });
            bindings.push((parameters.last().unwrap().name.to_string(), value.clone()));
        }
        for fact in program.proof_facts.span_or_empty(variant.where_facts) {
            if let ProofFact::Expression(expression) = fact {
                self.intake_instantiated_predicate(
                    definition.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                );
            }
        }
    }

    /// Rejoin only retained scalar Value operands. Unsupported type/const
    /// applications contribute no predicate; lexical spelling selects nothing.
    fn selected_case_indices(
        &self,
        definition: &typed_trees::data::DataDefinition,
        reference: TypeReferenceHandle,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        environment: &[(String, StructuralTerm)],
    ) -> Option<
        Vec<(
            typed_trees::data::TypeParameter,
            typed_trees::data::DataField,
            StructuralTerm,
        )>,
    > {
        use typed_trees::types::TypeReferenceNode;
        let program = self.program;
        let binders = program.data_type_parameters(definition);
        if binders.is_empty() {
            return Some(Vec::new());
        }
        let application = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { .. } => reference,
            TypeReferenceNode::Named { .. } => {
                program
                    .data_constructor_definition(reference, definition.symbol)?
                    .generic_instance?
            }
            _ => return None,
        };
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(application)
        else {
            return None;
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if arguments.len() != binders.len() {
            return None;
        }
        let mut result = Vec::new();
        for (binder, descriptor) in program
            .typed_program()
            .data_value_parameter_fields(definition)
        {
            let position = binders
                .iter()
                .position(|candidate| candidate.symbol == binder.symbol)?;
            let argument = arguments[position];
            let actual = if let Some(literal) = program
                .typed_program()
                .static_const_literal_from_type_reference(argument)
            {
                StructuralTerm::Integer(literal.value_bignum()?)
            } else {
                let symbol = match program.type_reference_table.type_reference(argument) {
                    TypeReferenceNode::ConstExpression(expression) => {
                        let typed_trees::expression::ExpressionNode::Name(path) =
                            program.expression_table.expression(*expression)
                        else {
                            return None;
                        };
                        path.symbol
                    }
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    _ => return None,
                };
                if !symbol.is_valid() {
                    return None;
                }
                if let Some(parameter) = program
                    .state_parameter_by_symbol(state, symbol)
                    .filter(|parameter| !parameter.is_mutable)
                {
                    // The legacy environment is name-keyed. Decline a shadowed
                    // state formal rather than retarget its retained index to
                    // a later local with that spelling.
                    if program.statement_table.statements(state.statement_nodes).iter().any(|statement| matches!(statement, typed_trees::statement::StatementNode::LocalData(local) if local.name == parameter.name)) { return None; }
                    if !program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some_and(|primitive| primitive.is_fixed_integer() && program.primitive_type_reference(descriptor.type_reference) == Some(primitive))
                    {
                        return None;
                    }
                    environment
                        .iter()
                        .find(|(name, _)| name == parameter.name.as_str())?
                        .1
                        .clone()
                } else if program
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| {
                        parameter.symbol == symbol && match parameter.kind {
                            typed_trees::data::TypeParameterKind::Value { type_reference }
                            | typed_trees::data::TypeParameterKind::Const { type_reference } => program.primitive_type_reference(type_reference).is_some_and(|primitive| primitive.is_fixed_integer() && program.primitive_type_reference(descriptor.type_reference) == Some(primitive)),
                            _ => false,
                        }
                    })
                {
                    StructuralTerm::BoundValue(symbol)
                } else {
                    return None;
                }
            };
            result.push((binder.clone(), descriptor.clone(), actual));
        }
        Some(result)
    }

    pub(in crate::proof_contracts::contract_entailment) fn intake_entry_domain_predicates(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) {
        let program = self.program;
        let parameters = program.state_parameters(state);
        // Arrival already admitted these values. Index equations name their
        // captured entry observations, never a later same-spelled local or
        // replacement scalar. Borrowed referents acquire no owned capture.
        let bindings = parameters
            .iter()
            .filter_map(|parameter| {
                self.captured_parameter_projection(
                    parameter.symbol,
                    &[],
                    language_semantics::content::ContentPlaceVersion::Entry,
                )
                .map(|value| (parameter.symbol, value))
            })
            .collect::<Vec<_>>();
        for parameter in parameters {
            let captured = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
                .map(|(_, value)| value);
            self.intake_admitted_value_predicates(
                parameter.type_reference,
                captured.cloned().unwrap_or_else(|| {
                    StructuralTerm::Variable(parameter.name.as_str().to_owned())
                }),
            );
            if let Some(value) = captured {
                self.intake_admitted_value_indices(
                    parameter.type_reference,
                    value,
                    state.symbol,
                    parameters,
                    &bindings,
                );
            }
        }
        let entry = program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        for contract in program
            .state_contracts(state)
            .iter()
            .chain(program.machine_contracts(machine).iter().filter(|_| entry))
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if !membership.domain_arguments.is_empty() {
                    continue;
                }
                let Some(reference) = crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    membership.value,
                ) else {
                    continue;
                };
                let Some(value) = super::super::structural_terms::structural_term(
                    program,
                    Some((machine, state, reference)),
                    None,
                    membership.value,
                    None,
                ) else {
                    continue;
                };
                self.intake_domain_predicates(
                    membership.domain_symbol,
                    reference,
                    value,
                    &mut Vec::new(),
                );
            }
        }
    }

    pub(super) fn intake_admitted_value_predicates(
        &mut self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
    ) {
        self.intake_admitted_scalar_bounds(reference, &value);
        let program = self.program;
        // Only independently admitted values donate declared refinements.
        // A pending cast or constructor may know its carrier before admission.
        if let Some(meaning) =
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
            && meaning.0.is_fixed_integer()
            && super::super::structural_terms::scalar_operand_is_observed(
                program, reference, &value,
            )
        {
            for (minimum, maximum) in
                crate::constrained_range_bounds(program, reference, false).unwrap_or_default()
            {
                for (operator, bound) in [
                    (BinaryOperator::GreaterOrEqual, minimum),
                    (BinaryOperator::LessOrEqual, maximum),
                ] {
                    self.intake_equation(
                        StructuralTerm::ScalarBinary {
                            references: [reference; 2],
                            operator,
                            meaning: [meaning; 2],
                            left: Box::new(value.clone()),
                            right: Box::new(StructuralTerm::Integer(bound)),
                        },
                        super::value_predicates::boolean_term(true),
                        0,
                    );
                }
            }
        }
        for row in crate::field_domains::declared_type_domain_rows(program, reference) {
            let Some(projected) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &row.path,
            ) else {
                continue;
            };
            let Some(projected_type) =
                crate::project_type_reference_from_segments(program, reference, &row.path)
            else {
                continue;
            };
            self.intake_domain_predicates(
                row.domain_symbol,
                projected_type,
                projected,
                &mut Vec::new(),
            );
        }
        self.intake_admitted_data_predicates(reference, value, &mut Vec::new());
    }

    /// An independently admitted value retains its explicit index application
    /// under the captured telescope that established it. Entry values and
    /// completed results share this operation; destination annotations do not.
    pub(super) fn intake_admitted_value_indices(
        &mut self,
        reference: TypeReferenceHandle,
        value: &StructuralTerm,
        callable: SymbolHandle,
        parameters: &[StateParameter],
        bindings: &[(SymbolHandle, StructuralTerm)],
    ) {
        use typed_trees::types::TypeReferenceNode;
        let program = self.program;
        for (_, descriptor, argument) in program.data_application_value_fields(reference) {
            let index =
                if let Some(literal) = program.static_const_literal_from_type_reference(argument) {
                    literal.value_bignum().map(StructuralTerm::Integer)
                } else {
                    match program.type_reference_table.type_reference(argument) {
                        TypeReferenceNode::ConstExpression(expression) => {
                            super::selected_contracts::bound_term(
                                program,
                                *expression,
                                callable,
                                parameters,
                                callable,
                                bindings,
                                Some(value),
                                0,
                            )
                        }
                        TypeReferenceNode::Named { symbol, .. } => bindings
                            .iter()
                            .find(|(formal, _)| formal == symbol)
                            .map(|(_, value)| value.clone()),
                        _ => None,
                    }
                };
            let Some(index) = index else {
                continue;
            };
            let path = [facts::PlaceSegment::Field {
                symbol: descriptor.symbol,
            }];
            let Some(descriptor) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &path,
            ) else {
                continue;
            };
            self.intake_equation(descriptor, index, 0);
        }
    }

    /// The carrier belongs to this completed value, even after conversion or
    /// a later write to its former source. A reference descriptor alone does
    /// not establish a scalar observation. These intrinsic integer facts say
    /// nothing about user domains, a call's private body or a pending result.
    pub(super) fn intake_admitted_scalar_bounds(
        &mut self,
        reference: TypeReferenceHandle,
        value: &StructuralTerm,
    ) {
        let program = self.program;
        let Some(meaning) =
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
        else {
            return;
        };
        // Address geometry remains symbolic and target-independent; the
        // hosted implementation's Addr integer_bounds is not that contract.
        if !meaning.0.is_fixed_integer() {
            return;
        }
        let Some((minimum, maximum)) = meaning.0.integer_bounds() else {
            return;
        };
        if !super::super::structural_terms::scalar_operand_is_observed(program, reference, value) {
            return;
        }
        for (operator, bound) in [
            (BinaryOperator::GreaterOrEqual, minimum),
            (BinaryOperator::LessOrEqual, maximum),
        ] {
            let predicate = StructuralTerm::ScalarBinary {
                references: [reference; 2],
                operator,
                meaning: [meaning; 2],
                left: Box::new(value.clone()),
                right: Box::new(StructuralTerm::Integer(
                    numerics::bignum::BigInt::from_i128(bound),
                )),
            };
            self.intake_equation(predicate, super::value_predicates::boolean_term(true), 0);
        }
    }

    /// A completed atomic load is a fresh snapshot of a declaration-valid
    /// resident. Its bound belongs to this captured value, never to a cached
    /// value of the cell or the receiving annotation's requested predicate.
    pub(super) fn intake_atomic_observation_bounds(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        value: &StructuralTerm,
    ) {
        let program = self.program;
        let Some(load) = crate::atomic_load_carrier(program, expression) else {
            return;
        };
        let Some(bounds) = crate::proof_contracts::arithmetic_domains::atomic_resident_interval(
            program,
            machine,
            Some(state),
            load.place,
        ) else {
            return;
        };
        let Some(meaning) =
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
        else {
            return;
        };
        if !meaning.0.is_fixed_integer()
            || !super::super::structural_terms::scalar_operand_is_observed(
                program, reference, value,
            )
        {
            return;
        }
        for (operator, bound) in [
            (BinaryOperator::GreaterOrEqual, bounds.low()),
            (BinaryOperator::LessOrEqual, bounds.high()),
        ] {
            let Some(bound) = bound else { continue };
            let predicate = StructuralTerm::ScalarBinary {
                references: [reference; 2],
                operator,
                meaning: [meaning; 2],
                left: Box::new(value.clone()),
                right: Box::new(StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(
                    bound,
                ))),
            };
            self.intake_equation(predicate, super::value_predicates::boolean_term(true), 0);
        }
    }

    /// Only admitted inputs and completed call results enter here. A pending
    /// constructor's receiving annotation is not evidence of its own `where`.
    /// Readable input references expose their entry value; produced reference
    /// descriptors still cannot manufacture observations of their referents.
    fn intake_admitted_data_predicates(
        &mut self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        active: &mut Vec<SymbolHandle>,
    ) {
        let program = self.program;
        let Some(definition) =
            crate::field_domains::readable_nominal_definition(program, reference)
        else {
            return;
        };
        if active.contains(&definition.symbol) {
            return;
        }
        active.push(definition.symbol);
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        for (parameter, value) in self.data_application_static_bindings(reference) {
            bindings.push((parameter.name.to_string(), value));
            parameters.push(parameter);
        }
        // A runtime Value binder denotes its represented descriptor word.
        // Admission of this completed value already checked that word against
        // the application at formation. It is independent of the source home
        // or expression from which an earlier caller obtained the index.
        for (binder, descriptor, _) in program.data_application_value_fields(reference) {
            let path = [facts::PlaceSegment::Field {
                symbol: descriptor.symbol,
            }];
            let Some(index) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &path,
            ) else {
                continue;
            };
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == binder.symbol)
                && let Some((_, actual)) = bindings
                    .iter()
                    .find(|(name, _)| *name == parameter.name.as_str())
            {
                self.intake_equation(index, actual.clone(), 0);
                continue;
            }
            let name = format!(
                "\0value_{}_{}",
                binder.symbol.arena_index(),
                binder.symbol.generation()
            );
            parameters.push(StateParameter {
                symbol: binder.symbol,
                name: name.as_str().into(),
                type_reference: descriptor.type_reference,
                ..Default::default()
            });
            bindings.push((name, index));
        }
        // Unknown case identity admits only common fields. Payload predicates
        // enter through the separately authenticated selected-case arm; reading
        // every variant here would project inactive same-named payloads onto
        // one subject and lend their unrelated default-domain facts.
        for field in program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
        {
            let path = [facts::PlaceSegment::Field {
                symbol: field.symbol,
            }];
            let Some(projected) = super::super::observed_projections::project_observed_value(
                program,
                reference,
                value.clone(),
                &path,
            ) else {
                continue;
            };
            let Some(projected_type) =
                crate::project_type_reference_from_segments(program, reference, &path)
            else {
                continue;
            };
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: projected_type,
                ..Default::default()
            });
            bindings.push((field.name.to_string(), projected.clone()));
            self.intake_admitted_scalar_bounds(projected_type, &projected);
            self.intake_admitted_data_predicates(projected_type, projected, active);
        }
        for fact in program.proof_facts.span_or_empty(definition.where_facts) {
            if let ProofFact::Expression(expression) = fact {
                self.intake_instantiated_predicate(
                    definition.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                );
            }
        }
        active.pop();
    }

    pub(in crate::proof_contracts::contract_entailment) fn data_application_static_bindings(
        &self,
        reference: TypeReferenceHandle,
    ) -> Vec<(StateParameter, StructuralTerm)> {
        use typed_trees::types::TypeReferenceNode;
        let program = self.program;
        let Some(definition) =
            crate::field_domains::readable_nominal_definition(program, reference)
        else {
            return Vec::new();
        };
        let carrier = program
            .type_reference_table
            .peeled_type_reference(reference);
        let application = if matches!(
            program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Generic { .. }
        ) {
            carrier
        } else if let Some(application) = definition.generic_instance {
            application
        } else {
            return Vec::new();
        };
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(application)
        else {
            return Vec::new();
        };
        let Some(template) = program.unique_data_definition_by_symbol(*base_symbol) else {
            return Vec::new();
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let parameters = program.data_type_parameters(template);
        if parameters.len() != arguments.len() {
            return Vec::new();
        }
        parameters
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| {
                let (typed_trees::data::TypeParameterKind::Const { type_reference }
                | typed_trees::data::TypeParameterKind::Value { type_reference }) = parameter.kind
                else {
                    return None;
                };
                if !program
                    .primitive_type_reference(type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                {
                    return None;
                }
                let value = if let Some(literal) = program
                    .typed_program()
                    .static_const_literal_from_type_reference(*argument)
                {
                    StructuralTerm::Integer(literal.value_bignum()?)
                } else {
                    match program.type_reference_table.type_reference(*argument) {
                        TypeReferenceNode::Named { symbol, .. } => {
                            if !symbol.is_valid()
                                || program.symbols.get(*symbol).kind
                                    != symbols::SymbolKind::TypeParameter
                            {
                                return None;
                            }
                            StructuralTerm::BoundValue(*symbol)
                        }
                        TypeReferenceNode::ConstExpression(expression) => {
                            if let Some(path) = program
                                .expression_table
                                .name_path_node(*expression)
                                .filter(|path| {
                                    path.symbol.is_valid()
                                        && program.symbols.get(path.symbol).kind
                                            == symbols::SymbolKind::TypeParameter
                                })
                            {
                                StructuralTerm::BoundValue(path.symbol)
                            } else if matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Const { .. }
                            ) {
                                super::super::structural_terms::structural_term(
                                    program,
                                    None,
                                    None,
                                    *expression,
                                    None,
                                )?
                            } else {
                                // A runtime Value recipe belongs to its
                                // completed index/descriptor observation. This
                                // static intake has no supplying frame and
                                // cannot replay the former source expression.
                                return None;
                            }
                        }
                        _ => return None,
                    }
                };
                Some((
                    StateParameter {
                        symbol: parameter.symbol,
                        name: parameter.name.clone(),
                        type_reference,
                        is_const: true,
                        ..Default::default()
                    },
                    value,
                ))
            })
            .collect()
    }

    fn intake_domain_predicates(
        &mut self,
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        active: &mut Vec<SymbolHandle>,
    ) {
        let program = self.program;
        if active.contains(&symbol) || !typed_trees::domain::has_symbol_only_proof(program, symbol)
        {
            return;
        }
        let Some(domain) = program
            .domain_definition(symbol)
            .filter(|domain| domain.alias.is_none())
        else {
            return;
        };
        active.push(symbol);
        let parameters = [StateParameter {
            symbol: domain.subject_symbol,
            name: domain.subject_name.clone(),
            type_reference: reference,
            ..Default::default()
        }];
        let bindings = [(domain.subject_name.as_str().to_owned(), value)];
        for fact in program.proof_facts(domain) {
            match fact {
                ProofFact::Expression(expression) => {
                    self.intake_instantiated_predicate(symbol, &parameters, *expression, &bindings)
                }
                ProofFact::Membership(membership) if membership.domain_arguments.is_empty() => {
                    let Some(reference) = crate::parameter_expression_result_type_reference(
                        program,
                        symbol,
                        &parameters,
                        membership.value,
                    ) else {
                        continue;
                    };
                    let Some(term) = super::super::structural_terms::structural_term(
                        program,
                        None,
                        Some((symbol, &parameters)),
                        membership.value,
                        None,
                    ) else {
                        continue;
                    };
                    let value = Self::substitute_call_term(&term, &bindings, &[]);
                    self.intake_domain_predicates(
                        membership.domain_symbol,
                        reference,
                        value,
                        active,
                    );
                }
                _ => {}
            }
        }
        active.pop();
    }

    fn intake_instantiated_predicate(
        &mut self,
        owner: SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        bindings: &[(String, StructuralTerm)],
    ) {
        let program = self.program;
        if let Some(binary) = program.expression_table.binary_node(expression) {
            if binary.operator == BinaryOperator::And {
                self.intake_instantiated_predicate(owner, parameters, binary.left, bindings);
                self.intake_instantiated_predicate(owner, parameters, binary.right, bindings);
                return;
            }
            if super::super::structural_terms::has_identity_equality_meaning(
                program, owner, parameters, expression,
            ) && let Some((left, right)) =
                super::super::structural_terms::structural_term_pair_for_parameters(
                    program, owner, parameters, binary,
                )
            {
                let left = Self::substitute_call_term(&left, bindings, &[]);
                let right = Self::substitute_call_term(&right, bindings, &[]);
                if binary.operator == BinaryOperator::Equal {
                    self.intake_equation(left, right, 0);
                } else if binary.operator == BinaryOperator::NotEqual {
                    std::sync::Arc::make_mut(&mut self.disequalities).push((left, right));
                }
            }
        }
        if let Some(term) = super::super::structural_terms::structural_term(
            program,
            None,
            Some((owner, parameters)),
            expression,
            None,
        ) {
            let term = Self::substitute_call_term(&term, bindings, &[]);
            self.intake_equation(term, super::value_predicates::boolean_term(true), 0);
        }
    }
}
