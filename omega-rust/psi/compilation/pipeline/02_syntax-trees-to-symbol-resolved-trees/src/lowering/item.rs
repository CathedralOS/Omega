//! Root item dispatch.
//!
//! `lower_item` records the item's authored-expression exposure, routes each
//! syntax item kind to the module owning its carrier area, and pushes the
//! result into the lowerer's trees. Every product target member and its
//! independent portable interface lower together; requested realizations never
//! decide which source bodies are retained or checked.

use crate::lowering::data::lower_data_definition;
use crate::lowering::domain::lower_domain_definition;
use crate::lowering::machine::lower_machine_into;
use crate::lowering::measure::lower_measure_definition;
use crate::lowering::operator::lower_operator_definition;
use crate::lowering::proposition::lower_proposition_definition;
use crate::lowering::trait_definition::lower_trait_definition;
use crate::lowering::type_reference::lower_child_type_references;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_item(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    declaration: syntax::item::ItemHandle,
) -> Result<(), Diagnostic> {
    let inputs = crate::resolution::type_inputs::TypeInputScope::new(lowerer);
    let item = syntax_trees.root_item(declaration);
    let conformance_ordinal = lowerer.symbol_resolved_trees.conformances.len();
    let data_ordinal = lowerer.symbol_resolved_trees.data_definitions.len();
    lowerer.with_authored_expression_exposure(item_expression_exposure(item), |lowerer| {
        lower_item_with_exposure(lowerer, syntax_trees, item)
    })?;
    if matches!(item, syntax::item::Item::Conformance(_)) {
        lowerer
            .conformance_inputs
            .push((declaration, conformance_ordinal));
    }
    if matches!(item, syntax::item::Item::Data(_))
        && let Some(sources) = &mut lowerer.equation_sources
    {
        sources.data_declarations.push((declaration, data_ordinal));
    }
    inputs.finish(lowerer, symbols::SymbolHandle::invalid());
    Ok(())
}

