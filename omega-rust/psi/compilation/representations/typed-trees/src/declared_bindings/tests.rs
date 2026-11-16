use super::*;
use crate::data::{DataDefinition, DataField, DataMember, DataVariant};
use crate::signature::StateParameter;
use crate::statement::TableLocalData;
use crate::types::TypeReferenceNode;
use arena::HandleSpan;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn graph(extra_locals: usize) -> (TypedTrees, State, [SymbolHandle; 4], TypeReferenceHandle) {
    let mut symbols = SymbolTableBuilder::default();
    let binders = std::array::from_fn(|_| {
        symbols.insert_root(SymbolKind::Local, SymbolNameRef::Static("same"))
    });
    let filler: Vec<_> = (0..extra_locals)
        .map(|_| symbols.insert_root(SymbolKind::Local, SymbolNameRef::Static("filler")))
        .collect();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let parameters = program.state_parameters.insert_many([StateParameter {
        symbol: binders[0],
        type_reference: reference,
        ..Default::default()
    }]);
    let statement_nodes =
        program
            .statement_table
            .insert_statements(std::iter::once(binders[1]).chain(filler).map(|symbol| {
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    type_reference: reference,
                    ..Default::default()
                })
            }));
    (
        program,
        State {
            parameters,
            statement_nodes,
            ..Default::default()
        },
        binders,
        reference,
    )
}

fn scanned(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !program.symbols.symbols().nodes().is_valid(symbol) {
        return None;
    }
    let mut rows = program
        .tables
        .state_parameters
        .span(state.parameters)?
        .iter()
        .map(|p| (p.symbol, p.type_reference))
        .chain(
            program
                .statement_table
                .checked_statements(state.statement_nodes)?
                .iter()
                .filter_map(StatementNode::as_local_data)
                .map(|p| (p.symbol, p.type_reference)),
        )
        .filter(|(binder, _)| *binder == symbol);
    let (_, reference) = rows.next()?;
    (rows.next().is_none()
        && program
            .type_reference_table
            .contains_type_reference(reference))
    .then_some(reference)
}

fn matches_scan(program: &TypedTrees, state: &State, symbols: &[SymbolHandle]) {
    for &symbol in symbols {
        assert_eq!(
            program.declared_binding_type_by_symbol(state, symbol),
            scanned(program, state, symbol)
        );
    }
}

#[test]
fn binding_lookup_preserves_owner_duplicates_and_generations() {
    for extra in [0, 64] {
        binding_owner_controls(extra);
    }
}

fn binding_owner_controls(extra: usize) {
    let (mut program, mut state, binders, reference) = graph(extra);
    matches_scan(&program, &state, &binders);
    let foreign = State {
        parameters: Default::default(),
        statement_nodes: Default::default(),
        ..state.clone()
    };
    assert_eq!(
        program.declared_binding_type_by_symbol(&foreign, binders[0]),
        None
    );
    let stale = SymbolHandle::from_parts(binders[1].arena_index(), binders[1].generation() + 1);
    assert_eq!(program.declared_binding_type_by_symbol(&state, stale), None);
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: binders[0],
            type_reference: reference,
            ..Default::default()
        }),
    );
    matches_scan(&program, &state, &binders);
    assert_eq!(
        program.declared_binding_type_by_symbol(&state, binders[0]),
        None
    );
    let local = state.statement_nodes.start();
    let StatementNode::LocalData(local) = program.statement_table.statement_mut(local) else {
        panic!("local")
    };
    local.type_reference =
        TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1);
    matches_scan(&program, &state, &binders);
    assert_eq!(
        program.declared_binding_type_by_symbol(&state, binders[1]),
        None
    );
    state.parameters = HandleSpan::from_parts(
        arena::Handle::from_parts(
            state.parameters.start().arena_index(),
            state.parameters.start().generation() + 1,
        ),
        state.parameters.count(),
    );
    matches_scan(&program, &state, &binders);
}

