//! Invocation-local typed-tree scratch for calling-signature instantiation.
//!
//! Reifying a boundary application writes fresh type-reference and parameter
//! rows into an arena the capture path owns, while every passthrough row —
//! substituted arguments, unchanged subterms, declaration lookups — keeps the
//! source handle space. The scratch therefore seeds exactly the arenas a
//! signature or type-identity read can index: trait telescopes, state
//! signatures and parameters, data/proposition declarations, const and domain
//! vocabulary, the operator spellings consulted by closed-integer endpoint
//! evaluation, and the expression table behind `ConstExpression`/`Range`
//! terms — plus the authored-declaration-selection custody those endpoints
//! join through. The checked compilation's statement, machine-body, wire,
//! measure, and proof arenas — the bulk of a real `TypedTrees` — stay
//! uncloned.

use crate::capture::PackageReviewInput;
use typed_trees::{TypedTreeTables, TypedTrees};

pub(super) fn trees(compilation: &PackageReviewInput<'_>) -> TypedTrees {
    let source = &compilation.typed;
    let mut tables = TypedTreeTables::default();
    tables.const_declarations = source.tables.const_declarations.clone();
    tables.data_definitions = source.tables.data_definitions.clone();
    tables.data_type_parameters = source.tables.data_type_parameters.clone();
    tables.domain_definitions = source.tables.domain_definitions.clone();
    tables.domain_path_members = source.tables.domain_path_members.clone();
    tables.propositions = source.tables.propositions.clone();
    tables.proposition_binders = source.tables.proposition_binders.clone();
    tables.state_parameters = source.tables.state_parameters.clone();
    tables.traits = source.tables.traits.clone();
    tables.trait_requirements = source.tables.trait_requirements.clone();
    tables.trait_machine_signatures = source.tables.trait_machine_signatures.clone();
    tables.signature_invokes = source.tables.signature_invokes.clone();
    tables.signature_contracts = source.tables.signature_contracts.clone();
    tables.operators = source.tables.operators.clone();
    tables.expression_table = source.tables.expression_table.clone();
    tables.type_reference_table = source.tables.type_reference_table.clone();
    let mut projected =
        TypedTrees::with_roots(source.roots.clone(), tables, source.symbols.clone());
    projected
        .retain_authored_declaration_selections(source.authored_declaration_selections().clone());
    projected.plan_laid_layouts = source.plan_laid_layouts.clone();
    projected.placed_view_plans = source.placed_view_plans.clone();
    projected.semantic_domains = source.semantic_domains.clone();
    projected.external_bindings = source.external_bindings.clone();
    projected.machine_specializations = source.machine_specializations.clone();
    projected.open_index_normalizations = source.open_index_normalizations.clone();
    projected
}