fn lower_item_with_exposure(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    item: &syntax::item::Item,
) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::Data(data_definition) => {
            let lowered = lower_data_definition(lowerer, syntax_trees, data_definition)?;
            crate::lowering::wire::derive_wire_schema(lowerer, &lowered);
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
        // A bare bodyless signature, token-bearing or not, is admitted only
        // as an exact compiler-catalog primitive declared by the sealed
        // toolchain source; it lowers to the catalog's declaration, and every
        // other such signature rejects with the body/boundary/external-leaf
        // guidance.
        syntax::item::Item::Machine(machine)
            if machine.target.is_none()
                && !machine.target_family_interface
                && crate::lowering::operator::is_bare_bodyless_signature(machine) =>
        {
            let operator = crate::lowering::operator::lower_bare_bodyless_signature(
                lowerer,
                syntax_trees,
                machine,
            )?;
            lowerer.symbol_resolved_trees.operators.push(operator);
        }
        syntax::item::Item::Machine(machine) => {
            // Product members keep their exact authored target and separate
            // symbols. The bodyless generated head owns the portable path;
            // Build helpers instead retain their admitted execution selection.
            lower_machine_into(lowerer, syntax_trees, machine)?;
        }
        syntax::item::Item::Trait(trait_definition) => {
            let trait_definition = lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
            lowerer.symbol_resolved_trees.traits.push(trait_definition);
        }
        syntax::item::Item::Conformance(conformance) => {
            let arguments =
                lower_child_type_references(lowerer, syntax_trees, conformance.trait_arguments)?;
            let type_parameters = crate::lowering::data::lower_type_parameters(
                lowerer,
                syntax_trees,
                conformance.type_parameters,
            )?;
            let conformance_name = conformance
                .alias
                .as_ref()
                .unwrap_or(&conformance.trait_name)
                .as_str(syntax_trees);
            let mut rows = Vec::new();
            for member in syntax_trees.items.conformance_members(conformance.members) {
                match member {
                    syntax::item::ConformanceMember::Machine(machine) => {
                        rows.push(lower_closed_machine_row(
                            lowerer,
                            syntax_trees,
                            conformance_name,
                            None,
                            None,
                            machine,
                            symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline,
                        )?);
                    }
                    syntax::item::ConformanceMember::Synthesized(machine) => {
                        rows.push(lower_closed_machine_row(
                            lowerer,
                            syntax_trees,
                            conformance_name,
                            None,
                            None,
                            machine,
                            symbol_resolved_trees::trait_definition::ConformanceRowSource::Synthesized,
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
                            conformance_name,
                            Some(declaring_trait),
                            Some(*requirement_ordinal),
                            machine,
                            symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault,
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
                            .joined_path_in(syntax_trees);
                        let target_members = syntax_trees.items.identifier_path_members(*target);
                        let authored_realization_source_span = target_members
                            .first()
                            .zip(target_members.last())
                            .and_then(|(first, last)| {
                                (first.source_span().source_id == last.source_span().source_id)
                                    .then(|| {
                                        source::SourceSpan::new(
                                            first.source_span().source_id,
                                            source::Span::new(
                                                first.source_span().span.start,
                                                last.source_span().span.end,
                                            ),
                                        )
                                    })
                            });
                        rows.push(
                            symbol_resolved_trees::trait_definition::ConformanceRow {
                                declaring_trait: symbols::SymbolHandle::invalid(),
                                declaring_trait_name: crate::lowering::name::lower_name(syntax_trees,
                                    declaring_trait,
                                ),
                                requirement: symbols::SymbolHandle::invalid(),
                                requirement_name: crate::lowering::name::lower_name(syntax_trees, requirement),
                                provisional_requirement_ordinal: None,
                                realization_machine: symbols::SymbolHandle::invalid(),
                                realization_state: symbols::SymbolHandle::invalid(),
                                realization_name:
                                    symbol_resolved_trees::name::DiagnosticName::generated(
                                        realization_name,
                                    ),
                                authored_realization_source_span,
                                provisional_realization_ordinal: None,
                                source: symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference,
                            },
                        );
                    }
                }
            }
            lowerer.symbol_resolved_trees.conformances.push(
                symbol_resolved_trees::trait_definition::Conformance {
                    symbol: symbols::SymbolHandle::invalid(),
                    is_public: conformance.is_public,
                    lifetime_parameters: conformance
                        .lifetime_parameters
                        .iter()
                        .map(|name| crate::lowering::name::lower_name(syntax_trees, name))
                        .collect(),
                    type_parameters,
                    trait_name: crate::lowering::name::lower_name(
                        syntax_trees,
                        &conformance.trait_name,
                    ),
                    trait_symbol: symbols::SymbolHandle::invalid(),
                    trait_lifetime_arguments: conformance
                        .trait_lifetime_arguments
                        .iter()
                        .map(|name| crate::lowering::name::lower_name(syntax_trees, name))
                        .collect(),
                    arguments,
                    alias: conformance
                        .alias
                        .as_ref()
                        .map(|name| crate::lowering::name::lower_name(syntax_trees, name)),
                    rows,
                },
            );
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
        // Const values exist only until symbol resolution: every use
        // substitutes the initializer. A detached resolved root remains for
        // later resolution continuations, not as runtime constant storage.
        syntax::item::Item::Const(definition) => {
            let pending = lowerer.const_resolution_mode
                != crate::resolution::lowerer::ConstResolutionMode::Complete
                && crate::constant::requires_const_initializer_evaluation(syntax_trees, definition)
                && !crate::constant::pending_const_initializer_leaves(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref().ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "initializer preparation lost its declaration selector",
                        )
                        .with_source_span(definition.name.source_span())
                    })?,
                )
                .map_err(|reason| {
                    diagnostics::Diagnostic::error(reason)
                        .with_source_span(definition.name.source_span())
                })?
                .is_empty();
            if lowerer.const_resolution_mode
                == crate::resolution::lowerer::ConstResolutionMode::Complete
            {
                // The private dependency frontier selects identities and types,
                // not value predicates whose declared geometry may be pending.
                // Complete resolution validates every declaration, used or not.
                crate::constant::validate_const_definition(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref(),
                )?;
            }
            let initializer =
                crate::constant::retain_const_initializer(lowerer, syntax_trees, definition)?;
            let evaluated_value = if pending || definition.pending_target_observation {
                // Preparation retains a real unevaluated expression, never a
                // placeholder value or manufactured public declaration identity.
                None
            } else if definition.is_public
                && lowerer.const_resolution_mode
                    == crate::resolution::lowerer::ConstResolutionMode::Complete
            {
                Some(
                    crate::constant::evaluated_declaration_value(
                        syntax_trees,
                        definition,
                        lowerer.constant_selection.as_ref(),
                    )
                    .map_err(|reason| {
                        diagnostics::Diagnostic::error(format!(
                            "public const `{}` has no canonical declaration identity: {reason}",
                            crate::constant::semantic_const_name(syntax_trees, definition),
                        ))
                        .with_source_span(definition.name.source_span())
                    })?,
                )
            } else {
                // Preparation discovers dependencies before their declared
                // geometry is complete. A public literal may therefore lack an
                // encoding here, just like a private literal. Complete resolution
                // still requires its canonical public identity after evaluation.
                crate::constant::evaluated_declaration_value(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref(),
                )
                .ok()
            };
            let declared_type = crate::lowering::type_reference::lower_type_reference_handle(
                lowerer,
                syntax_trees,
                definition.type_reference,
            )?;
            let ordinal = lowerer.symbol_resolved_trees.roots.const_declarations.len();
            let declaration = lowerer.symbol_resolved_trees.roots.const_declarations.push(
                symbol_resolved_trees::constant::ConstDeclaration {
                    symbol: symbols::SymbolHandle::invalid(),
                    is_public: definition.is_public,
                    declared_type,
                    initializer,
                    authored_initializer: lowerer
                        .pending_const_initializers
                        .last()
                        .filter(|_| definition.normalization.is_some())
                        // Literal roots remain authored evidence too. Otherwise
                        // erasing an operator-free computed root could masquerade
                        // as a declaration which never needed evaluation.
                        .map_or(initializer, |record| record.authored),
                    initializer_source_span: syntax_trees.expressions.source_span(definition.value),
                    pending_target_observation: definition.pending_target_observation,
                    evaluated_value,
                },
            );
            if let Some(sources) = &mut lowerer.equation_sources {
                sources.declared_types.push((
                    crate::preparation::machine_equations::DeclaredTypeOrigin::Constant(ordinal),
                    definition.type_reference,
                ));
            }
            lowerer.pending_const_declarations.push(
                crate::resolution::lowerer::PendingConstDeclaration {
                    declaration,
                    scope: definition.scope.clone(),
                    semantic_name: crate::constant::semantic_const_name(syntax_trees, definition),
                    source_span: definition.name.source_span(),
                    is_public: definition.is_public,
                },
            );
        }
        syntax::item::Item::Module(module) => {
            lowerer
                .namespace_declarations
                .record_module(syntax_trees, module.path);
        }
        syntax::item::Item::Use(import) => {
            lowerer
                .namespace_declarations
                .record_import(syntax_trees, import.path);
        }
        syntax::item::Item::Capability(_) | syntax::item::Item::Package(_) => {}
    }

    Ok(())
}