#[test]
fn binding_lookup_refreshes_same_shape_replacement_and_retained_snapshots() {
    for extra in [0, 64] {
        binding_snapshot_controls(extra);
    }
}

fn binding_snapshot_controls(extra: usize) {
    let (mut program, state, binders, _) = graph(extra);
    matches_scan(&program, &state, &binders);
    let snapshot = program.clone();
    let mut incoming = arena::Arena::default();
    incoming.insert(StateParameter {
        symbol: binders[2],
        type_reference: scanned(&program, &state, binders[0]).unwrap(),
        ..Default::default()
    });
    assert_ne!(incoming.revision(), program.state_parameters.revision());
    program.tables.state_parameters = incoming;
    matches_scan(&program, &state, &binders);
    matches_scan(&snapshot, &state, &binders);
    let mut incoming = crate::statement::StatementTable::new();
    let mut statements = snapshot
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    let StatementNode::LocalData(local) = &mut statements[0] else {
        panic!("local")
    };
    local.symbol = binders[3];
    incoming.insert_statements(statements);
    assert_ne!(
        incoming.declaration_revision(),
        program.statement_table.declaration_revision()
    );
    program.tables.statement_table = incoming;
    matches_scan(&program, &state, &binders);
    matches_scan(&snapshot, &state, &binders);
    let frozen = crate::FrozenTypedTrees::from(snapshot);
    matches_scan(&frozen, &state, &binders);
}

#[test]
fn expression_edits_retain_the_selected_body_preparation() {
    let (mut program, state, binders, _) = graph(64);
    matches_scan(&program, &state, &binders);
    let scope = BindingScope {
        parameters: program.state_parameters.revision(),
        statements: program.statement_table.binding_storage_identity(),
    };
    let preparation = || {
        program.declaration_indexes.state_declarations.with(
            scope.clone(),
            || panic!("prepared body"),
            |index| index.bodies[0].1.preparation.clone(),
        )
    };
    let prepared = preparation();
    program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    matches_scan(&program, &state, &binders);
    let after = program.declaration_indexes.state_declarations.with(
        scope,
        || panic!("retained body"),
        |index| index.bodies[0].1.preparation.clone(),
    );
    assert!(std::sync::Arc::ptr_eq(&prepared, &after));
}

#[test]
fn alternating_body_scopes_retain_both_preparations() {
    let (program, state, binders, _) = graph(64);
    let other = State {
        parameters: Default::default(),
        ..state.clone()
    };
    for _ in 0..3 {
        matches_scan(&program, &state, &binders);
        matches_scan(&program, &other, &binders);
    }
    let scope = BindingScope {
        parameters: program.state_parameters.revision(),
        statements: program.statement_table.binding_storage_identity(),
    };
    let prepared = program.declaration_indexes.state_declarations.with(
        scope.clone(),
        || panic!("prepared scopes"),
        |index| {
            index
                .bodies
                .iter()
                .map(|(_, body)| body.preparation.clone())
                .collect::<Vec<_>>()
        },
    );
    matches_scan(&program, &state, &binders);
    matches_scan(&program, &other, &binders);
    program.declaration_indexes.state_declarations.with(
        scope,
        || panic!("retained scopes"),
        |index| {
            assert_eq!(index.bodies.len(), 2);
            for (before, (_, after)) in prepared.iter().zip(&index.bodies) {
                assert!(std::sync::Arc::ptr_eq(before, &after.preparation));
            }
        },
    );
}

