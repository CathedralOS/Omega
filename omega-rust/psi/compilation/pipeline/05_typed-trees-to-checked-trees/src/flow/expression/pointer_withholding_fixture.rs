//! Build the pointer-withholding test through ordinary range evaluation.
//! The fixture owns storage, but publishes no incoming or call permission.

use super::Execution;
use crate::flow::context::FlowBuildContext;
use arena::{Handle, HandleSpan};
use checked_trees::{
    CheckFacts, CheckedScalarExpressionPlans, FlowOperandFact, FlowStateFact, FlowStatementFact,
    FlowStorageOperation, FlowStorageSnapshot, StateBorrowFact,
};
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use language_core::ReferenceAccess;
use numerics::literals::IntegerLiteral;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionNode, TableIndexedExpression, TableNamePath, TableRangeExpression},
    machine::Machine,
    name::Identifier,
    signature::StateParameter,
    state::State,
    statement::StatementNode,
    types::{FixedArrayLength, TypeReferenceNode},
};

pub(crate) struct ProducedRangeProjection {
    pub(crate) program: TypedTrees,
    pub(crate) facts: CheckFacts,
    pub(crate) state: Handle<FlowStateFact>,
    pub(crate) whole: Handle<FlowOperandFact>,
    pub(crate) projected: Handle<FlowOperandFact>,
    pub(crate) original_address: validation::CanonicalPlace,
}

pub(crate) fn produce_builtin_range() -> ProducedRangeProjection {
    let mut symbols = SymbolTableBuilder::default();
    let byte = symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u8"));
    let machine_symbol =
        symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("range_owner"));
    let state_symbol = symbols
        .insert_children(
            machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .start();
    let container = symbols
        .insert_children(
            state_symbol,
            [(SymbolKind::Parameter, SymbolNameRef::Static("descriptors"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let byte = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: byte,
            name: Identifier::from("u8"),
        });
    let descriptor = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: byte,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: descriptor,
            length: FixedArrayLength::Literal(2),
        });
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: descriptor,
        });
    let view = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: slice,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    let mut names = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut names, Identifier::from("descriptors"));
    let mut name_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut name_symbols, container);
    let collection = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath::new(
            names,
            name_symbols,
            container,
            container,
        )));
    let start = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let end = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
    let range = program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive: false,
        }));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index: range,
            }));
    let statements = program
        .statement_table
        .insert_statements([StatementNode::Expression(indexed)]);
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::from("entry"),
        statement_nodes: statements,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: container,
            name: Identifier::from("descriptors"),
            type_reference: array,
            ..Default::default()
        },
    );
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::from("range_owner"),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state.clone());
    program.push_machine(machine.clone());
    assert!(validation::indexed_operation_has_builtin_meaning(
        &program,
        &machine,
        Some(&state),
        indexed,
        Some(array),
    ));
    for bound in [start, end] {
        assert!(validation::has_builtin_bound_expression_meaning(
            &program,
            &machine,
            Some(&state),
            bound,
        ));
    }

    let mut facts = CheckFacts::default();
    // This empty row supplies the declaration scope for owned storage only.
    // It contains no incoming grant, loan, call, or completed-call authority.
    facts.borrow.states.insert(StateBorrowFact {
        machine_symbol,
        state_symbol,
        ..Default::default()
    });
    let scalar_expressions = CheckedScalarExpressionPlans::default();
    let mutations = crate::flow::StateMutationSummaryCache::default();
    let mut semantic = FactPlan::default();
    let mut context = FlowBuildContext::new(
        &program,
        &facts.borrow,
        &facts.proof,
        &semantic,
        &scalar_expressions,
        &facts.operators,
        None,
        &mutations,
    );
    context.begin_output_page(2);
    let entry = context
        .control
        .storage_snapshots
        .insert(FlowStorageSnapshot {
            operation: FlowStorageOperation::Entry,
            ..Default::default()
        });
    let statement_rows = context.control.statements.insert_many([FlowStatementFact {
        storage_snapshot: entry,
        ..Default::default()
    }]);
    let mut state_calls = HandleSpan::empty();
    let mut contexts = HandleSpan::empty();
    let mut constraints = HandleSpan::empty();
    let (whole, projected, original_address) = {
        let mut execution = Execution::new(
            &program,
            &facts.borrow,
            &facts.proof,
            &facts.domains,
            &machine,
            &state,
            0,
            statement_rows.start(),
            &mut semantic,
            &mut context,
            &mut state_calls,
            &[],
            &mut contexts,
            &mut constraints,
        );
        // This evaluates the Name and both bounds, then calls the real
        // capture_indexed_referents producer from the Indexed expression arm.
        let value = execution
            .evaluated_operand_as(indexed, view, &mut contexts, &mut constraints)
            .capture;
        assert_eq!(value.expression, indexed);
        assert_eq!(value.storage_snapshot, entry);
        assert_eq!(value.state_symbol, state_symbol);
        let receiver = *execution.context.control.operands.get(value.backing);
        assert_eq!(receiver.expression, collection);
        assert_eq!(receiver.storage_snapshot, entry);
        assert!(!receiver.referents.is_empty());
        let inputs = value.evaluated_inputs.handles().collect::<Vec<_>>();
        assert_eq!(inputs.len(), 2);
        for (input, expression, coordinate) in [(inputs[0], start, 0), (inputs[1], end, 2)] {
            let input = execution.context.control.operands.get(input);
            assert_eq!(input.expression, expression);
            assert_eq!(execution.captured_coordinate(input), Some(coordinate));
        }
        let whole = execution.context.control.operands.insert(value);
        let formed = execution.project_call_place(whole, &[], entry).unwrap();
        assert_eq!(formed.addresses.len(), 1);
        let original_address = formed.addresses[0].place.clone();
        assert_eq!(original_address.root, PlaceRoot::Symbol(container));
        assert_eq!(
            original_address.segments,
            [PlaceSegment::FixedRange { start: 0, end: 2 }],
        );
        let whole_slots = execution.call_slot_values(whole, &[], entry).unwrap();
        assert_eq!(whole_slots, [(whole, Vec::new())]);
        let path = [PlaceSegment::FixedIndex { index: 0 }];
        let slots = execution.call_slot_values(whole, &path, entry).unwrap();
        assert_eq!(slots.len(), 1);
        let (projected, projection) = &slots[0];
        assert_ne!(*projected, whole);
        assert_eq!(projection, &path);
        let projected_value = execution.context.control.operands.get(*projected);
        assert_eq!(projected_value.value_source, whole);
        assert_eq!(projected_value.storage_snapshot, entry);
        assert!(
            execution
                .project_call_place(*projected, projection, entry)
                .unwrap()
                .addresses
                .is_empty(),
        );
        let unknown = execution
            .context
            .control
            .storage_snapshots
            .insert(FlowStorageSnapshot::default());
        for cut in [Handle::invalid(), unknown] {
            assert!(execution.call_slot_values(whole, &path, cut).is_none());
        }
        (whole, *projected, original_address)
    };
    let flow_state = context.control.states.insert(FlowStateFact {
        machine_symbol,
        state_symbol,
        statements: statement_rows,
        calls: state_calls,
        ..Default::default()
    });
    let flow = context.take_output_pages();
    drop(context);
    facts.flow = flow;
    facts.semantic = semantic;
    ProducedRangeProjection {
        program,
        facts,
        state: flow_state,
        whole,
        projected,
        original_address,
    }
}
