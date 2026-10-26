//! The typing entrance, the [`TypingCohort`] every typing route runs, and the
//! `Lowerer` state every typing module borrows.
//!
//! `lower_symbol_resolved_trees` turns one `SymbolResolvedTrees` into
//! `TypedTrees`, or returns the first `Diagnostic`. Build-time evaluation
//! calls it directly, and `seeded_continuation` extends the retained typed
//! base it produced. It runs in this order:
//!
//! 1. Three checks that need case membership as a distinct node, before
//!    lowering erases it: `==` against bare payload-bearing case names,
//!    synthesized `Equatable` row prerequisites, and case-dispatch exhaustiveness.
//! 2. Builds the `Lowerer`, copies the service reaches, semantic domains,
//!    external bindings and authored declaration selections, then runs
//!    [`type_resolved_cohort`] over `TypingCohort::whole()`: constants, data,
//!    domains, propositions, mathematical definitions, machines (with a
//!    token-binding view when the machine has one), measures, operators,
//!    traits, conformances, wire schemas, and each constant's materialized and
//!    authored initializers. Every form except measures and conformances is
//!    lowered inside `with_type_reference_exposure`, which sets whether the
//!    type selections recorded for that declaration count as public interface
//!    or private implementation.
//! 3. `Lowerer::finish` normalizes trait requirement guarantees, rebuilds the
//!    typed trees, shifting each evidence forwarding past the proof-output
//!    calls erased before it and dropping derived lookup state, and closes the
//!    program through [`close_typed_cohort`]: satisfied declarations, machine
//!    progress premises, domain constraint normalization, proof membership
//!    interning, qualification casts, fixed byte array literals, range
//!    argument validation, provider default finalization, target-family
//!    publication from resolved declaration handles, and dynamic call target
//!    binding. The compiler consumes those records; it does not reconstruct
//!    them after typing.
//!
//! [`type_resolved_cohort`] and [`close_typed_cohort`] are the two operations
//! every typing route shares. A route states what it types by one
//! [`TypingCohort`]: the fresh product route covers a whole package, and the
//! two continuation routes in `seeded_continuation` cover the suffix they were
//! admitted against, so a step added to typing is added to all three routes by
//! construction instead of by remembering two more call sites.
//!
//! `signatures` is not a step of its own. The entrance never calls it; it is
//! the shared parameter, type-parameter and callable-interface vocabulary each
//! declaration form binds through.
//!
//! `lowerer::seeded_continuation` is an orchestration seam rather than a step:
//! it wraps this same entrance so a retained typed base can be extended
//! append-only, and fails the transaction rather than mutating a base that has
//! drifted.
//!
//! Children: `name` converts resolved names to typed identifiers, `progress`
//! holds the terminating-record normalization, and `seeded_continuation` is
//! the append-only extension route. `declaration_exposure`,
//! `exact_top_level_data_symbol` and `exact_field_symbol` are small helpers
//! that declaration, expression and seeded-continuation code also read.

use crate::declarations::data::lower_data_definition;
use crate::declarations::domain::lower_domain_definition;
use crate::declarations::machine::{lower_machine, lower_token_binding_view};
use crate::declarations::operator::lower_operator_definition;
use crate::declarations::trait_definition::lower_trait_definition;
use crate::type_reference::domain_constraints::normalize_domain_constraints;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) mod name;
pub(crate) mod progress;
pub(crate) mod seeded_continuation;
mod target_machine_families;