fn matches_global_roster(program: &TypedTrees, symbols: &[SymbolHandle]) {
    for &symbol in symbols {
        let expected = (|| -> Option<Vec<(u32, u32, u32)>> {
            let mut expected = Vec::new();
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
                    let _ = program.tables.state_parameters.span(state.parameters)?;
                    for (statement_position, statement) in program
                        .statement_table
                        .checked_statements(state.statement_nodes)?
                        .iter()
                        .enumerate()
                    {
                        if statement
                            .as_local_data()
                            .is_some_and(|local| local.symbol == symbol)
                        {
                            expected.push((
                                machine_position as u32,
                                state_position as u32,
                                statement_position as u32,
                            ));
                        }
                    }
                }
            }
            Some(expected)
        })()
        .unwrap_or_default();
        assert_eq!(program.local_data_positions_for_symbol(symbol), expected);
        let unique = match expected.as_slice() {
            &[(machine, state, statement)] => {
                let machine = &program.machines()[machine as usize];
                let state = &program.machine_states(machine)[state as usize];
                program.statement_table.statements(state.statement_nodes)[statement as usize]
                    .as_local_data()
            }
            _ => None,
        };
        assert_eq!(program.unique_local_data_for_symbol(symbol), unique);
    }
}

fn prepared_body(program: &TypedTrees, state: &State) -> std::sync::Arc<()> {
    program.declaration_indexes.state_declarations.with(
        program.binding_scope(),
        || panic!("prepared owner"),
        |index| {
            index
                .bodies
                .iter()
                .find(|(scope, _)| *scope == body_scope(state))
                .unwrap()
                .1
                .preparation
                .clone()
        },
    )
}

#[test]
fn published_and_detached_queries_share_rows_and_preserve_root_multiplicity() {
    let (mut program, state, binders, _) = graph(64);
    let mut statements = program
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    let StatementNode::LocalData(local) = &mut statements[0] else {
        panic!("local")
    };
    local.symbol = binders[3];
    let detached = State {
        statement_nodes: program.statement_table.insert_statements(statements),
        ..Default::default()
    };
    matches_scan(&program, &state, &binders);
    let prepared = prepared_body(&program, &state);
    matches_global_roster(&program, &binders);
    assert!(program.unique_local_data_for_symbol(binders[1]).is_none());
    let states = program.machine_states.insert_many([state.clone()]);
    program.push_machine(Machine {
        states,
        ..Default::default()
    });
    let roots = program.roots.machines;
    matches_global_roster(&program, &binders);
    assert_eq!(
        program
            .unique_local_data_for_symbol(binders[1])
            .unwrap()
            .symbol,
        binders[1]
    );
    // This body sorts before the published one but has a different first local.
    // Published coordinates must not refer to movable family-vector positions.
    matches_scan(&program, &detached, &binders);
    matches_global_roster(&program, &binders);
    assert_eq!(
        program
            .unique_local_data_for_symbol(binders[1])
            .unwrap()
            .symbol,
        binders[1]
    );
    program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    matches_global_roster(&program, &binders);
    assert!(std::sync::Arc::ptr_eq(
        &prepared,
        &prepared_body(&program, &state)
    ));
    program.push_machine(Machine {
        states,
        ..Default::default()
    });
    matches_global_roster(&program, &binders);
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
    let parameters = program.state_parameters(&state);
    assert_eq!(
        program.state_position_for_parameter_symbols(parameters),
        Some((0, 0))
    );
    program.roots.machines = HandleSpan::empty();
    matches_global_roster(&program, &binders);
    program.roots.machines = roots;
    matches_global_roster(&program, &binders);
    assert!(std::sync::Arc::ptr_eq(
        &prepared,
        &prepared_body(&program, &state)
    ));
    program.roots.machines = HandleSpan::from_parts(
        arena::Handle::from_parts(roots.start().arena_index(), roots.start().generation() + 1),
        roots.count(),
    );
    matches_global_roster(&program, &binders);
    program.roots.machines = roots;
    matches_global_roster(&crate::FrozenTypedTrees::from(program), &binders);
}

