/*
One owner retains declaration coordinates for selected bodies and publishes a
flat root view for global lookup. Neither view retains a second copy of types.
A name query supplies metadata only; it authorizes no access or establishment.
*/

use crate::TypedTrees;
use crate::declaration_index::SymbolRows;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::machine::Machine;
use crate::signature::StateParameter;
use crate::state::State;
use crate::statement::{StatementNode, TableLocalData};
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

const DIRECT_BINDING_ROWS: usize = 32;

#[derive(Clone)]
pub(crate) struct BindingScope {
    parameters: u64,
    statements: std::sync::Arc<()>,
}

impl PartialEq for BindingScope {
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && std::sync::Arc::ptr_eq(&self.statements, &other.statements)
    }
}
impl Eq for BindingScope {}

type BodyScope = (u32, u32, u32, u32, u32, u32);

fn body_scope(state: &State) -> BodyScope {
    (
        state.parameters.start().arena_index(),
        state.parameters.start().generation(),
        state.parameters.count(),
        state.statement_nodes.start().arena_index(),
        state.statement_nodes.start().generation(),
        state.statement_nodes.count(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindingPosition {
    Parameter(u32),
    Local(u32),
}

fn body_coordinates<'a>(
    parameters: &'a [StateParameter],
    statements: &'a [StatementNode],
) -> impl Iterator<Item = (SymbolHandle, BindingPosition)> + 'a {
    parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, BindingPosition::Parameter(ordinal as u32)))
        .chain(
            statements
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| {
                    statement
                        .as_local_data()
                        .map(|local| (local.symbol, BindingPosition::Local(ordinal as u32)))
                }),
        )
}

#[derive(Clone, Default)]
pub(crate) struct StateDeclarationIndex {
    bodies: Vec<(BodyScope, PreparedBindings)>,
    roots: Option<RootBindings>,
}

#[derive(Clone)]
struct PreparedBindings {
    rows: SymbolRows<BindingPosition>,
    #[cfg(test)]
    preparation: std::sync::Arc<()>,
}

#[derive(Clone, PartialEq, Eq)]
struct RootScope {
    machines: u64,
    states: u64,
    roots: HandleSpan<Machine>,
}

impl RootScope {
    fn of(program: &TypedTrees) -> Self {
        Self {
            machines: program.tables.machines.revision(),
            states: program.tables.machine_states.revision(),
            roots: program.roots.machines,
        }
    }

    fn matches(&self, program: &TypedTrees) -> bool {
        self.roots == program.roots.machines
            && program.tables.machines.revision() == self.machines
            && program.tables.machine_states.revision() == self.states
    }
}

#[derive(Clone, Copy)]
struct RootBody {
    scope: BodyScope,
    machine: u32,
    state: u32,
}

#[derive(Clone, Copy)]
struct RootBinding {
    body: u32,
    position: BindingPosition,
}

#[derive(Clone)]
struct RootBindings {
    scope: RootScope,
    bodies: Vec<RootBody>,
    body_scopes: Vec<BodyScope>,
    rows: SymbolRows<RootBinding>,
}

impl RootBindings {
    fn position(&self, body: BodyScope, symbol: SymbolHandle) -> Option<BindingPosition> {
        let mut positions = self
            .rows
            .get(symbol)
            .iter()
            .filter(|binding| self.bodies[binding.body as usize].scope == body)
            .map(|binding| binding.position);
        let position = positions.next()?;
        // Root aliases multiply published occurrences, not declarations inside
        // the one body this scoped query denotes.
        positions.all(|other| other == position).then_some(position)
    }

    fn local_positions(&self, symbol: SymbolHandle) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        self.rows.get(symbol).iter().filter_map(|binding| {
            let BindingPosition::Local(statement) = binding.position else {
                return None;
            };
            let body = &self.bodies[binding.body as usize];
            Some((body.machine, body.state, statement))
        })
    }
}

impl StateDeclarationIndex {
    fn body_ready(&self, state: &State) -> Option<&PreparedBindings> {
        let scope = body_scope(state);
        let position = self
            .bodies
            .binary_search_by_key(&scope, |(scope, _)| *scope)
            .ok()?;
        Some(&self.bodies[position].1)
    }