/// These judgments require authored membership and case nodes, which typing
/// erases. Every producer of new resolved bodies must run them before lowering,
/// including retained body completion and generated-source extension. Checking
/// the declaration-only predecessor does not validate its later implementations.
pub(crate) fn validate_resolved_semantics(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    // Decision 11: user-written `==` against bare payload-bearing case names
    // must be rejected BEFORE membership lowering synthesizes its internal
    // tag-equality compares, which are deliberately the same typed shape.
    crate::expressions::equality::validate_equality_operands(symbol_resolved_trees)?;

    // Synthesized Equatable rows check their structural prerequisites at
    // the implementation, before any `==` site tries to expand them.
    crate::expressions::equatable::validate_equatable_conformances(symbol_resolved_trees)?;

    // Exhaustiveness counting over case domains also needs the resolved
    // trees: membership is still a distinct node here, so case arms and
    // domain arms are recognizable before lowering erases them into tag
    // compares and classifier expansions.
    crate::expressions::exhaustiveness::validate_case_dispatch_exhaustiveness(symbol_resolved_trees)
}

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    validate_resolved_semantics(symbol_resolved_trees)?;
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
    // Forwardings are copied at their resolved coordinates first, so the walk
    // sees the same forwarding set the resolved program carries; the fresh
    // close-out re-seats each statement index once typing's erased proof-output
    // calls are known.
    lowerer.typed_trees.evidence_forwardings = symbol_resolved_trees
        .evidence_forwardings
        .iter()
        .map(|forwarding| lower_evidence_forwarding(forwarding, &[]))
        .collect();
    // The whole authored package: every frontier at zero.
    let cohort = TypingCohort::default();
    type_resolved_cohort(&mut lowerer, &cohort)?;
    lowerer.finish(&cohort)
}

/// What one typing run covers: where each resolved declaration family
/// continues, which retained machine bodies the run completes in place, and
/// where the run's typed tables begin.
///
/// A whole authored package is the default: every frontier at zero. A
/// continuation derives them from the exact pair it was admitted against, so
/// the shared walk types exactly the declarations resolution appended and the
/// shared close-out finalizes exactly the rows this run produced. The typed
/// checkpoints name that same boundary on the receiving side, which is not the
/// resolved frontier: typing appends rows of its own (fresh symbols,
/// synthesized instances) that no resolved declaration counts.
#[derive(Default)]
pub(crate) struct TypingCohort<'completed> {
    pub(crate) const_declarations: usize,
    pub(crate) data_definitions: usize,
    pub(crate) domain_definitions: usize,
    pub(crate) propositions: usize,
    pub(crate) mathematical_definitions: usize,
    pub(crate) machines: usize,
    pub(crate) measures: usize,
    pub(crate) operators: usize,
    pub(crate) traits: usize,
    pub(crate) conformances: usize,
    pub(crate) wire_schemas: usize,
    /// Retained machine bodies this run completes in place, in declaration
    /// order. Their destinations land beside the appended machines'.
    pub(crate) completed_machines: &'completed [SymbolHandle],
    /// Typed checkpoint: machines, type references and expressions the
    /// retained base already published.
    pub(crate) typed_machines: usize,
    pub(crate) typed_type_references: usize,
    pub(crate) typed_expressions: usize,
}

impl<'completed> TypingCohort<'completed> {
    /// The suffix past an exact retained pair. Every family continues where
    /// the resolved base ends; a family the continuation cannot extend has an
    /// empty suffix because admission proved the base already holds it.
    pub(crate) fn continuing(resolved: &SymbolResolvedTrees, typed: &TypedTrees) -> Self {
        Self {
            const_declarations: resolved.const_declarations.len(),
            data_definitions: resolved.data_definitions.len(),
            domain_definitions: resolved.domain_definitions.len(),
            propositions: resolved.propositions.len(),
            mathematical_definitions: resolved.mathematical_definitions.len(),
            machines: resolved.machines.len(),
            measures: resolved.measures.len(),
            operators: resolved.operators.len(),
            traits: resolved.traits.len(),
            conformances: resolved.conformances.len(),
            wire_schemas: resolved.wire_schemas.len(),
            completed_machines: &[],
            typed_machines: typed.machines().len(),
            typed_type_references: typed.type_reference_table.type_reference_count(),
            typed_expressions: typed.expression_table.expression_count(),
        }
    }

    /// Name the retained bodies this run completes in place.
    pub(crate) fn completing(mut self, machines: &'completed [SymbolHandle]) -> Self {
        self.completed_machines = machines;
        self
    }
}

