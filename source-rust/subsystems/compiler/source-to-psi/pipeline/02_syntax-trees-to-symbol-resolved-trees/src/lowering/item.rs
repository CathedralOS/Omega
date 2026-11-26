/*
    Root item dispatch.

    `lower_item` records the item's authored-expression exposure, routes each
    syntax item kind to the module owning its carrier area, and pushes the
    result into the lowerer's trees. Every product target member and its
    independent portable interface lower together; requested realizations never
    decide which source bodies are retained or checked.
*/

use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
use crate::resolution::lowering::data::lower_data_definition;
use crate::resolution::lowering::data::lower_type_parameters;
use crate::resolution::lowering::domain::lower_domain_definition;
use crate::resolution::lowering::lowerer::ConstResolutionMode;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::machine::lower_machine_into;
use crate::resolution::lowering::measure::lower_measure_definition;
use crate::resolution::lowering::name::lower_name;
use crate::resolution::lowering::name::lower_names;
use crate::resolution::lowering::operator::lower_operator_definition;
use crate::resolution::lowering::proposition::lower_proposition_definition;
use crate::resolution::lowering::trait_definition::lower_trait_definition;
use crate::resolution::lowering::type_reference::lower_child_type_references;
use crate::resolution::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::tables::syntax_items;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::trait_definition::ConformanceRowSource;
use diagnostics::Diagnostic;
use syntax::item::Item;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_item(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    declaration: syntax::item::ItemHandle,
) -> Result<(), Diagnostic> {
    let inputs = crate::resolution::lowering::type_inputs::TypeInputScope::new(lowerer);
    let item = syntax_trees.root_item(declaration);
    let conformance_ordinal = lowerer.symbol_resolved_trees.conformances.len();
    let trait_ordinal = lowerer.symbol_resolved_trees.traits.len();
    let data_ordinal = lowerer.symbol_resolved_trees.data_definitions.len();
    use crate::resolution::names::runtime_callable_scopes::CallableScopeOwner;
    let owner = match item {
        Item::Data(_) => CallableScopeOwner::Data(data_ordinal),
        Item::TypeAlias(_) => {
            CallableScopeOwner::Alias(lowerer.symbol_resolved_trees.type_aliases.len())
        }
        Item::Domain(_) => {
            CallableScopeOwner::Domain(lowerer.symbol_resolved_trees.domain_definitions.len())
        }
        Item::Trait(_) => CallableScopeOwner::Trait(trait_ordinal),
        Item::Conformance(_) => CallableScopeOwner::Conformance(conformance_ordinal),
        Item::Operator(_) => {
            CallableScopeOwner::Operator(lowerer.symbol_resolved_trees.operators.len())
        }
        Item::Machine(machine)
            if machine.target.is_none()
                && !machine.target_family_interface
                && crate::resolution::lowering::machine_supply::is_bare_bodyless_signature(
                    machine,
                ) =>
        {
            CallableScopeOwner::Operator(lowerer.symbol_resolved_trees.operators.len())
        }
        Item::Proposition(_) => {
            CallableScopeOwner::Proposition(lowerer.symbol_resolved_trees.propositions.len())
        }
        Item::Measure(_) => {
            CallableScopeOwner::Measure(lowerer.symbol_resolved_trees.measures.len())
        }
        _ => CallableScopeOwner::Root,
    };
    lowerer.with_runtime_callable_scope(owner, |lowerer| {
        lowerer.with_authored_expression_exposure(item_expression_exposure(item), |lowerer| {
            lower_item_with_exposure(lowerer, syntax_trees, item)
        })
    })?;
    let input = match item {
        Item::Conformance(_) => Some(
            crate::resolution::lowering::operand_owners::DeclarationInput::Conformance(
                conformance_ordinal,
            ),
        ),
        Item::Trait(_) => Some(
            crate::resolution::lowering::operand_owners::DeclarationInput::Trait(trait_ordinal),
        ),
        _ => None,
    };
    if let Some(input) = input {
        lowerer.declaration_inputs.push((declaration, input));
    }
    if matches!(item, Item::Data(_))
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
    item: &Item,
) -> Result<(), Diagnostic> {
    match item {
        syntax::item::Item::TypeAlias(alias) => {
            let alias = crate::resolution::lowering::type_aliases::lower_type_alias_definition(
                lowerer,
                syntax_trees,
                alias,
            )?;
            lowerer.symbol_resolved_trees.type_aliases.push(alias);
        }
        syntax::item::Item::Data(data_definition) => {
            let lowered = lower_data_definition(lowerer, syntax_trees, data_definition)?;
            crate::resolution::lowering::wire::derive_wire_schema(lowerer, &lowered);
            lowerer.symbol_resolved_trees.data_definitions.push(lowered);
        }
        Item::Domain(domain_definition) => {
            let domain_definition =
                lower_domain_definition(lowerer, syntax_trees, domain_definition)?;
            lowerer
                .symbol_resolved_trees
                .domain_definitions
                .push(domain_definition);
        }
        Item::Machine(machine) => {
            lower_machine_into(lowerer, syntax_trees, machine)?;
        }
        Item::Trait(trait_definition) => {
            let trait_definition = lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
            lowerer.symbol_resolved_trees.traits.push(trait_definition);
        }
        Item::Conformance(conformance) => {
            let arguments =
                lower_child_type_references(lowerer, syntax_trees, conformance.trait_arguments)?;
            let type_parameters =
                lower_type_parameters(lowerer, syntax_trees, conformance.type_parameters)?;
            let conformance_name = conformance
                .alias
                .as_ref()
                .unwrap_or(&conformance.trait_name)
                .as_str(syntax_trees);
            let mut rows = Vec::new();
            for member in syntax_trees.items.conformance_members(conformance.members) {
                match member {
                    syntax::item::ConformanceMember::Machine(machine) => {
                        let edge_trait = syntax_items(syntax_trees)
                            .satisfies_clauses(machine.satisfies)
                            .iter()
                            .find(|clause| {
                                clause.requirement.as_ref().is_some_and(|name| {
                                    name.as_str(syntax_trees) == machine.name.as_str(syntax_trees)
                                })
                            })
                            .map(|clause| clause.trait_name.clone());
                        rows.push(lower_closed_machine_row(
                            lowerer,
                            syntax_trees,
                            conformance_name,
                            edge_trait.as_ref(),
                            None,
                            machine,
                            ConformanceRowSource::Inline,
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
                            ConformanceRowSource::Synthesized,
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
                            ConformanceRowSource::TraitDefault,
                        )?);
                    }
                    /*
                        A selection row declares no machine. The slot keeps
                        the authored path and its source span; closed
                        conformance normalization resolves that path to one
                        exact existing machine from where it was written and
                        records it as the realization, so every later stage
                        calls and checks the selected declaration itself.
                    */
                    syntax::item::ConformanceMember::Selection {
                        declaring_trait,
                        requirement,
                        target,
                    } => {
                        let target_members =
                            syntax_items(syntax_trees).identifier_path_members(*target);
                        let authored_realization_source_span = target_members
                            .first()
                            .zip(target_members.last())
                            .and_then(|(first, last)| {
                                (first.source_span().source_id == last.source_span().source_id)
                                    .then(|| {
                                        source_text::SourceSpan::new(
                                            first.source_span().source_id,
                                            source_text::Span::new(
                                                first.source_span().span.start,
                                                last.source_span().span.end,
                                            ),
                                        )
                                    })
                            });
                        rows.push(crate::source_resolved::trait_definition::ConformanceRow {
                            declaring_trait: symbols::SymbolHandle::invalid(),
                            declaring_trait_name: lower_name(syntax_trees, declaring_trait),
                            requirement: symbols::SymbolHandle::invalid(),
                            requirement_name: lower_name(syntax_trees, requirement),
                            provisional_requirement_ordinal: None,
                            realization_machine: symbols::SymbolHandle::invalid(),
                            realization_state: symbols::SymbolHandle::invalid(),
                            realization_name: DiagnosticName::generated(
                                target_members.joined_path_in(syntax_trees),
                            ),
                            authored_realization_source_span,
                            provisional_realization_ordinal: None,
                            source: ConformanceRowSource::Reference,
                        });
                    }
                }
            }
            lowerer.symbol_resolved_trees.conformances.push(
                crate::source_resolved::trait_definition::Conformance {
                    symbol: symbols::SymbolHandle::invalid(),
                    is_public: conformance.is_public,
                    lifetime_parameters: lower_names(
                        syntax_trees,
                        &conformance.lifetime_parameters,
                    ),
                    type_parameters,
                    trait_name: lower_name(syntax_trees, &conformance.trait_name),
                    trait_symbol: symbols::SymbolHandle::invalid(),
                    trait_lifetime_arguments: lower_names(
                        syntax_trees,
                        &conformance.trait_lifetime_arguments,
                    ),
                    arguments,
                    alias: conformance
                        .alias
                        .as_ref()
                        .map(|name| lower_name(syntax_trees, name)),
                    rows,
                },
            );
        }
        Item::Measure(measure) => {
            let measure = lower_measure_definition(lowerer, syntax_trees, measure)?;
            lowerer.symbol_resolved_trees.measures.push(measure);
        }
        Item::Operator(operator) => {
            let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
            lowerer.symbol_resolved_trees.operators.push(operator);
        }
        Item::Proposition(proposition) => {
            let proposition = lower_proposition_definition(lowerer, syntax_trees, proposition)?;
            lowerer.symbol_resolved_trees.propositions.push(proposition);
        }
        Item::Const(definition) => {
            let needs_destination =
                crate::resolution::declarations::generic_data::const_destination_requires_proof(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref(),
                );
            let pending = lowerer.const_resolution_mode != ConstResolutionMode::Complete
                && crate::resolution::constants::requires_const_initializer_evaluation(
                    syntax_trees,
                    definition,
                )
                && !crate::resolution::constants::pending_const_initializer_leaves(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref().ok_or_else(|| {
                        Diagnostic::error("initializer preparation lost its declaration selector")
                            .with_source_span(definition.name.source_span())
                    })?,
                )
                .map_err(|reason| {
                    Diagnostic::error(reason).with_source_span(definition.name.source_span())
                })?
                .is_empty();
            if lowerer.const_resolution_mode == ConstResolutionMode::Complete {
                crate::resolution::constants::validate_const_definition(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref(),
                )?;
            }
            let initializer = crate::resolution::constants::retain_const_initializer(
                lowerer,
                syntax_trees,
                definition,
            )?;
            let evaluated_value = if pending
                || definition.preparation().pending_target_observation
                || needs_destination
            {
                None
            } else if definition.is_public
                && lowerer.const_resolution_mode == ConstResolutionMode::Complete
                && !crate::resolution::constants::requires_const_initializer_evaluation(
                    syntax_trees,
                    definition,
                )
            {
                Some(
                    crate::resolution::constants::evaluated_declaration_value(
                        syntax_trees,
                        definition,
                        lowerer.constant_selection.as_ref(),
                    )
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "public const `{}` has no canonical declaration identity: {reason}",
                            crate::resolution::constants::semantic_const_name(
                                syntax_trees,
                                definition
                            ),
                        ))
                        .with_source_span(definition.name.source_span())
                    })?,
                )
            } else {
                crate::resolution::constants::evaluated_declaration_value(
                    syntax_trees,
                    definition,
                    lowerer.constant_selection.as_ref(),
                )
                .ok()
            };
            let declared_type =
                lower_type_reference_handle(lowerer, syntax_trees, definition.type_reference)?;
            let ordinal = lowerer.symbol_resolved_trees.roots.const_declarations.len();
            let declaration = lowerer.symbol_resolved_trees.roots.const_declarations.push(
                crate::source_resolved::constant::ConstDeclaration {
                    symbol: symbols::SymbolHandle::invalid(),
                    is_public: definition.is_public,
                    declared_type,
                    initializer,
                    authored_initializer: lowerer
                        .pending_const_initializers
                        .last()
                        .filter(|_| definition.preparation().normalization.is_some())
                        .map_or(initializer, |record| record.authored),
                    initializer_source_span: syntax_trees
                        .expressions
                        .source_span(definition.value()),
                    pending_target_observation: definition.preparation().pending_target_observation,
                    evaluated_value,
                },
            );
            if let Some(sources) = &mut lowerer.equation_sources {
                sources.declared_types.push((
                    crate::resolution::declarations::machine_equations::DeclaredTypeOrigin::Constant(ordinal),
                    definition.type_reference,
                ));
            }
            lowerer.pending_const_declarations.push(
                crate::resolution::lowering::lowerer::PendingConstDeclaration {
                    declaration,
                    scope: definition.scope.clone(),
                    semantic_name: crate::resolution::constants::semantic_const_name(
                        syntax_trees,
                        definition,
                    ),
                    source_span: definition.name.source_span(),
                    is_public: definition.is_public,
                },
            );
        }
        Item::Use(import) => lowerer.namespace_declarations.record_import(syntax_trees, import),
        Item::Capability(_) | Item::Package(_) => {}
    }

    Ok(())
}