#[test]
fn published_rows_refresh_same_shape_storage_and_state_span_edits() {
    let (mut program, state, binders, _) = graph(64);
    let mut statements = program
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    let StatementNode::LocalData(local) = &mut statements[0] else {
        panic!("local")
    };
    local.symbol = binders[3];
    let other = State {
        statement_nodes: program.statement_table.insert_statements(statements),
        ..state.clone()
    };
    let states = program
        .machine_states
        .insert_many([state.clone(), other.clone()]);
    let first = HandleSpan::from_parts(states.start(), 1);
    let second = HandleSpan::from_parts(states.handle_at(1), 1);
    program.push_machine(Machine {
        states: first,
        ..Default::default()
    });
    matches_scan(&program, &state, &binders);
    matches_global_roster(&program, &binders);
    let snapshot = program.clone();
    let prepared = prepared_body(&program, &state);
    let mut incoming = arena::Arena::default();
    incoming.insert_many([other.clone(), state.clone()]);
    assert_ne!(incoming.revision(), program.machine_states.revision());
    program.tables.machine_states = incoming;
    matches_global_roster(&program, &binders);
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
    assert!(program.unique_local_data_for_symbol(binders[3]).is_some());
    matches_global_roster(&snapshot, &binders);
    program.tables.machine_states = snapshot.tables.machine_states.clone();
    matches_global_roster(&program, &binders);
    program
        .tables
        .machine_states
        .get_mut(states.start())
        .statement_nodes = other.statement_nodes;
    matches_global_roster(&program, &binders);
    program
        .tables
        .machine_states
        .get_mut(states.start())
        .statement_nodes = state.statement_nodes;
    matches_global_roster(&program, &binders);
    let mut incoming = arena::Arena::default();
    incoming.append_to_span(
        &mut HandleSpan::empty(),
        Machine {
            states: second,
            ..Default::default()
        },
    );
    assert_ne!(incoming.revision(), program.machines.revision());
    *program.tables.machines = incoming.into();
    matches_global_roster(&program, &binders);
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
    assert!(program.unique_local_data_for_symbol(binders[3]).is_some());
    assert!(std::sync::Arc::ptr_eq(
        &prepared,
        &prepared_body(&program, &state)
    ));
    matches_global_roster(&crate::FrozenTypedTrees::from(snapshot), &binders);
}

#[test]
fn frozen_published_reads_do_not_take_the_mutable_owner_lock() {
    let (mut program, state, binders, reference) = graph(64);
    let states = program.machine_states.insert_many([state.clone()]);
    program.push_machine(Machine {
        states,
        ..Default::default()
    });
    let program = crate::FrozenTypedTrees::from(program);
    let index = &program.declaration_indexes.state_declarations;
    index.with(
        program.binding_scope(),
        || panic!("sealed owner"),
        |index| {
            assert!(
                index.bodies.is_empty(),
                "publishing must not materialize every body index"
            );
        },
    );
    let observed = std::thread::scope(|threads| {
        index.with_mut(
            program.binding_scope(),
            StateDeclarationIndex::default,
            |_| {
                let (send, receive) = std::sync::mpsc::channel();
                let program = &program;
                let state = &state;
                threads.spawn(move || {
                    let scoped = program.declared_binding_type_by_symbol(state, binders[1]);
                    let global = program
                        .unique_local_data_for_symbol(binders[1])
                        .map(|local| local.symbol);
                    send.send((scoped, global)).unwrap();
                });
                receive.recv_timeout(std::time::Duration::from_secs(2))
            },
        )
    })
    .expect("sealed reads must finish while the mutable slot is locked");
    assert_eq!(observed, (Some(reference), Some(binders[1])));
}

