use crate::declarations::data::lower_data_definition;
use crate::declarations::domain::lower_domain_definition;
use crate::declarations::domain_constraints::normalize_domain_constraints;
use crate::declarations::machine::lower_machine;
use crate::declarations::operator::lower_operator_definition;
use crate::declarations::trait_definition::lower_trait_definition;
use crate::expressions::qualification_casts::normalize_qualification_casts;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;

pub(crate) mod name;
pub(crate) mod progress;
pub(crate) mod seeded_continuation;

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    // Decision 11: user-written `==` against bare payload-bearing case names
    // must be rejected BEFORE membership lowering synthesizes its internal
    // tag-equality compares, which are deliberately the same typed shape.
    crate::expressions::equality::validate_equality_operands(symbol_resolved_trees)?;

    // Equatable conformance prerequisites error at the conformance item,
    // before any `==` site tries to expand against a malformed type.
    crate::expressions::equatable::validate_equatable_conformances(symbol_resolved_trees)?;

    // Exhaustiveness counting over case domains also needs the resolved
    // trees: membership is still a distinct node here, so case arms and
    // domain arms are recognizable before lowering erases them into tag
    // compares and classifier expansions.
    crate::expressions::exhaustiveness::validate_case_dispatch_exhaustiveness(
        symbol_resolved_trees,
    )?;

    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
        equality_scope: None,
        type_reference_exposure:
            Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
    };
    lowerer.typed_trees.service_reaches = symbol_resolved_trees.service_reaches.clone();
    lowerer.typed_trees.service_reach_rows = symbol_resolved_trees.service_reach_rows.clone();
    lowerer.typed_trees.authored_service_reach_rows = symbol_resolved_trees
        .authored_service_reach_rows
        .iter()
        .map(lower_authored_service_reach_row)
        .collect();
    lowerer.typed_trees.semantic_domains = symbol_resolved_trees.semantic_domains.clone();
    lowerer.typed_trees.external_bindings = symbol_resolved_trees.external_bindings.clone();
    lowerer.typed_trees.retain_authored_declaration_selections(
        symbol_resolved_trees
            .authored_declaration_selections()
            .clone(),
    );
    for declaration in &symbol_resolved_trees.const_declarations {
        lowerer.lower_const_declaration(declaration)?;
    }
    lowerer.typed_trees.evidence_forwardings = symbol_resolved_trees
        .evidence_forwardings
        .iter()
        .map(|forwarding| typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: forwarding.machine_symbol,
            state_symbol: forwarding.state_symbol,
            statement_index: forwarding.statement_index,
            source_statement_index: forwarding.statement_index,
            target: crate::lowerer::name::lower_name(&forwarding.target),
            source: crate::lowerer::name::lower_name(&forwarding.source),
            source_conformance: forwarding.source_conformance,
        })
        .collect();

    for data_definition in &symbol_resolved_trees.data_definitions {
        lowerer.lower_data_declaration(data_definition)?;
    }

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(domain_definition.is_public),
            |lowerer| lower_domain_definition(lowerer, domain_definition),
        )?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for proposition in &symbol_resolved_trees.propositions {
        let proposition = lowerer.with_type_reference_exposure(
            declaration_exposure(proposition.is_public),
            |lowerer| {
                crate::expressions::proposition::lower_proposition_definition(lowerer, proposition)
            },
        )?;
        lowerer.typed_trees.push_proposition(proposition);
    }

    for machine in &symbol_resolved_trees.machines {
        lowerer.lower_machine_declaration(machine)?;
    }

    for measure in &symbol_resolved_trees.measures {
        let measure =
            crate::declarations::measure::lower_measure_definition(&mut lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lowerer
            .with_type_reference_exposure(declaration_exposure(operator.is_public), |lowerer| {
                lower_operator_definition(lowerer, operator)
            })?;
        lowerer.typed_trees.push_operator(operator);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        lowerer.lower_trait_declaration(trait_definition)?;
    }

    for conformance in &symbol_resolved_trees.conformances {
        let conformance_exposure = declaration_exposure(conformance.is_public);
        if let symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(carrier_name) =
            &conformance.subject
        {
            crate::type_reference::retain_type_reference_selection(
                symbol_resolved_trees,
                &mut lowerer.typed_trees,
                carrier_name,
                conformance.carrier_symbol,
                conformance_exposure,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
        }
        crate::type_reference::retain_type_reference_selection(
            symbol_resolved_trees,
            &mut lowerer.typed_trees,
            &conformance.trait_name,
            conformance.trait_symbol,
            conformance_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
        let source_type_parameters = conformance.type_parameters;
        let mut arguments = arena::HandleSpan::empty();
        for argument in symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .span_or_empty(conformance.arguments)
        {
            let argument = lowerer
                .with_type_reference_exposure(conformance_exposure, |lowerer| {
                    crate::type_reference::lower_type_reference_into_table(lowerer, argument)
                })?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        let trait_lifetime_arguments = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|argument| {
                let ordinal = conformance
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter == argument)
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "conformance `{}` target trait lifetime `'{}' does not name an in-scope conformance lifetime binder",
                            conformance
                                .alias
                                .as_ref()
                                .map_or("<unnamed-conformance>", |name| name.as_str()),
                            argument.as_str(),
                        ))
                    })?;
                u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "conformance `{}` target trait lifetime ordinal exceeds the compiler limit",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed-conformance>", |name| name.as_str()),
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut conformance = typed_trees::trait_definition::Conformance {
            symbol: conformance.symbol,
            is_public: conformance.is_public,
            lifetime_parameters: conformance
                .lifetime_parameters
                .iter()
                .map(crate::lowerer::name::lower_name)
                .collect(),
            type_parameters: arena::HandleSpan::empty(),
            subject: match &conformance.subject {
                symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                    type_name,
                ) => typed_trees::trait_definition::ConformanceSubject::Carrier(
                    crate::lowerer::name::lower_name(type_name),
                ),
                symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                    typed_trees::trait_definition::ConformanceSubject::Subjectless
                }
            },
            carrier_symbol: conformance.carrier_symbol,
            trait_name: crate::lowerer::name::lower_name(&conformance.trait_name),
            trait_symbol: conformance.trait_symbol,
            trait_lifetime_arguments,
            arguments,
            alias: conformance.alias.as_ref().map(crate::lowerer::name::lower_name),
            implementation: match &conformance.implementation {
                symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                    typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
                }
                symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } => {
                    typed_trees::trait_definition::ConformanceImplementation::Closed {
                        rows: rows
                            .iter()
                            .map(|row| typed_trees::trait_definition::ConformanceRow {
                                declaring_trait: row.declaring_trait,
                                declaring_trait_name: crate::lowerer::name::lower_name(&row.declaring_trait_name),
                                requirement: row.requirement,
                                requirement_name: crate::lowerer::name::lower_name(&row.requirement_name),
                                realization_machine: row.realization_machine,
                                realization_state: row.realization_state,
                                realization_name: crate::lowerer::name::lower_name(&row.realization_name),
                                source: match row.source {
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => typed_trees::trait_definition::ConformanceRowSource::Inline,
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => typed_trees::trait_definition::ConformanceRowSource::Reference,
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => typed_trees::trait_definition::ConformanceRowSource::TraitDefault,
                                },
                            })
                            .collect(),
                    }
                }
            },
        };
        conformance.type_parameters =
            lowerer.with_type_reference_exposure(conformance_exposure, |lowerer| {
                crate::declarations::data::lower_type_parameters(lowerer, source_type_parameters)
            })?;
        // Inline/default realization machines close over the conformance
        // name's telescope. Publish that telescope as the machine template's
        // own generic surface as well, so ordinary specialization can clone
        // and substitute the selected row instead of leaving family symbols
        // such as `Element` in executable checked code. Referenced external
        // machines keep their independently declared telescope.
        let realization_machines = match &conformance.implementation {
            typed_trees::trait_definition::ConformanceImplementation::Closed { rows } => rows
                .iter()
                .filter(|row| {
                    matches!(
                        row.source,
                        typed_trees::trait_definition::ConformanceRowSource::Inline
                            | typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                    )
                })
                .map(|row| row.realization_machine)
                .collect::<Vec<_>>(),
            typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                Vec::new()
            }
        };
        for machine in lowerer.typed_trees.machines_mut() {
            if realization_machines.contains(&machine.symbol) {
                machine.lifetime_parameters = conformance.lifetime_parameters.clone();
                machine.type_parameters = conformance.type_parameters;
            }
        }
        lowerer.typed_trees.push_conformance(conformance);
    }

    for wire_schema in &symbol_resolved_trees.wire_schemas {
        let wire_schema = lowerer.with_type_reference_exposure(
            declaration_exposure(wire_schema.is_public),
            |lowerer| crate::declarations::wire::lower_wire_schema(lowerer, wire_schema),
        )?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }

    lowerer.lower_const_initializer_evidence(0)?;
    lowerer.finish()
}

