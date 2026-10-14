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

/// One batch's exact semantic input and reusable read-only signature vocabulary.
/// Mutable rows and derived indexes never survive between method projections:
/// equal-sized substitutions can otherwise alias revision-keyed lookup results.
pub(crate) struct SignatureProjection<'a> {
    compilation: PackageReviewInput<'a>,
    projected: Option<TypedTrees>,
}

impl<'a> SignatureProjection<'a> {
    pub(crate) fn new(compilation: PackageReviewInput<'a>) -> Self {
        Self {
            compilation,
            projected: None,
        }
    }

    pub(crate) fn input(&self) -> PackageReviewInput<'a> {
        self.compilation
    }

    pub(super) fn trees(&mut self) -> &mut TypedTrees {
        let projected = match self.projected.take() {
            None => trees(&self.compilation),
            Some(previous) => {
                // Signature instantiation appends only type references, static
                // parameters and value parameters. Move the immutable arenas,
                // restore those three source arenas with their source indexes,
                // and mint fresh program-level indexes and identity interners.
                // No scratch handle escapes into policy.
                let source = self.compilation.checking_input;
                let mut tables = previous.tables;
                tables.type_reference_table = source.tables.type_reference_table.clone();
                tables.data_type_parameters = source.tables.data_type_parameters.clone();
                tables.state_parameters = source.tables.state_parameters.clone();
                let mut projected =
                    TypedTrees::with_roots(previous.roots, tables, previous.symbols);
                projected.retain_authored_declaration_selections(
                    self.compilation.authored_declaration_selections().clone(),
                );
                projected.plan_laid_layouts = previous.plan_laid_layouts;
                projected.plan_laid_target_layouts = previous.plan_laid_target_layouts;
                projected.plan_laid_schema_fields = previous.plan_laid_schema_fields;
                projected.placed_view_plans = previous.placed_view_plans;
                projected.semantic_domains = previous.semantic_domains;
                projected.external_bindings = previous.external_bindings;
                projected.machine_specializations = previous.machine_specializations;
                projected
            }
        };
        self.projected.insert(projected)
    }
}

fn trees(compilation: &PackageReviewInput<'_>) -> TypedTrees {
    let source = compilation.checking_input;
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
    projected.retain_authored_declaration_selections(
        compilation.authored_declaration_selections().clone(),
    );
    projected.plan_laid_layouts = source.plan_laid_layouts.clone();
    projected.plan_laid_target_layouts = source.plan_laid_target_layouts.clone();
    projected.plan_laid_schema_fields = source.plan_laid_schema_fields.clone();
    projected.placed_view_plans = source.placed_view_plans.clone();
    projected.semantic_domains = source.semantic_domains.clone();
    projected.external_bindings = source.external_bindings.clone();
    projected.machine_specializations = source.machine_specializations.clone();
    projected
}
