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
    /// The occurrence owns the call's selection ledger independently of the
    /// static application and its eventual executable target.
    Expression(ExpressionHandle, &'call TableCallExpression),
    Statement(&'call crate::statement::TableCall),
}

impl<'call> StaticArgumentCall<'call> {
    /// The exact static map selection is independent of a method invocation's
    /// own type/const/machine operands. A qualifier can supply the former even
    /// when the latter is empty, and specialization must preserve both.
    pub fn selection_application(
        &self,
    ) -> crate::static_applications::StaticMachineApplicationHandle {
        match self {
            Self::Expression(_, call) => call.static_requirement_dispatch.as_ref(),
            Self::Statement(call) => call.static_requirement_dispatch.as_ref(),
        }
        .map(|dispatch| dispatch.selection_application)
        .unwrap_or_default()
    }

    /// Retained source operands, or zero while no application has been recorded.
    pub fn application(&self) -> crate::static_applications::StaticMachineApplicationHandle {
        match self {
            Self::Expression(_, call) => call.authored_machine_application,
            Self::Statement(call) => call.authored_machine_application,
        }
    }

    /// Original declaration identity survives retargeting to an executable clone.
    pub fn registration_operation(&self, program: &TypedTrees) -> SymbolHandle {
        let application = self.application();
        if application.is_valid() {
            return program
                .expression_table
                .static_machine_application(application)
                .registration_operation;
        }
        match self {
            Self::Expression(_, call) => call.target_symbol,
            Self::Statement(call) => call.target_symbol,
        }
    }

    /// Source scope is retained independently of a folded type or live call.
    pub fn lexical_owner(&self, program: &TypedTrees) -> SymbolHandle {
        let application = self.application();
        if application.is_valid() {
            program
                .expression_table
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        }
    }

    /// Source operands survive executable specialization. The live static
    /// list is used only while no source application has been retained.
    pub fn arguments(&self, program: &'call TypedTrees) -> &'call [StaticMachineArgument] {
        let application = self.application();
        if application.is_valid() {
            return program
                .expression_table
                .static_machine_application_arguments(application);
        }
        match self {
            Self::Expression(_, call) => &call.machine_arguments,
            Self::Statement(call) => &call.machine_arguments,
        }
    }
}

pub type StaticArgumentClassifier = fn(&TypedTrees, StaticArgumentCall<'_>) -> StaticArgumentRole;

/// A consumer of exact typed references. Return false from an arena callback
/// when a node was already visited; symbol consumers may preserve repeated edges.
pub trait TypedReferences: Extend<SymbolHandle> {
    /// Nested callable telescopes own their fresh binders independently of
    /// the surrounding declaration. Ordinary dependency sinks need no scope.
    fn enter_lexical_scope(&mut self, _: SymbolHandle) {}
    fn leave_lexical_scope(&mut self) {}
    fn authored_selection(&mut self, _: crate::AuthoredDeclarationSelectionOccurrenceId) {}
    /// Source scope is declaration custody, not permission to execute its body.
    fn declaration_description_dependency(&mut self, _: SymbolHandle) {}
    /// Exact member body retained for a materialized target layout, distinct
    /// from an authored use of that member in portable source.
    fn layout_realization_dependency(&mut self, symbol: SymbolHandle) {
        self.extend([symbol]);
    }
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
    fn declaration_description_dependency(&mut self, symbol: SymbolHandle) {
        self.descriptor_symbols.insert(symbol);
    }
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        if !self.classify_static_arguments.is_some_and(|classify| {
            classify(program, call) == StaticArgumentRole::DeclarationDescription
        }) {
            return true;
        }
        let mut description = ReferenceCollector::default();
        for argument in call.arguments(program) {
            visit_static_argument(program, &mut description, argument);
        }
        visit_static_machine_bindings(program, &mut description, call.application());
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

/// A live type occurrence, not ownership attached to a shared arena node.
/// The same handle can occur beneath distinct supplying declarations.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TypeOccurrence {
    pub lexical_owner: SymbolHandle,
    pub reference: TypeReferenceHandle,
}

/// Source application discovery uses the same declaration/body traversal as
/// dependency collection. Arena capacity also contains discarded descriptors;
/// scanning it would invent application roots with no supplying declaration.
/// This inventory grants neither reachability nor execution authority.
pub fn collect_program_type_occurrences(program: &TypedTrees) -> Vec<TypeOccurrence> {
    let mut occurrences = TypeOccurrences::default();
    visit_program(program, &mut occurrences);
    occurrences.types
}

/// Exact authored call applications reached from retained declaration/body
/// roots. This inventory creates neither execution reach nor admission.
pub fn collect_program_conformance_occurrences(
    program: &TypedTrees,
) -> Vec<crate::static_applications::StaticMachineApplicationHandle> {
    let mut occurrences = TypeOccurrences::default();
    visit_program(program, &mut occurrences);
    program
        .expression_table
        .static_machine_applications()
        .filter_map(|(handle, _)| occurrences.applications.contains(&handle).then_some(handle))
        .collect()
}

#[derive(Default)]
struct TypeOccurrences {
    owner: SymbolHandle,
    outer_owners: Vec<SymbolHandle>,
    types: Vec<TypeOccurrence>,
    applications: HashSet<crate::static_applications::StaticMachineApplicationHandle>,
    seen_types: Vec<Vec<(SymbolHandle, TypeReferenceHandle)>>,
    seen_expressions: Vec<Vec<(SymbolHandle, ExpressionHandle)>>,
    seen_mathematical_types: Vec<Vec<(SymbolHandle, crate::mathematical::MathematicalTypeHandle)>>,
}

/// Index shared nodes by arena slot, retaining generation and supplying scope.
/// Searching the whole occurrence roster at every child would turn ordinary
/// source discovery into quadratic work on the complete library graph.
fn retain_scoped_occurrence<Element>(
    seen: &mut Vec<Vec<(SymbolHandle, arena::Handle<Element>)>>,
    owner: SymbolHandle,
    reference: arena::Handle<Element>,
) -> bool {
    let slot = reference.arena_index() as usize;
    if seen.len() <= slot {
        seen.resize_with(slot + 1, Vec::new);
    }
    let occurrence = (owner, reference);
    if seen[slot].contains(&occurrence) {
        return false;
    }
    seen[slot].push(occurrence);
    true
}

impl Extend<SymbolHandle> for TypeOccurrences {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}

impl TypedReferences for TypeOccurrences {
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        for handle in [call.application(), call.selection_application()] {
            if handle.is_valid() {
                let source = program.expression_table.static_machine_application(handle);
                if source.lexical_owner == self.owner && source.conformance_selection.is_some() {
                    self.applications.insert(handle);
                }
            }
        }
        true
    }

    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.owner = symbol;
        true
    }

    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.outer_owners.push(self.owner);
        if owner.is_valid() {
            self.owner = owner;
        }
    }

    fn leave_lexical_scope(&mut self) {
        if let Some(owner) = self.outer_owners.pop() {
            self.owner = owner;
        }
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        let occurrence = TypeOccurrence {
            lexical_owner: self.owner,
            reference,
        };
        if !retain_scoped_occurrence(&mut self.seen_types, self.owner, reference) {
            return false;
        }
        self.types.push(occurrence);
        true
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        retain_scoped_occurrence(&mut self.seen_expressions, self.owner, expression)
    }

    fn mathematical_type(
        &mut self,
        reference: crate::mathematical::MathematicalTypeHandle,
    ) -> bool {
        retain_scoped_occurrence(&mut self.seen_mathematical_types, self.owner, reference)
    }
}