fn lower_authored_service_reach_row(
    row: &symbol_resolved_trees::signature::AuthoredServiceReachRow,
) -> typed_trees::signature::AuthoredServiceReachRow {
    typed_trees::signature::AuthoredServiceReachRow {
        owner: row.owner,
        keyword_source_spans: row.keyword_source_spans.clone(),
        targets: row
            .targets
            .iter()
            .map(
                |target| typed_trees::signature::AuthoredServiceReachTarget {
                    service: target.service,
                    source_span: target.source_span,
                },
            )
            .collect(),
        installation_bound: row.installation_bound,
    }
}

pub(crate) fn exact_top_level_data_symbol(
    source: &SymbolResolvedTrees,
    definition: &symbol_resolved_trees::data::DataDefinition,
) -> bool {
    definition.symbol.is_valid()
        && source.symbols.get(definition.symbol).kind == symbols::SymbolKind::Data
        && source.symbols.get(definition.symbol).parent == source.symbols.root()
        && source.symbols.name(definition.symbol) == definition.name.as_str()
}

pub(crate) fn exact_field_symbol(
    source: &SymbolResolvedTrees,
    owner: symbols::SymbolHandle,
    field: &symbol_resolved_trees::data::DataField,
) -> bool {
    field.symbol.is_valid()
        && source.symbols.get(field.symbol).kind == symbols::SymbolKind::Field
        && source.symbols.get(field.symbol).parent == owner
        && source.symbols.name(field.symbol) == field.name.as_str()
}

