use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Return each structural value occurrence referenced by one contract fact.
///
/// A member or indexed expression is one complete place. Runtime index
/// expressions are additional occurrences because their evaluated values
/// select which place the fact describes and must therefore participate in
/// revision invalidation too.
pub(crate) fn fact_referenced_occurrences(
    program: &dyn ::validation::CheckingInput,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> Vec<ExpressionHandle> {
    fact_referenced_occurrences_in_state(program, fact, None)
}

/// Declaration-owned requirements may omit a private descriptor's payload
/// dependency. Call substitution keeps the unfiltered occurrences: a stable
/// callee formal does not freeze the caller's replaceable descriptor home.
pub(crate) fn fact_referenced_occurrences_in_state(
    program: &dyn ::validation::CheckingInput,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    state: Option<symbols::SymbolHandle>,
) -> Vec<ExpressionHandle> {
    let mut occurrences = Vec::new();
    match program.proof_facts.get(fact) {
        typed_trees::domain::ProofFact::Expression(expression) => {
            append_occurrences(program, *expression, state, &mut occurrences);
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            append_occurrences(program, membership.value, state, &mut occurrences);
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                append_occurrences(program, *argument, state, &mut occurrences);
            }
        }
    }
    occurrences.sort_by_key(|expression| expression.arena_index());
    occurrences.dedup();
    occurrences
}

pub(crate) fn append_expression_occurrences(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    occurrences: &mut Vec<ExpressionHandle>,
) {
    append_occurrences(program, expression, None, occurrences);
}

fn append_occurrences(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    state: Option<symbols::SymbolHandle>,
    occurrences: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(_) => {}
        ExpressionNode::Match(dispatch) => {
            append_occurrences(program, dispatch.subject, state, occurrences);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    append_occurrences(program, pattern, state, occurrences);
                }
                append_occurrences(program, arm.value, state, occurrences);
            }
        }
        ExpressionNode::Name(_) => occurrences.push(expression),
        ExpressionNode::Member(member) => {
            // A completed private descriptor observes its captured extent,
            // not the bytes a callee can overwrite through it. Reuse the
            // descriptor-home judgment used by contract term reconstruction;
            // a mutable borrowed descriptor slot or same-named nominal member
            // does not gain this independence from its contents.
            let measure =
                validation::expression_place_type_reference(program, member.receiver, &[])
                    .and_then(|receiver| {
                        validation::collection_operations::collection_measure_member(
                            program,
                            member,
                            validation::collection_operations::MeasureReceiver::Declared(receiver),
                        )
                    });
            if measure == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
                && state.is_some_and(|state| {
                    stable_descriptor_occurrence(program, member.receiver, state)
                })
            {
                return;
            }
            // Collection metadata observes its receiver, not a nominal field
            // named `len` or `capacity`. Keeping the receiver here supplies a
            // real storage dependency to both revision invalidation and review;
            // appending an unresolved field instead loses that relationship.
            // Final selection custody is not published yet. Use the shared
            // receiver-type classifier, so same-spelled record fields retain
            // their ordinary complete-place identity.
            if measure.is_some()
                || matches!(
                    program.expression_table.expression(member.receiver),
                    ExpressionNode::Call(_)
                )
            {
                append_occurrences(program, member.receiver, state, occurrences);
            } else {
                occurrences.push(expression);
                append_selector_occurrences(program, member.receiver, occurrences);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            occurrences.push(expression);
            append_selector_occurrences(program, indexed.collection, occurrences);
            append_occurrences(program, indexed.index, state, occurrences);
        }
        ExpressionNode::Borrow(inner) => {
            append_occurrences(program, inner.target, state, occurrences);
        }
        ExpressionNode::Atomic(atomic) => {
            append_occurrences(program, atomic.value, state, occurrences);
            append_occurrences(program, atomic.result, state, occurrences);
        }
        ExpressionNode::Binary(binary) => {
            append_occurrences(program, binary.left, state, occurrences);
            append_occurrences(program, binary.right, state, occurrences);
        }
        ExpressionNode::Cast(cast) => {
            append_occurrences(program, cast.value, state, occurrences);
        }
        ExpressionNode::Call(call) => {
            append_occurrences(program, call.receiver, state, occurrences);
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_occurrences(program, *argument, state, occurrences);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_occurrences(program, *value, state, occurrences);
            }
        }
        ExpressionNode::Range(range) => {
            append_occurrences(program, range.start, state, occurrences);
            append_occurrences(program, range.end, state, occurrences);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_occurrences(program, field.value, state, occurrences);
            }
        }
        ExpressionNode::Unary(unary) => {
            append_occurrences(program, unary.operand, state, occurrences);
        }
        typed_trees::spelled_literal_expression_pattern!() => {}
    }
}

fn stable_descriptor_occurrence(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    state_symbol: symbols::SymbolHandle,
) -> bool {
    let Some(place) = validation::canonical_place_from_expression(program, expression) else {
        return false;
    };
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    let parent = program.symbols.get(root).parent;
    parent == state_symbol
        && validation::find_state(program, parent).is_some_and(|(_, state)| {
            validation::stable_slice_descriptor_place(program, state, root, &place.segments)
                .is_some()
        })
}

/// A selected field remains one complete storage dependency, but selectors
/// beneath its receiver are independent values that can redirect the read.
/// Non-place producers compute a completed value from their operands, so their
/// ordinary value dependencies must also survive any later source replay.
pub(crate) fn append_selector_occurrences(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    occurrences: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            append_selector_occurrences(program, member.receiver, occurrences)
        }
        ExpressionNode::Indexed(indexed) => {
            append_selector_occurrences(program, indexed.collection, occurrences);
            append_expression_occurrences(program, indexed.index, occurrences);
        }
        ExpressionNode::Borrow(borrow) => {
            append_selector_occurrences(program, borrow.target, occurrences)
        }
        ExpressionNode::Name(_) | typed_trees::literal_expression_pattern!() => {}
        _ => append_expression_occurrences(program, expression, occurrences),
    }
}