fn item_expression_exposure(item: &Item) -> AuthoredDeclarationSelectionExposure {
    use crate::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

    match item {
        syntax::item::Item::Data(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::TypeAlias(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Conformance(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Domain(definition) if definition.is_public => Exposure::PublicInterface,
        syntax::item::Item::Proposition(definition) if definition.is_public => {
            Exposure::PublicInterface
        }
        syntax::item::Item::Machine(machine) => {
            crate::resolution::lowering::machine::machine_interface_exposure(machine)
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
    source: ConformanceRowSource,
) -> Result<crate::source_resolved::trait_definition::ConformanceRow, Diagnostic> {
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
    realization.name =
        syntax::identifier::Identifier::new(realization_name.clone(), machine.name.source_span());
    realization.attached_data = None;
    realization.satisfies = if source == ConformanceRowSource::Inline {
        machine.satisfies
    } else {
        arena::HandleSpan::empty()
    };
    lower_machine_into(lowerer, syntax_trees, &realization)?;
    Ok(crate::source_resolved::trait_definition::ConformanceRow {
        declaring_trait: symbols::SymbolHandle::invalid(),
        declaring_trait_name: declaring_trait
            .map(|name| lower_name(syntax_trees, name))
            .unwrap_or_default(),
        requirement: symbols::SymbolHandle::invalid(),
        requirement_name: lower_name(syntax_trees, &requirement_name),
        provisional_requirement_ordinal: requirement_ordinal,
        realization_machine: symbols::SymbolHandle::invalid(),
        realization_state: symbols::SymbolHandle::invalid(),
        realization_name: DiagnosticName::generated(realization_name),
        authored_realization_source_span: None,
        provisional_realization_ordinal: Some(realization_ordinal),
        source,
    })
}