#[test]
fn legacy_named_value_and_signature_queries_keep_source_order() {
    let (mut program, state, binders, reference) = graph(0);
    let other = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let owned_data = program
        .machine_owned_data
        .insert_many([crate::machine::OwnedData {
            symbol: binders[1],
            type_reference: other,
            ..Default::default()
        }]);
    let states = program
        .machine_states
        .insert_many([state.clone(), state.clone()]);
    program.push_machine(Machine {
        states,
        owned_data,
        ..Default::default()
    });
    assert_eq!(
        program.first_named_value_declared_type(binders[0], binders[1]),
        Some(other)
    );
    assert_eq!(
        program.first_named_value_declared_type(binders[0], binders[0]),
        Some(reference)
    );
    assert_eq!(
        program.first_named_value_declared_type(SymbolHandle::invalid(), SymbolHandle::invalid()),
        None
    );
    let mut requested = program.state_parameters(&state).to_vec();
    requested[0].type_reference = other;
    assert_eq!(
        program.state_position_for_parameter_symbols(&requested),
        Some((0, 0))
    );
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
}

#[test]
fn call_annotations_retain_rows_but_statement_replacement_invalidates() {
    let (mut program, mut state, binders, _) = graph(64);
    let mut statements = program
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    statements.push(StatementNode::Call(Default::default()));
    state.statement_nodes = program.statement_table.insert_statements(statements);
    let call = state
        .statement_nodes
        .handle_at(state.statement_nodes.count() - 1);
    matches_scan(&program, &state, &binders);
    let prepared = prepared_body(&program, &state);
    let revision = program.statement_table.declaration_revision();
    for _ in 0..128 {
        program
            .statement_table
            .call_mut(call)
            .unwrap()
            .authored_machine_application = Default::default();
        matches_scan(&program, &state, &binders);
        assert!(std::sync::Arc::ptr_eq(
            &prepared,
            &prepared_body(&program, &state)
        ));
    }
    assert!(program.statement_table.declaration_revision() > revision);
    assert!(
        program
            .statement_table
            .call_mut(state.statement_nodes.start())
            .is_none()
    );
    let stale = arena::Handle::from_parts(call.arena_index(), call.generation() + 1);
    assert!(program.statement_table.call_mut(stale).is_none());
    let local = program
        .statement_table
        .statement(state.statement_nodes.start())
        .clone();
    *program.statement_table.statement_mut(call) = local;
    matches_scan(&program, &state, &binders);
    assert!(!std::sync::Arc::ptr_eq(
        &prepared,
        &prepared_body(&program, &state)
    ));
    assert!(
        program
            .declared_binding_type_by_symbol(&state, binders[1])
            .is_none()
    );
}

#[test]
fn statement_binding_identity_is_not_logical_or_recorded_data() {
    let (mut program, state, binders, _) = graph(64);
    matches_scan(&program, &state, &binders);
    let key = program.binding_scope();
    let original = program.statement_table.clone();
    let bytes =
        compiler_storage::encode(&original, compiler_storage::RecordLimits::default()).unwrap();
    program
        .statement_table
        .statement_mut(state.statement_nodes.start());
    assert!(key != program.binding_scope());
    assert_eq!(original, program.statement_table);
    assert_eq!(
        bytes,
        compiler_storage::encode(
            &program.statement_table,
            compiler_storage::RecordLimits::default()
        )
        .unwrap()
    );
    let restored: crate::statement::StatementTable =
        compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(original, restored);
    assert!(!std::sync::Arc::ptr_eq(
        &original.binding_storage_identity(),
        &restored.binding_storage_identity()
    ));
}

