//! Retain runtime callable contracts and bind calls to their evaluated operands.
//!
//! Copies can substitute types while keeping the same formal symbols, so reuse
//! rows by the resolved contract handle. Reserve each row before lowering its
//! children so recursive references can resolve.

use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_contracts(lowerer: &mut crate::lowerer::Lowerer<'_>) -> Result<(), Diagnostic> {
    let mut pending = Vec::new();
    lowerer.typed_trees.runtime_callable_lowering =
        typed::callable_values::RuntimeCallableLowering::from_contracts(
            &lowerer.typed_trees.runtime_callable_contracts,
        )
        .map_err(Diagnostic::error)?;
    for (source, row) in lowerer
        .source_trees
        .tables
        .declarations
        .runtime_callable_contracts
        .iter()
    {
        let coordinate = (source.arena_index(), source.generation());
        if let Some(handle) = lowerer
            .typed_trees
            .runtime_callable_lowering
            .get(source.arena_index())
        {
            if lowerer
                .typed_trees
                .runtime_callable_contracts
                .get(handle)
                .resolved_source
                != Some(coordinate)
            {
                return Err(Diagnostic::error(
                    "runtime callable contract has stale source ownership",
                ));
            }
            continue;
        }
        let handle = lowerer.typed_trees.runtime_callable_contracts.insert(
            typed::callable_values::RuntimeCallableContract {
                signature: typed::signature::StateSignature {
                    symbol: row.signature.symbol,
                    ..Default::default()
                },
                lexical_owner: row.lexical_owner,
                resolved_source: Some(coordinate),
            },
        );
        lowerer
            .typed_trees
            .runtime_callable_lowering
            .retain(source.arena_index(), handle)
            .map_err(Diagnostic::error)?;
        pending.push((handle, &row.signature));
    }
    for (handle, signature) in pending {
        let signature =
            crate::signatures::callable_signature::lower_state_signature(lowerer, signature)?;
        lowerer
            .typed_trees
            .runtime_callable_contracts
            .get_mut(handle)
            .signature = signature;
    }
    Ok(())
}

pub(crate) fn lower_contract_reference(
    program: &typed::TypedTrees,
    source: resolved::callable_values::RuntimeCallableContractHandle,
) -> Result<typed::callable_values::RuntimeCallableContractHandle, Diagnostic> {
    let coordinate = (source.arena_index(), source.generation());
    let handle = program
        .runtime_callable_lowering
        .get(source.arena_index())
        .ok_or_else(|| {
            Diagnostic::error("runtime callable type lost its declaration-owned contract")
        })?;
    if !program.runtime_callable_contracts.is_valid(handle)
        || program
            .runtime_callable_contracts
            .get(handle)
            .resolved_source
            != Some(coordinate)
    {
        return Err(Diagnostic::error(
            "runtime callable type has stale declaration ownership",
        ));
    }
    Ok(handle)
}

/// A value binding owns a call even when its name also denotes a builtin or a
/// global machine. Non-callable values still take this route and reject by type.
pub(crate) fn callee_is_value(
    program: Option<&resolved::SymbolResolvedTrees>,
    expressions: &resolved::expression::ExpressionTable,
    callee: resolved::expression::ExpressionHandle,
) -> bool {
    program.is_some_and(|program| {
        crate::expressions::expression::table::member_binding::callee_is_value(
            program,
            expressions,
            callee,
        )
    })
}

