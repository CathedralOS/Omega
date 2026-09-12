//! Pre-resolution synthesis of closed generic data inside the name-resolution owner.
//!
//! These routines consume syntax and return syntax. Discovered templates,
//! substitutions and pending instances are private working state. Durable
//! instance declarations remain in SyntaxTrees, not a second program schema.

use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use std::collections::{HashMap, HashSet};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    ConstDefinition, DataDefinition, DataMember, Item, ProofFact, TypeParameter, TypeParameterKind,
};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod arguments;
mod const_evaluation;
pub(crate) mod constant_selection;
mod discovery;
mod eligibility;
mod module_constants;
mod substitution;
mod synthesis;
mod uses;

#[cfg(test)]
mod division_tests;
#[cfg(test)]
mod fact_division_tests;
#[cfg(test)]
mod module_constant_tests;
#[cfg(test)]
mod remainder_tests;
#[cfg(test)]
mod sum_constructor_tests;

use arguments::*;
use const_evaluation::*;
use discovery::*;
use eligibility::*;
use substitution::*;
use synthesis::desugar_generic_data_instances;
use uses::*;

/// Constant-expression arguments in concrete data fields and their parameter types.
/// Open templates and machine lexical scopes cannot borrow a standalone probe.
/// These handles borrow the input syntax; no provisional program escapes.
/// Each tuple retains the argument, parameter type, and real owner's public exposure.
pub fn closed_data_const_argument_expressions(
    syntax: &SyntaxTrees,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let positions = collect_data_type_reference_positions(syntax, false);
    let public_positions = collect_data_type_reference_positions(syntax, true);
    collect_closed_const_arguments(syntax, &positions, &public_positions, false)
}

/// Authored constant arguments in nongeneric machine type owners. Discovery
/// supplies destinations, not value-selection authority: callers must resolve
/// each expression in its original machine and state scope before evaluation.
pub fn closed_machine_const_arguments(
    syntax: &SyntaxTrees,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let (positions, public_positions) = collect_machine_type_reference_positions(syntax);
    collect_closed_const_arguments(syntax, &positions, &public_positions, true)
}

fn collect_closed_const_arguments(
    syntax: &SyntaxTrees,
    positions: &[TypeReferenceHandle],
    public_positions: &[TypeReferenceHandle],
    include_named: bool,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let mut pending = Vec::new();
    let mut retain_arguments = |parameters: &[TypeParameter], arguments: &[TypeReferenceHandle]| {
        if parameters.len() != arguments.len() {
            return;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let TypeParameterKind::Const { type_reference } = parameter.kind
                && match syntax.type_references.type_reference(*argument) {
                    TypeReferenceNode::ConstExpression(_) => true,
                    TypeReferenceNode::Named(name) if include_named => {
                        syntax
                            .type_references
                            .const_argument_normalization(*argument)
                            .is_none()
                            && CanonicalConstValue::from_atom(name.as_str()).is_none()
                            && name.as_str().parse::<i128>().is_err()
                            && !matches!(name.as_str(), "true" | "false")
                    }
                    _ => false,
                }
                && !pending.iter().any(|(existing, _, _)| existing == argument)
            {
                pending.push((
                    *argument,
                    type_reference,
                    public_positions.contains(argument),
                ));
            }
        }
    };
    for position in positions {
        match syntax.type_references.type_reference(*position) {
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let mut definitions = syntax.root_items().filter_map(|item| match item {
                    Item::Data(definition) if definition.name.as_str() == base_name.as_str() => {
                        Some(definition)
                    }
                    _ => None,
                });
                let Some(definition) = definitions.next() else {
                    continue;
                };
                // Ambiguous template selection is not resolved by visitation order.
                if definitions.next().is_some() {
                    continue;
                }
                retain_arguments(
                    syntax.items.type_parameters(definition.type_parameters),
                    syntax.type_references.type_reference_handles(*arguments),
                );
            }
            TypeReferenceNode::Constrained { constraints, .. } => {
                for constraint in syntax.type_references.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    let Some(parameters) =
                        unique_domain_index_parameters(syntax, domain.name.as_str())
                    else {
                        continue;
                    };
                    retain_arguments(
                        parameters,
                        syntax
                            .type_references
                            .type_reference_handles(domain.arguments),
                    );
                }
            }
            _ => {}
        }
    }
    if include_named {
        let concrete = concrete_machine_expression_handles(syntax);
        for (handle, expression) in syntax.expressions.iter_expressions() {
            if !concrete.contains(&handle.arena_index()) {
                continue;
            }
            let ExpressionNode::Cast(cast) = expression else {
                continue;
            };
            let name = syntax
                .expressions
                .identifier_path_members(cast.semantic_domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let Some(parameters) = unique_domain_index_parameters(syntax, &name) else {
                continue;
            };
            retain_arguments(
                parameters,
                syntax
                    .type_references
                    .type_reference_handles(cast.semantic_domain_arguments),
            );
        }
    }
    pending
}

fn unique_domain_index_parameters<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &str,
) -> Option<&'syntax [TypeParameter]> {
    let mut definitions = syntax.root_items().filter_map(|item| match item {
        Item::Domain(definition) if definition.name.as_str() == name => Some(definition),
        _ => None,
    });
    let definition = definitions.next()?;
    if definitions.next().is_some() {
        return None;
    }
    domain_index_parameters(syntax, definition)
}

/// Canonicalize one source const declaration against its own declared type.
///
/// This is the narrow handoff used by declaration/API retention. The returned
/// value's structural encoding is semantic material; its display text remains
/// diagnostic-only. Constrained public constants stay unsupported until their
/// declaration-site proof obligations are checked rather than erased here.
pub(crate) fn canonicalize_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<CanonicalConstValue, String> {
    canonicalize_selected_declared_const_definition(syntax, definition, None)
}

