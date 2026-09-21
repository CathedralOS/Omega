use super::super::{CheckedTrees, PrimitiveType, Scalar};
use super::{CheckedCallScalarArgument, ExpressionNode, LoweringError};
use checked_trees::CheckedScalarExpressionRole;
#[test]
fn eliminated_extent_preserves_collection_evaluation_bounds_and_selection() {
    // This is the existing composed-Unit scalar extent producer fixture, not
    // a claim that array-return transport for attached receivers is closed.
    let source = r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) reaches Host {
            let length: u64 = (self.values[1..4]).len;
            transition length == 3 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Host::exit(1); }
            state no(&mut self) { Host::exit(2); }
        }
        machine make_array(output: &mut u8) -> [i32; 5] { output = 1; [7, 9, 11, 13, 15] }
        machine donor(output: &mut u8) -> [i32; 5] { make_array(output) }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let original = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("existing checked extent fixture");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("enter");
    let state = &original.machine_states(machine)[0];
    let state_symbol = state.symbol;
    let checked_trees::statement::StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("length initializer");
    };
    let source = local.initial_value;
    let value = original
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == 0
                && expression.expression.primitive_type() == Some(PrimitiveType::U64)
        })
        .expect("retained compile-known extent")
        .expression
        .clone();
    assert!(matches!(&value, Scalar::IntegerLiteral { literal } if literal.value_u64() == Some(3)));
    let element = CheckedCallScalarArgument::Pure(value);
    let validate = |checked: &CheckedTrees| {
        super::super::validate(
            checked,
            state_symbol,
            0,
            source,
            PrimitiveType::U64,
            &element,
        )
    };
    validate(&original).expect("actual producer extent has static source and bounds evidence");
    let ExpressionNode::Member(member) = original.expression_table.expression(source) else {
        panic!("extent");
    };
    let projection = member.receiver;
    let ExpressionNode::Indexed(indexed) = original.expression_table.expression(projection) else {
        panic!("subslice");
    };
    let collection = indexed.collection;
    let range = indexed.index;
    let maker = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make_array")
        .expect("maker");
    let target = original.machine_states(maker)[0].symbol;
    let call = original
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target_symbol == target)
                .then_some(expression.clone())
        })
        .expect("effectful donor call");
    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(collection) = call;
    assert!(matches!(
        validate(&changed),
        Err(LoweringError::Unsupported(
            "eliminated subslice extent requires retained source evaluation and view bounds custody"
        ))
    ));

    let mut changed = original.clone();
    let ExpressionNode::Range(bounds) = changed.expression_table.expression(range) else {
        panic!("bounds");
    };
    let end = bounds.end;
    *changed.typed.expression_table.expression_mut(end) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(6));
    assert!(
        validate(&changed).is_err(),
        "eliminated view exceeds static source extent"
    );

    let mut changed = original.clone();
    changed
        .facts
        .operators
        .uses
        .append(checked_trees::CheckedOperatorUseFact {
            expression: projection,
            spelling: language_core::OperatorSpelling::Index,
            ..Default::default()
        });
    assert!(
        validate(&changed).is_err(),
        "eliminated view changed selected spelling"
    );
}