/// Enumerate declaration roots without traversing their expressions, types or
/// proofs. Choosing a work frontier does not require analyzing excluded bodies.
pub fn collect_program_declarations(program: &TypedTrees) -> Vec<SymbolHandle> {
    struct Declarations(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for Declarations {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for Declarations {
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.0.push(symbol);
            false
        }
    }
    let mut declarations = Declarations(Vec::new());
    visit_program(program, &mut declarations);
    declarations.0
}

/// Exact declaration roots of an operation on shared canonical arenas. Excluded
/// roots remain available for semantic lookup but do not independently own work.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        Self::from_symbols(collect_program_declarations(program))
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
        let handle = facts.handle_at(offset as u32);
        if retained.proof_fact(handle) {
            visit_proof_fact(program, retained, fact);
        }
    }
}

/// Report exact symbol identity; the sink decides which symbol kinds it needs.
fn visit_symbol(retained: &mut impl TypedReferences, symbol: SymbolHandle) {
    retained.extend([symbol]);
}

/// A closed requirement is a generated root, not a child of its source trait.
/// Its receipt supplies the original declaration and complete actuals; those
/// remain dependencies even after executable retargeting erased static syntax.
/// The application's supplying scope is descriptive custody only.
fn visit_callable_symbol(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    symbol: SymbolHandle,
) {
    visit_symbol(retained, symbol);
    let Some(application) = program.closed_requirement_application(symbol) else {
        return;
    };
    visit_symbol(retained, application.declaring_trait);
    visit_symbol(retained, application.template);
    application.trait_scope.visit_references(program, retained);
    for reference in application
        .trait_arguments
        .iter()
        .chain(application.local_arguments)
    {
        visit_type_reference(program, retained, *reference);
    }
    let source = program
        .expression_table
        .static_machine_application(application.application);
    visit_symbol(retained, source.binding_declaration);
    retained.declaration_description_dependency(source.lexical_owner);
    visit_static_machine_bindings(program, retained, application.application);
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
    visit_callable_symbol(program, retained, argument.symbol);
    visit_type_reference(program, retained, argument.type_reference);
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            visit_static_argument(program, retained, nested);
        }
    }
}