/// Expected callable types select code once, before ordinary flow checking.
/// Other callable values keep their descriptor and are checked as assignments.
pub(crate) fn prepare_named_value(
    program: &mut typed::TypedTrees,
    value: typed::expression::ExpressionHandle,
    destination: typed::types::TypeReferenceHandle,
    lexical_owner: symbols::SymbolHandle,
) -> Result<bool, Diagnostic> {
    use typed::expression::{ExpressionNode, StaticMachineArgument};
    use typed::types::TypeReferenceNode;
    let mut carrier = destination;
    let contract = loop {
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
            TypeReferenceNode::Callable { contract } => break *contract,
            _ => return Ok(false),
        }
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(value).clone() else {
        return Ok(false);
    };
    let names = program
        .expression_table
        .name_path_members(path.members)
        .to_vec();
    let bound_kind = path
        .symbol
        .is_valid()
        .then(|| program.symbols.get(path.symbol).kind);
    let namespace = path.head_symbol;
    let conformance = program.conformance_by_symbol(namespace);
    let (selected, conformance, requirement) = if let Some(conformance) = conformance {
        let machine = program
            .machine_by_own_or_state_symbol(path.symbol)
            .map(|machine| machine.symbol);
        let mut rows = conformance.rows.iter().filter(|row| {
            row.requirement == path.symbol || machine == Some(row.realization_machine)
        });
        let Some(row) = rows.next() else {
            return Err(Diagnostic::error(
                "named machine value lost its exact implementation requirement row",
            )
            .with_source_span(program.expression_table.source_span(value)));
        };
        if rows.next().is_some() {
            return Err(Diagnostic::error(
                "named machine value has ambiguous implementation requirement rows",
            )
            .with_source_span(program.expression_table.source_span(value)));
        }
        (row.realization_machine, conformance.symbol, row.requirement)
    } else if matches!(
        bound_kind,
        Some(symbols::SymbolKind::Machine | symbols::SymbolKind::State)
    ) {
        (
            path.symbol,
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::invalid(),
        )
    } else {
        return Ok(false);
    };
    let machine = program
        .machine_by_own_or_state_symbol(selected)
        .ok_or_else(|| {
            Diagnostic::error("named machine value has no selected executable declaration")
                .with_source_span(program.expression_table.source_span(value))
        })?;
    let entry = program.machine_states(machine).first().ok_or_else(|| {
        Diagnostic::error("named machine value has no selected entry state")
            .with_source_span(program.expression_table.source_span(value))
    })?;
    if machine.supply_mode.is_boundary_declaration() {
        return Err(Diagnostic::error("a local machine value requires a local machine; use a proxy for a boundary requirement")
            .with_source_span(program.expression_table.source_span(value)));
    }
    if !program.machine_callable_parameters(machine).is_empty() {
        return Err(Diagnostic::error(
            "runtime machine value requires every static argument closed before construction",
        )
        .with_source_span(program.expression_table.source_span(value)));
    }
    let (conformance, requirement) = if !conformance.is_valid() {
        program
            .owned_conformance_realization(machine.symbol)
            .map_or((conformance, requirement), |(origin, row)| {
                (origin.symbol, row.requirement)
            })
    } else {
        (conformance, requirement)
    };
    let selected = StaticMachineArgument {
        path: names.into_boxed_slice(),
        symbol: entry.symbol,
        ..Default::default()
    };
    let target = machine.symbol;
    let target_name = machine.name.clone();
    let application =
        program
            .expression_table
            .retain_static_machine_application(target, target_name, &[]);
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = lexical_owner;
    *program.expression_table.expression_mut(value) =
        ExpressionNode::MachineValue(typed::callable_values::MachineValue {
            type_reference: destination,
            selected,
            application,
            conformance,
            requirement,
        });
    program
        .expression_table
        .set_construction_required_type_reference(value, destination);
    // Construction retains the destination contract; proof checks refinement
    // against the selected body's effective contract before lowering publishes it.
    let _ = contract;
    Ok(true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallOwner {
    State {
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    },
    Declaration {
        owner: symbols::SymbolHandle,
    },
    Signature {
        owner: symbols::SymbolHandle,
        lexical_owner: symbols::SymbolHandle,
        parameters: arena::HandleSpan<typed::signature::StateParameter>,
        result: typed::types::TypeReferenceHandle,
        type_parameters: arena::HandleSpan<typed::data::TypeParameter>,
        trait_scope: Option<typed::trait_definition::RetainedTraitScope>,
        contract: Option<typed::callable_values::RuntimeCallableContractHandle>,
    },
}

impl Default for CallOwner {
    fn default() -> Self {
        Self::Declaration {
            owner: symbols::SymbolHandle::invalid(),
        }
    }
}

impl CallOwner {
    fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::State { machine, .. } => machine,
            Self::Declaration { owner } | Self::Signature { owner, .. } => owner,
        }
    }

    fn result_type(
        self,
        program: &typed::TypedTrees,
        expression: typed::expression::ExpressionHandle,
    ) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
        let (parameters, result) = match self {
            Self::State { state, .. } => {
                let state = program.state_by_symbol(state).ok_or_else(|| {
                    Diagnostic::error("callable source scope lost its exact state")
                })?;
                (program.state_parameters(state), state.return_type)
            }
            Self::Signature {
                parameters, result, ..
            } => (
                program.state_parameters.span(parameters).ok_or_else(|| {
                    Diagnostic::error("callable source scope lost its formal telescope")
                })?,
                result,
            ),
            Self::Declaration { .. } => (&[][..], Default::default()),
        };
        crate::typing_queries::scoped_expression_type_reference(
            program, parameters, result, expression,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallUse {
    Expression(typed::expression::ExpressionHandle),
    Statement(typed::statement::StatementHandle),
}

/// Each association is authenticated by the exact handle supplied by its arena.
/// The immutable inventory owns these rows for one complete program walk.
struct ArenaAssociations<Node, Row> {
    rows: Vec<(arena::Handle<Node>, Row)>,
}
impl<Node, Row> Default for ArenaAssociations<Node, Row> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}
impl<Node, Row: Default> ArenaAssociations<Node, Row> {
    fn prepare(handles: impl Iterator<Item = arena::Handle<Node>>) -> Self {
        let mut rows = Self::default();
        for handle in handles {
            rows.retain(handle, Row::default());
        }
        rows
    }
    fn retain(&mut self, handle: arena::Handle<Node>, row: Row) {
        let slot = handle.arena_index() as usize;
        self.rows
            .resize_with(self.rows.len().max(slot + 1), Default::default);
        self.rows[slot] = (handle, row);
    }
    fn get(&self, handle: arena::Handle<Node>) -> Option<&Row> {
        let (actual, row) = self.rows.get(handle.arena_index() as usize)?;
        (handle.is_valid() && *actual == handle).then_some(row)
    }
    fn get_mut(&mut self, handle: arena::Handle<Node>) -> Option<&mut Row> {
        let (actual, row) = self.rows.get_mut(handle.arena_index() as usize)?;
        (handle.is_valid() && *actual == handle).then_some(row)
    }
}

#[derive(Default)]
struct ExpressionIndex {
    state_scopes: Vec<usize>,
    state_visit: Option<usize>,
    visited_scopes: Vec<usize>,
    assigned: Option<typed::callable_values::RuntimeCallableContractHandle>,
}
#[derive(Default)]
struct TypeIndex {
    state_scopes: Vec<usize>,
    state_visit: Option<usize>,
    visited_scopes: Vec<usize>,
}
#[derive(Default)]
struct StatementIndex {
    scope: Option<usize>,
    assigned: Option<typed::callable_values::RuntimeCallableContractHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeCoordinate {
    kind: u8,
    owner: (u32, u32),
    supplying_owner: (u32, u32),
    parameters: (u32, u32, u32),
    result: (u32, u32),
    type_parameters: (u32, u32, u32),
    contract: (u32, u32),
}
impl CallOwner {
    fn coordinate(self) -> ScopeCoordinate {
        let handle = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut coordinate = ScopeCoordinate {
            kind: 0,
            owner: handle(self.symbol()),
            supplying_owner: (0, 0),
            parameters: (0, 0, 0),
            result: (0, 0),
            type_parameters: (0, 0, 0),
            contract: (0, 0),
        };
        match self {
            Self::Declaration { .. } => {}
            Self::State { state, .. } => {
                coordinate.kind = 1;
                coordinate.supplying_owner = handle(state);
            }
            Self::Signature {
                lexical_owner,
                parameters,
                result,
                type_parameters,
                contract,
                ..
            } => {
                coordinate.kind = 2;
                coordinate.supplying_owner = handle(lexical_owner);
                coordinate.result = (result.arena_index(), result.generation());
                coordinate.parameters = (
                    parameters.start().arena_index(),
                    parameters.start().generation(),
                    parameters.count(),
                );
                coordinate.type_parameters = (
                    type_parameters.start().arena_index(),
                    type_parameters.start().generation(),
                    type_parameters.count(),
                );
                if let Some(contract) = contract {
                    coordinate.contract = (contract.arena_index(), contract.generation());
                }
            }
        }
        coordinate
    }
}
struct CallScopes {
    declarations: ArenaAssociations<symbols::Symbol, Vec<(ScopeCoordinate, usize)>>,
    owners: Vec<CallOwner>,
}
impl CallScopes {
    fn prepare(program: &typed::TypedTrees) -> Self {
        Self {
            declarations: ArenaAssociations::prepare(
                program
                    .symbols
                    .symbols()
                    .nodes()
                    .iter()
                    .map(|(handle, _)| handle),
            ),
            owners: vec![CallOwner::default()],
        }
    }
    fn intern(&mut self, owner: CallOwner) -> Result<usize, Diagnostic> {
        if owner == CallOwner::default() {
            return Ok(0);
        }
        let scopes = self.declarations.get_mut(owner.symbol()).ok_or_else(|| {
            Diagnostic::error("runtime callable scope has a stale declaration owner")
        })?;
        let coordinate = owner.coordinate();
        let position = match scopes.binary_search_by_key(&coordinate, |(key, _)| *key) {
            Ok(position) => {
                let scope = scopes[position].1;
                // Retained trait scope is obtained once from the exact supplying
                // declaration. Check its full value rather than treating the
                // coordinate as a semantic substitute for that application.
                if self.owners[scope] != owner {
                    return Err(Diagnostic::error(
                        "runtime callable scope changed its retained supplying application",
                    ));
                }
                return Ok(scope);
            }
            Err(position) => position,
        };
        let scope = self.owners.len();
        self.owners.push(owner);
        scopes.insert(position, (coordinate, scope));
        Ok(scope)
    }
    fn owner(&self, scope: usize) -> CallOwner {
        self.owners[scope]
    }
}

struct CallIndexes {
    expressions: ArenaAssociations<typed::expression::ExpressionNode, ExpressionIndex>,
    references: ArenaAssociations<typed::types::TypeReferenceNode, TypeIndex>,
    statements: ArenaAssociations<typed::statement::StatementNode, StatementIndex>,
    contracts: ArenaAssociations<typed::callable_values::RuntimeCallableContract, bool>,
    scopes: CallScopes,
}
impl CallIndexes {
    fn collect(program: &typed::TypedTrees) -> Result<Self, Diagnostic> {
        let mut contexts = Self {
            expressions: ArenaAssociations::prepare(
                program
                    .expression_table
                    .iter_backing_expressions()
                    .map(|(handle, _)| handle),
            ),
            references: ArenaAssociations::prepare(
                program
                    .type_reference_table
                    .type_reference_rows()
                    .map(|(handle, _)| handle),
            ),
            statements: ArenaAssociations::default(),
            contracts: ArenaAssociations::prepare(
                program
                    .runtime_callable_contracts
                    .iter()
                    .map(|(handle, _)| handle),
            ),
            scopes: CallScopes::prepare(program),
        };
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let owner = CallOwner::State {
                    machine: machine.symbol,
                    state: state.symbol,
                };
                let scope = contexts.scopes.intern(owner)?;
                let mut expressions = StateExpressions {
                    indexes: &mut contexts,
                    scope,
                    error: None,
                };
                for parameter in program.state_parameters(state) {
                    typed::references::visit_type_reference(
                        program,
                        &mut expressions,
                        parameter.type_reference,
                    );
                }
                typed::references::visit_type_reference(
                    program,
                    &mut expressions,
                    state.return_type,
                );
                for contract in program.state_contracts(state) {
                    for fact in contract.facts.handles() {
                        for expression in program.tables.proof_fact_expression_roots(fact) {
                            typed::references::visit_expression(
                                program,
                                &mut expressions,
                                expression,
                            );
                        }
                    }
                }
                for (handle, statement) in program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                {
                    if !program.statement_table.statement_is_valid(handle) {
                        return Err(Diagnostic::error(
                            "runtime callable inventory encountered a stale statement",
                        ));
                    }
                    expressions.indexes.statements.retain(
                        handle,
                        StatementIndex {
                            scope: Some(scope),
                            assigned: None,
                        },
                    );
                    for (_, expression) in program.tables.expression_roots(statement) {
                        typed::references::visit_expression(program, &mut expressions, expression);
                    }
                }
                if let Some(error) = expressions.error {
                    return Err(error);
                }
            }
        }
        Ok(contexts)
    }
}

