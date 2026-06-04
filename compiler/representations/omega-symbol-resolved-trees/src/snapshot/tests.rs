use super::SymbolResolvedTreesSnapshot;
use crate::SymbolResolvedTrees;
use crate::expression::ExpressionNode;
use crate::machine::{Machine, MachineStorage};
use crate::name::DiagnosticName;
use crate::state::{State, StateStorage};
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::types::TypeReference;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[test]
fn snapshots_materialize_resolved_roots_and_table_counts() {
    let mut program = SymbolResolvedTrees::default();
    let guard = program
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Integer(1));
    let statements =
        program
            .tables
            .declarations
            .state_statements
            .insert_many([Statement::Transition(Transition {
                target: TransitionTarget::Terminal,
                continuation: None,
                guard: TransitionGuard::When(guard),
            })]);
    let state = program.tables.declarations.machine_states.append(State {
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("entry"),
        storage: StateStorage {
            parameters: HandleSpan::empty(),
            return_type: Some(TypeReference::Named {
                symbol: SymbolHandle::invalid(),
                name: DiagnosticName::generated("i32"),
            }),
            statements,
            statement_nodes: HandleSpan::empty(),
        },
    });
    let states = program
        .tables
        .declarations
        .machine_state_handles
        .insert_many([state]);
    program.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("main"),
        attached_data: None,
        storage: MachineStorage {
            type_parameters: HandleSpan::empty(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states,
        },
    });
    program.rebuild_tables();

    let snapshot = SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(&program);
    assert_eq!(snapshot.roots.machines.len(), 1);
    assert_eq!(snapshot.roots.machines[0].states.len(), 1);
    assert_eq!(snapshot.tables.statement_count, 1);
    assert_eq!(snapshot.tables.expression_count, 1);
    assert_eq!(snapshot.tables.type_reference_count, 1);
    assert!(snapshot.to_json_pretty().is_ok());
}