/// Selected entries and inferred operands have the same dependency law as
/// authored operands. The original binder itself is a declaration description,
/// not executable code. The sink's call classification still decides whether
/// an occurrence describes a declaration or actually depends on its value.
pub fn visit_static_machine_bindings(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: crate::static_applications::StaticMachineApplicationHandle,
) {
    visit_static_machine_bindings_inner(program, retained, application, &mut HashSet::new());
}

fn visit_static_machine_bindings_inner(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: crate::static_applications::StaticMachineApplicationHandle,
    visited: &mut HashSet<crate::static_applications::StaticMachineApplicationHandle>,
) {
    if !application.is_valid() || !visited.insert(application) {
        return;
    }
    if let Some(selection) = &program
        .expression_table
        .static_machine_application(application)
        .conformance_selection
        && let Some(selected) = selection.application.as_deref()
    {
        visit_closed_conformance_inner(program, retained, selected, visited);
    }
    for applied in program.applied_machine_callables(application) {
        visit_static_machine_bindings_inner(program, retained, applied.provider, visited);
        visit_signature(program, retained, applied.required);
        visit_signature(program, retained, applied.selected);
    }
    for selected in program
        .expression_table
        .static_machine_application_selected_machines(application)
    {
        visit_static_argument(program, retained, selected);
    }
    for binding in program
        .expression_table
        .static_machine_application_bindings(application)
    {
        use crate::static_applications::StaticMachineBindingValue;
        match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => {
                visit_type_reference(program, retained, *reference)
            }
            StaticMachineBindingValue::RuntimeValue {
                type_reference,
                subject,
            } => {
                visit_type_reference(program, retained, *type_reference);
                visit_symbol(retained, *subject);
            }
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                visit_static_argument(program, retained, argument)
            }
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
    visit_closed_conformance_inner(program, retained, application, &mut HashSet::new());
}