fn scan_field(program: &TypedTrees, symbol: SymbolHandle) -> Option<&DataField> {
    let mut fields = Vec::new();
    for data in program
        .tables
        .data_definitions
        .span(program.roots.data_definitions)?
    {
        for member in program.tables.data_members.span(data.members)? {
            match member {
                DataMember::Field(field) => fields.push(field),
                DataMember::Variant(variant) => {
                    fields.extend(program.tables.data_payload_fields.span(variant.payload)?)
                }
            }
        }
    }
    let mut fields = fields.into_iter().filter(|field| field.symbol == symbol);
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

#[test]
fn stale_statement_and_name_spans_refuse_small_and_indexed_queries() {
    for extra in [0, 64] {
        let (mut program, mut state, binders, reference) = graph(extra);
        matches_scan(&program, &state, &binders);
        let statements = state.statement_nodes;
        state.statement_nodes = HandleSpan::from_parts(
            arena::Handle::from_parts(
                statements.start().arena_index(),
                statements.start().generation() + 1,
            ),
            statements.count(),
        );
        matches_scan(&program, &state, &binders);
        assert_eq!(
            program.declared_binding_type_by_symbol(&state, binders[0]),
            None
        );
        state.statement_nodes = statements;
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, crate::name::Identifier::generated("same"));
        let name = program.expression_table.insert(ExpressionNode::Name(
            crate::expression::TableNamePath::new(
                members,
                Default::default(),
                binders[0],
                binders[0],
            ),
        ));
        assert_eq!(program.declared_binding_type(&state, name), Some(reference));
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(name) else {
            panic!("name")
        };
        path.members = HandleSpan::from_parts(
            arena::Handle::from_parts(
                members.start().arena_index(),
                members.start().generation() + 1,
            ),
            members.count(),
        );
        assert_eq!(program.declared_binding_type(&state, name), None);
    }
}

#[test]
fn incomplete_root_projection_refuses_every_prefix_and_refreshes() {
    let (mut program, state, binders, _) = graph(64);
    let states = program
        .machine_states
        .insert_many([state.clone(), state.clone()]);
    program.push_machine(Machine {
        states: HandleSpan::from_parts(states.start(), 1),
        ..Default::default()
    });
    assert!(program.unique_local_data_for_symbol(binders[1]).is_some());
    let snapshot = crate::FrozenTypedTrees::from(program.clone());
    program
        .tables
        .machines
        .get_mut(program.roots.machines.start())
        .states = states;
    program
        .tables
        .machine_states
        .get_mut(states.handle_at(1))
        .statement_nodes = HandleSpan::from_parts(
        arena::Handle::from_parts(
            state.statement_nodes.start().arena_index(),
            state.statement_nodes.start().generation() + 1,
        ),
        state.statement_nodes.count(),
    );
    matches_global_roster(&program, &binders);
    assert!(
        program
            .local_data_positions_for_symbol(binders[1])
            .is_empty()
    );
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
    assert!(snapshot.unique_local_data_for_symbol(binders[1]).is_some());
    program
        .tables
        .machines
        .get_mut(program.roots.machines.start())
        .states = HandleSpan::from_parts(states.start(), 1);
    assert!(program.unique_local_data_for_symbol(binders[1]).is_some());
    program
        .tables
        .machines
        .get_mut(program.roots.machines.start())
        .states = HandleSpan::from_parts(
        arena::Handle::from_parts(
            states.start().arena_index(),
            states.start().generation() + 1,
        ),
        1,
    );
    assert_eq!(program.unique_local_data_for_symbol(binders[1]), None);
}

