use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::lowerer::Lowerer;
use crate::machine::lower_machine_into;
use crate::measure::lower_measure_definition;
use crate::operator::lower_operator_definition;
use crate::proposition::lower_proposition_definition;
use crate::trait_definition::lower_trait_definition;
use crate::type_reference::lower_child_type_references;
use crate::wire::lower_wire_schema;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_item(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    item: &syntax::item::Item,
) -> Result<(), Diagnostic> {
    lowerer.with_authored_expression_exposure(item_expression_exposure(item), |lowerer| {
        lower_item_with_exposure(lowerer, syntax_trees, item)
    })
}

fn lower_item_with_exposure(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    item: &syntax::item::Item,
) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::Data(data_definition) => {
            let lowered = lower_data_definition(lowerer, syntax_trees, data_definition)?;
            lowerer.symbol_resolved_trees.data_definitions.push(lowered);
        }
        syntax::item::Item::Domain(domain_definition) => {
            let domain_definition =
                lower_domain_definition(lowerer, syntax_trees, domain_definition)?;
            lowerer
                .symbol_resolved_trees
                .domain_definitions
                .push(domain_definition);
        }
        syntax::item::Item::Machine(machine) => {
            // A machine still carrying a target marker here was NOT selected:
            // the pre-resolution filter (pipeline/target_machines.rs) clears
            // the selected target's marker and validates the loud edges, so a
            // marked machine is inert. (Without the filter, EVERY target machine stays inert and
            // its call sites fail resolution loudly -- never a silent success.)
            if machine.target.is_none() {
                lower_machine_into(lowerer, syntax_trees, machine)?;
            }
        }
        syntax::item::Item::Trait(trait_definition) => {
            let trait_definition = lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
            lowerer.symbol_resolved_trees.traits.push(trait_definition);
        }
        syntax::item::Item::Conformance(conformance) => {
            let arguments =
                lower_child_type_references(lowerer, syntax_trees, conformance.trait_arguments)?;
            let type_parameters = crate::data::lower_type_parameters(
                lowerer,
                syntax_trees,
                conformance.type_parameters,
            )?;
            let implementation = match &conformance.body {
                syntax::item::ConformanceBody::AttachedRequirementMachines => {
                    psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
                }
                syntax::item::ConformanceBody::Closed { members } => {
                    let conformance_name = conformance
                        .alias
                        .as_ref()
                        .unwrap_or(&conformance.trait_name)
                        .as_str();
                    let mut rows = Vec::new();
                    for member in syntax_trees.items.conformance_members(*members) {
                        match member {
                            syntax::item::ConformanceMember::Machine(machine) => {
                                rows.push(lower_closed_machine_row(
                                    lowerer,
                                    syntax_trees,
                                    conformance,
                                    conformance_name,
                                    None,
                                    None,
                                    machine,
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline,
                                )?);
                            }
                            syntax::item::ConformanceMember::TraitDefault {
                                declaring_trait,
                                requirement_ordinal,
                                machine,
                            } => {
                                rows.push(lower_closed_machine_row(
                                    lowerer,
                                    syntax_trees,
                                    conformance,
                                    conformance_name,
                                    Some(declaring_trait),
                                    Some(*requirement_ordinal),
                                    machine,
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault,
                                )?);
                            }
                            syntax::item::ConformanceMember::Reference {
                                declaring_trait,
                                requirement,
                                target,
                            } => {
                                let realization_name = syntax_trees
                                    .items
                                    .identifier_path_members(*target)
                                    .iter()
                                    .map(|member| member.as_str())
                                    .collect::<Vec<_>>()
                                    .join("::");
                                let target_members = syntax_trees
                                    .items
                                    .identifier_path_members(*target);
                                let authored_realization_source_span = target_members
                                    .first()
                                    .zip(target_members.last())
                                    .and_then(|(first, last)| {
                                        (first.source_span().source_id
                                            == last.source_span().source_id)
                                            .then(|| {
                                                psi_source::SourceSpan::new(
                                                    first.source_span().source_id,
                                                    psi_source::Span::new(
                                                        first.source_span().span.start,
                                                        last.source_span().span.end,
                                                    ),
                                                )
                                            })
                                    });
                                rows.push(
                                    psi_symbol_resolved_trees::trait_definition::ConformanceRow {
                                        declaring_trait: psi_symbols::SymbolHandle::invalid(),
                                        declaring_trait_name: crate::name::lower_name(
                                            declaring_trait,
                                        ),
                                        requirement: psi_symbols::SymbolHandle::invalid(),
                                        requirement_name: crate::name::lower_name(requirement),
                                        provisional_requirement_ordinal: None,
                                        realization_machine: psi_symbols::SymbolHandle::invalid(),
                                        realization_state: psi_symbols::SymbolHandle::invalid(),
                                        realization_name:
                                            psi_symbol_resolved_trees::name::DiagnosticName::generated(
                                                realization_name,
                                            ),
                                        authored_realization_source_span,
                                        provisional_realization_ordinal: None,
                                        source: psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference,
                                    },
                                );
                            }
                        }
                    }
                    psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed {
                        rows,
                    }
                }
            };
            lowerer
                .symbol_resolved_trees
                .conformances
                .push(psi_symbol_resolved_trees::trait_definition::Conformance {
                symbol: psi_symbols::SymbolHandle::invalid(),
                is_public: conformance.is_public,
                lifetime_parameters: conformance
                    .lifetime_parameters
                    .iter()
                    .map(crate::name::lower_name)
                    .collect(),
                type_parameters,
                subject: match &conformance.subject {
                    syntax::item::ConformanceSubject::Carrier(type_name) => {
                        psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                            crate::name::lower_name(type_name),
                        )
                    }
                    syntax::item::ConformanceSubject::Subjectless => {
                        psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
                    }
                },
                carrier_symbol: psi_symbols::SymbolHandle::invalid(),
                trait_name: crate::name::lower_name(&conformance.trait_name),
                trait_symbol: psi_symbols::SymbolHandle::invalid(),
                trait_lifetime_arguments: conformance
                    .trait_lifetime_arguments
                    .iter()
                    .map(crate::name::lower_name)
                    .collect(),
                arguments,
                alias: conformance.alias.as_ref().map(crate::name::lower_name),
                implementation,
            });
        }
        syntax::item::Item::Measure(measure) => {
            let measure = lower_measure_definition(lowerer, syntax_trees, measure)?;
            lowerer.symbol_resolved_trees.measures.push(measure);
        }
        syntax::item::Item::Operator(operator) => {
            let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
            lowerer.symbol_resolved_trees.operators.push(operator);
        }
        syntax::item::Item::Proposition(proposition) => {
            let proposition = lower_proposition_definition(lowerer, syntax_trees, proposition)?;
            lowerer.symbol_resolved_trees.propositions.push(proposition);
        }
        syntax::item::Item::WireData(wire_data) => {
            let wire_schema = lower_wire_schema(lowerer, syntax_trees, wire_data)?;
            // Chapter 20: numbers are INERT schema facts -- a numbered data
            // is ALSO a plain program type (see
            // data_definition_from_wire_schema; the Message/Sample twin
            // corpus pattern was forced by this line's absence).
            let data_definition =
                crate::wire::data_definition_from_wire_schema(lowerer, &wire_schema);
            lowerer.symbol_resolved_trees.wire_schemas.push(wire_schema);
            lowerer
                .symbol_resolved_trees
                .data_definitions
                .push(data_definition);
        }
        // Const values exist only until symbol resolution: every use
        // substitutes the initializer. Retain only a provenance symbol for
        // authored-selection and package-authority custody.
        syntax::item::Item::Const(definition) => {
            crate::constant::validate_const_definition(lowerer, syntax_trees, definition)?;
            let canonical_value_encoding = if definition.is_public {
                Some(
                    psi_generic_instances::canonicalize_declared_const_definition(
                        syntax_trees,
                        definition,
                    )
                    .map_err(|reason| {
                        psi_diagnostics::Diagnostic::error(format!(
                            "public const `{}` has no canonical declaration identity: {reason}",
                            crate::constant::semantic_const_name(definition),
                        ))
                        .with_source_span(definition.name.source_span())
                    })?
                    .encoding,
                )
            } else {
                None
            };
            let declared_type = crate::type_reference::lower_type_reference_handle(
                lowerer,
                syntax_trees,
                definition.type_reference,
            )?;
            lowerer.symbol_resolved_trees.roots.const_declarations.push(
                psi_symbol_resolved_trees::constant::ConstDeclaration {
                    symbol: psi_symbols::SymbolHandle::invalid(),
                    is_public: definition.is_public,
                    declared_type,
                    initializer_source_span: syntax_trees.expressions.source_span(definition.value),
                    canonical_value_encoding,
                },
            );
            lowerer
                .pending_const_declarations
                .push(crate::lowerer::PendingConstDeclaration {
                    semantic_name: crate::constant::semantic_const_name(definition),
                    source_span: definition.name.source_span(),
                    is_public: definition.is_public,
                });
        }
        syntax::item::Item::Capability(_)
        | syntax::item::Item::Module(_)
        | syntax::item::Item::Package(_)
        | syntax::item::Item::Target(_)
        | syntax::item::Item::Use(_) => {}
    }

    Ok(())
}

