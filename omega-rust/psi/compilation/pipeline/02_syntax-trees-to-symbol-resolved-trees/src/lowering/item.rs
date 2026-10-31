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
use crate::lowering::name::lower_names;
use crate::lowering::operator::lower_operator_definition;
use crate::lowering::proposition::lower_proposition_definition;
use crate::lowering::trait_definition::lower_trait_definition;
use crate::lowering::type_reference::lower_child_type_references;
use crate::resolution::lowerer::Lowerer;
use crate::tables::syntax_items;
use crate::tables::syntax_type_references;
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
                            &conformance.trait_name,
                            conformance.trait_lifetime_arguments.as_slice(),
                            conformance.trait_arguments,
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
                            &conformance.trait_name,
                            conformance.trait_lifetime_arguments.as_slice(),
                            conformance.trait_arguments,
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
                            &conformance.trait_name,
                            conformance.trait_lifetime_arguments.as_slice(),
                            conformance.trait_arguments,
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
                        let realization_name = syntax_items(&syntax_trees)
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
                    lifetime_parameters: lower_names(
                        syntax_trees,
                        &conformance.lifetime_parameters,
                    ),
                    type_parameters,
                    trait_name: crate::lowering::name::lower_name(
                        syntax_trees,
                        &conformance.trait_name,
                    ),
                    trait_symbol: symbols::SymbolHandle::invalid(),
                    trait_lifetime_arguments: lower_names(
                        syntax_trees,
                        &conformance.trait_lifetime_arguments,
                    ),
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
    trait_name: &syntax::identifier::Identifier,
    trait_lifetime_arguments: &[syntax::identifier::Identifier],
    trait_arguments: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
    declaring_trait: Option<&syntax::identifier::Identifier>,
    requirement_ordinal: Option<usize>,
    machine: &syntax::item::Machine,
    source: symbol_resolved_trees::trait_definition::ConformanceRowSource,
) -> Result<symbol_resolved_trees::trait_definition::ConformanceRow, Diagnostic> {
    let requirement_name = machine.name.clone();
    if source == symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
        && let Some((realization_name, authored_realization_source_span)) = sealed_requirement_call(
            syntax_trees,
            trait_name,
            trait_lifetime_arguments,
            trait_arguments,
            &requirement_name,
            machine,
        )
    {
        // The member's only body is a call to a machine that already satisfies
        // this requirement. That machine is the realization. A second machine
        // would be a different plan, and body completion cannot retarget the
        // row after the empty-body pass.
        return Ok(symbol_resolved_trees::trait_definition::ConformanceRow {
            declaring_trait: symbols::SymbolHandle::invalid(),
            declaring_trait_name: declaring_trait
                .map(|name| crate::lowering::name::lower_name(syntax_trees, name))
                .unwrap_or_default(),
            requirement: symbols::SymbolHandle::invalid(),
            requirement_name: crate::lowering::name::lower_name(syntax_trees, &requirement_name),
            provisional_requirement_ordinal: requirement_ordinal,
            realization_machine: symbols::SymbolHandle::invalid(),
            realization_state: symbols::SymbolHandle::invalid(),
            realization_name: symbol_resolved_trees::name::DiagnosticName::generated(
                realization_name,
            ),
            authored_realization_source_span: Some(authored_realization_source_span),
            provisional_realization_ordinal: None,
            source: symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference,
        });
    }
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

/// An inline member whose body is one static call is the callee when that
/// callee already satisfies this exact requirement. A helper that does not
/// satisfy the requirement stays a real call.
fn sealed_requirement_call(
    syntax_trees: &SyntaxTrees,
    trait_name: &syntax::identifier::Identifier,
    trait_lifetime_arguments: &[syntax::identifier::Identifier],
    trait_arguments: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
    requirement_name: &syntax::identifier::Identifier,
    machine: &syntax::item::Machine,
) -> Option<(String, source::SourceSpan)> {
    let (callee_path, callee_span) = sole_static_call(syntax_trees, machine)?;
    let mut matched = None;
    for item in syntax_trees.root_items() {
        let syntax::item::Item::Machine(candidate) = item else {
            continue;
        };
        if candidate.name.as_str(syntax_trees) != callee_path {
            continue;
        }
        let satisfies = candidate_satisfies_requirement(
            syntax_trees,
            candidate,
            trait_name,
            trait_lifetime_arguments,
            trait_arguments,
            requirement_name,
        );
        if !satisfies {
            continue;
        }
        if matched.is_some() {
            return None;
        }
        matched = Some((callee_path.clone(), callee_span));
    }
    matched
}

fn candidate_satisfies_requirement(
    syntax_trees: &SyntaxTrees,
    candidate: &syntax::item::Machine,
    trait_name: &syntax::identifier::Identifier,
    trait_lifetime_arguments: &[syntax::identifier::Identifier],
    trait_arguments: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
    requirement_name: &syntax::identifier::Identifier,
) -> bool {
    syntax_items(&syntax_trees)
        .satisfies_clauses(candidate.satisfies)
        .iter()
        .any(|clause| {
            clause.trait_name.as_str(syntax_trees) == trait_name.as_str(syntax_trees)
                && clause.requirement.as_ref().is_some_and(|requirement| {
                    requirement.as_str(syntax_trees) == requirement_name.as_str(syntax_trees)
                })
                && identifiers_match(
                    syntax_trees,
                    &clause.lifetime_arguments,
                    trait_lifetime_arguments,
                )
                && syntax_type_arguments_match(syntax_trees, clause.arguments, trait_arguments)
        })
}

fn identifiers_match(
    syntax_trees: &SyntaxTrees,
    left: &[syntax::identifier::Identifier],
    right: &[syntax::identifier::Identifier],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.as_str(syntax_trees) == right.as_str(syntax_trees))
}