#[test]
fn incomplete_field_relation_refuses_every_prefix_and_refreshes() {
    let mut symbols = SymbolTableBuilder::default();
    let fields = std::array::from_fn::<_, 2, _>(|_| {
        symbols.insert_root(SymbolKind::Field, SymbolNameRef::Static("field"))
    });
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let first = program
        .data_members
        .insert_many([DataMember::Field(DataField {
            symbol: fields[0],
            ..Default::default()
        })]);
    let payload = program.data_payload_fields.insert_many([DataField {
        symbol: fields[1],
        ..Default::default()
    }]);
    let second = program
        .data_members
        .insert_many([DataMember::Variant(DataVariant {
            payload,
            ..Default::default()
        })]);
    program.push_data_definition(DataDefinition {
        members: first,
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        members: second,
        ..Default::default()
    });
    assert!(program.unique_declared_field_by_symbol(fields[0]).is_some());
    let snapshot = crate::FrozenTypedTrees::from(program.clone());
    let DataMember::Variant(variant) = program.tables.data_members.get_mut(second.start()) else {
        panic!("variant")
    };
    variant.payload = HandleSpan::from_parts(
        arena::Handle::from_parts(
            payload.start().arena_index(),
            payload.start().generation() + 1,
        ),
        payload.count(),
    );
    assert_eq!(program.unique_declared_field_by_symbol(fields[0]), None);
    assert_eq!(program.unique_declared_field_by_symbol(fields[1]), None);
    assert!(
        program
            .data_field_positions_for_symbol(fields[0])
            .is_empty()
    );
    let DataMember::Variant(variant) = program.tables.data_members.get_mut(second.start()) else {
        panic!("variant")
    };
    variant.payload = payload;
    assert!(program.unique_declared_field_by_symbol(fields[0]).is_some());
    let definition = program.roots.data_definitions.handle_at(1);
    program.tables.data_definitions.get_mut(definition).members = HandleSpan::from_parts(
        arena::Handle::from_parts(
            second.start().arena_index(),
            second.start().generation() + 1,
        ),
        second.count(),
    );
    assert_eq!(program.unique_declared_field_by_symbol(fields[0]), None);
    program.tables.data_definitions.get_mut(definition).members = second;
    assert!(program.unique_declared_field_by_symbol(fields[0]).is_some());
    assert!(
        snapshot
            .unique_declared_field_by_symbol(fields[0])
            .is_some()
    );
    assert!(
        snapshot
            .unique_declared_field_by_symbol(fields[1])
            .is_some()
    );
}

#[test]
fn field_lookup_preserves_payload_uniqueness_and_storage_changes() {
    let mut symbols = SymbolTableBuilder::default();
    let fields = std::array::from_fn::<_, 3, _>(|_| {
        symbols.insert_root(SymbolKind::Field, SymbolNameRef::Static("same"))
    });
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let payload = program.data_payload_fields.insert_many([DataField {
        symbol: fields[1],
        ..Default::default()
    }]);
    let members = program.data_members.insert_many([
        DataMember::Field(DataField {
            symbol: fields[0],
            ..Default::default()
        }),
        DataMember::Variant(DataVariant {
            payload,
            ..Default::default()
        }),
    ]);
    program.push_data_definition(DataDefinition {
        members,
        ..Default::default()
    });
    let assert_fields = |program: &TypedTrees| {
        for &symbol in &fields {
            assert_eq!(
                program.unique_declared_field_by_symbol(symbol),
                scan_field(program, symbol)
            );
        }
    };
    assert_fields(&program);
    let snapshot = program.clone();
    let mut incoming = arena::Arena::default();
    incoming.insert_many([
        DataMember::Field(DataField {
            symbol: fields[2],
            ..Default::default()
        }),
        DataMember::Variant(DataVariant {
            payload,
            ..Default::default()
        }),
    ]);
    assert_ne!(incoming.revision(), program.data_members.revision());
    program.tables.data_members = incoming;
    assert_fields(&program);
    assert_fields(&snapshot);
    program.data_payload_fields.get_mut(payload.start()).symbol = fields[2];
    assert_fields(&program);
    assert_eq!(program.unique_declared_field_by_symbol(fields[2]), None);
    let stale = SymbolHandle::from_parts(fields[0].arena_index(), fields[0].generation() + 1);
    assert_eq!(program.unique_declared_field_by_symbol(stale), None);
    let roster = program.roots.data_definitions;
    program.roots.data_definitions = HandleSpan::from_parts(
        arena::Handle::from_parts(
            roster.start().arena_index(),
            roster.start().generation() + 1,
        ),
        roster.count(),
    );
    assert_fields(&program);
    program.roots.data_definitions = roster;
    assert_fields(&program);
    assert_fields(&crate::FrozenTypedTrees::from(snapshot));
}
