//! Ownership traversal over the live typed declaration roster.
//!
//! Backing arenas keep stable handles, including discarded or descriptor-only
//! nodes. Semantic passes walk references from current roots, not arena capacity.
//! The same traversal drives executable expression membership and machine
//! dependencies; it neither admits declarations nor supplies checking evidence.

use crate::{
    TypedTrees,
    domain::ProofFact,
    expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
        TableMatchArm, TableNamePath,
    },
    machine::TraitConformance,
    proposition::{PropositionApplication, PropositionBinderArgumentKind, PropositionFormula},
    signature::{AuthoredInvocationTarget, SignatureContract},
    statement::{StatementNode, TransitionTargetNode},
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};
use std::collections::HashSet;
use symbols::SymbolHandle;

/// The caller supplies semantic operand roles; traversal never recognizes an
/// intrinsic from its spelling or grants execution authority to a description.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StaticArgumentRole {
    ValueDependency,
    DeclarationDescription,
}

#[derive(Clone, Copy)]
pub enum StaticArgumentCall<'call> {
    Expression(&'call TableCallExpression),
    Statement(&'call crate::statement::TableCall),
}

impl StaticArgumentCall<'_> {
    pub fn arguments(&self) -> &[StaticMachineArgument] {
        match self {
            Self::Expression(call) => &call.machine_arguments,
            Self::Statement(call) => &call.machine_arguments,
        }
    }
}

pub type StaticArgumentClassifier = fn(&TypedTrees, StaticArgumentCall<'_>) -> StaticArgumentRole;

/// A consumer of exact typed references. Return false from an arena callback
/// when a node was already visited; symbol consumers may preserve repeated edges.
pub trait TypedReferences: Extend<SymbolHandle> {
    fn authored_selection(&mut self, _: crate::AuthoredDeclarationSelectionOccurrenceId) {}
    fn declaration(&mut self, _: SymbolHandle) -> bool {
        true
    }
    fn expression(&mut self, _: ExpressionHandle) -> bool {
        true
    }
    fn statement(&mut self, _: &StatementNode) -> bool {
        true
    }
    fn call_arguments(&mut self, _: &TypedTrees, _: StaticArgumentCall<'_>) -> bool {
        true
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        true
    }
    /// Expressions owned by type work, distinct from executable body roots.
    fn type_expression(&mut self, _: ExpressionHandle) -> bool {
        true
    }
    fn proof_fact(&mut self, _: arena::Handle<ProofFact>) -> bool {
        true
    }
    fn mathematical_type(&mut self, _: crate::mathematical::MathematicalTypeHandle) -> bool {
        true
    }
}

/// Transient owned-handle inventory. Ordering follows backing arena identity for
/// deterministic semantic-domain interning; no arena nodes are copied or erased.
pub struct ProgramReferences {
    pub declarations: Vec<SymbolHandle>,
    pub symbols: HashSet<SymbolHandle>,
    /// Declaration dependencies retained as descriptions, not executable edges.
    pub descriptor_symbols: HashSet<SymbolHandle>,
    pub expressions: HashSet<ExpressionHandle>,
    pub type_references: Vec<TypeReferenceHandle>,
    pub proof_facts: Vec<arena::Handle<ProofFact>>,
    /// Exact ledger identities owned by these roots, not selections inferred
    /// from declaration positions or source spelling. Shared copies share IDs.
    pub authored_selection_occurrences: HashSet<crate::AuthoredDeclarationSelectionOccurrenceId>,
}
#[derive(Default)]
struct ReferenceCollector {
    selection: DeclarationSelection,
    declarations: Vec<SymbolHandle>,
    symbols: HashSet<SymbolHandle>,
    descriptor_symbols: HashSet<SymbolHandle>,
    classify_static_arguments: Option<StaticArgumentClassifier>,
    expressions: HashSet<ExpressionHandle>,
    types: HashSet<TypeReferenceHandle>,
    facts: HashSet<arena::Handle<ProofFact>>,
    mathematical_types: HashSet<crate::mathematical::MathematicalTypeHandle>,
    authored_selection_occurrences: HashSet<crate::AuthoredDeclarationSelectionOccurrenceId>,
}
impl Extend<SymbolHandle> for ReferenceCollector {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.symbols.extend(symbols);
    }
}
impl TypedReferences for ReferenceCollector {
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        if !self.classify_static_arguments.is_some_and(|classify| {
            classify(program, call) == StaticArgumentRole::DeclarationDescription
        }) {
            return true;
        }
        let mut description = ReferenceCollector::default();
        for argument in call.arguments() {
            visit_static_argument(program, &mut description, argument);
        }
        self.descriptor_symbols.extend(description.symbols);
        // Selection provenance remains an obligation even when the operand is
        // not a host type or executable machine dependency.
        self.authored_selection_occurrences
            .extend(description.authored_selection_occurrences);
        false
    }
    fn authored_selection(&mut self, occurrence: crate::AuthoredDeclarationSelectionOccurrenceId) {
        self.authored_selection_occurrences.insert(occurrence);
    }
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        if !self.selection.contains(symbol) {
            return false;
        }
        self.declarations.push(symbol);
        true
    }
    fn expression(&mut self, handle: ExpressionHandle) -> bool {
        self.expressions.insert(handle)
    }
    fn type_reference(&mut self, handle: TypeReferenceHandle) -> bool {
        self.types.insert(handle)
    }
    fn proof_fact(&mut self, handle: arena::Handle<ProofFact>) -> bool {
        self.facts.insert(handle)
    }
    fn mathematical_type(&mut self, handle: crate::mathematical::MathematicalTypeHandle) -> bool {
        self.mathematical_types.insert(handle)
    }
}
pub fn collect_program_references(program: &TypedTrees) -> ProgramReferences {
    collect_selected_program_references(program, &DeclarationSelection::default())
}