fn visit_closed_conformance_inner(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &crate::typed_trees::ClosedConformanceApplication,
    visited: &mut HashSet<crate::static_applications::StaticMachineApplicationHandle>,
) {
    application.trait_scope.visit_references(program, retained);
    visit_static_machine_bindings_inner(
        program,
        retained,
        application.callable_application,
        visited,
    );
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
/// `where` facts, trait requirement signatures, named implementation rows,
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
            for alternative in plan.target_layouts(program) {
                retained.layout_realization_dependency(alternative.machine);
            }
            for geometry in plan.geometries(program) {
                for demand in &geometry.private_callback_demands {
                    visit_closed_conformance(program, retained, &demand.slot_application);
                }
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
        visit_type_parameters(program, retained, conformance.type_parameters);
        visit_symbol(retained, conformance.trait_symbol);
        for row in &conformance.rows {
            visit_symbol(retained, row.declaring_trait);
            visit_symbol(retained, row.requirement);
            visit_symbol(retained, row.realization_machine);
            visit_symbol(retained, row.realization_state);
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
    retained.enter_lexical_scope(signature.symbol);
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
    retained.leave_lexical_scope();
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
                FixedArrayLength::Expression(value) => {
                    visit_type_reference(program, retained, *value)
                }
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
            symbol, arguments, ..
        } => {
            visit_symbol(retained, *symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                visit_type_reference(program, retained, *argument);
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
    if let Some(operation) = program.expression_table.open_index_operation(expression) {
        visit_symbol(retained, operation.provider);
        visit_symbol(retained, operation.operator);
        visit_symbol(retained, operation.algebra_trait);
        visit_type_reference(program, retained, operation.operand_type);
        if let Some(application) = &operation.application {
            visit_static_argument(program, retained, application);
        }
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
            visit_call_expression(program, retained, expression, call);
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

/// Static selection and method invocation can have different source rows.
/// Traverse the explicitly selected map as a semantic dependency, not merely
/// its realized callee or the unrelated method operands. This neither chooses
/// another implementation nor grants runtime boundary authority.
fn visit_static_requirement_dispatch(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    dispatch: &crate::typed_trees::StaticRequirementDispatch,
    invocation: crate::static_applications::StaticMachineApplicationHandle,
) {
    visit_symbol(retained, dispatch.realization_machine);
    visit_symbol(retained, dispatch.realization_state);
    visit_symbol(retained, dispatch.requirement);
    visit_symbol(retained, dispatch.contract_signature);
    visit_symbol(retained, dispatch.declaring_trait);
    let selected = dispatch.selection_application;
    if selected.is_valid() && selected != invocation {
        let application = program
            .expression_table
            .static_machine_application(selected);
        visit_symbol(retained, application.registration_operation);
        for argument in program
            .expression_table
            .static_machine_application_arguments(selected)
        {
            visit_static_argument(program, retained, argument);
        }
        visit_static_machine_bindings(program, retained, selected);
    }
}

/// Retain the call target and every static argument of one typed call
/// expression, including static requirement dispatch and sealed operations.
fn visit_call_expression(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    expression: ExpressionHandle,
    call: &TableCallExpression,
) {
    visit_callable_symbol(program, retained, call.target_symbol);
    // Named operators can retain their complete call path before an exact
    // target is bound. Their declarations are dependencies even in contracts,
    // which have no executable value-origin row. Keep the canonical candidate
    // set; this traversal neither chooses an overload nor admits the call.
    for operator in crate::operator::named_expression_call_candidates(program, call) {
        visit_symbol(retained, operator.symbol);
    }
    visit_symbol(retained, call.static_machine_parameter);
    if call.authored_machine_application.is_valid() {
        visit_symbol(
            retained,
            program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .registration_operation,
        );
    }
    visit_expression(program, retained, call.receiver);
    if let Some(dispatch) = &call.static_requirement_dispatch {
        visit_static_requirement_dispatch(
            program,
            retained,
            dispatch,
            call.authored_machine_application,
        );
    }
    if retained.call_arguments(program, StaticArgumentCall::Expression(expression, call)) {
        for argument in StaticArgumentCall::Expression(expression, call).arguments(program) {
            visit_static_argument(program, retained, argument);
        }
        visit_static_machine_bindings(program, retained, call.authored_machine_application);
    }
    if let Some(quotient) = &call.quotient_operation {
        visit_static_argument(program, retained, &quotient.representative_operation);
        for theorem in quotient.theorem_evidence.iter() {
            visit_static_argument(program, retained, &theorem.application);
        }
    }
    if let Some(layout) = &call.private_layout_operation {
        visit_static_argument(program, retained, &layout.selected_slot);
    }
    if let Some(query) = &call.layout_query {
        visit_static_argument(program, retained, &query.operand);
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
            visit_callable_symbol(program, retained, call.target_symbol);
            for operator in crate::operator::named_statement_call_candidates(program, call) {
                visit_symbol(retained, operator.symbol);
            }
            visit_symbol(retained, call.static_machine_parameter);
            if call.authored_machine_application.is_valid() {
                visit_symbol(
                    retained,
                    program
                        .expression_table
                        .static_machine_application(call.authored_machine_application)
                        .registration_operation,
                );
            }
            if let Some(dispatch) = &call.static_requirement_dispatch {
                visit_static_requirement_dispatch(
                    program,
                    retained,
                    dispatch,
                    call.authored_machine_application,
                );
            }
            if retained.call_arguments(program, StaticArgumentCall::Statement(call)) {
                for argument in StaticArgumentCall::Statement(call).arguments(program) {
                    visit_static_argument(program, retained, argument);
                }
                visit_static_machine_bindings(program, retained, call.authored_machine_application);
            }
        }
        StatementNode::LocalData(local) => {
            visit_type_reference(program, retained, local.type_reference);
        }
        StatementNode::Transition(transition) => {
            for target in transition.targets() {
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
            TypeParameterKind::Implementation => visit_symbol(retained, parameter.symbol),
        }
    }
}

pub fn visit_machine(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    machine: &crate::machine::Machine,
) {
    retained.enter_lexical_scope(machine.symbol);
    // Capture admission replays the authored map and the exact supplying
    // invocation. Selecting a captured executable therefore retains those
    // declarations; an unrelated checking projection retains none of them.
    for (_, capture) in program
        .captured_machine_applications
        .iter()
        .filter(|(_, capture)| capture.instance == machine.symbol)
    {
        visit_symbol(retained, capture.template);
        let source = program
            .expression_table
            .static_machine_application(capture.supplying_application);
        retained.declaration_description_dependency(source.lexical_owner);
        visit_symbol(retained, capture.conformance_application.declaration);
        visit_static_machine_bindings(program, retained, capture.binding_application);
        visit_static_machine_bindings(program, retained, capture.supplying_application);
        visit_closed_conformance(program, retained, &capture.conformance_application);
    }
    for specialization in program
        .machine_specializations
        .iter()
        .filter(|row| row.instance == machine.symbol)
    {
        visit_type_parameters(program, retained, specialization.template_parameters);
        visit_static_machine_bindings(program, retained, specialization.binding_application);
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

    for conformance in program.machine_trait_conformances(machine) {
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
    retained.leave_lexical_scope();
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

#[cfg(test)]
mod conformance_occurrence_tests {
    use super::*;
    use crate::static_applications::StaticConformanceSelection;
    #[derive(Default)]
    struct Sink(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for Sink {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, iter: T) {
            self.0.extend(iter);
        }
    }
    impl TypedReferences for Sink {}

    fn occurrence_call(
        program: &mut TypedTrees,
        owner: SymbolHandle,
    ) -> (
        ExpressionHandle,
        crate::static_applications::StaticMachineApplicationHandle,
    ) {
        let application = program.expression_table.retain_static_machine_application(
            Default::default(),
            Default::default(),
            &[],
        );
        let source = program
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = owner;
        source.conformance_selection = Some(Box::new(StaticConformanceSelection::default()));
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_machine_application: application,
                    exact_coercion_target: Default::default(),
                    receiver: Default::default(),
                    target_symbol: Default::default(),
                    static_machine_parameter: Default::default(),
                    target: Default::default(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    layout_query: None,
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }));
        (expression, application)
    }

    #[test]
    fn conformance_census_excludes_removed_same_owner_expression() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (removed, application) = occurrence_call(&mut program, owner);
        // The owner still exists, and the expression/application arena rows
        // survive removal of the actual body edge.
        let mut machine = crate::machine::Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                initial_value: removed,
                ..Default::default()
            },
        );
        machine.owned_data = Default::default();
        program.push_machine(machine);
        assert!(
            program
                .expression_table
                .iter_expressions()
                .any(|(h, _)| h == removed)
        );
        assert!(
            !program
                .live_conformance_occurrence_applications()
                .contains(&application)
        );
    }

    #[test]
    fn conformance_census_keeps_uncalled_retained_body() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (expression, application) = occurrence_call(&mut program, owner);
        let mut machine = crate::machine::Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                initial_value: expression,
                ..Default::default()
            },
        );
        program.push_machine(machine);
        assert_eq!(
            program.live_conformance_occurrence_applications(),
            vec![application]
        );
    }

    #[test]
    fn conformance_census_keeps_non_machine_declaration_expression() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (expression, application) = occurrence_call(&mut program, owner);
        program.push_const_declaration(crate::constant::ConstDeclaration {
            symbol: owner,
            authored_initializer: expression,
            ..Default::default()
        });
        assert_eq!(
            program.live_conformance_occurrence_applications(),
            vec![application]
        );
    }

    #[test]
    fn cyclic_derived_occurrence_views_do_not_recurse_or_admit() {
        let mut program = TypedTrees::default();
        let a = program.expression_table.retain_static_machine_application(
            SymbolHandle::invalid(),
            Default::default(),
            &[],
        );
        let b = program.expression_table.retain_static_machine_application(
            SymbolHandle::invalid(),
            Default::default(),
            &[],
        );
        // Deliberately malformed derivation metadata: reference collection is
        // neither selection admission nor a trusted receipt constructor.
        for (handle, next) in [(a, b), (b, a)] {
            program
                .expression_table
                .static_machine_application_mut(handle)
                .conformance_selection = Some(Box::new(StaticConformanceSelection {
                operand_ordinal: 0,
                application: Some(Box::new(crate::typed_trees::ClosedConformanceApplication {
                    callable_application: next,
                    ..Default::default()
                })),
            }));
        }
        let mut sink = Sink::default();
        visit_static_machine_bindings(&program, &mut sink, a);
        assert!(sink.0.is_empty());
        assert_eq!(
            program
                .expression_table
                .static_machine_applications()
                .count(),
            2
        );
    }
}