    fn ready_position(
        &self,
        program: &TypedTrees,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<Option<BindingPosition>> {
        // Outer None means unprepared. A ready refusal must not turn into a
        // fallback that chooses a declaration from another context.
        if let Some(body) = self.body_ready(state) {
            return Some(match body.rows.get(symbol) {
                [position] => Some(*position),
                _ => None,
            });
        }
        let roots = self.roots.as_ref()?;
        let body = body_scope(state);
        if !roots.scope.matches(program) || roots.body_scopes.binary_search(&body).is_err() {
            return None;
        }
        Some(roots.position(body, symbol))
    }

    fn body(
        &mut self,
        state: &State,
        parameters: &[StateParameter],
        statements: &[StatementNode],
    ) -> &PreparedBindings {
        let scope = body_scope(state);
        let position = match self
            .bodies
            .binary_search_by_key(&scope, |(scope, _)| *scope)
        {
            Ok(position) => position,
            Err(position) => {
                let mut rows = Vec::with_capacity(parameters.len() + statements.len());
                rows.extend(body_coordinates(parameters, statements));
                self.bodies.insert(
                    position,
                    (
                        scope,
                        PreparedBindings {
                            rows: SymbolRows::new(rows),
                            #[cfg(test)]
                            preparation: Default::default(),
                        },
                    ),
                );
                position
            }
        };
        &self.bodies[position].1
    }

    fn prepare_roots(&mut self, program: &TypedTrees) {
        let scope = RootScope::of(program);
        if self
            .roots
            .as_ref()
            .is_some_and(|roots| roots.scope.matches(program))
        {
            return;
        }
        self.roots =
            Some(
                self.build_roots(program, scope.clone())
                    .unwrap_or_else(|| RootBindings {
                        scope,
                        bodies: Vec::new(),
                        body_scopes: Vec::new(),
                        rows: SymbolRows::default(),
                    }),
            );
    }

    fn build_roots(&self, program: &TypedTrees, scope: RootScope) -> Option<RootBindings> {
        // Invalid metadata offers no declaration: a partial root projection
        // cannot prove uniqueness. The query caches that refusal, not a prefix.
        // A root view streams the same coordinates, borrowing selected bodies
        // when available. It does not allocate an index for every tiny body or
        // insert a reversed root roster into the selected family one by one.
        let mut bodies = Vec::new();
        let mut body_scopes = Vec::new();
        let mut rows = Vec::new();
        for (machine_position, machine) in program
            .tables
            .machines
            .span(program.roots.machines)?
            .iter()
            .enumerate()
        {
            for (state_position, state) in program
                .tables
                .machine_states
                .span(machine.states)?
                .iter()
                .enumerate()
            {
                let parameters = program.tables.state_parameters.span(state.parameters)?;
                let statements = program
                    .statement_table
                    .checked_statements(state.statement_nodes)?;
                let scope = body_scope(state);
                let indexed = parameters.len() + statements.len() > DIRECT_BINDING_ROWS;
                if indexed {
                    body_scopes.push(scope);
                }
                let mut body = None;
                let mut publish = |symbol, position| {
                    if !indexed && matches!(position, BindingPosition::Parameter(_)) {
                        return;
                    }
                    let body = *body.get_or_insert_with(|| {
                        let body =
                            u32::try_from(bodies.len()).expect("root binding body count overflow");
                        bodies.push(RootBody {
                            scope,
                            machine: machine_position as u32,
                            state: state_position as u32,
                        });
                        body
                    });
                    rows.push((symbol, RootBinding { body, position }));
                };
                if let Some(prepared) = self.body_ready(state) {
                    for (symbol, &position) in prepared.rows.rows() {
                        publish(symbol, position);
                    }
                } else {
                    for (symbol, position) in body_coordinates(parameters, statements) {
                        publish(symbol, position);
                    }
                }
            }
        }
        body_scopes.sort_unstable();
        body_scopes.dedup();
        let mut rows = SymbolRows::new(rows);
        rows.densify();
        Some(RootBindings {
            scope,
            bodies,
            body_scopes,
            rows,
        })
    }

    pub(crate) fn densify(&mut self) {
        for (_, body) in &mut self.bodies {
            body.rows.densify();
        }
    }
}

impl TypedTrees {
    fn binding_scope(&self) -> BindingScope {
        BindingScope {
            parameters: self.tables.state_parameters.revision(),
            statements: self.statement_table.binding_storage_identity(),
        }
    }