fn syntax_type_arguments_match(
    syntax_trees: &SyntaxTrees,
    left: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
    right: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
) -> bool {
    let left = syntax_trees.type_references.type_reference_handles(left);
    let right = syntax_trees.type_references.type_reference_handles(right);
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            syntax_type_spelling(syntax_trees, *left).is_some_and(|spelling| {
                syntax_type_spelling(syntax_trees, *right) == Some(spelling)
            })
        })
}

fn syntax_type_spelling(
    syntax_trees: &SyntaxTrees,
    reference: syntax::types::TypeReferenceHandle,
) -> Option<String> {
    match syntax_trees.type_references.type_reference(reference) {
        syntax::types::TypeReferenceNode::Named(name) => {
            Some(name.as_str(syntax_trees).to_string())
        }
        syntax::types::TypeReferenceNode::Unit => Some("()".to_string()),
        syntax::types::TypeReferenceNode::Reference { referee, .. } => Some(format!(
            "&{}",
            syntax_type_spelling(syntax_trees, *referee)?
        )),
        syntax::types::TypeReferenceNode::Slice { element_type } => Some(format!(
            "[{}]",
            syntax_type_spelling(syntax_trees, *element_type)?
        )),
        syntax::types::TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax_type_references(&syntax_trees)
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| syntax_type_spelling(syntax_trees, *argument))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            Some(format!("{}<{arguments}>", base_name.as_str(syntax_trees)))
        }
        _ => None,
    }
}

fn sole_static_call(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Option<(String, source::SourceSpan)> {
    let [state] = syntax_trees.items.state_handles(machine.states) else {
        return None;
    };
    let state = syntax_trees.items.state(*state);
    let [statement] = syntax_trees.items.statements(state.statements) else {
        return None;
    };
    let syntax::statement::StatementNode::Transition(transition) =
        syntax_trees.statements.statement(*statement)
    else {
        return None;
    };
    if transition.continuation.is_valid()
        || transition.guard != syntax::statement::TransitionGuardNode::Always
        || transition.exit != syntax::statement::TransitionExit::Ordinary
        || !syntax_trees
            .statements
            .outcome_proof_selectors(transition.proof_selectors)
            .is_empty()
    {
        return None;
    }
    let syntax::statement::TransitionTargetNode::Value(expression) =
        syntax_trees.statements.transition_target(transition.target)
    else {
        return None;
    };
    let syntax::expression::ExpressionNode::Call(call) =
        syntax_trees.expressions.expression(*expression)
    else {
        return None;
    };
    if !call.target_is_static {
        return None;
    }
    let target_name = call.target.as_str(syntax_trees).to_string();
    let target_span = call.target.source_span();
    if !call.receiver.is_valid() {
        return Some((target_name, target_span));
    }
    let syntax::expression::ExpressionNode::Name(path) =
        syntax_trees.expressions.expression(call.receiver)
    else {
        return None;
    };
    let members = syntax_trees.expressions.identifier_path_members(*path);
    let first = members.first()?;
    let mut names = members
        .iter()
        .map(|member| member.as_str(syntax_trees).to_string())
        .collect::<Vec<_>>();
    names.push(target_name);
    let span = source::SourceSpan::new(
        first.source_span().source_id,
        source::Span::new(first.source_span().span.start, target_span.span.end),
    );
    Some((names.join("::"), span))
}