struct StateExpressions<'indexes> {
    indexes: &'indexes mut CallIndexes,
    scope: usize,
    error: Option<Diagnostic>,
}
impl Extend<symbols::SymbolHandle> for StateExpressions<'_> {
    fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, _: T) {}
}
impl typed::references::TypedReferences for StateExpressions<'_> {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn type_reference(&mut self, reference: typed::types::TypeReferenceHandle) -> bool {
        let Some(row) = self.indexes.references.get_mut(reference) else {
            self.error.get_or_insert_with(|| {
                Diagnostic::error(
                    "runtime callable state inventory encountered a stale type reference",
                )
            });
            return false;
        };
        if row.state_visit == Some(self.scope) {
            return false;
        }
        row.state_visit = Some(self.scope);
        row.state_scopes.push(self.scope);
        true
    }
    fn callable_contract(
        &mut self,
        _: typed::callable_values::RuntimeCallableContractHandle,
    ) -> bool {
        false
    }
    fn expression(&mut self, expression: typed::expression::ExpressionHandle) -> bool {
        let Some(row) = self.indexes.expressions.get_mut(expression) else {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("runtime callable state inventory encountered a stale expression")
            });
            return false;
        };
        if row.state_visit == Some(self.scope) {
            return false;
        }
        row.state_visit = Some(self.scope);
        row.state_scopes.push(self.scope);
        true
    }
}

