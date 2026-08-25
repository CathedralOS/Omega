use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::domain_constraints::normalize_domain_constraints;
use crate::invariant::lower_invariant_definition;
use crate::machine::lower_machine;
use crate::operator::lower_operator_definition;
use crate::qualification_casts::normalize_qualification_casts;
use crate::trait_definition::lower_trait_definition;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_typed_trees::TypedTrees;

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    // Decision 11: user-written `==` against bare payload-bearing case names
    // must be rejected BEFORE membership lowering synthesizes its internal
    // tag-equality compares, which are deliberately the same typed shape.
    crate::equality::validate_equality_operands(symbol_resolved_trees)?;

    // Equatable conformance prerequisites error at the conformance item,
    // before any `==` site tries to expand against a malformed type.
    crate::equatable::validate_equatable_conformances(symbol_resolved_trees)?;

    // Exhaustiveness counting over case domains also needs the resolved
    // trees: membership is still a distinct node here, so case arms and
    // domain arms are recognizable before lowering erases them into tag
    // compares and classifier expansions.
    crate::exhaustiveness::validate_case_dispatch_exhaustiveness(symbol_resolved_trees)?;

    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
        equality_scope: None,
    };
    lowerer.typed_trees.service_reaches = symbol_resolved_trees.service_reaches.clone();
    lowerer.typed_trees.service_reach_rows = symbol_resolved_trees.service_reach_rows.clone();
    lowerer.typed_trees.semantic_domains = symbol_resolved_trees.semantic_domains.clone();
    lowerer.typed_trees.external_bindings = symbol_resolved_trees.external_bindings.clone();
    lowerer.typed_trees.retain_authored_declaration_selections(
        symbol_resolved_trees
            .authored_declaration_selections()
            .clone(),
    );
    lowerer.typed_trees.evidence_forwardings = symbol_resolved_trees
        .evidence_forwardings
        .iter()
        .map(
            |forwarding| psi_typed_trees::typed_trees::EvidenceForwarding {
                machine_symbol: forwarding.machine_symbol,
                state_symbol: forwarding.state_symbol,
                statement_index: forwarding.statement_index,
                source_statement_index: forwarding.statement_index,
                target: crate::name::lower_name(&forwarding.target),
                source: crate::name::lower_name(&forwarding.source),
                source_conformance: forwarding.source_conformance,
            },
        )
        .collect();

    for invariant_definition in &symbol_resolved_trees.invariant_definitions {
        let invariant_definition = lower_invariant_definition(&mut lowerer, invariant_definition)?;
        lowerer
            .typed_trees
            .push_invariant_definition(invariant_definition);
    }

    for data_definition in &symbol_resolved_trees.data_definitions {
        let data_definition = lower_data_definition(&mut lowerer, data_definition)?;
        lowerer.typed_trees.push_data_definition(data_definition);
    }

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lower_domain_definition(&mut lowerer, domain_definition)?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for proposition in &symbol_resolved_trees.propositions {
        let proposition =
            crate::proposition::lower_proposition_definition(&mut lowerer, proposition)?;
        lowerer.typed_trees.push_proposition(proposition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lower_machine(&mut lowerer, machine)?;
        lowerer.typed_trees.push_machine(machine);
    }

    for measure in &symbol_resolved_trees.measures {
        let measure = crate::measure::lower_measure_definition(&mut lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lower_operator_definition(&mut lowerer, operator)?;
        lowerer.typed_trees.push_operator(operator);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        let trait_definition = lower_trait_definition(&mut lowerer, trait_definition)?;
        lowerer.typed_trees.push_trait_definition(trait_definition);
    }

    for conformance in &symbol_resolved_trees.conformances {
        let source_type_parameters = conformance.type_parameters;
        let mut arguments = psi_arena::HandleSpan::empty();
        for argument in symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .span_or_empty(conformance.arguments)
        {
            let argument =
                crate::type_reference::lower_type_reference_into_table(&mut lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        let mut conformance = psi_typed_trees::trait_definition::Conformance {
            symbol: conformance.symbol,
            lifetime_parameters: conformance
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters: psi_arena::HandleSpan::empty(),
            subject: match &conformance.subject {
                psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                    type_name,
                ) => psi_typed_trees::trait_definition::ConformanceSubject::Carrier(
                    crate::name::lower_name(type_name),
                ),
                psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                    psi_typed_trees::trait_definition::ConformanceSubject::Subjectless
                }
            },
            carrier_symbol: conformance.carrier_symbol,
            trait_name: crate::name::lower_name(&conformance.trait_name),
            trait_symbol: conformance.trait_symbol,
            arguments,
            alias: conformance.alias.as_ref().map(crate::name::lower_name),
            implementation: match &conformance.implementation {
                psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                    psi_typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
                }
                psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } => {
                    psi_typed_trees::trait_definition::ConformanceImplementation::Closed {
                        rows: rows
                            .iter()
                            .map(|row| psi_typed_trees::trait_definition::ConformanceRow {
                                declaring_trait: row.declaring_trait,
                                declaring_trait_name: crate::name::lower_name(&row.declaring_trait_name),
                                requirement: row.requirement,
                                requirement_name: crate::name::lower_name(&row.requirement_name),
                                realization_machine: row.realization_machine,
                                realization_state: row.realization_state,
                                realization_name: crate::name::lower_name(&row.realization_name),
                                source: match row.source {
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => psi_typed_trees::trait_definition::ConformanceRowSource::Inline,
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => psi_typed_trees::trait_definition::ConformanceRowSource::Reference,
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => psi_typed_trees::trait_definition::ConformanceRowSource::TraitDefault,
                                },
                            })
                            .collect(),
                    }
                }
            },
        };
        for parameter in symbol_resolved_trees.data_type_parameters(source_type_parameters) {
            let parameter = crate::data::lower_type_parameter(&mut lowerer, parameter)?;
            lowerer
                .typed_trees
                .push_conformance_type_parameter(&mut conformance, parameter);
        }
        // Inline/default realization machines close over the conformance
        // name's telescope. Publish that telescope as the machine template's
        // own generic surface as well, so ordinary specialization can clone
        // and substitute the selected row instead of leaving family symbols
        // such as `Element` in executable checked code. Referenced external
        // machines keep their independently declared telescope.
        let realization_machines = match &conformance.implementation {
            psi_typed_trees::trait_definition::ConformanceImplementation::Closed { rows } => rows
                .iter()
                .filter(|row| {
                    matches!(
                        row.source,
                        psi_typed_trees::trait_definition::ConformanceRowSource::Inline
                            | psi_typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                    )
                })
                .map(|row| row.realization_machine)
                .collect::<Vec<_>>(),
            psi_typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
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
        let wire_schema = crate::wire::lower_wire_schema(&mut lowerer, wire_schema)?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }

    lowerer.finish()
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
    pub(crate) equality_scope: Option<crate::equatable::EqualityScope>,
}

impl Lowerer<'_> {
    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        crate::progress::normalize_progress_premises(&mut self.typed_trees)?;
        let TypedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            semantic_domains,
            external_bindings,
            plan_laid_layouts: _,
            placed_view_plans: _,
            wire_placements: _,
            wire_encode_obligations: _,
            wire_schema_plans: _,
            machine_specializations: _,
            boundary_calling_plans: _,
            open_index_normalizations: _,
            evidence_forwardings,
            proof_output_calls,
        } = self.typed_trees;

        let mut trees = TypedTrees::with_roots(roots, tables, symbols);
        // The copied semantic interners survive the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
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
        normalize_domain_constraints(self.source_trees, &mut trees)?;
        normalize_qualification_casts(&mut trees)?;
        Ok(trees)
    }
}

#[cfg(test)]
mod tests;