#[test]
fn slice_backed_extent_keeps_the_retained_view_bounds_plan() {
    // `bytes[a..b].len` over a slice has no static extent; its fold is sound
    // only while the same authored view is materialized by a retained
    // `ByteSequenceSubslice` bounds plan at the same statement.
    let source = r#"
        boundary trait Host { machine exit(code: u64); }
        machine take(view: &[u8], counts: [u64; 2]) reaches Host { Host::exit(counts[0]); }
        data Root {}
        machine Root::inspect(&mut self, bytes: &[u8])
        requires bytes.len >= 3
        reaches Host
        {
            take(bytes[1..3], [bytes[1..3].len, 9]);
            Host::exit(7);
        }
        machine Root::enter(&mut self) reaches Host { Host::exit(3); }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let original = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("slice-backed extent fixture");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::inspect")
        .expect("inspect");
    let state = &original.machine_states(machine)[0];
    let state_symbol = state.symbol;
    let (extent, _) = original
        .expression_table
        .iter_expressions()
        .find(|(_, expression)| {
            matches!(expression, ExpressionNode::Member(member)
            if member.member.as_str() == "len"
                && matches!(
                    original.expression_table.expression(member.receiver),
                    ExpressionNode::Indexed(_)
                ))
        })
        .expect("authored subslice extent");
    let element = original
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == 0
                && matches!(
                    expression.role,
                    CheckedScalarExpressionRole::ArrayElement { .. }
                )
                && matches!(&expression.expression, Scalar::IntegerLiteral { literal } if literal.value_u64() == Some(2))
        })
        .expect("retained folded extent element")
        .expression
        .clone();
    let argument = CheckedCallScalarArgument::Pure(element);
    let validate = |checked: &CheckedTrees| {
        super::super::validate(
            checked,
            state_symbol,
            0,
            extent,
            PrimitiveType::U64,
            &argument,
        )
    };
    // Lowering `inspect` end-to-end still fails further downstream: the
    // entry-contract length bound is not yet representable in Terminal
    // (`OperationProofUnavailable`). Custody is the gate this item owns.
    validate(&original)
        .expect("same-statement retained view supplies the eliminated extent custody");

    // The same view spelled as the sibling call's argument carries the
    // retained bounds plan the fold borrows.
    let (view, _) = retained_subslice(&original).expect("take argument retains its subslice plan");

    let mut changed = original.clone();
    drop_subslice_sources(&mut changed);
    assert!(
        validate(&changed).is_err(),
        "dropped view plan loses custody"
    );

    let ExpressionNode::Indexed(indexed) = original.expression_table.expression(view) else {
        panic!("retained view");
    };
    let range = indexed.index;
    let collection = indexed.collection;
    let ExpressionNode::Range(bounds) = original.expression_table.expression(range) else {
        panic!("retained bounds");
    };
    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(bounds.end) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(4));
    assert!(
        validate(&changed).is_err(),
        "retained view with different bounds loses custody"
    );

    let mut changed = original.clone();
    if let ExpressionNode::Name(path) = changed.typed.expression_table.expression_mut(collection) {
        path.symbol = symbols::SymbolHandle::invalid();
    }
    assert!(
        validate(&changed).is_err(),
        "retained view over a different collection loses custody"
    );

    // A view materialized by a different statement does not rescue the fold.
    let moved = r#"
        boundary trait Host { machine exit(code: u64); }
        machine take(view: &[u8], counts: [u64; 2]) reaches Host { Host::exit(counts[0]); }
        data Root {}
        machine Root::inspect(&mut self, bytes: &[u8])
        requires bytes.len >= 3
        reaches Host
        {
            let counts: [u64; 2] = [bytes[1..3].len, 9];
            take(bytes[1..3], counts);
            Host::exit(7);
        }
        machine Root::enter(&mut self) reaches Host { Host::exit(3); }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(moved)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let moved = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("cross-statement extent fixture");
    let moved_machine = moved
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::inspect")
        .expect("inspect");
    let moved_state = moved.machine_states(moved_machine)[0].symbol;
    let (extent, _) = moved
        .expression_table
        .iter_expressions()
        .find(|(_, expression)| {
            matches!(expression, ExpressionNode::Member(member)
            if member.member.as_str() == "len"
                && matches!(
                    moved.expression_table.expression(member.receiver),
                    ExpressionNode::Indexed(_)
                ))
        })
        .expect("authored subslice extent");
    let moved_element = moved
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|expression| {
            expression.state == moved_state
                && expression.statement_ordinal == 0
                && matches!(
                    expression.role,
                    CheckedScalarExpressionRole::ArrayElement { .. }
                )
                && matches!(&expression.expression, Scalar::IntegerLiteral { literal } if literal.value_u64() == Some(2))
        })
        .expect("retained folded extent element")
        .expression
        .clone();
    assert!(
        matches!(
            super::super::validate(
                &moved,
                moved_state,
                0,
                extent,
                PrimitiveType::U64,
                &CheckedCallScalarArgument::Pure(moved_element),
            ),
            Err(LoweringError::Unsupported(_)),
        ),
        "a view retained at another statement loses custody"
    );
}

/// First `collection[a..b]` argument retained by a view/bounds plan anywhere
/// in the flow stores that materialize one: call structural arguments,
/// control-transfer sources, and composed successor transfers.
fn retained_subslice(
    checked: &CheckedTrees,
) -> Option<(checked_trees::expression::ExpressionHandle, u32)> {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan as Terminator,
        CheckedStructuralControlTransferSourcePlan as Transfer,
        CheckedUnitEffectOperationPlan as Operation,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    fn subslice_source(
        source: &Source,
    ) -> Option<(checked_trees::expression::ExpressionHandle, u32)> {
        match source {
            Source::ByteSequenceSubslice {
                expression,
                parameter_index,
                ..
            } => Some((*expression, *parameter_index)),
            _ => None,
        }
    }
    let effects = &checked.facts.flow.terminal_unit_effects;
    let graphs = &checked.facts.flow.terminal_scalar_graphs;
    let mut operations: Vec<&Operation> = effects
        .machines
        .iter()
        .flat_map(|machine| machine.operations.iter())
        .chain(effects.composed_machines.iter().flat_map(|machine| {
            machine
                .states
                .iter()
                .flat_map(|state| state.operations.iter())
        }))
        .chain(graphs.machines.iter().flat_map(|machine| {
            machine
                .states
                .iter()
                .flat_map(|state| state.unit_operations.iter())
        }))
        .collect();
    while let Some(operation) = operations.pop() {
        for argument in operation_structural_arguments(operation) {
            if let Some(found) = subslice_source(&argument.source) {
                return Some(found);
            }
        }
        if let Operation::EstablishStructuralValue { calls, .. } = operation {
            operations.extend(calls.iter().map(|call| call.operation()));
        }
    }
    for (_, transfer) in graphs.structural_transfers.iter() {
        if let Transfer::ByteSequenceSubslice {
            expression,
            parameter_index,
        } = transfer.source
        {
            return Some((expression, parameter_index));
        }
    }
    for machine in &effects.composed_machines {
        for state in &machine.states {
            let mut successors = Vec::new();
            match &state.terminator {
                Terminator::Jump { successor } => successors.push(successor),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => successors.extend([when_true, when_false]),
                Terminator::GuardedJumps { arms, fallback } => {
                    successors.extend(arms.iter().map(|arm| &arm.successor));
                    successors.push(fallback);
                }
                Terminator::ClosedSum { subject, cases } => {
                    if let Some(found) = subslice_source(&subject.source) {
                        return Some(found);
                    }
                    successors.extend(cases.iter().map(|case| &case.successor));
                }
                _ => {}
            }
            for successor in successors {
                for transfer in &successor.transfers {
                    if let Transfer::ByteSequenceSubslice {
                        expression,
                        parameter_index,
                    } = transfer.source
                    {
                        return Some((expression, parameter_index));
                    }
                }
            }
        }
    }
    None
}

