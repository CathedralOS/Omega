use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub(crate) use checked_trees::CapturedPlace as BorrowAccessPlace;

pub(crate) fn borrow_access_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessPlace> {
    // An indexed recast can cover more than its selected element. Replay the
    // same footprint judgment used by retained local loans before ordinary
    // place descent, retaining the complete literal or conservative suffix.
    let mut viewed = expression;
    viewed = program.expression_table.peeled_expression_handle(viewed);
    if let Some(cast) = program.expression_table.cast_node(viewed)
        && cast.form.is_recast()
    {
        let mut source = cast.value;
        source = program.expression_table.peeled_expression_handle(source);
        if matches!(
            program.expression_table.expression(source),
            ExpressionNode::Indexed(_)
        ) {
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.state_by_symbol(state_symbol)?;
            let access = cast.form.recast_access()?;
            if let Some(footprint) = validation::validate_literal_indexed_recast_footprint(
                program,
                machine,
                state,
                statement_index,
                expression,
                cast.target_type,
                access,
            ) {
                let mut place = borrow_access_place(
                    program,
                    state_symbol,
                    statement_index,
                    footprint.collection(),
                    machine_symbol,
                )?;
                place.segments.push(facts::PlaceSegment::FixedRange {
                    start: footprint.start(),
                    end: footprint.end(),
                });
                return Some(place);
            }
            let source = validation::validate_runtime_indexed_recast_source(
                program,
                machine,
                state,
                statement_index,
                expression,
                cast.target_type,
                access,
            )?;
            return borrow_access_place(
                program,
                state_symbol,
                statement_index,
                source,
                machine_symbol,
            );
        }
    }
    // Formation, flow and returned-value capture share one address algebra.
    // In particular, a computed backing root is not absence of a loan.
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    Some(BorrowAccessPlace {
        incoming_reference: Default::default(),
        root: place.root,
        segments: place.segments,
    })
}