/// Type the resolved declarations one cohort still owes, in the order the
/// resolved package records them.
///
/// One operation for three routes: the fresh product route runs it over a
/// whole package, and the two continuation routes run it over the suffix they
/// were admitted against. Typing a declaration form is the same work in every
/// route, so the walk has one statement per form and no route chooses among
/// forms.
pub(crate) fn type_resolved_cohort(
    lowerer: &mut Lowerer<'_>,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    let source = lowerer.source_trees;
    for declaration in source
        .const_declarations
        .iter()
        .skip(cohort.const_declarations)
    {
        lowerer.lower_const_declaration(declaration)?;
    }
    for data_definition in source.data_definitions.iter().skip(cohort.data_definitions) {
        lowerer.lower_data_declaration(data_definition)?;
    }
    for domain_definition in source
        .domain_definitions
        .iter()
        .skip(cohort.domain_definitions)
    {
        let domain_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(domain_definition.is_public),
            |lowerer| lower_domain_definition(lowerer, domain_definition),
        )?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }
    for proposition in source.propositions.iter().skip(cohort.propositions) {
        let proposition = lowerer.with_type_reference_exposure(
            declaration_exposure(proposition.is_public),
            |lowerer| {
                crate::declarations::proposition::lower_proposition_definition(lowerer, proposition)
            },
        )?;
        lowerer.typed_trees.push_proposition(proposition);
    }
    for definition in source
        .mathematical_definitions
        .iter()
        .skip(cohort.mathematical_definitions)
    {
        let definition = lowerer.with_type_reference_exposure(
            declaration_exposure(definition.is_public),
            |lowerer| {
                crate::declarations::mathematical::lower_mathematical_definition(
                    lowerer, definition,
                )
            },
        )?;
        lowerer.typed_trees.push_mathematical_definition(definition);
    }
    for (ordinal, machine) in source.machines.iter().enumerate() {
        if ordinal < cohort.machines {
            // A retained machine keeps its typed declaration; only a body this
            // run was handed is retyped, before the run's appended machines,
            // where the authored completion route has always lowered it.
            if cohort.completed_machines.contains(&machine.symbol) {
                complete_retained_machine_body(lowerer, machine.symbol)?;
            }
            continue;
        }
        lowerer.lower_machine_declaration(machine)?;
    }
    for measure in source.measures.iter().skip(cohort.measures) {
        let measure = crate::declarations::measure::lower_measure_definition(lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }
    for operator in source.operators.iter().skip(cohort.operators) {
        let operator = lowerer
            .with_type_reference_exposure(declaration_exposure(operator.is_public), |lowerer| {
                lower_operator_definition(lowerer, operator)
            })?;
        lowerer.typed_trees.push_operator(operator);
    }
    for trait_definition in source.traits.iter().skip(cohort.traits) {
        lowerer.lower_trait_declaration(trait_definition)?;
    }
    for conformance in source.conformances.iter().skip(cohort.conformances) {
        crate::declarations::conformance::lower_conformance(lowerer, conformance)?;
    }
    for wire_schema in source.wire_schemas.iter().skip(cohort.wire_schemas) {
        let wire_schema = lowerer.with_type_reference_exposure(
            declaration_exposure(wire_schema.is_public),
            |lowerer| crate::declarations::wire::lower_wire_schema(lowerer, wire_schema),
        )?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }
    // Constant initializers are the last evidence the resolved declarations
    // owe; they are lowered against the complete declaration set like every
    // other body.
    lowerer.lower_const_initializer_evidence(cohort.const_declarations)
}

/// Complete one retained machine's authored body in place. The typed
/// declaration already holds the retained interface, so only the
/// implementation delta is retyped; a missing row is a retained-base
/// violation, not an authored error.
fn complete_retained_machine_body(
    lowerer: &mut Lowerer<'_>,
    symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    let lost_base = || {
        Diagnostic::error(
            "authored-body completion changed its retained declaration or implementation frontier",
        )
    };
    let ordinal = lowerer
        .typed_trees
        .machine_position_by_symbol(symbol)
        .ok_or_else(lost_base)?;
    let mut declaration = lowerer.typed_trees.machines()[ordinal].clone();
    let implementation = lowerer.source_trees.machine(symbol).ok_or_else(lost_base)?;
    crate::declarations::machine::complete_machine_body(lowerer, implementation, &mut declaration)?;
    lowerer.typed_trees.machines_mut()[ordinal] = declaration;
    Ok(())
}