fn enter_scope(scopes: &mut Vec<usize>, scope: usize) -> bool {
    match scopes.binary_search(&scope) {
        Ok(_) => false,
        Err(position) => {
            scopes.insert(position, scope);
            true
        }
    }
}

/// Type occurrences remain owned by their exact lexical/formal context while
/// binding runs before the other cohort normalization. Admission reads the final
/// normalized rows rather than a provisional or display-based identity.
#[derive(Default)]
pub(crate) struct RuntimeCallableAdmission {
    types: Vec<(Vec<CallOwner>, typed::types::TypeReferenceHandle)>,
}
impl RuntimeCallableAdmission {
    pub(crate) fn extend(&mut self, other: Self) {
        self.types.extend(other.types);
    }
}

struct CallInventory<'program> {
    program: &'program typed::TypedTrees,
    indexes: CallIndexes,
    owner: CallOwner,
    scope: usize,
    outer: Vec<(CallOwner, usize)>,
    pending_signature: Option<CallOwner>,
    active_contracts: Vec<typed::callable_values::RuntimeCallableContractHandle>,
    ordered: Vec<(CallOwner, CallUse)>,
    statements: Vec<(CallOwner, CallUse)>,
    statement: Option<typed::statement::StatementHandle>,
    admission: RuntimeCallableAdmission,
    type_frontier: usize,
    error: Option<Diagnostic>,
}
impl CallInventory<'_> {
    fn select_owner(&mut self, owner: CallOwner) {
        match self.indexes.scopes.intern(owner) {
            Ok(scope) => {
                self.owner = owner;
                self.scope = scope;
            }
            Err(error) => {
                self.error.get_or_insert(error);
            }
        }
    }
    fn owners(&self, expression: typed::expression::ExpressionHandle) -> Vec<usize> {
        if matches!(self.owner, CallOwner::Signature { .. }) {
            return vec![self.scope];
        }
        let states = self
            .indexes
            .expressions
            .get(expression)
            .into_iter()
            .flat_map(|row| &row.state_scopes)
            .copied()
            .filter(|scope| self.indexes.scopes.owner(*scope).symbol() == self.owner.symbol())
            .collect::<Vec<_>>();
        if states.is_empty() {
            vec![self.scope]
        } else {
            states
        }
    }
}
impl Extend<symbols::SymbolHandle> for CallInventory<'_> {
    fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, _: T) {}
}
impl typed::references::TypedReferences for CallInventory<'_> {
    fn declaration(&mut self, owner: symbols::SymbolHandle) -> bool {
        self.select_owner(CallOwner::Declaration { owner });
        true
    }
    fn callable_signature(&mut self, signature: &typed::signature::StateSignature) {
        let runtime = self.active_contracts.last().copied().filter(|contract| {
            std::ptr::eq(
                &self
                    .program
                    .runtime_callable_contracts
                    .get(*contract)
                    .signature,
                signature,
            )
        });
        let lexical_owner = runtime.map_or(self.owner.symbol(), |contract| {
            self.program
                .runtime_callable_contracts
                .get(contract)
                .lexical_owner
        });
        let scope_owner = runtime.map_or(signature.symbol, |_| lexical_owner);
        self.pending_signature = Some(CallOwner::Signature {
            owner: signature.symbol,
            lexical_owner,
            parameters: signature.parameters,
            result: signature.return_type,
            type_parameters: signature.type_parameters,
            trait_scope: self
                .program
                .closed_requirement_application(scope_owner)
                .map(|application| application.trait_scope),
            contract: runtime,
        });
    }
    fn enter_lexical_scope(&mut self, owner: symbols::SymbolHandle) {
        self.outer.push((self.owner, self.scope));
        let owner = self
            .pending_signature
            .take()
            .filter(|signature| signature.symbol() == owner)
            .unwrap_or(CallOwner::Declaration { owner });
        self.select_owner(owner);
    }
    fn leave_lexical_scope(&mut self) {
        if let Some((owner, scope)) = self.outer.pop() {
            self.owner = owner;
            self.scope = scope;
        }
    }
    fn callable_contract(
        &mut self,
        contract: typed::callable_values::RuntimeCallableContractHandle,
    ) -> bool {
        if !self.program.runtime_callable_contracts.is_valid(contract) {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("callable type lost its exact retained contract row")
            });
            return false;
        }
        let Some(entered) = self.indexes.contracts.get_mut(contract) else {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("callable type lost its exact retained contract index")
            });
            return false;
        };
        if std::mem::replace(entered, true) {
            return false;
        }
        let program = self.program;
        self.active_contracts.push(contract);
        typed::references::visit_signature(
            program,
            self,
            &program.runtime_callable_contracts.get(contract).signature,
        );
        self.active_contracts.pop();
        false
    }
    fn type_reference(&mut self, reference: typed::types::TypeReferenceHandle) -> bool {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("runtime callable inventory encountered a stale type reference")
            });
            return false;
        }
        let Some(row) = self.indexes.references.get_mut(reference) else {
            self.error.get_or_insert_with(|| {
                Diagnostic::error(
                    "runtime callable inventory encountered an unindexed type reference",
                )
            });
            return false;
        };
        if !enter_scope(&mut row.visited_scopes, self.scope) {
            return false;
        }
        if reference.arena_index() as usize > self.type_frontier
            && matches!(
                self.program.type_reference_table.type_reference(reference),
                typed::types::TypeReferenceNode::Callable { .. }
            )
        {
            let mut scopes = self
                .outer
                .iter()
                .map(|(owner, _)| *owner)
                .collect::<Vec<_>>();
            scopes.push(self.owner);
            if !matches!(self.owner, CallOwner::Signature { .. }) {
                scopes.extend(
                    row.state_scopes
                        .iter()
                        .map(|scope| self.indexes.scopes.owner(*scope))
                        .filter(|owner| owner.symbol() == self.owner.symbol()),
                );
            }
            self.admission.types.push((scopes, reference));
        }
        true
    }
    fn expression(&mut self, expression: typed::expression::ExpressionHandle) -> bool {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("runtime callable inventory encountered a stale expression")
            });
            return false;
        }
        let owners = self.owners(expression);
        let Some(row) = self.indexes.expressions.get_mut(expression) else {
            self.error.get_or_insert_with(|| {
                Diagnostic::error("runtime callable inventory encountered an unindexed expression")
            });
            return false;
        };
        let mut entered = false;
        for scope in owners {
            entered |= enter_scope(&mut row.visited_scopes, scope);
        }
        entered
    }
    fn leave_expression(&mut self, expression: typed::expression::ExpressionHandle) {
        if self
            .program
            .expression_table
            .call_node(expression)
            .is_some_and(|call| call.runtime_call.is_some())
        {
            let owners = self.owners(expression);
            self.ordered.extend(owners.into_iter().map(|scope| {
                (
                    self.indexes.scopes.owner(scope),
                    CallUse::Expression(expression),
                )
            }));
        }
    }
    fn statement_handle(&mut self, statement: typed::statement::StatementHandle) {
        self.statement = Some(statement);
    }
    fn statement(&mut self, statement: &typed::statement::StatementNode) -> bool {
        if statement
            .as_call()
            .is_some_and(|call| call.runtime_call.is_some())
        {
            let Some(handle) = self.statement else {
                self.error.get_or_insert_with(|| {
                    Diagnostic::error("runtime statement call lost its retained statement owner")
                });
                return true;
            };
            let Some(scope) = self
                .indexes
                .statements
                .get(handle)
                .and_then(|row| row.scope)
            else {
                self.error.get_or_insert_with(|| {
                    Diagnostic::error("runtime statement call has no enclosing state")
                });
                return true;
            };
            self.statements
                .push((self.indexes.scopes.owner(scope), CallUse::Statement(handle)));
        }
        true
    }
}

