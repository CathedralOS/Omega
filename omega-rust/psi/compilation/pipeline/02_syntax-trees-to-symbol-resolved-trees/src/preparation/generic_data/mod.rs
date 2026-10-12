//! Pre-resolution synthesis of closed generic data inside the name-resolution owner.
//!
//! These routines consume syntax and return syntax. Discovered templates,
//! substitutions and pending instances are private working state. Durable
//! instance declarations remain in SyntaxTrees, not a second program schema.
//! Exact declaration lookup comes from the shared
//! `preparation::declaration_selection` owner.
//!
//! Closed declaration specialization is demanded by authored applications
//! and inline storage. Reference identity alone does not demand a recursively
//! growing catalog, even for const-bearing or constrained declarations. Closed
//! argument preparation remains independent of declaration copying. Data facts
//! define default domains; impossible domains are legal to name, not to inhabit
//! without establishing their predicates. Typed preparation elaborates field
//! views without reopening the frozen program during checking.

use crate::preparation::declaration_selection;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::IntegerLiteral;
use std::collections::{HashMap, HashSet};
use symbols::JoinedPath;
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
mod discovery;
pub(super) mod domain_heads;
mod eligibility;
pub(crate) use crate::preparation::type_equations as equations;
pub(crate) use equations::{template_type_equation_offsets, validate_materialized_type_equations};
pub(crate) mod module_constants;
mod substitution;
mod synthesis;
mod uses;
mod work_domain;

pub(crate) use uses::statement_type_inputs;

pub(crate) use arguments::{
    DomainCarrier, DomainHead, closed_argument_identity, domain_application_head,
    domain_index_telescope, monomorphizable_argument_slugs, normalize_domain_head_positions,
    selected_domain_family, type_reference_slug,
};
pub(in crate::preparation::generic_data) use arguments::{
    canonicalize_monomorphizable_argument_handles, closed_const_argument_identity,
    complete_argument_tuple,
};
pub(crate) use const_evaluation::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, canonicalize_selected_index_expression,
    closed_fixed_array_length, evaluate_const_fact_binary, evaluate_const_fact_expression,
    evaluate_const_membership_fact, prove_const_literal_predicates,
    prove_declared_const_domain_constraints,
};
pub(in crate::preparation::generic_data) use const_evaluation::{
    EvaluatedConst, SourceTraitApplication, canonicalize_closed_domain_indices,
    canonicalize_const_definition, canonicalize_selected_declaration_value,
    capture_machine_runtime_template_names, checked_fact_integer,
    collect_data_type_reference_positions, collect_lexical_type_reference_inventory,
    const_integer_in_envelope, const_integer_type, domain_index_parameters,
    evaluate_const_argument_expression, generic_const_integer_types, integer_literal_value,
    normalize_generic_template_const_expressions, prepare_closed_application,
    prove_const_leaf_domain_constraints, qualified_const_name, replace_const_expression_names_from,
    replace_machine_const_expression_names_from, syntax_type_identity, validate_const_index_type,
    validate_syntax_integer_range,
};
pub(in crate::preparation) use const_evaluation::{
    canonicalize_selected_const_definition, collect_type_positions,
};
pub(crate) use discovery::{ClosedArgumentIdentity, ClosedConstraintIdentity, selected_data_item};
pub(in crate::preparation::generic_data) use discovery::{
    GenericData, GenericDataShape, Instantiation, PendingRewrite, selected_generic_data,
};
pub(crate) use substitution::{
    closed_name_identity, substitute_member, type_reference_mentions_parameter,
};
use synthesis::desugar_generic_data_instances;
pub(crate) use synthesis::desugar_generic_data_instances_in;
pub(crate) use work_domain::GenericDataWorkDomain;

/// Source-owned const-expression positions, not every node in the arena.
/// Normalization keeps an archived original operand for custody after replacing
/// its declared argument. That recipe is not a second pending evaluation job.
/// An original handle still reached by a live declaration remains a position;
/// no source-span or retained-value correspondence reconstructs its receiver.
pub fn source_const_expression_positions(
    syntax: &SyntaxTrees,
) -> Vec<(
    TypeReferenceHandle,
    syntax_trees::expression::ExpressionHandle,
)> {
    let inventory = collect_lexical_type_reference_inventory(syntax);
    let mut expressions = Vec::new();
    for position in inventory.positions {
        // Static machine/requirement operands retain their invocation telescope,
        // including runtime Value roles. A genuine container use of the same
        // handle still owes its own declared const destination below.
        if inventory.invocation_arguments.contains(&position)
            && !inventory.container_arguments.contains(&position)
        {
            continue;
        }
        if syntax
            .type_references
            .const_argument_normalization(position)
            .is_some()
        {
            continue;
        }
        if let TypeReferenceNode::ConstExpression(expression) =
            syntax.type_references.type_reference(position)
            && !expressions
                .iter()
                .any(|(retained, _)| *retained == position)
        {
            expressions.push((position, *expression));
        }
    }
    expressions
}