fn declaration_exposure(
    is_public: bool,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;

    if is_public {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

fn machine_interface_exposure(
    machine: &symbol_resolved_trees::machine::Machine,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    let is_exported_boundary = matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
    );
    declaration_exposure(machine.is_public || is_exported_boundary)
}

pub fn lower_symbol_resolved_trees_owned(
    symbol_resolved_trees: SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut typed_trees = lower_symbol_resolved_trees(&symbol_resolved_trees)?;
    typed_trees.symbols = symbol_resolved_trees.symbols;
    Ok(typed_trees)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
    /// The value-typing scope of the state body currently being lowered;
    /// `==` expansion uses it to find an operand's data type.
    pub(crate) equality_scope: Option<crate::expressions::equatable::EqualityScope>,
    /// None lowers a compiler-derived type without inventing an authored
    /// occurrence. Original generic applications retain their own exposure.
    pub(crate) type_reference_exposure:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
}

impl Lowerer<'_> {
    fn lower_const_declaration(
        &mut self,
        declaration: &symbol_resolved_trees::constant::ConstDeclaration,
    ) -> Result<(), Diagnostic> {
        let declared_type = self.with_type_reference_exposure(
            declaration_exposure(declaration.is_public),
            |lowerer| {
                crate::type_reference::lower_type_reference_into_table(
                    lowerer,
                    &declaration.declared_type,
                )
            },
        )?;
        self.typed_trees
            .push_const_declaration(typed_trees::constant::ConstDeclaration {
                symbol: declaration.symbol,
                is_public: declaration.is_public,
                declared_type,
                initializer_source_span: declaration.initializer_source_span,
                canonical_value_encoding: declaration.canonical_value_encoding.clone(),
                authored_initializer: typed_trees::expression::ExpressionHandle::invalid(),
                materialized_initializer: typed_trees::expression::ExpressionHandle::invalid(),
            });
        Ok(())
    }

    fn lower_data_declaration(
        &mut self,
        data_definition: &symbol_resolved_trees::data::DataDefinition,
    ) -> Result<(), Diagnostic> {
        let data_definition = self.with_type_reference_exposure(
            declaration_exposure(data_definition.is_public),
            |lowerer| lower_data_definition(lowerer, data_definition),
        )?;
        self.typed_trees.push_data_definition(data_definition);
        Ok(())
    }

    fn lower_machine_declaration(
        &mut self,
        machine: &symbol_resolved_trees::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let machine = self
            .with_type_reference_exposure(machine_interface_exposure(machine), |lowerer| {
                lower_machine(lowerer, machine)
            })?;
        self.typed_trees.push_machine(machine);
        Ok(())
    }

    fn lower_trait_declaration(
        &mut self,
        trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
    ) -> Result<(), Diagnostic> {
        let trait_definition = self.with_type_reference_exposure(
            declaration_exposure(trait_definition.is_public),
            |lowerer| lower_trait_definition(lowerer, trait_definition),
        )?;
        self.typed_trees.push_trait_definition(trait_definition);
        Ok(())
    }

    fn lower_const_initializer_evidence(&mut self, frontier: usize) -> Result<(), Diagnostic> {
        for ordinal in frontier..self.source_trees.const_declarations.len() {
            let declaration = &self.source_trees.const_declarations[ordinal];
            let (symbol, original, materialized) = (
                declaration.symbol,
                declaration.authored_initializer,
                declaration.initializer,
            );
            let handle = self
                .typed_trees
                .tables
                .const_declarations
                .iter()
                .find_map(|(handle, declaration)| (declaration.symbol == symbol).then_some(handle))
                .ok_or_else(|| {
                    Diagnostic::error("constant fold lost its exact typed declaration")
                })?;
            if !self
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_is_valid(materialized)
            {
                return Err(Diagnostic::error(
                    "constant lost its materialized initializer",
                ));
            }
            let materialized_initializer =
                crate::expressions::expression::lower_expression_handle(self, materialized)?;
            self.typed_trees
                .tables
                .const_declarations
                .get_mut(handle)
                .materialized_initializer = materialized_initializer;
            if !original.is_valid() {
                continue;
            }
            let authored_initializer =
                crate::expressions::expression::lower_expression_handle(self, original)?;
            let declaration = self.typed_trees.tables.const_declarations.get_mut(handle);
            declaration.authored_initializer = authored_initializer;
        }
        Ok(())
    }

    pub(crate) fn with_type_reference_exposure<T>(
        &mut self,
        exposure: impl Into<
            Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
        >,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous = std::mem::replace(&mut self.type_reference_exposure, exposure.into());
        let result = operation(self);
        self.type_reference_exposure = previous;
        result
    }

    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        crate::declarations::machine::settle_satisfied_declarations(&mut self.typed_trees)?;
        crate::lowerer::progress::normalize_progress_premises(&mut self.typed_trees)?;
        let TypedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            authored_service_reach_rows,
            semantic_domains,
            external_bindings,
            plan_laid_layouts: _,
            placed_view_plans: _,
            wire_placements: _,
            wire_encode_obligations: _,
            wire_schema_plans: _,
            machine_specializations: _,
            boundary_calling_plans: _,
            fused_service_erasures: _,
            open_index_normalizations: _,
            evidence_forwardings,
            proof_output_calls,
            ranking_expression_custody,
        } = self.typed_trees;

        let mut trees = TypedTrees::with_roots(roots, tables, symbols);
        // The copied semantic interners survive the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.authored_service_reach_rows = authored_service_reach_rows;
        trees.semantic_domains = semantic_domains;
        trees.external_bindings = external_bindings;
        trees.evidence_forwardings = evidence_forwardings
            .into_iter()
            .map(|mut forwarding| {
                let erased_before = proof_output_calls
                    .iter()
                    .filter(|package| {
                        package.machine_symbol == forwarding.machine_symbol
                            && package.state_symbol == forwarding.state_symbol
                            && package.source_statement_index < forwarding.statement_index
                    })
                    .count();
                forwarding.statement_index =
                    forwarding.statement_index.saturating_sub(erased_before);
                forwarding
            })
            .collect();
        trees.proof_output_calls = proof_output_calls;
        trees.ranking_expression_custody = ranking_expression_custody;
        normalize_domain_constraints(self.source_trees, &mut trees)?;
        normalize_qualification_casts(self.source_trees, &mut trees)?;
        crate::expressions::fixed_byte_array_literals::land_exact_fixed_byte_array_literals(
            &mut trees,
        )?;
        crate::type_reference::validate_range_arguments(self.source_trees, &trees)?;
        Ok(trees)
    }
}

#[cfg(test)]
mod tests;
