//! The indexed steps of one assignment target.
//!
//! Every indexed step of a target is an element selection: a literal selector
//! names a static element and must lie within its array's declared extent; a
//! runtime selector is the statement's retained `AssignmentIndex { depth }`
//! scalar (depth counted from the target inward, the coordinate
//! `validation::assignment_target_selectors` defines) and becomes a
//! `RuntimeIndex` path segment. The planner proves no bound for a runtime
//! element: Terminal re-proves `index < extent` for the segment's obligation.
use super::super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedUnitStructuralPathSegment, ExpressionNode,
    PrimitiveType, TypeReferenceNode,
};
use typed_trees::expression::ExpressionHandle;

#[derive(Clone)]
pub(in crate::execution::terminal_unit) struct TargetSelectors {
    /// The target's selector expressions from the target inward.
    selectors: Vec<ExpressionHandle>,
    /// Segments resolved for `Index` selectors an erased borrow alias
    /// injects into the walk — expressions the authored target never
    /// spelled, so no `AssignmentIndex` coordinate exists for them.
    resolved: Vec<(ExpressionHandle, CheckedUnitStructuralPathSegment)>,
}

impl TargetSelectors {
    /// Admit every indexed step of `target`, or `None` when one of them is
    /// not an element of a declared fixed array or slice: a range (a
    /// subslice), a literal outside a declared extent, a selector into a
    /// non-collection carrier, or a runtime selector whose scalar coordinate the
    /// statement does not retain. A slice carries no declared literal
    /// extent, so its steps leave every index unproven here for the
    /// segment's Terminal obligation.
    pub(in crate::execution::terminal_unit) fn resolve(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        target: ExpressionHandle,
    ) -> Option<Self> {
        let selectors = validation::assignment_target_selectors(program, target);
        let mut cursor = target;
        let mut depth = 0usize;
        loop {
            match program.expression_table.expression(cursor) {
                ExpressionNode::Indexed(indexed) => {
                    if selectors.get(depth) != Some(&indexed.index) {
                        return None;
                    }
                    let collection = validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        indexed.collection,
                    )?;
                    let collection = validation::unwrapped_type_reference(program, collection)?;
                    let literal_length =
                        match program.type_reference_table.type_reference(collection) {
                            TypeReferenceNode::FixedArray { .. } => program
                                .declared_array_extent(collection)?
                                .literal()
                                .map(usize::try_from)
                                .transpose()
                                .ok()?,
                            // A slice's extent is live, not declared: no literal
                            // bound exists to check, and the emitted segment's
                            // Terminal obligation re-proves `index < extent`
                            // against the view's own length witness. Selector
                            // capture is independent of byte/element operation
                            // vocabulary: both must retain the pre-RHS address.
                            TypeReferenceNode::Slice { .. } => None,
                            _ => return None,
                        };
                    match program.expression_table.expression(indexed.index) {
                        ExpressionNode::Integer(literal) => {
                            let index = usize::try_from(literal.value_bignum()?.to_u64()?).ok()?;
                            if literal_length.is_some_and(|length| index >= length) {
                                return None;
                            }
                        }
                        _ => retained(
                            facts,
                            machine,
                            state,
                            statement_index,
                            u32::try_from(depth).ok()?,
                            indexed.index,
                        )?,
                    }
                    depth += 1;
                    cursor = indexed.collection;
                }
                ExpressionNode::Member(member) => cursor = member.receiver,
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        if depth != selectors.len() {
            return None;
        }
        // A `let x: &mut T = &mut place[selector]` alias injects the loan's
        // captured `Index` segment into a later use's walk. Pre-resolve
        // each such selector in the state's borrow ledger so the walk's
        // `runtime_segment` can mint its segment; selectors the ledger
        // cannot resolve stay unproven there.
        let mut resolved = Vec::new();
        if let Some(borrow_state) = facts
            .borrow
            .states
            .iter()
            .map(|(_, borrow_state)| borrow_state)
            .find(|borrow_state| {
                borrow_state.machine_symbol == machine.symbol
                    && borrow_state.state_symbol == state.symbol
            })
        {
            for loan in facts.borrow.loans.span_or_empty(borrow_state.loans) {
                for segment in facts.borrow.loan_segments(loan) {
                    let facts::PlaceSegment::Index { expression } = segment else {
                        continue;
                    };
                    if resolved
                        .iter()
                        .any(|(candidate, _): &(ExpressionHandle, _)| candidate == expression)
                    {
                        continue;
                    }
                    if let Some(segment) = super::super::calls::borrowed_index_segment(
                        program,
                        state.symbol,
                        loan.statement_index,
                        *expression,
                    ) {
                        resolved.push((*expression, segment));
                    }
                }
            }
        }
        Some(Self {
            selectors,
            resolved,
        })
    }

    /// The checked segment of the runtime element `expression` selects. An
    /// authored target selector always carries the `AssignmentIndex`
    /// coordinate; an alias-injected borrow selector resolves through the
    /// side table `resolve` built.
    pub(in crate::execution::terminal_unit) fn runtime_segment(
        &self,
        expression: ExpressionHandle,
    ) -> Option<CheckedUnitStructuralPathSegment> {
        if let Some(depth) = self
            .selectors
            .iter()
            .position(|selector| *selector == expression)
        {
            return Some(CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                    depth: u32::try_from(depth).ok()?,
                },
            ));
        }
        self.resolved
            .iter()
            .find_map(|(candidate, segment)| (*candidate == expression).then(|| segment.clone()))
    }
}

/// The statement's `AssignmentIndex { depth }` coordinate retains exactly this
/// authored selector at an integer carrier: a bound pure expression, or the
/// computation root the ordinary scalar evaluator completes.
fn retained(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    depth: u32,
    index: ExpressionHandle,
) -> Option<()> {
    let integer = |primitive_type: Option<PrimitiveType>| {
        primitive_type.is_some_and(|primitive| primitive.is_fixed_integer())
    };
    let role = CheckedScalarExpressionRole::AssignmentIndex { depth };
    if let Some((binding, value)) =
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement_index, role)
    {
        let primitive_type = crate::values::scalar_expression_type(value);
        return (binding.expression == index && integer(primitive_type)).then_some(());
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state.symbol, statement_index, role)?;
    if root.machine != machine.symbol || !computations.nodes.is_valid(root.root) {
        return None;
    }
    let node = computations.nodes.get(root.root);
    (node.authored_root == index && integer(Some(node.primitive_type))).then_some(())
}