/// Exact declaration roots of an operation on shared canonical arenas. Excluded
/// roots remain available for semantic lookup but do not independently own work.
#[derive(Clone)]
pub struct DeclarationSelection {
    excluded: bool,
    symbols: Vec<SymbolHandle>,
}

impl Default for DeclarationSelection {
    fn default() -> Self {
        Self::all()
    }
}

impl DeclarationSelection {
    pub fn all() -> Self {
        Self {
            excluded: true,
            symbols: Vec::new(),
        }
    }

    pub fn contains(&self, symbol: SymbolHandle) -> bool {
        self.symbols
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |symbol| {
                (symbol.arena_index(), symbol.generation())
            })
            .is_ok()
            != self.excluded
    }

    pub fn is_all(&self) -> bool {
        self.excluded && self.symbols.is_empty()
    }

    pub fn from_program(program: &TypedTrees) -> Self {
        Self::from_symbols(collect_program_references(program).declarations)
    }

    /// Select already-owned roots; this set grants no semantic authority.
    pub fn from_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> Self {
        let mut symbols = symbols.into_iter().collect::<Vec<_>>();
        symbols.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        Self {
            excluded: false,
            symbols,
        }
    }

    pub fn intersect(&self, other: &Self) -> Self {
        if other.excluded {
            if self.excluded {
                let mut result = self.clone();
                result.symbols.extend(other.symbols.iter().copied());
                result
                    .symbols
                    .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                result.symbols.dedup();
                return result;
            }
            return other.intersect(self);
        }
        let mut result = other.clone();
        result.symbols.retain(|symbol| self.contains(*symbol));
        result
    }

    pub fn excluding(&self, other: &Self) -> Self {
        if other.excluded {
            let mut complement = other.clone();
            complement.excluded = false;
            return self.intersect(&complement);
        }
        if self.excluded {
            let mut result = self.clone();
            result.symbols.extend(other.symbols.iter().copied());
            result
                .symbols
                .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            result.symbols.dedup();
            result
        } else {
            let mut result = self.clone();
            result.symbols.retain(|symbol| !other.contains(*symbol));
            result
        }
    }

    /// A selected application produced this new declaration in the same arenas.
    pub fn include(&mut self, symbol: SymbolHandle) {
        let position = self
            .symbols
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |symbol| {
                (symbol.arena_index(), symbol.generation())
            });
        match (self.excluded, position) {
            (false, Err(position)) => self.symbols.insert(position, symbol),
            (true, Ok(position)) => {
                self.symbols.remove(position);
            }
            _ => {}
        }
    }
}

pub fn collect_selected_program_references(
    program: &TypedTrees,
    selection: &DeclarationSelection,
) -> ProgramReferences {
    collect_program_references_with_roles(program, selection, None)
}