fn item_expression_exposure(
    item: &syntax::item::Item,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    match item {
        syntax::item::Item::Data(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::Conformance(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Domain(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::Proposition(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Machine(machine) => {
            crate::lowering::machine::machine_interface_exposure(machine)
        }
        syntax::item::Item::Operator(operator) if operator.is_public => Exposure::PublicInterface,
        syntax::item::Item::Trait(definition) if definition.is_public => Exposure::PublicInterface,
        _ => Exposure::PrivateImplementation,
    }
}

fn lower_closed_machine_row(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    conformance_name: &str,
    declaring_trait: Option<&syntax::identifier::Identifier>,
    requirement_ordinal: Option<usize>,
    machine: &syntax::item::Machine,
    source: symbol_resolved_trees::trait_definition::ConformanceRowSource,
) -> Result<symbol_resolved_trees::trait_definition::ConformanceRow, Diagnostic> {
    let requirement_name = machine.name.clone();
    let realization_name = declaring_trait.map_or_else(
        || {
            format!(
                "{conformance_name}::{}",
                requirement_name.as_str(syntax_trees)
            )
        },
        |declaring_trait| {
            format!(
                "{conformance_name}::{}::{}",
                declaring_trait.as_str(syntax_trees),
                requirement_name.as_str(syntax_trees),
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
    // A member belongs to the implementation's map, not to any data
    // declaration; its parameters name every data type explicitly.
    realization.attached_data = None;
    lower_machine_into(lowerer, syntax_trees, &realization)?;
    Ok(symbol_resolved_trees::trait_definition::ConformanceRow {
        declaring_trait: symbols::SymbolHandle::invalid(),
        declaring_trait_name: declaring_trait
            .map(|name| crate::lowering::name::lower_name(syntax_trees, name))
            .unwrap_or_default(),
        requirement: symbols::SymbolHandle::invalid(),
        requirement_name: crate::lowering::name::lower_name(syntax_trees, &requirement_name),
        provisional_requirement_ordinal: requirement_ordinal,
        realization_machine: symbols::SymbolHandle::invalid(),
        realization_state: symbols::SymbolHandle::invalid(),
        realization_name: symbol_resolved_trees::name::DiagnosticName::generated(realization_name),
        authored_realization_source_span: None,
        provisional_realization_ordinal: Some(realization_ordinal),
        source,
    })
}