/// The shared declaration walk supplies all current expression owners. Bind only
/// new or unresolved uses, in operand dependency order, before effect admission.
pub(crate) fn bind_calls(
    program: &mut typed::TypedTrees,
    expression_frontier: usize,
    type_frontier: usize,
) -> Result<RuntimeCallableAdmission, Diagnostic> {
    let mut inventory = CallInventory {
        program,
        indexes: CallIndexes::collect(program)?,
        owner: CallOwner::default(),
        scope: 0,
        outer: Vec::new(),
        pending_signature: None,
        active_contracts: Vec::new(),
        ordered: Vec::new(),
        statements: Vec::new(),
        statement: None,
        admission: RuntimeCallableAdmission::default(),
        type_frontier,
        error: None,
    };
    typed::references::visit_program(program, &mut inventory);
    // Retained arena storage is not another declaration root. The shared
    // walk follows each live Callable type into its complete signature.
    if let Some(error) = inventory.error {
        return Err(error);
    }
    let mut uses = inventory.ordered;
    uses.extend(inventory.statements);
    let admission = inventory.admission;
    let mut indexes = inventory.indexes;
    for (owner, site) in uses {
        let binding = match site {
            CallUse::Expression(expression) => program
                .expression_table
                .call_node(expression)
                .and_then(|call| call.runtime_call),
            CallUse::Statement(statement) => program
                .statement_table
                .statement(statement)
                .as_call()
                .and_then(|call| call.runtime_call),
        }
        .ok_or_else(|| Diagnostic::error("runtime call lost its evaluated callee operand"))?;
        let new_use = match site {
            CallUse::Expression(expression) => {
                expression.arena_index() as usize > expression_frontier
            }
            CallUse::Statement(_) => binding.callee.arena_index() as usize > expression_frontier,
        };
        if !new_use
            && program
                .runtime_callable_contracts
                .is_valid(binding.contract)
        {
            continue;
        }
        let type_reference = owner.result_type(program, binding.callee)?.ok_or_else(|| {
            Diagnostic::error(
            "runtime call callee has no complete declared value type in its exact lexical scope",
        ).with_source_span(program.expression_table.source_span(binding.callee))
        })?;
        let mut carrier = type_reference;
        let mut seen = Vec::new();
        let contract = loop {
            if !program
                .type_reference_table
                .contains_type_reference(carrier)
                || seen.contains(&carrier)
            {
                return Err(Diagnostic::error(
                    "runtime call callee has an unresolved or cyclic value type",
                ));
            }
            seen.push(carrier);
            match program.type_reference_table.type_reference(carrier) {
                typed::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    carrier = *base_type
                }
                typed::types::TypeReferenceNode::Callable { contract } => break *contract,
                _ => {
                    return Err(Diagnostic::error(
                        "the evaluated call operand is not a runtime callable",
                    )
                    .with_source_span(program.expression_table.source_span(binding.callee)));
                }
            }
        };
        if !program.runtime_callable_contracts.is_valid(contract) {
            return Err(Diagnostic::error(
                "runtime call callee lost its retained structural signature",
            ));
        }
        let assigned = match site {
            CallUse::Expression(expression) => indexes
                .expressions
                .get_mut(expression)
                .map(|row| &mut row.assigned),
            CallUse::Statement(statement) => indexes
                .statements
                .get_mut(statement)
                .map(|row| &mut row.assigned),
        }
        .ok_or_else(|| {
            Diagnostic::error("runtime call lost its exact retained invocation index")
        })?;
        if assigned
            .replace(contract)
            .is_some_and(|previous| previous != contract)
        {
            return Err(Diagnostic::error(
                "one runtime call expression is shared by incompatible lexical contract scopes",
            ));
        }
        match site {
            CallUse::Expression(expression) => {
                let typed::expression::ExpressionNode::Call(call) =
                    program.expression_table.expression_mut(expression)
                else {
                    return Err(Diagnostic::error(
                        "runtime expression call changed while binding",
                    ));
                };
                let Some(binding) = &mut call.runtime_call else {
                    return Err(Diagnostic::error(
                        "runtime expression call lost its callee while binding",
                    ));
                };
                binding.contract = contract;
            }
            CallUse::Statement(statement) => {
                let typed::statement::StatementNode::Call(call) =
                    program.statement_table.statement_mut(statement)
                else {
                    return Err(Diagnostic::error(
                        "runtime statement call changed while binding",
                    ));
                };
                let Some(binding) = &mut call.runtime_call else {
                    return Err(Diagnostic::error(
                        "runtime statement call lost its callee while binding",
                    ));
                };
                binding.contract = contract;
            }
        }
    }
    Ok(admission)
}