/// Use the consumer's established operand classification while preserving the
/// same declaration, value, proof and occurrence traversal for both roles.
pub fn collect_program_references_with_roles(
    program: &TypedTrees,
    selection: &DeclarationSelection,
    classify_static_arguments: Option<StaticArgumentClassifier>,
) -> ProgramReferences {
    let mut roots = ReferenceCollector {
        selection: selection.clone(),
        classify_static_arguments,
        ..ReferenceCollector::default()
    };
    visit_program(program, &mut roots);
    let mut type_references = roots.types.into_iter().collect::<Vec<_>>();
    type_references.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    let mut proof_facts = roots.facts.into_iter().collect::<Vec<_>>();
    proof_facts.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    ProgramReferences {
        declarations: roots.declarations,
        symbols: roots.symbols,
        descriptor_symbols: roots.descriptor_symbols,
        expressions: roots.expressions,
        type_references,
        proof_facts,
        authored_selection_occurrences: roots.authored_selection_occurrences,
    }
}

fn visit_program(program: &TypedTrees, roots: &mut impl TypedReferences) {
    visit_declarations(program, roots);
    for machine in program.machines() {
        if roots.declaration(machine.symbol) {
            visit_machine(program, roots, machine);
        }
    }
}

/// Current proof facts, preserving their exact handles for checked evidence joins.
pub fn program_proof_facts(
    program: &TypedTrees,
) -> impl Iterator<Item = (arena::Handle<ProofFact>, &ProofFact)> {
    let mut roots = ProofFactCollector::default();
    visit_program(program, &mut roots);
    let mut facts = roots.facts.into_iter().collect::<Vec<_>>();
    facts.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    facts
        .into_iter()
        .map(move |handle| (handle, program.proof_facts.get(handle)))
}

#[derive(Default)]
struct ProofFactCollector {
    facts: HashSet<arena::Handle<ProofFact>>,
}
impl Extend<SymbolHandle> for ProofFactCollector {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}
impl TypedReferences for ProofFactCollector {
    fn statement(&mut self, _: &StatementNode) -> bool {
        false
    }
    fn expression(&mut self, _: ExpressionHandle) -> bool {
        false
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        false
    }
    fn mathematical_type(&mut self, _: crate::mathematical::MathematicalTypeHandle) -> bool {
        false
    }
    fn proof_fact(&mut self, handle: arena::Handle<ProofFact>) -> bool {
        self.facts.insert(handle);
        false
    }
}

fn visit_proof_facts(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    facts: arena::HandleSpan<ProofFact>,
) {
    for (offset, fact) in program.proof_facts.span_or_empty(facts).iter().enumerate() {
        let handle = arena::Handle::from_parts(
            facts.start().arena_index() + offset as u32,
            facts.start().generation(),
        );
        if retained.proof_fact(handle) {
            visit_proof_fact(program, retained, fact);
        }
    }
}

/// Report exact symbol identity; the sink decides which symbol kinds it needs.
fn visit_symbol(retained: &mut impl TypedReferences, symbol: SymbolHandle) {
    retained.extend([symbol]);
}

/// Substitution removes a constant's Name node, not its declaration dependency.
/// Every consumer of an owned occurrence still needs the declaration for value
/// origins and independent initializer replay. Other retained targets are not
/// executable edges: describing a Product machine cannot grant its body Build
/// execution. Those references retain their ordinary operation-owned traversal.
fn visit_authored_selection(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    occurrence: crate::AuthoredDeclarationSelectionOccurrenceId,
) {
    retained.authored_selection(occurrence);
    if let Some(selection) = program.authored_declaration_selections().get(occurrence)
        && let crate::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target()
        && program.symbols.get(target.selected_symbol()).kind == symbols::SymbolKind::Const
    {
        visit_symbol(retained, target.selected_symbol());
    }
}

/// Retain every machine referenced by a `StaticMachineArgument`: its selected
/// entry-state symbol plus any nested application arguments.
pub fn visit_static_argument(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    argument: &StaticMachineArgument,
) {
    visit_symbol(retained, argument.symbol);
    visit_type_reference(program, retained, argument.type_reference);
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            visit_static_argument(program, retained, nested);
        }
    }
}

/// Retain every machine named inside one closed conformance application:
/// machine arguments, static argument applications, and row realizations.
pub fn visit_closed_conformance(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &crate::typed_trees::ClosedConformanceApplication,
) {
    for argument in &application.const_arguments {
        use crate::typed_trees::ClosedConformanceConstArgument;
        let (parameter, argument) = match argument {
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier,
                ..
            } => (*parameter_carrier, *declared_carrier),
            ClosedConformanceConstArgument::CallerBinder {
                parameter_carrier,
                binder_carrier,
                ..
            } => (*parameter_carrier, *binder_carrier),
        };
        visit_type_reference(program, retained, parameter);
        visit_type_reference(program, retained, argument);
    }
    for machine in application.machine_arguments.iter() {
        visit_symbol(retained, *machine);
    }
    for argument in application.arguments.iter() {
        visit_static_argument(program, retained, argument);
    }
    for row in &application.rows {
        visit_symbol(retained, row.realization_machine);
        visit_symbol(retained, row.realization_state);
    }
}