fn item_expression_exposure(
    item: &syntax::item::Item,
) -> psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    match item {
        syntax::item::Item::Data(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::Conformance(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Domain(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::Proposition(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        // A boundary machine is an exported callable surface even when it is
        // not separately spelled `pub`. Its signature and contracts are
        // package interface; lower_machine_into overrides executable state
        // bodies back to private implementation exposure.
        syntax::item::Item::Machine(machine) if machine.is_public || machine.boundary => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Operator(operator) if operator.is_public => Exposure::PublicInterface,
        syntax::item::Item::Trait(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::WireData(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        _ => Exposure::PrivateImplementation,
    }
}

fn lower_closed_machine_row(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    conformance: &syntax::item::ConformanceItem,
    conformance_name: &str,
    declaring_trait: Option<&syntax::identifier::Identifier>,
    requirement_ordinal: Option<usize>,
    machine: &syntax::item::Machine,
    source: psi_symbol_resolved_trees::trait_definition::ConformanceRowSource,
) -> Result<psi_symbol_resolved_trees::trait_definition::ConformanceRow, Diagnostic> {
    let requirement_name = machine.name.clone();
    let namespace = match &conformance.subject {
        syntax::item::ConformanceSubject::Carrier(type_name) => {
            format!("{}::{conformance_name}", type_name.as_str())
        }
        syntax::item::ConformanceSubject::Subjectless => conformance_name.to_owned(),
    };
    let realization_name = declaring_trait.map_or_else(
        || format!("{}::{}", namespace, requirement_name.as_str(),),
        |declaring_trait| {
            format!(
                "{}::{}::{}",
                namespace,
                declaring_trait.as_str(),
                requirement_name.as_str(),
            )
        },
    );
    let realization_ordinal = lowerer.symbol_resolved_trees.machines.len();
    let mut realization = machine.clone();
    // The realization's semantic path is compiler-normalized, but the machine
    // is still authored by this exact conformance member. Retain that source
    // provenance so package ownership follows the declaration instead of
    // degrading to an unresolved generated root.
    realization.name =
        syntax::identifier::Identifier::new(realization_name.clone(), machine.name.source_span());
    realization.attached_data = match &conformance.subject {
        syntax::item::ConformanceSubject::Carrier(type_name) => Some(type_name.clone()),
        syntax::item::ConformanceSubject::Subjectless => None,
    };
    lower_machine_into(lowerer, syntax_trees, &realization)?;
    Ok(
        psi_symbol_resolved_trees::trait_definition::ConformanceRow {
            declaring_trait: psi_symbols::SymbolHandle::invalid(),
            declaring_trait_name: declaring_trait
                .map(crate::name::lower_name)
                .unwrap_or_default(),
            requirement: psi_symbols::SymbolHandle::invalid(),
            requirement_name: crate::name::lower_name(&requirement_name),
            provisional_requirement_ordinal: requirement_ordinal,
            realization_machine: psi_symbols::SymbolHandle::invalid(),
            realization_state: psi_symbols::SymbolHandle::invalid(),
            realization_name: psi_symbol_resolved_trees::name::DiagnosticName::generated(
                realization_name,
            ),
            authored_realization_source_span: None,
            provisional_realization_ordinal: Some(realization_ordinal),
            source,
        },
    )
}