pub(crate) fn admit_types(
    program: &typed::TypedTrees,
    admission: &RuntimeCallableAdmission,
) -> Result<(), Diagnostic> {
    for (scopes, reference) in &admission.types {
        let typed::types::TypeReferenceNode::Callable { contract } =
            program.type_reference_table.type_reference(*reference)
        else {
            return Err(Diagnostic::error(
                "callable type changed its structural carrier before admission",
            ));
        };
        if !program.runtime_callable_contracts.is_valid(*contract) {
            return Err(Diagnostic::error(
                "callable type has no retained contract for identity admission",
            ));
        }
        let row = program.runtime_callable_contracts.get(*contract);
        let mut owner = row.lexical_owner;
        let mut seen = Vec::new();
        let mut binders = Vec::new();
        while owner.is_valid() {
            if seen.contains(&owner) {
                return Err(Diagnostic::error(
                    "callable type has a cyclic lexical scope",
                ));
            }
            seen.push(owner);
            let exact_signature = scopes.iter().rev().find_map(|scope| match scope {
                CallOwner::Signature {
                    owner: formal_owner,
                    parameters,
                    type_parameters,
                    ..
                } if *formal_owner == owner => Some((*parameters, *type_parameters)),
                _ => None,
            });
            let generic_parameters = exact_signature.map_or_else(
                || program.declaration_type_parameters(owner),
                |(_, parameters)| program.data_type_parameters.span_or_empty(parameters),
            );
            for (ordinal, parameter) in generic_parameters.iter().enumerate() {
                if !binders
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
                {
                    binders.push((
                        parameter.symbol,
                        format!("$Outer{}G{ordinal}", seen.len() - 1),
                    ));
                }
            }
            let value_parameters = exact_signature
                .map(|(parameters, _)| program.state_parameters.span_or_empty(parameters))
                .or_else(|| {
                    program
                        .state_by_symbol(owner)
                        .map(|state| program.state_parameters(state))
                });
            if let Some(parameters) = value_parameters {
                for (ordinal, parameter) in parameters.iter().enumerate() {
                    if !binders
                        .iter()
                        .any(|(symbol, _)| *symbol == parameter.symbol)
                    {
                        binders.push((
                            parameter.symbol,
                            format!("$Outer{}V{ordinal}", seen.len() - 1),
                        ));
                    }
                }
            }
            owner = program.symbols.get(owner).parent;
        }
        program
            .runtime_callable_type_identity(typed::type_identity::TypeIdentityRequest {
                binders: &binders,
                ..typed::type_identity::TypeIdentityRequest::ordinary(*reference)
            })
            .map_err(|error| {
                let diagnostic = Diagnostic::error(format!(
                    "runtime callable has no complete structural type identity: {error}"
                ));
                match program.symbols.symbol_source_span(row.signature.symbol) {
                    Some(span) => diagnostic.with_source_span(span),
                    None => diagnostic,
                }
            })?;
    }
    Ok(())
}