/// References owned by retained non-machine declarations: data fields and
/// `where` facts, trait requirement signatures, conformance carriers,
/// proposition binders/bodies, measure bodies, const initializers, operator
/// signatures, domain facts, and wire field types.
pub fn visit_declarations(program: &TypedTrees, retained: &mut impl TypedReferences) {
    for data in program.data_definitions() {
        if !retained.declaration(data.symbol) {
            continue;
        }
        visit_symbol(retained, data.symbol);
        for plan in program
            .plan_laid_layouts
            .iter()
            .filter(|plan| plan.data_symbol == data.symbol)
        {
            visit_symbol(retained, plan.schema_symbol);
            visit_symbol(retained, plan.policy_symbol);
            visit_symbol(retained, plan.policy_plan_machine_symbol);
            for demand in &plan.private_callback_demands {
                visit_closed_conformance(program, retained, &demand.slot_application);
            }
        }
        for plan in program
            .placed_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == data.symbol)
        {
            visit_symbol(retained, plan.schema_symbol);
            visit_symbol(retained, plan.policy_symbol);
            visit_symbol(retained, plan.policy_plan_machine_symbol);
            for field in &plan.fields {
                visit_type_reference(program, retained, field.accessor_type);
                visit_type_reference(program, retained, field.value_type);
                if let Some(resident) = &field.atomic_resident {
                    visit_type_reference(program, retained, resident.resident_type);
                }
            }
        }
        visit_type_parameters(program, retained, data.type_parameters);
        if let Some(instance) = data.generic_instance {
            visit_type_reference(program, retained, instance);
        }
        if let Some(quotient) = &data.quotient {
            visit_type_reference(program, retained, quotient.carrier);
            visit_symbol(retained, quotient.relation_symbol);
        }
        visit_proof_facts(program, retained, data.where_facts);
        for member in program.data_members(data) {
            match member {
                crate::data::DataMember::Field(field) => {
                    visit_type_reference(program, retained, field.type_reference);
                }
                crate::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        visit_type_reference(program, retained, field.type_reference);
                    }
                    visit_proof_facts(program, retained, variant.where_facts);
                }
            }
        }
    }

    for definition in program.traits() {
        if !retained.declaration(definition.symbol) {
            continue;
        }
        visit_symbol(retained, definition.symbol);
        visit_type_parameters(program, retained, definition.type_parameters);
        for requirement in program
            .trait_requirements(definition)
            .iter()
            .chain(definition.refines.iter())
        {
            visit_symbol(retained, requirement.symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(requirement.arguments)
            {
                visit_type_reference(program, retained, *argument);
            }
        }
        for refinement in &definition.refinement_clauses {
            visit_signature(program, retained, &refinement.signature);
        }
        for bound in &definition.conformance_bounds {
            for argument in &bound.arguments {
                visit_type_reference(program, retained, *argument);
            }
        }
        for signature in program.trait_machine_signatures(definition) {
            visit_signature(program, retained, signature);
        }
    }

    for conformance in program.conformances() {
        if !retained.declaration(conformance.symbol) {
            continue;
        }
        visit_symbol(retained, conformance.symbol);
        visit_symbol(retained, conformance.carrier_symbol);
        visit_type_parameters(program, retained, conformance.type_parameters);
        visit_symbol(retained, conformance.trait_symbol);
        if let crate::trait_definition::ConformanceImplementation::Closed { rows } =
            &conformance.implementation
        {
            for row in rows {
                visit_symbol(retained, row.declaring_trait);
                visit_symbol(retained, row.requirement);
                visit_symbol(retained, row.realization_machine);
                visit_symbol(retained, row.realization_state);
            }
        }
        for argument in program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        {
            visit_type_reference(program, retained, *argument);
        }
    }

    for proposition in program.propositions() {
        if !retained.declaration(proposition.symbol) {
            continue;
        }
        visit_symbol(retained, proposition.symbol);
        for binder in program.proposition_binders(proposition) {
            visit_symbol(retained, binder.symbol);
            if let crate::proposition::PropositionBinderKind::Const { type_reference } = binder.kind
            {
                visit_type_reference(program, retained, type_reference);
            }
        }
        for parameter in program.proposition_parameters(proposition) {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        match &proposition.body {
            crate::proposition::PropositionBody::Witness { evidence } => {
                visit_type_reference(program, retained, *evidence);
            }
            crate::proposition::PropositionBody::Transparent { proposition } => {
                visit_proposition_formula(program, retained, proposition);
            }
            crate::proposition::PropositionBody::Primitive => {}
        }
    }

    for definition in program.mathematical_definitions() {
        if !retained.declaration(definition.symbol) {
            continue;
        }
        visit_type_parameters(program, retained, definition.binders);
        for parameter in program.mathematical_parameters(definition.parameters) {
            visit_mathematical_type(program, retained, parameter.ty);
        }
        visit_mathematical_type(program, retained, definition.result);
        if let crate::mathematical::MathematicalBody::Definition(expression) = definition.body {
            visit_expression(program, retained, expression);
        }
    }

    for measure in program.measures() {
        if !retained.declaration(measure.symbol) {
            continue;
        }
        visit_symbol(retained, measure.symbol);
        if let Some(parameter) = &measure.parameter {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, measure.return_type);
        for expression in program.expression_table.expression_handles(measure.body) {
            visit_expression(program, retained, *expression);
        }
    }

    for declaration in program.const_declarations() {
        if !retained.declaration(declaration.symbol) {
            continue;
        }
        visit_symbol(retained, declaration.symbol);
        visit_type_reference(program, retained, declaration.declared_type);
        visit_expression(program, retained, declaration.authored_initializer);
        visit_expression(program, retained, declaration.materialized_initializer);
    }

    for operator in program
        .operators()
        .iter()
        .chain(program.machine_token_bindings())
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
    {
        if !retained.declaration(operator.symbol) {
            continue;
        }
        visit_symbol(retained, operator.symbol);
        visit_type_parameters(program, retained, operator.type_parameters);
        for parameter in program
            .tables
            .state_parameters
            .span_or_empty(operator.parameters)
        {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, operator.return_type);
        for contract in program
            .tables
            .signature_contracts
            .span_or_empty(operator.contracts)
        {
            visit_contract(program, retained, contract);
        }
    }

    for domain in program.domain_definitions() {
        if !retained.declaration(domain.symbol) {
            continue;
        }
        visit_symbol(retained, domain.symbol);
        visit_type_parameters(program, retained, domain.type_parameters);
        visit_type_reference(program, retained, domain.target_type);
        for argument in &domain.index_arguments {
            visit_type_reference(program, retained, *argument);
        }
        visit_proof_facts(program, retained, domain.facts);
    }

    for schema in program.wire_schemas() {
        if !retained.declaration(schema.symbol) {
            continue;
        }
        visit_symbol(retained, schema.symbol);
        for member in program.tables.wire_members.span_or_empty(schema.members) {
            if let crate::wire::WireMember::Field(field) = member {
                visit_type_reference(program, retained, field.type_reference);
            }
        }
    }
}

/// Retain machines referenced by one proof fact: embedded expressions,
/// membership values and domain arguments, or proposition applications.
fn visit_proof_fact(program: &TypedTrees, retained: &mut impl TypedReferences, fact: &ProofFact) {
    match fact {
        ProofFact::Expression(expression) => {
            visit_expression(program, retained, *expression);
        }
        ProofFact::Membership(membership) => {
            if let Some(occurrence) = membership.authored_domain_selection {
                visit_authored_selection(program, retained, occurrence);
            }
            visit_expression(program, retained, membership.value);
            visit_symbol(retained, membership.domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments)
            {
                visit_type_reference(program, retained, *argument);
            }
        }
        ProofFact::Proposition(application) => {
            visit_proposition_application(program, retained, application);
        }
    }
}

/// Retain machines named by one proposition application: machine-kind binder
/// arguments plus every expression argument.
fn visit_proposition_application(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &PropositionApplication,
) {
    visit_symbol(retained, application.proposition);
    for binder in application.binder_arguments.iter() {
        if binder.kind == PropositionBinderArgumentKind::Machine {
            visit_symbol(retained, binder.symbol);
        }
    }
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        visit_expression(program, retained, *argument);
    }
}