/// Authored constant arguments in concrete data fields and constant declarations.
/// Open templates and machine lexical scopes cannot borrow a standalone probe.
/// These handles borrow the input syntax; no provisional program escapes. That
/// borrow is why discovery names the forest directly instead of taking the
/// resolution request, which consumes the forest it prepares.
/// Each tuple retains the argument, parameter type, and real owner's public exposure.
pub fn closed_data_const_argument_expressions(
    syntax: &SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>, Vec<Diagnostic>> {
    let selection =
        declaration_selection::ConstantSelection::new(syntax, sources, top_level_bindings)?;
    let positions = collect_data_type_reference_positions(syntax, false);
    let public_positions = collect_data_type_reference_positions(syntax, true);
    Ok(collect_closed_const_arguments(
        syntax,
        &selection,
        &positions,
        &public_positions,
        false,
        &[],
    ))
}

/// Declaration-owned constant arguments. Discovery supplies destinations, not
/// value-selection authority: each expression resolves in its original lexical
/// scope before evaluation; free const binders remain for specialization.
pub fn original_scope_const_arguments(
    syntax: &SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>, Vec<Diagnostic>> {
    let selection =
        declaration_selection::ConstantSelection::new(syntax, sources, top_level_bindings)?;
    let inventory = collect_lexical_type_reference_inventory(syntax);
    Ok(collect_closed_const_arguments(
        syntax,
        &selection,
        &inventory.positions,
        &inventory.public_positions,
        true,
        &inventory.trait_applications,
    ))
}

fn collect_closed_const_arguments(
    syntax: &SyntaxTrees,
    selection: &declaration_selection::ConstantSelection<'_>,
    positions: &[TypeReferenceHandle],
    public_positions: &[TypeReferenceHandle],
    include_machine_casts: bool,
    trait_applications: &[SourceTraitApplication],
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
                    TypeReferenceNode::Named(name) => {
                        syntax
                            .type_references
                            .const_argument_normalization(*argument)
                            .is_none()
                            && ConstScalarSpelling::from_bare_name(name.as_str()).is_none()
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
                let Ok(definition) = selection.data(syntax, base_name) else {
                    continue;
                };
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
                    let Some(parameters) = unique_domain_index_parameters(
                        syntax,
                        selection,
                        domain.name.as_str(),
                        domain.name.source_span(),
                    ) else {
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
    // The source selection owner supplies exact headers, just as trait-default
    // synthesis does. A leaf name or a copied ABI signature cannot supply this
    // receiving telescope.
    let headers = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Trait(definition) = item else {
                return None;
            };
            Some((
                selection.trait_lookup(&definition.name).unique()?,
                definition.type_parameters,
            ))
        })
        .collect::<Vec<_>>();
    for application in trait_applications {
        let Some(symbol) = selection.trait_lookup(&application.name).unique() else {
            continue;
        };
        let mut matching = headers.iter().filter(|(header, _)| *header == symbol);
        let Some((_, parameters)) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            continue;
        }
        retain_arguments(
            syntax.items.type_parameters(*parameters),
            syntax
                .type_references
                .type_reference_handles(application.arguments),
        );
    }
    if include_machine_casts {
        for (_, expression) in syntax.expressions.iter_expressions() {
            let ExpressionNode::Cast(cast) = expression else {
                continue;
            };
            if !syntax
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .any(|argument| positions.contains(argument))
            {
                continue;
            }
            let members = syntax
                .expressions
                .identifier_path_members(cast.semantic_domain);
            let Some(first) = members.first() else {
                continue;
            };
            if members.iter().any(|member| {
                let span = member.source_span();
                span.source_id != first.source_span().source_id || span.span.start > span.span.end
            }) || members
                .windows(2)
                .any(|pair| pair[0].source_span().span.end > pair[1].source_span().span.start)
            {
                // Malformed custody cannot select a family through a partial path.
                continue;
            }
            let mut reference = first.source_span();
            for member in members.iter().skip(1) {
                reference.span.end = member.source_span().span.end;
            }
            let name = members.joined_path();
            let Some(parameters) =
                unique_domain_index_parameters(syntax, selection, &name, reference)
            else {
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
    selection: &declaration_selection::ConstantSelection<'_>,
    name: &str,
    reference: source::SourceSpan,
) -> Option<&'syntax [TypeParameter]> {
    // Declaration names include their carrier; an authored qualifier may be
    // module-relative. Borrow the telescope only after ordinary source-owned
    // family selection, never by comparing those different spellings.
    let (_, definition) = selection.domain_family(syntax, name, reference, None)?;
    domain_index_parameters(syntax, definition)
}

/// Canonicalize one source const declaration against its own declared type.
///
/// This is the narrow handoff used by declaration/API retention. The returned
/// value's structural encoding is semantic material; its display text remains
/// diagnostic-only. A constrained carrier canonicalizes at its base and then
/// owes declaration-site proof: each unindexed domain constraint must
/// discharge against the canonical value through the same exact owner
/// selection and fact evaluator `where`-membership uses, or the declaration
/// stays fenced rather than publishing an unproved identity.
pub fn canonicalize_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<CanonicalConstValue, String> {
    canonicalize_selected_declared_const_definition(syntax, definition, None)
}

pub(crate) fn canonicalize_selected_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    let value = canonicalize_selected_declaration_value(syntax, definition, selection)?;
    if let TypeReferenceNode::Constrained { constraints, .. } = syntax
        .tables
        .type_references
        .type_reference(definition.type_reference)
    {
        prove_declared_const_domain_constraints(
            syntax,
            definition,
            syntax.tables.type_references.constraints(*constraints),
            &value,
            selection,
        )?;
    }
    Ok(value)
}

/// Find `Base<Args..>` spellings in FIELD type position where `Base` is a
/// generic data definition, synthesize one concrete instance record per
/// distinct spelling (the parameter substituted for the argument), and rewrite
/// the field spellings to the instances' plain names.
/// One forest and the custody its generic data applications close under.
pub struct GenericDataRequest<'a> {
    pub syntax: SyntaxTrees,
    /// The loader's exact source/import custody. A source-free forest has
    /// none; its instances then synthesize without header selection.
    pub sources: Option<std::sync::Arc<source::SourceMap>>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    /// A generated unit's immutable predecessor, whose retained nominal
    /// arguments the unit may name without owning their syntax. Templates
    /// still come only from this unit. Requires `sources`.
    pub retained_base: Option<&'a symbol_resolved_trees::SymbolResolvedTrees>,
}

impl GenericDataRequest<'_> {
    /// A source-free forest with no scoped top-level bindings.
    pub fn new(syntax: SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
            retained_base: None,
        }
    }
}

