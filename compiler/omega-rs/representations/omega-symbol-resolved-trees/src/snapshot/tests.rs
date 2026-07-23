use super::SymbolResolvedTreesSnapshot;
use crate::SymbolResolvedTrees;
use crate::domain::DomainDefinition;
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
        .insert(ExpressionNode::Integer(
            omega_core::literals::IntegerLiteral::from_value(1),
        ));
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
            contracts: HandleSpan::empty(),
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
        boundary: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        effect_row: Default::default(),
        service_reach_row: Default::default(),
        storage: MachineStorage {
            type_parameters: HandleSpan::empty(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            decrease_view_arguments: HandleSpan::empty(),
            decrease_range: Default::default(),
            effects: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states,
        },
    });
    program.domain_definitions.push(DomainDefinition {
        name: DiagnosticName::generated("i64::Km"),
        target_type: TypeReference::Unit,
        semantic_id: omega_core::semantics::SemanticDomainId(17),
        facets: omega_core::semantics::DomainFacets {
            predicate: true,
            semantic: Some(omega_core::semantics::SemanticDomainId(17)),
        },
        ..Default::default()
    });
    program.rebuild_tables();

    let snapshot = SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(&program);
    assert_eq!(snapshot.roots.machines.len(), 1);
    assert_eq!(snapshot.roots.machines[0].states.len(), 1);
    assert_eq!(snapshot.roots.domain_definitions[0].semantic_id, 17);
    assert!(snapshot.roots.domain_definitions[0].facets.predicate);
    assert_eq!(
        snapshot.roots.domain_definitions[0].facets.semantic,
        Some(17)
    );
    assert_eq!(snapshot.tables.statement_count, 1);
    assert_eq!(snapshot.tables.expression_count, 1);
    assert_eq!(
        snapshot.tables.type_reference_count, 2,
        "the state return and domain carrier are both rebuilt into the type table"
    );
    assert!(snapshot.to_json_pretty().is_ok());
}