/// Retain machines named by one proposition formula body.
fn visit_proposition_formula(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    formula: &PropositionFormula,
) {
    match formula {
        PropositionFormula::Application(application) => {
            visit_proposition_application(program, retained, application);
        }
        PropositionFormula::BooleanExpression(expression) => {
            visit_expression(program, retained, *expression);
        }
    }
}

/// Retain machines referenced by one state signature or operator signature:
/// parameter types, return type, invokes targets, and contract facts.
fn visit_signature(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    signature: &crate::signature::StateSignature,
) {
    visit_type_parameters(program, retained, signature.type_parameters);
    visit_proof_facts(program, retained, signature.where_facts);
    for parameter in program
        .tables
        .state_parameters
        .span_or_empty(signature.parameters)
    {
        visit_type_reference(program, retained, parameter.type_reference);
    }
    visit_type_reference(program, retained, signature.return_type);
    for invocation in program
        .tables
        .signature_invokes
        .span_or_empty(signature.invokes)
    {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            visit_symbol(retained, target);
        }
    }
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        visit_contract(program, retained, contract);
    }
}

/// Retain machines referenced by one signature contract's proof facts.
fn visit_contract(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    contract: &SignatureContract,
) {
    visit_proof_facts(program, retained, contract.facts);
}

/// Retain every machine symbol appearing in one type reference, including
/// const-expression index payloads that can call machines.
pub fn visit_type_reference(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    type_reference: TypeReferenceHandle,
) {
    if !type_reference.is_valid() || !retained.type_reference(type_reference) {
        return;
    }
    for origin in program
        .type_reference_table
        .constant_origins(type_reference)
        .into_iter()
        .flatten()
    {
        visit_symbol(retained, origin.declaration);
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            visit_type_reference(program, retained, *referee);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            visit_type_reference(program, retained, *base_type);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    crate::types::TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        if retained.type_expression(*minimum) {
                            visit_expression(program, retained, *minimum);
                        }
                        if retained.type_expression(*maximum) {
                            visit_expression(program, retained, *maximum);
                        }
                    }
                    crate::types::TypeConstraintNode::Domain(domain) => {
                        visit_symbol(retained, domain.symbol);
                        for argument in &domain.arguments {
                            visit_type_reference(program, retained, *argument);
                        }
                    }
                    crate::types::TypeConstraintNode::Named(_)
                    | crate::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            visit_type_reference(program, retained, *element_type);
            match length {
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    visit_symbol(retained, *symbol);
                }
                FixedArrayLength::ConstCall { machine, .. } => visit_symbol(retained, *machine),
                FixedArrayLength::Literal(_) => {}
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            visit_type_reference(program, retained, *element_type);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            visit_symbol(retained, *base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                visit_type_reference(program, retained, *argument);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if retained.type_expression(*expression) {
                visit_expression(program, retained, *expression);
            }
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            visit_symbol(retained, *symbol);
            if let Some(conformance) = conformance {
                visit_symbol(retained, *conformance);
            }
        }
        TypeReferenceNode::Named { symbol, .. } => {
            visit_symbol(retained, *symbol);
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Retain every machine referenced by one typed expression node, recursively.
pub fn visit_expression(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() || !retained.expression(expression) {
        return;
    }
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        visit_authored_selection(program, retained, occurrence);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(match_expression) => {
            visit_expression(program, retained, match_expression.subject);
            for arm in program.expression_table.match_arms(match_expression.arms) {
                visit_match_arm(program, retained, arm);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit_expression(program, retained, *element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            visit_expression(program, retained, atomic.value);
            visit_expression(program, retained, atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            visit_expression(program, retained, binary.left);
            visit_expression(program, retained, binary.right);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::Cast(cast) => {
            visit_expression(program, retained, cast.value);
            visit_type_reference(program, retained, cast.target_type);
            visit_type_reference(program, retained, cast.result_type);
            visit_symbol(retained, cast.semantic_domain_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                visit_type_reference(program, retained, *argument);
            }
        }
        ExpressionNode::Call(call) => {
            visit_call_expression(program, retained, call);
        }
        ExpressionNode::Indexed(indexed) => {
            visit_expression(program, retained, indexed.collection);
            visit_expression(program, retained, indexed.index);
        }
        ExpressionNode::Member(member) => {
            visit_expression(program, retained, member.receiver);
            visit_symbol(retained, member.member_symbol);
        }
        ExpressionNode::Borrow(borrow) => {
            visit_expression(program, retained, borrow.target);
        }
        ExpressionNode::Name(path) => {
            visit_name_path(program, retained, path);
        }
        ExpressionNode::Range(range) => {
            visit_expression(program, retained, range.start);
            visit_expression(program, retained, range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            visit_symbol(retained, literal.type_symbol);
            if let Some(case_symbol) = literal.case_symbol {
                visit_symbol(retained, case_symbol);
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                visit_expression(program, retained, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            visit_expression(program, retained, unary.operand);
        }
        ExpressionNode::ZeroValue(type_reference) => {
            visit_type_reference(program, retained, *type_reference);
        }
    }
}

/// Retain machines referenced by one match arm pattern and value.
fn visit_match_arm(program: &TypedTrees, retained: &mut impl TypedReferences, arm: &TableMatchArm) {
    if let crate::expression::MatchPattern::Value(pattern) = &arm.pattern {
        visit_expression(program, retained, *pattern);
    }
    visit_expression(program, retained, arm.value);
}

/// Retain machines named by a resolved name path (head, member, and final
/// symbols can all resolve to machines).
fn visit_name_path(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    path: &TableNamePath,
) {
    visit_symbol(retained, path.head_symbol);
    visit_symbol(retained, path.symbol);
    for member in program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    {
        visit_symbol(retained, *member);
    }
}

/// Retain the call target and every static argument of one typed call
/// expression, including static requirement dispatch and sealed-operation
/// applications.
fn visit_call_expression(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    call: &TableCallExpression,
) {
    visit_symbol(retained, call.target_symbol);
    // Named operators can retain their complete call path before an exact
    // target is bound. Their declarations are dependencies even in contracts,
    // which have no executable value-origin row. Keep the canonical candidate
    // set; this traversal neither chooses an overload nor admits the call.
    for operator in crate::operator::named_expression_call_candidates(program, call) {
        visit_symbol(retained, operator.symbol);
    }
    visit_symbol(retained, call.static_machine_parameter);
    visit_expression(program, retained, call.receiver);
    if let Some(dispatch) = &call.static_requirement_dispatch {
        visit_symbol(retained, dispatch.realization_machine);
        visit_symbol(retained, dispatch.realization_state);
        visit_symbol(retained, dispatch.requirement);
        visit_symbol(retained, dispatch.declaring_trait);
    }
    if retained.call_arguments(program, StaticArgumentCall::Expression(call)) {
        for argument in call.machine_arguments.iter() {
            visit_static_argument(program, retained, argument);
        }
    }
    if let Some(quotient) = &call.quotient_operation {
        visit_static_argument(program, retained, &quotient.representative_operation);
        for theorem in quotient.theorem_evidence.iter() {
            visit_static_argument(program, retained, &theorem.application);
        }
    }
    if let Some(query) = &call.layout_query {
        visit_static_argument(program, retained, &query.operand);
    }
    if let Some(layout) = &call.private_layout_operation {
        visit_static_argument(program, retained, &layout.selected_slot);
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        visit_expression(program, retained, *argument);
    }
}

/// Retain `via` expressions on machine satisfies rows as call edges.
fn visit_conformance(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    conformance: &TraitConformance,
) {
    visit_expression(program, retained, conformance.via_expression);
    for argument in program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    {
        visit_type_reference(program, retained, *argument);
    }
}

/// Symbol/type edges and arena-owned expression roots of one statement.
fn visit_statement(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    statement: &StatementNode,
) {
    if !retained.statement(statement) {
        return;
    }
    for (_, expression) in program.statement_table.expression_roots(statement) {
        visit_expression(program, retained, expression);
    }
    match statement {
        StatementNode::Call(call) => {
            if let Some(occurrence) = call.authored_call_selection {
                visit_authored_selection(program, retained, occurrence);
            }
            visit_symbol(retained, call.target_symbol);
            for operator in crate::operator::named_statement_call_candidates(program, call) {
                visit_symbol(retained, operator.symbol);
            }
            visit_symbol(retained, call.static_machine_parameter);
            if let Some(dispatch) = &call.static_requirement_dispatch {
                visit_symbol(retained, dispatch.realization_machine);
                visit_symbol(retained, dispatch.realization_state);
            }
            if retained.call_arguments(program, StaticArgumentCall::Statement(call)) {
                for argument in call.machine_arguments.iter() {
                    visit_static_argument(program, retained, argument);
                }
            }
        }
        StatementNode::LocalData(local) => {
            visit_type_reference(program, retained, local.type_reference);
        }
        StatementNode::Transition(transition) => {
            for target in [transition.target, transition.continuation] {
                if !program.statement_table.transition_target_is_valid(target) {
                    continue;
                }
                if let TransitionTargetNode::Named {
                    path,
                    authored_call_selection,
                    ..
                } = program.statement_table.transition_target(target)
                {
                    if let Some(occurrence) = authored_call_selection {
                        visit_authored_selection(program, retained, *occurrence);
                    }
                    visit_symbol(retained, path.symbol);
                    visit_symbol(retained, path.head_symbol);
                }
            }
        }
        StatementNode::RootBinding(_)
        | StatementNode::AssemblyFact(_)
        | StatementNode::Assignment(_)
        | StatementNode::Expression(_) => {}
    }
}

fn visit_type_parameters(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    parameters: arena::HandleSpan<crate::data::TypeParameter>,
) {
    use crate::data::TypeParameterKind;
    for parameter in program.data_type_parameters.span_or_empty(parameters) {
        match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                visit_type_reference(program, retained, *type_reference)
            }
            TypeParameterKind::Machine { contract } => {
                if let Some(signature) = contract.structural() {
                    visit_signature(program, retained, signature);
                }
            }
            TypeParameterKind::Proposition { contract } => {
                for parameter in program
                    .tables
                    .state_parameters
                    .span_or_empty(contract.parameters)
                {
                    visit_type_reference(program, retained, parameter.type_reference);
                }
            }
            TypeParameterKind::Type => {}
        }
    }
}

pub fn visit_machine(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    machine: &crate::machine::Machine,
) {
    for specialization in program
        .machine_specializations
        .iter()
        .filter(|row| row.instance == machine.symbol)
    {
        visit_type_parameters(program, retained, specialization.template_parameters);
        for realization in &specialization.operator_realizations {
            for argument in &realization.arguments {
                use crate::operator::ClosedOperatorApplicationArgument;
                let reference = match argument {
                    ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                        *type_reference
                    }
                    ClosedOperatorApplicationArgument::Const {
                        declared_carrier, ..
                    } => *declared_carrier,
                };
                visit_type_reference(program, retained, reference);
            }
        }
        for application in &specialization.conformance_applications {
            visit_closed_conformance(program, retained, application);
        }
    }
    // Exact typed ranking-witness expressions are compiler custody for
    // termination evidence; they are machine-keyed rows so retention here is
    // conservative (reachable machines also produce their own edges).
    for custody in program
        .ranking_expression_custody
        .iter()
        .filter(|row| row.machine == machine.symbol)
    {
        for subject in &custody.subjects {
            visit_expression(program, retained, *subject);
        }
        for argument in &custody.view_arguments {
            visit_expression(program, retained, *argument);
        }
        if let Some(rank_range) = custody.rank_range {
            visit_expression(program, retained, rank_range);
        }
    }
    for output in program
        .proof_output_calls
        .iter()
        .filter(|row| row.machine_symbol == machine.symbol)
    {
        visit_expression(program, retained, output.call);
    }
    let from = machine.symbol;
    visit_type_parameters(program, retained, machine.type_parameters);
    visit_type_reference(program, retained, machine.attached_data_application);
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(machine.contracts)
    {
        visit_contract(program, retained, contract);
    }

    for invocation in program.machine_invokes(machine) {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            visit_symbol(retained, target);
        }
    }

    for row in program
        .authored_service_reach_rows
        .iter()
        .filter(|row| row.owner == from)
    {
        for target in &row.targets {
            visit_symbol(retained, target.service);
        }
    }

    for conformance in program.declared_machine_trait_conformances(machine) {
        visit_symbol(retained, conformance.requirement_symbol);
        visit_conformance(program, retained, conformance);
    }

    for owned in program.machine_owned_data(machine) {
        visit_type_reference(program, retained, owned.type_reference);
        visit_expression(program, retained, owned.initial_value);
    }

    for bound in &machine.conformance_bounds {
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, state.return_type);
        for contract in program.state_contracts(state) {
            visit_proof_facts(program, retained, contract.facts);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            visit_statement(program, retained, statement);
        }
    }
}

fn visit_mathematical_type(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    handle: crate::mathematical::MathematicalTypeHandle,
) {
    use crate::mathematical::MathematicalType;
    if !handle.is_valid() || !retained.mathematical_type(handle) {
        return;
    }
    match program.mathematical_type(handle) {
        MathematicalType::Ordinary(reference) => {
            visit_type_reference(program, retained, *reference)
        }
        MathematicalType::Arrow {
            domain, codomain, ..
        } => {
            visit_mathematical_type(program, retained, *domain);
            visit_mathematical_type(program, retained, *codomain);
        }
        MathematicalType::Application { callee, arguments } => {
            visit_mathematical_type(program, retained, *callee);
            for argument in program.expression_table.expression_handles(*arguments) {
                visit_expression(program, retained, *argument);
            }
        }
    }
}