/// Run Psi's target-neutral pre-resolution generic-data normalization and
/// return the only syntax tree downstream stages may consume.
///
/// Taking ownership prevents orchestration code from retaining an unnormalized
/// sibling or reaching into the elaborator as an in-place syntax mutator. With
/// source custody, temporary header symbols stay private and each erased
/// constant argument retains its selected declaration coordinates for the
/// complete resolver's checked join.
pub fn normalize_generic_data(
    request: GenericDataRequest<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let GenericDataRequest {
        mut syntax,
        sources,
        top_level_bindings,
        retained_base,
    } = request;
    let Some(sources) = sources else {
        if retained_base.is_some() || !top_level_bindings.is_empty() {
            return Err(vec![Diagnostic::error(
                "generic data normalization against a retained base or scoped bindings requires source custody",
            )]);
        }
        let (syntax, warnings) = normalize_generic_data_with_warnings(syntax)?;
        for warning in warnings {
            eprintln!("{warning}");
        }
        return Ok(syntax);
    };
    let selection = declaration_selection::ConstantSelection::with_retained_base(
        &syntax,
        Some(sources),
        top_level_bindings,
        retained_base,
    )?;
    // Module constant admission replays domain obligations too. Bind lexical
    // carrier heads before that admission, not only before data substitution.
    domain_heads::normalize(&mut syntax, Some(&selection))?;
    crate::preparation::module_normalization::validate_with_selection(&syntax, &selection)?;
    let mut warnings = Vec::new();
    synthesis::desugar_generic_data_instances_with_selection(
        &mut syntax,
        &mut warnings,
        Some(&selection),
        &[],
    )?;
    deduplicate_generic_warnings(&mut warnings);
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(syntax)
}

/// Replay direct structural arguments at their actual generic owner. The
/// encoded result is consistency data; original operands and the selected
/// template's parameter independently own carrier and operation admission.
pub(crate) fn validate_direct_const_arguments(
    syntax: &SyntaxTrees,
    base_name: &Identifier,
    arguments: HandleSpan<TypeReferenceHandle>,
    selection: Option<&declaration_selection::ConstantSelection>,
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
        if !crate::constant::normalized_const_result_matches(&value, normalization) {
            return Err(Diagnostic::error(
                "direct constant expression differs from its normalized value",
            )
            .with_source_span(normalization.reference));
        }
    }
    Ok(())
}

fn normalize_generic_data_with_warnings(
    mut syntax: SyntaxTrees,
) -> Result<(SyntaxTrees, Vec<Diagnostic>), Vec<Diagnostic>> {
    domain_heads::normalize(&mut syntax, None)?;
    crate::preparation::module_normalization::validate_module_normalization(&syntax)?;
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