    fn with_root_bindings<R>(&self, read: impl FnOnce(&RootBindings) -> R) -> R {
        let mut read = Some(read);
        let ready = self.declaration_indexes.state_declarations.with(
            self.binding_scope(),
            StateDeclarationIndex::default,
            |index| {
                let roots = index.roots.as_ref()?;
                (roots.scope.matches(self))
                    .then(|| read.take().expect("root reader available")(roots))
            },
        );
        if let Some(result) = ready {
            return result;
        }
        self.declaration_indexes.state_declarations.with_mut(
            self.binding_scope(),
            StateDeclarationIndex::default,
            |index| {
                index.prepare_roots(self);
                read.take().expect("root reader not yet called")(
                    index.roots.as_ref().expect("root view prepared"),
                )
            },
        )
    }

    pub(crate) fn prepare_root_state_declarations(&self) {
        self.with_root_bindings(|_| ());
    }

    /// The declared type of one plain name in this state. This infers nothing
    /// and supplies no authority when the expression or declaration is absent.
    pub fn declared_binding_type(
        &self,
        state: &State,
        name: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if !self.expression_table.expression_is_valid(name) {
            return None;
        }
        let ExpressionNode::Name(path) = self.expression_table.expression(name) else {
            return None;
        };
        let symbol = path.resolved_symbol()?;
        if self
            .expression_table
            .checked_name_path_members(path.members)?
            .len()
            != 1
        {
            return None;
        }
        self.declared_binding_type_by_symbol(state, symbol)
    }

    /// The unique exact binder in this state's parameter and local rosters.
    pub fn declared_binding_type_by_symbol(
        &self,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        let parameters = self.tables.state_parameters.span(state.parameters)?;
        let statements = self
            .statement_table
            .checked_statements(state.statement_nodes)?;
        let position = self.binding_position(state, parameters, statements, symbol)?;
        let declared = match position {
            BindingPosition::Parameter(ordinal) => parameters.get(ordinal as usize)?.type_reference,
            BindingPosition::Local(ordinal) => {
                statements
                    .get(ordinal as usize)?
                    .as_local_data()?
                    .type_reference
            }
        };
        self.type_reference_table
            .contains_type_reference(declared)
            .then_some(declared)
    }

    pub fn state_local_declaration(
        &self,
        state: &State,
        symbol: SymbolHandle,
    ) -> Option<(usize, &TableLocalData)> {
        let parameters = self.tables.state_parameters.span(state.parameters)?;
        let statements = self
            .statement_table
            .checked_statements(state.statement_nodes)?;
        let BindingPosition::Local(ordinal) =
            self.binding_position(state, parameters, statements, symbol)?
        else {
            return None;
        };
        let ordinal = ordinal as usize;
        Some((ordinal, statements.get(ordinal)?.as_local_data()?))
    }

    fn binding_position(
        &self,
        state: &State,
        parameters: &[StateParameter],
        statements: &[StatementNode],
        symbol: SymbolHandle,
    ) -> Option<BindingPosition> {
        if !self.symbols.symbols().nodes().is_valid(symbol) {
            return None;
        }
        // The measured tiny-body route costs less than preparing or locking an
        // index. Bound its inspected rows so growth cannot make queries quadratic.
        if parameters.len() + statements.len() <= DIRECT_BINDING_ROWS {
            let mut declarations = parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol == symbol)
                .map(|(position, _)| BindingPosition::Parameter(position as u32))
                .chain(
                    statements
                        .iter()
                        .enumerate()
                        .filter(|(_, statement)| {
                            statement
                                .as_local_data()
                                .is_some_and(|local| local.symbol == symbol)
                        })
                        .map(|(position, _)| BindingPosition::Local(position as u32)),
                );
            let position = declarations.next()?;
            return declarations.next().is_none().then_some(position);
        }
        // Ready immutable roots use the sealed owner without taking its mutex.
        // Mutable construction prepares only the selected missing body.
        let ready = self.declaration_indexes.state_declarations.with(
            self.binding_scope(),
            StateDeclarationIndex::default,
            |index| index.ready_position(self, state, symbol),
        );
        match ready {
            Some(position) => position,
            None => self.declaration_indexes.state_declarations.with_mut(
                self.binding_scope(),
                StateDeclarationIndex::default,
                |index| match index.body(state, parameters, statements).rows.get(symbol) {
                    [position] => Some(*position),
                    _ => None,
                },
            ),
        }
    }