/// Every retained `ByteSequenceSubslice` source replaced by the same whole
/// parameter, erasing the view/bounds custody the extent fold borrows.
fn drop_subslice_sources(checked: &mut CheckedTrees) {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan as Terminator,
        CheckedStructuralControlTransferSourcePlan as Transfer,
        CheckedUnitEffectOperationPlan as Operation,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    fn retire_argument(source: &mut Source) {
        if let Source::ByteSequenceSubslice {
            parameter_index, ..
        } = *source
        {
            *source = Source::Parameter { parameter_index };
        }
    }
    fn retire_operation(operation: &mut Operation) {
        for argument in operation_structural_arguments_mut(operation) {
            retire_argument(&mut argument.source);
        }
    }
    let effects = &mut checked.facts.flow.terminal_unit_effects;
    for machine in effects
        .machines
        .iter_mut()
        .map(|machine| machine.operations.iter_mut())
        .flatten()
        .chain(effects.composed_machines.iter_mut().flat_map(|machine| {
            machine
                .states
                .iter_mut()
                .flat_map(|state| state.operations.iter_mut())
        }))
    {
        retire_operation(machine);
    }
    for state in effects
        .composed_machines
        .iter_mut()
        .flat_map(|machine| machine.states.iter_mut())
    {
        let mut successors: Vec<&mut checked_trees::CheckedStructuralControlSuccessorPlan> =
            Vec::new();
        match &mut state.terminator {
            Terminator::Jump { successor } => successors.push(successor),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => successors.extend([when_true, when_false]),
            Terminator::GuardedJumps { arms, fallback } => {
                successors.extend(arms.iter_mut().map(|arm| &mut arm.successor));
                successors.push(fallback);
            }
            Terminator::ClosedSum { subject, cases } => {
                retire_argument(&mut subject.source);
                successors.extend(cases.iter_mut().map(|case| &mut case.successor));
            }
            _ => {}
        }
        for successor in successors {
            for transfer in &mut successor.transfers {
                if let Transfer::ByteSequenceSubslice {
                    parameter_index, ..
                } = transfer.source
                {
                    transfer.source = Transfer::Parameter {
                        index: parameter_index,
                    };
                }
            }
        }
    }
    let graphs = &mut checked.facts.flow.terminal_scalar_graphs;
    graphs.structural_transfers.for_each_mut(|_, transfer| {
        if let Transfer::ByteSequenceSubslice {
            parameter_index, ..
        } = transfer.source
        {
            transfer.source = Transfer::Parameter {
                index: parameter_index,
            };
        }
    });
    for operation in graphs
        .machines
        .iter_mut()
        .flat_map(|machine| machine.states.iter_mut())
        .flat_map(|state| state.unit_operations.iter_mut())
    {
        retire_operation(operation);
    }
}

fn operation_structural_arguments(
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> &[checked_trees::CheckedUnitStructuralArgumentPlan] {
    use checked_trees::CheckedUnitEffectOperationPlan as Operation;
    match operation {
        Operation::CallUnit {
            structural_arguments,
            ..
        }
        | Operation::ScalarCall {
            structural_arguments,
            ..
        }
        | Operation::StructuralCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryScalarCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryStructuralCall {
            structural_arguments,
            ..
        }
        | Operation::SelectedOperatorStructuralScalarCall {
            structural_arguments,
            ..
        }
        | Operation::SelectedOperatorStructuralCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => &[],
    }
}

fn operation_structural_arguments_mut(
    operation: &mut checked_trees::CheckedUnitEffectOperationPlan,
) -> &mut [checked_trees::CheckedUnitStructuralArgumentPlan] {
    use checked_trees::CheckedUnitEffectOperationPlan as Operation;
    match operation {
        Operation::CallUnit {
            structural_arguments,
            ..
        }
        | Operation::ScalarCall {
            structural_arguments,
            ..
        }
        | Operation::StructuralCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryScalarCall {
            structural_arguments,
            ..
        }
        | Operation::BoundaryStructuralCall {
            structural_arguments,
            ..
        }
        | Operation::SelectedOperatorStructuralScalarCall {
            structural_arguments,
            ..
        }
        | Operation::SelectedOperatorStructuralCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => &mut [],
    }
}
