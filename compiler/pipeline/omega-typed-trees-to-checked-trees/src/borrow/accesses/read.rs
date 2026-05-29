use super::*;
use records::append_argument_access;

pub(super) fn collect_read_accesses(
    expression: ExpressionHandle,
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_read_accesses(
                    *value,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_read_accesses(
                    range.start,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
            if range.end.is_valid() {
                collect_read_accesses(
                    range.end,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(
                binary.left,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );
            collect_read_accesses(
                binary.right,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(
                    call.receiver,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }

            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_read_accesses(
                    *argument,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(
            cast.value,
            program,
            access_segments,
            argument_accesses,
            accesses,
            state_symbol,
            statement_index,
            machine_symbol,
        ),
        ExpressionNode::Indexed(indexed) => {
            append_read_access(
                expression,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );

            collect_read_accesses(
                indexed.index,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );
        }
        ExpressionNode::Member(member) => {
            if !append_read_access(
                expression,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            ) {
                collect_read_accesses(
                    member.receiver,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Name(_) => {
            append_read_access(
                expression,
                program,
                access_segments,
                argument_accesses,
                accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );
        }
        ExpressionNode::Mutable(inner_expression) => collect_read_accesses(
            *inner_expression,
            program,
            access_segments,
            argument_accesses,
            accesses,
            state_symbol,
            statement_index,
            machine_symbol,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_read_accesses(
                    field.value,
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn append_read_access(
    expression: ExpressionHandle,
    program: &omega_typed_trees::TypedTrees,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
) -> bool {
    let Some(access_place) = borrow_access_place(
        program,
        state_symbol,
        statement_index,
        expression,
        machine_symbol,
    ) else {
        return false;
    };

    append_argument_access(
        access_segments,
        argument_accesses,
        accesses,
        access_place,
        BorrowAccessKind::Read,
    );
    true
}