/// Source checking retains pairs whose dependent callable operands have no
/// complete identity yet. Their exact typed views must settle that comparison
/// before the cohort can become checked input.
pub(crate) fn validate_pending_token_shapes(
    source: &resolved::SymbolResolvedTrees,
    program: &typed::TypedTrees,
) -> Result<(), Diagnostic> {
    fn view(
        program: &typed::TypedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Result<&typed::operator::OperatorDefinition, Diagnostic> {
        let mut views = program
            .machine_token_bindings()
            .iter()
            .chain(program.all_operators())
            .filter(|view| view.symbol == symbol);
        let Some(view) = views.next() else {
            return Err(Diagnostic::error(
                "pending operator operand comparison lost its exact typed declaration",
            ));
        };
        if views.next().is_some() {
            return Err(Diagnostic::error(
                "pending operator operand comparison has ambiguous typed declarations",
            ));
        }
        Ok(view)
    }

    for pending in &source.pending_token_shape_comparisons {
        let earlier = view(program, pending.earlier)?;
        let later = view(program, pending.later)?;
        let Some(spelling) = later.spelling else {
            return Err(Diagnostic::error(
                "pending operator operand comparison lost its fixed token",
            ));
        };
        if earlier.spelling != Some(spelling) {
            return Err(Diagnostic::error(
                "pending operator operand comparison changed its fixed token",
            ));
        }
        let key = |declaration: &typed::operator::OperatorDefinition| {
            typed::operator::operator_operand_signature(program, declaration).map_err(|error| {
                Diagnostic::error(format!(
                    "operator `{}` has no resolved operand identity: {error}",
                    program.operator_path_string(declaration.name),
                ))
            })
        };
        let earlier_key = key(earlier)?;
        let later_key = key(later)?;
        // These are the modes retained in the source token-shape key. Lifetime
        // spelling and callable predicates/effects do not separate overloads.
        let modes = |declaration: &typed::operator::OperatorDefinition| {
            program
                .operator_parameters(declaration)
                .iter()
                .map(|parameter| (parameter.is_mutable, parameter.is_const))
                .collect::<Vec<_>>()
        };
        if earlier_key == later_key && modes(earlier) == modes(later) {
            let diagnostic = Diagnostic::error(format!(
                "`{}` binds the fixed operator token `{}` already bound by `{}` for the \
                 same owner and operand shape ({later_key}); a second spelling needs a \
                 distinct operand shape or a separate owner",
                program.operator_path_string(later.name),
                spelling.symbol(),
                program.operator_path_string(earlier.name),
            ));
            return Err(match program.symbols.symbol_source_span(later.symbol) {
                Some(span) => diagnostic.with_source_span(span),
                None => diagnostic,
            });
        }
    }
    Ok(())
}