    pub fn local_data_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32, u32)> {
        self.with_root_bindings(|roots| roots.local_positions(symbol).collect())
    }

    /// Root aliases and duplicate binders retain their multiplicity, even if
    /// they denote the same prepared body span.
    pub fn unique_local_data_for_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&crate::statement::TableLocalData> {
        let (machine, state, statement) = self.with_root_bindings(|roots| {
            let mut positions = roots.local_positions(symbol);
            let position = positions.next()?;
            positions.next().is_none().then_some(position)
        })?;
        let machine = self
            .tables
            .machines
            .span(self.roots.machines)?
            .get(machine as usize)?;
        let state = self
            .tables
            .machine_states
            .span(machine.states)?
            .get(state as usize)?;
        self.statement_table
            .checked_statements(state.statement_nodes)?
            .get(statement as usize)?
            .as_local_data()
    }

    /// Preserve the legacy first-match query without retaining another copy
    /// of declared types. Owner-aware typing above reads declaration coordinates.
    pub fn first_named_value_declared_type(
        &self,
        symbol: SymbolHandle,
        head_symbol: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        if symbol.is_valid()
            && symbol == head_symbol
            && let Some((_, parameter)) = self
                .tables
                .state_parameters
                .iter()
                .find(|(_, parameter)| parameter.symbol == symbol)
        {
            return Some(parameter.type_reference);
        }
        let matches = |binder| {
            (symbol.is_valid() && binder == symbol)
                || (head_symbol.is_valid() && binder == head_symbol)
        };
        for machine in self.tables.machines.span(self.roots.machines)? {
            for owned in self.tables.machine_owned_data.span(machine.owned_data)? {
                if matches(owned.symbol) {
                    return Some(owned.type_reference);
                }
            }
            for state in self.tables.machine_states.span(machine.states)? {
                for parameter in self.tables.state_parameters.span(state.parameters)? {
                    if matches(parameter.symbol) {
                        return Some(parameter.type_reference);
                    }
                }
                for local in self
                    .statement_table
                    .checked_statements(state.statement_nodes)?
                    .iter()
                    .filter_map(StatementNode::as_local_data)
                {
                    if matches(local.symbol) {
                        return Some(local.type_reference);
                    }
                }
            }
        }
        for proposition in self.tables.propositions.span(self.roots.propositions)? {
            for parameter in self.tables.state_parameters.span(proposition.parameters)? {
                if matches(parameter.symbol) {
                    return Some(parameter.type_reference);
                }
            }
        }
        None
    }

    /// Match the full binder sequence in root order, including empty sequences
    /// and malformed duplicates; types and spellings do not change this query.
    pub fn state_position_for_parameter_symbols(
        &self,
        parameters: &[StateParameter],
    ) -> Option<(u32, u32)> {
        for (machine_position, machine) in self
            .tables
            .machines
            .span(self.roots.machines)?
            .iter()
            .enumerate()
        {
            for (state_position, state) in self
                .tables
                .machine_states
                .span(machine.states)?
                .iter()
                .enumerate()
            {
                let authored = self.tables.state_parameters.span(state.parameters)?;
                if authored.len() == parameters.len()
                    && authored
                        .iter()
                        .zip(parameters)
                        .all(|(left, right)| left.symbol == right.symbol)
                {
                    return Some((machine_position as u32, state_position as u32));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
