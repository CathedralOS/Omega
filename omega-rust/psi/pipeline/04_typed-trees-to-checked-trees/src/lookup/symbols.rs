use ::symbols::SymbolHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use std::cell::RefCell;

pub(crate) fn machine_state_count(program: &typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

// The machine table is queried once per expression through this module, so
// each program's first query indexes every machine symbol by its first
// position and later queries, hit or miss, probe that index. Freshness is the
// owner's `ProgramIdentity` AND the machine arena's own revision: the counter
// moves on every mutation-capable call, including same-length in-place
// rewrites, so a stale index — hit or miss — can never outlive the table it
// describes. Monomorphization appends machines mid-compile; each append
// rebuilds the index on the next query.
struct MachineIndex {
    owner: typed_trees::ProgramIdentity,
    revision: u64,
    positions: ::symbols::SymbolMap<usize>,
}

impl MachineIndex {
    fn of(program: &typed_trees::TypedTrees) -> Self {
        let machines = program.machines();
        let mut positions = ::symbols::SymbolMap::default();
        positions.reserve(machines.len());
        for (position, machine) in machines.iter().enumerate() {
            positions.entry(machine.symbol).or_insert(position);
        }
        Self {
            owner: program.identity,
            revision: program.tables.machines.revision(),
            positions,
        }
    }

    fn is_fresh_for(&self, program: &typed_trees::TypedTrees) -> bool {
        self.owner.get() == program.identity.get()
            && self.revision == program.tables.machines.revision()
    }
}

thread_local! {
    static MACHINE_INDEX: RefCell<Option<MachineIndex>> = const { RefCell::new(None) };
}

fn machine_index_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    MACHINE_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        if !slot
            .as_ref()
            .is_some_and(|index| index.is_fresh_for(program))
        {
            *slot = Some(MachineIndex::of(program));
        }
        slot.as_ref()?.positions.get(&symbol).copied()
    })
}

pub(crate) fn machine_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    machine_index_by_symbol(program, symbol).map(|index| &program.machines()[index])
}

// State tables are queried once per crash-entry operand through the facts
// builders: a per-machine `.find` rescans every state of that machine per
// query. Cache each (machine, state) query's exact scan verdict per program.
// Freshness is the owner's `ProgramIdentity` plus the machines and
// machine_states arena revisions — the counters move on every mutation,
// appends and in-place rewrites alike, so verdicts rebuild only when the
// tables they describe actually changed.
thread_local! {
    static STATE_INDEX: RefCell<
        Option<(
            typed_trees::ProgramIdentity,
            u64,
            u64,
            symbols::SymbolKeyMap<
                (SymbolHandle, SymbolHandle),
                Option<usize>,
            >,
        )>,
    > = const { RefCell::new(None) };
}

pub(crate) fn state_index_by_symbol(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    let states = program.machine_states(machine);
    STATE_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let machines_revision = program.tables.machines.revision();
        let states_revision = program.tables.machine_states.revision();
        let stale = match &*slot {
            Some((owner, seen_machines, seen_states, _)) => {
                owner.get() != program.identity.get()
                    || *seen_machines != machines_revision
                    || *seen_states != states_revision
            }
            None => true,
        };
        if stale {
            *slot = Some((
                program.identity,
                machines_revision,
                states_revision,
                symbols::SymbolKeyMap::default(),
            ));
        }
        let Some((_, _, _, verdicts)) = &mut *slot else {
            return None;
        };
        *verdicts
            .entry((machine.symbol, symbol))
            .or_insert_with(|| states.iter().position(|state| state.symbol == symbol))
    })
}

// Member resolution looks up a data declaration by its symbol once per
// member hop: a whole-table `.find` rescans every data definition per hop.
// Cache each query's exact scan verdict per program under the same law as
// the machine index — freshness is the owner's identity plus the
// data-definition arena's own revision, so a recycled `TypedTrees` or a
// same-length rewrite can never be served stale verdicts.
thread_local! {
    static DATA_DEF_INDEX: RefCell<
        Option<(
            typed_trees::ProgramIdentity,
            u64,
            symbols::SymbolKeyMap<SymbolHandle, Option<usize>>,
        )>,
    > = const { RefCell::new(None) };
}

fn data_definition_index_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    DATA_DEF_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let revision = program.tables.data_definitions.revision();
        let stale = match &*slot {
            Some((owner, seen, _)) => owner.get() != program.identity.get() || *seen != revision,
            None => true,
        };
        if stale {
            *slot = Some((program.identity, revision, symbols::SymbolKeyMap::default()));
        }
        let Some((_, _, verdicts)) = &mut *slot else {
            return None;
        };
        *verdicts.entry(symbol).or_insert_with(|| {
            program
                .data_definitions()
                .iter()
                .position(|definition| definition.symbol == symbol)
        })
    })
}

pub(crate) fn data_definition_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    data_definition_index_by_symbol(program, symbol).map(|index| &program.data_definitions()[index])
}

// Whether a data declaration owns an attached `::drop` machine is queried
// per field visit through the contents classifier: a whole-machine `.any`
// per query rescans every machine. Cache each data symbol's verdict per
// program; freshness is the owner identity plus the machines arena's own
// revision, matching the machine index's law.
thread_local! {
    static DROP_HOOK_INDEX: RefCell<
        Option<(
            typed_trees::ProgramIdentity,
            u64,
            symbols::SymbolKeyMap<SymbolHandle, bool>,
        )>,
    > = const { RefCell::new(None) };
}

pub(crate) fn attached_drop_machine_exists(
    program: &typed_trees::TypedTrees,
    data_symbol: SymbolHandle,
) -> bool {
    if !data_symbol.is_valid() {
        return false;
    }
    DROP_HOOK_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let revision = program.tables.machines.revision();
        let stale = match &*slot {
            Some((owner, seen, _)) => owner.get() != program.identity.get() || *seen != revision,
            None => true,
        };
        if stale {
            *slot = Some((program.identity, revision, symbols::SymbolKeyMap::default()));
        }
        let Some((_, _, verdicts)) = &mut *slot else {
            return false;
        };
        *verdicts.entry(data_symbol).or_insert_with(|| {
            program.machines().iter().any(|candidate| {
                candidate.attached_data_symbol == data_symbol
                    && candidate.name.as_str().ends_with("::drop")
            })
        })
    })
}

pub(crate) fn state_by_symbol<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::state::State> {
    state_index_by_symbol(program, machine, symbol)
        .map(|index| &program.machine_states(machine)[index])
}

pub(crate) fn machine_state_by_symbol(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let machine = machine_by_symbol(program, machine_symbol)?;
    let state = state_by_symbol(program, machine, state_symbol)?;
    Some((machine, state))
}

pub(crate) fn machine_symbol_from_type_reference_handle(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        // A `dyn Trait` retains the trait symbol. Exact checked conformance rows
        // drive devirtualization and descriptor lowering; changing this symbol
        // to a coincidentally unique carrier would discard that identity and
        // restore attached-machine discovery.
        typed_trees::types::TypeReferenceNode::DynamicTrait {
            symbol: trait_symbol,
            ..
        } => *trait_symbol,
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Borrow(inner) => {
            expression_root_symbol(inner.target, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

pub(crate) fn first_valid_name_path_symbol(
    path: &typed_trees::expression::TableNamePath,
    expressions: &typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}
