//! Where one statement writes: the place a `let` binds or an assignment
//! stores to, the value's source, and the finite candidates a write through
//! a reference local may reach. A selector whose value is already pinned
//! narrows to the one element it names.

use validation::CanonicalPlace;

use super::{
    ExpressionHandle, FactPlan, FlowBuildContext, PlaceHandle, StatementNode, SymbolHandle,
    contextual_expression_place,
};

/// The place one statement writes and the value it writes there.
pub(in crate::flow) struct StatementWrite {
    pub(in crate::flow) target_place: PlaceHandle,
    pub(in crate::flow) source_expression: ExpressionHandle,
    pub(in crate::flow) source_place: Option<PlaceHandle>,
    /// The finite origins a write through a reference local may reach.
    pub(in crate::flow) candidate_targets: Vec<CanonicalPlace>,
    pub(in crate::flow) target_is_exact: bool,
    pub(in crate::flow) stable_value_target: bool,
}

pub(in crate::flow) fn statement_write(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    store_target: arena::Handle<checked_trees::FlowOperandFact>,
) -> Option<StatementWrite> {
    // An assignment through a reference local of finite candidate origins
    // (flow/reference_places) rewrites exactly one candidate with a value the
    // write checker proved in the destination's declared domains; every
    // other candidate keeps its previous contents. Each candidate therefore
    // holds a declared domain after the write exactly when it held it
    // before, and those rows are re-established below.
    let mut candidate_targets: Vec<CanonicalPlace> = Vec::new();
    let mut target_is_exact = true;
    let (target_place, source_expression, source_place) = match statement {
        StatementNode::RootBinding(_)
        | StatementNode::Assembly(_)
        | StatementNode::AssemblyFact(_) => return None,
        StatementNode::LocalData(local_data) => (
            semantic.append_symbol_place(local_data.symbol),
            local_data.initial_value,
            contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                local_data.initial_value,
            ),
        ),
        StatementNode::Assignment(assignment) => {
            // Address composition already distinguishes a descriptor slot
            // from its pointee. Consume that completed operation, never a
            // post-RHS alias relation or a declaration's old initializer.
            if !build.control.operands.is_valid(store_target) {
                return None;
            }
            let captured = build.control.operands.get(store_target);
            let targets = build
                .control
                .operand_referents
                .span_or_empty(captured.referents)
                .iter()
                .map(|referent| CanonicalPlace {
                    root: referent.root,
                    segments: build
                        .control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                })
                .collect::<Vec<_>>();
            let target = targets.first()?;
            let target_place =
                crate::semantic::places::append_place(semantic, target.root, &target.segments);
            if targets.len() != 1 {
                target_is_exact = false;
                candidate_targets = targets;
            }
            let source_place = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.value,
            );
            (target_place, assignment.value, source_place)
        }
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {
            return None;
        }
    };
    Some(StatementWrite {
        target_place,
        source_expression,
        source_place,
        candidate_targets,
        target_is_exact,
        stable_value_target: false,
    })
}

/// Narrow each runtime `Index` segment of `target_place` whose selector
/// value is pinned in the active contexts, and report whether the narrowed
/// place is stable enough to carry value facts.
pub(in crate::flow) fn narrow_pinned_selectors(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    context_handles: &[facts::FactContextHandle],
    mut target_place: PlaceHandle,
) -> (PlaceHandle, bool) {
    // A runtime index can change without writing the collection. Until value
    // facts retain index dependencies, only immutable selectors carry values.
    // A selector whose live value is already pinned names the one exact
    // element this write hits, though: narrow its `Index` segment to the
    // `FixedIndex` it selects at this point -- the same narrowing the
    // requires-side read performs (`projected_formal_leaf_value` in
    // checks/contracts/direct.rs) -- so the recorded fact lands on the place
    // callers actually read instead of an unreachable runtime spelling.
    let mut target_segments: Vec<facts::PlaceSegment> = semantic
        .place_segments
        .span_or_empty(semantic.places.get(target_place).segments)
        .to_vec();
    crate::flow::narrow_pinned_index_segments(
        program,
        semantic,
        state_symbol,
        statement_index,
        context_handles,
        &mut target_segments,
    );
    let stable_value_target = crate::flow::compose_captured_view_segments(&mut target_segments)
        .is_some()
        && target_segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        });
    if stable_value_target
        && target_segments
            != semantic
                .place_segments
                .span_or_empty(semantic.places.get(target_place).segments)
    {
        let root = semantic.places.get(target_place).root;
        target_place = crate::semantic::places::append_place(semantic, root, &target_segments);
    }
    (target_place, stable_value_target)
}