pub(crate) fn canonicalize_selected_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&constant_selection::ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    if matches!(
        syntax
            .tables
            .type_references
            .type_reference(definition.type_reference),
        TypeReferenceNode::Constrained { .. }
    ) {
        return Err(
            "constrained const declarations require declaration-site proof checking before they can publish compatibility identity"
                .to_owned(),
        );
    }
    canonicalize_selected_const_definition(syntax, definition, definition.type_reference, selection)
}

/// Find `Base<Args..>` spellings in FIELD type position where `Base` is a
/// generic data definition, synthesize one concrete instance record per
/// distinct spelling (the parameter substituted for the argument), and rewrite
/// the field spellings to the instances' plain names.
/// Run Psi's target-neutral pre-resolution generic-data normalization and
/// return the only syntax tree downstream stages may consume.
///
/// Taking ownership prevents orchestration code from retaining an unnormalized
/// sibling or reaching into the elaborator as an in-place syntax mutator.
pub fn normalize_generic_data(syntax: SyntaxTrees) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let (syntax, warnings) = normalize_generic_data_with_warnings(syntax)?;
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(syntax)
}

/// Replay direct structural arguments at their actual generic owner. The
/// encoded label is consistency data; the selected template's parameter owns
/// the carrier used by the existing canonical encoder.
pub(crate) fn validate_direct_const_arguments(
    syntax: &SyntaxTrees,
    base_name: &Identifier,
    arguments: HandleSpan<TypeReferenceHandle>,
    selection: Option<&constant_selection::ConstantSelection>,
) -> Result<(), Diagnostic> {
    let arguments = syntax.type_references.type_reference_handles(arguments);
    let has_direct = arguments.iter().any(|argument| {
        syntax
            .type_references
            .const_argument_normalization(*argument)
            .is_some_and(crate::constant::normalization_requires_expression)
    });
    if !has_direct {
        return Ok(());
    }
    let selection = selection.ok_or_else(|| {
        Diagnostic::error("direct constant argument has no declaration selection context")
            .with_source_span(base_name.source_span())
    })?;
    let definition = selection
        .data(syntax, base_name)
        .map_err(|message| Diagnostic::error(message).with_source_span(base_name.source_span()))?;
    let parameters = syntax.items.type_parameters(definition.type_parameters);
    if parameters.len() != arguments.len() {
        return Err(
            Diagnostic::error("direct constant argument lost its exact template slot")
                .with_source_span(base_name.source_span()),
        );
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(normalization) = syntax
            .type_references
            .const_argument_normalization(*argument)
        else {
            continue;
        };
        if !crate::constant::normalization_requires_expression(normalization) {
            continue;
        }
        crate::constant::validate_normalized_expression(syntax, normalization)?;
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return Err(Diagnostic::error(
                "direct constant argument no longer selects a const parameter",
            )
            .with_source_span(normalization.reference));
        };
        let value = canonicalize_selected_index_expression(
            syntax,
            type_reference,
            type_reference,
            normalization.authored_expression,
            Some(selection),
        )
        .map_err(|message| Diagnostic::error(message).with_source_span(normalization.reference))?;
        if value.encoding != normalization.canonical_result_encoding {
            return Err(Diagnostic::error(
                "direct constant expression differs from its normalized value",
            )
            .with_source_span(normalization.reference));
        }
    }
    Ok(())
}

/// Normalize with the loader's exact source/import custody. Temporary header
/// symbols stay private; each erased constant argument retains its selected
/// declaration coordinates for the complete resolver's checked join.
pub fn normalize_generic_data_with_sources_and_top_level_bindings(
    syntax: SyntaxTrees,
    sources: std::sync::Arc<source::SourceMap>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    normalize_generic_data_with_retained_base(syntax, sources, bindings, None)
}

/// Generated units may name retained nominal arguments without owning their
/// syntax. Their immutable predecessor participates in the same header resolver;
/// templates still come only from this unit, preserving the extension frontier.
pub fn normalize_generic_data_with_retained_base(
    mut syntax: SyntaxTrees,
    sources: std::sync::Arc<source::SourceMap>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    retained: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let selection = constant_selection::ConstantSelection::with_retained_base(
        &syntax,
        Some(sources),
        bindings,
        retained,
    )?;
    crate::module_normalization::validate_with_selection(&syntax, &selection)?;
    let mut warnings = Vec::new();
    synthesis::desugar_generic_data_instances_with_selection(
        &mut syntax,
        &mut warnings,
        Some(&selection),
    )?;
    deduplicate_generic_warnings(&mut warnings);
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(syntax)
}

fn normalize_generic_data_with_warnings(
    mut syntax: SyntaxTrees,
) -> Result<(SyntaxTrees, Vec<Diagnostic>), Vec<Diagnostic>> {
    crate::module_normalization::validate_module_normalization(&syntax)?;
    let mut warnings = Vec::new();
    desugar_generic_data_instances(&mut syntax, &mut warnings)?;
    deduplicate_generic_warnings(&mut warnings);
    Ok((syntax, warnings))
}

fn deduplicate_generic_warnings(warnings: &mut Vec<Diagnostic>) {
    let mut origins = Vec::new();
    warnings.retain(|warning| {
        let Some(origin) = warning.source_span else {
            return true;
        };
        if warning.is_error()
            || origin.source_id.0 == usize::MAX
            || origin.span.start >= origin.span.end
        {
            return true;
        }
        // Generic clones retain the same authored origin. Missing or invalid
        // spans do not establish that two diagnostics describe the same site.
        if origins.contains(&origin) {
            return false;
        }
        origins.push(origin);
        true
    });
}
