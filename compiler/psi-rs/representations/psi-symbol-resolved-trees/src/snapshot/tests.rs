use super::{MachineSupplySnapshot, SymbolResolvedTreesSnapshot};
use crate::SymbolResolvedTrees;
use crate::domain::DomainDefinition;
use crate::expression::ExpressionNode;
use crate::machine::{Machine, MachineStorage};
use crate::name::DiagnosticName;
use crate::state::{State, StateStorage};
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::types::TypeReference;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[test]
fn snapshots_materialize_resolved_roots_and_table_counts() {
    let mut program = SymbolResolvedTrees::default();
    let guard = program
        .tables
        .bodies
        .expressions
        .insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(1),
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
                source_span: Default::default(),
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
        service_reach_row: Default::default(),
        storage: MachineStorage {
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            decrease_view_arguments: HandleSpan::empty(),
            decrease_range: Default::default(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            states,
        },
    });
    program.domain_definitions.push(DomainDefinition {
        name: DiagnosticName::generated("i64::Km"),
        target_type: TypeReference::Unit,
        predicate_body: psi_language_semantics::DomainPredicateBody::Present,
        semantic_id: psi_language_semantics::SemanticDomainId(17),
        semantic_roles: psi_language_semantics::DomainSemanticRoles {
            denotation_dimension: Some(psi_language_semantics::SemanticDomainId(17)),
            arithmetic_policy: None,
        },
        establishment_routes: vec![
            psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition: psi_symbols::SymbolHandle::from_arena_index(19),
                requirement: psi_symbols::SymbolHandle::from_arena_index(20),
            },
        ],
        ..Default::default()
    });
    program.rebuild_tables();

    let snapshot = SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(&program);
    assert_eq!(snapshot.roots.machines.len(), 1);
    assert_eq!(
        snapshot.roots.domain_definitions[0].predicate_body,
        "present"
    );
    assert_eq!(
        snapshot.roots.machines[0].supply,
        MachineSupplySnapshot::CheckedBody
    );
    assert_eq!(snapshot.roots.machines[0].states.len(), 1);
    assert_eq!(snapshot.roots.domain_definitions[0].semantic_id, 17);
    assert_eq!(
        snapshot.roots.domain_definitions[0]
            .semantic_roles
            .denotation_dimension,
        Some(17)
    );
    assert_eq!(
        snapshot.roots.domain_definitions[0]
            .semantic_roles
            .arithmetic_policy,
        None
    );
    assert_eq!(
        snapshot.roots.domain_definitions[0].establishment_routes[0].kind,
        "checked_requirement"
    );
    assert_eq!(
        snapshot.roots.domain_definitions[0].establishment_routes[0].source_symbol,
        19
    );
    assert_eq!(
        snapshot.roots.domain_definitions[0].establishment_routes[0].requirement_symbol,
        Some(20)
    );
    assert_eq!(snapshot.tables.statement_count, 1);
    assert_eq!(snapshot.tables.expression_count, 1);
    assert_eq!(
        snapshot.tables.type_reference_count, 2,
        "the state return and domain carrier are both rebuilt into the type table"
    );
    assert!(snapshot.to_json_pretty().is_ok());
}