/// Close one typing run: settle what its declarations satisfied, normalize the
/// identities every typed consumer reads, and publish the run's target-machine
/// families.
///
/// One operation for three routes; the cohort names each row set's frontier,
/// so a retained base is finalized once, when it was typed, and this run's
/// rows once, here.
pub(crate) fn close_typed_cohort(
    lowerer: &mut Lowerer<'_>,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    crate::declarations::machine::settle_satisfied_declarations_from(
        &mut lowerer.typed_trees,
        cohort.typed_machines,
    )?;
    crate::lowerer::progress::normalize_machine_progress_premises(
        &mut lowerer.typed_trees,
        cohort.typed_machines,
    )?;
    lowerer
        .typed_trees
        .instantiate_nominal_machine_parameter_contracts()
        .and_then(|_| lowerer.typed_trees.prepare_applied_callable_signatures())
        .map_err(|message| Diagnostic::error(message.to_owned()))?;
    normalize_domain_constraints(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.typed_type_references,
    )?;
    crate::contracts::proof_facts::normalize_membership_selections(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
    )?;
    // Idempotent over retained facts: an already-interned instance re-derives
    // the same name, so only facts appended past the checkpoint change.
    crate::contracts::proof_facts::intern_proof_membership_instances(&mut lowerer.typed_trees)?;
    crate::expressions::qualification_casts::normalize_qualification_casts(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
    )?;
    crate::expressions::fixed_byte_array_literals::land_exact_fixed_byte_array_literals(
        &mut lowerer.typed_trees,
        cohort.typed_expressions,
        cohort.typed_machines,
        cohort.completed_machines,
    )?;
    crate::type_reference::validate_range_arguments(lowerer.source_trees, &lowerer.typed_trees)?;
    crate::declarations::provider_defaults::finalize_provider_default_calls(
        &mut lowerer.typed_trees,
    )?;
    target_machine_families::append(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        cohort.machines,
    )?;
    bind_dynamic_call_targets(&mut lowerer.typed_trees)
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

/// Append the reached-service rows a later resolved cohort added, keeping the
/// retained rows' identities. Resolution may reorder rows by owner, so the
/// retained prefix is matched by owner and only unseen owners are appended.
pub(crate) fn append_authored_service_reach_rows(
    target: &mut TypedTrees,
    source: &SymbolResolvedTrees,
    retained: &[symbol_resolved_trees::signature::AuthoredServiceReachRow],
) {
    target.authored_service_reach_rows.extend(
        source
            .authored_service_reach_rows
            .iter()
            .filter(|row| !retained.iter().any(|retained| retained.owner == row.owner))
            .map(lower_authored_service_reach_row),
    );
}

/// A resolved forwarding's statement index addresses the resolved statement
/// list. Typing erases each machine's proof-output calls before it, so the
/// typed index subtracts them and keeps the resolved coordinate beside it for
/// lexical evidence scope.
pub(crate) fn lower_evidence_forwarding(
    forwarding: &symbol_resolved_trees::statement::EvidenceForwarding,
    proof_output_calls: &[typed_trees::typed_trees::ProofOutputCall],
) -> typed_trees::typed_trees::EvidenceForwarding {
    let erased_before = proof_output_calls
        .iter()
        .filter(|package| {
            package.machine_symbol == forwarding.machine_symbol
                && package.state_symbol == forwarding.state_symbol
                && package.source_statement_index < forwarding.statement_index
        })
        .count();
    typed_trees::typed_trees::EvidenceForwarding {
        machine_symbol: forwarding.machine_symbol,
        state_symbol: forwarding.state_symbol,
        statement_index: forwarding.statement_index.saturating_sub(erased_before),
        source_statement_index: forwarding.statement_index,
        target: crate::lowerer::name::lower_name(&forwarding.target),
        source: crate::lowerer::name::lower_name(&forwarding.source),
        source_conformance: forwarding.source_conformance,
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

pub(crate) fn declaration_exposure(
    is_public: bool,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;

    if is_public {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
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
                evaluated_value: declaration.evaluated_value.clone(),
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
        let typed_machine = self
            .with_type_reference_exposure(Some(machine.interface_exposure), |lowerer| {
                lower_machine(lowerer, machine)
            })?;
        if let Some(view) = lower_token_binding_view(self, machine, &typed_machine)? {
            self.typed_trees.push_machine_token_binding(view);
        }
        self.typed_trees.push_machine(typed_machine);
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

    /// Close the fresh product route. Only this route does what follows, and
    /// each step exists for a reason the continuation routes do not share:
    /// trait requirement guarantees are normalized once, here, because a
    /// continuation cannot introduce a trait requirement; the rebuild drops
    /// derived lookup state and re-seats each evidence forwarding at its typed
    /// coordinate, which no append-only continuation may do to a retained base.
    pub(crate) fn finish(mut self, cohort: &TypingCohort<'_>) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        crate::lowerer::progress::normalize_trait_progress_premises(&mut self.typed_trees)?;
        self.typed_trees =
            rebuild_fresh_typed_trees(std::mem::take(&mut self.typed_trees), self.source_trees);
        close_typed_cohort(&mut self, cohort)?;
        Ok(self.typed_trees)
    }
}

/// Rebuild the typed trees the fresh product route just produced, holding
/// every table and root and re-deriving the storage a stage consumer must not
/// inherit: lookup state derived from those tables, and the build-time
/// deferrals, specializations and placement plans later phases produce.
fn rebuild_fresh_typed_trees(typed: TypedTrees, source: &SymbolResolvedTrees) -> TypedTrees {
    // The retained interner tables travel; restarting their handle space
    // would silently rebind rows that store their IDs.
    let TypedTrees {
        // The tag identifies the instance being taken apart, not any
        // piece of it, so it does not travel with the fields.
        identity: _,
        // Build-context projection follows typing; no declaration view
        // exists in this freshly lowered source program.
        product_declaration_frontier: _,
        excluded_authored_selection_occurrences,
        roots,
        tables,
        symbols,
        service_reaches,
        service_reach_rows,
        authored_service_reach_rows,
        semantic_domains,
        // Applied domain theories are derived after authored typing, alongside
        // specialization; the fresh source entrance regenerates them later.
        applied_domain_applications: _,
        external_bindings,
        plan_laid_layouts: _,
        plan_laid_target_layouts: _,
        plan_laid_schema_fields: _,
        placed_view_plans: _,
        placement_lifecycle_plans: _,
        placement_policy_invocations: _,
        placed_borrow_factories,
        wire_placements: _,
        wire_encode_obligations: _,
        wire_schema_plans: _,
        machine_specializations: _,
        // Capture applications are produced after authored typing.
        captured_machine_applications: _,
        captured_machine_states: _,
        trait_implementation_applications: _,
        // Checking instantiates selector slots after specialization.
        dynamic_slots: _,
        // Family records are produced after all typed bodies settle.
        target_machine_families: _,
        boundary_completion_declarations: _,
        evidence_forwardings: _,
        proof_output_calls,
        // Derived lookup state rebuilds from the retained tables.
        declaration_indexes: _,
        callable_identities: _,
        callable_identity_memo: _,
        ranking_expression_custody,
        // Typing precedes every build-time evaluation deferral, so no
        // pending endpoint mark can exist here to carry forward.
        pending_const_range_endpoints: _,
        constant_recipes: _,
        constant_recipe_closure: _,
        type_identities,
    } = typed;
    let mut trees = TypedTrees::with_roots(roots, tables, symbols);
    trees.type_identities = type_identities;
    trees.placed_borrow_factories = placed_borrow_factories;
    trees.excluded_authored_selection_occurrences = excluded_authored_selection_occurrences;
    // The copied semantic interners survive the rebuild.
    trees.service_reaches = service_reaches;
    trees.service_reach_rows = service_reach_rows;
    trees.authored_service_reach_rows = authored_service_reach_rows;
    trees.semantic_domains = semantic_domains;
    trees.external_bindings = external_bindings;
    trees.evidence_forwardings = source
        .evidence_forwardings
        .iter()
        .map(|forwarding| lower_evidence_forwarding(forwarding, &proof_output_calls))
        .collect();
    trees.proof_output_calls = proof_output_calls;
    trees.ranking_expression_custody = ranking_expression_custody;
    trees
}

/// Receiver types are available only after typing. Publish requirement handles
/// now so direct stage consumers need no compiler-owned repair pass. Later
/// elaboration and specialization must bind any calls they introduce themselves.
fn bind_dynamic_call_targets(trees: &mut TypedTrees) -> Result<(), Diagnostic> {
    validation::resolve_dynamic_call_targets(trees).map_err(|diagnostics| {
        // This stage reports its first failure, like its other typing rules.
        diagnostics.into_iter().next().unwrap_or_else(|| {
            Diagnostic::error("dynamic call target binding failed without a diagnostic")
        })
    })
}
