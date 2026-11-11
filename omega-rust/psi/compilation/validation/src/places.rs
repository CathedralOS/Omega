/*
Canonical places, storage coordinates, collection extents, and projected
receiver and storage-path relations (`value_custody::places`).
*/
pub(crate) mod binding_observations {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;

    pub fn stable_slice_descriptor_place(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let reference = stable_reference_descriptor_place(program, state, symbol, segments)?;
        crate::slice_view_shell(program, reference, &[])?;
        Some(reference)
    }

    pub(crate) fn stable_reference_descriptor_place(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let root_reference = crate::state_root_type(program, state, symbol)?;
        let mut reference = root_reference;
        for (position, segment) in segments.iter().enumerate() {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            if !symbol.is_valid() || crate::reference_shell(program, reference).is_some() {
                return None;
            }
            reference = super::project_type_reference_from_segments(
                program,
                root_reference,
                &segments[..=position],
            )?;
        }
        crate::reference_shell(program, reference)?;
        (!binding_is_exclusively_exposed(program, state, symbol)
            && !binding_is_written(program, state, symbol, BindingObservation::Descriptor))
        .then_some(reference)
    }

    pub(crate) fn binding_is_exclusively_exposed(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
    ) -> bool {
        binding_has_exclusive_exposure(program, state, symbol, BindingObservation::Descriptor)
    }

    fn binding_has_exclusive_exposure(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        observation: BindingObservation,
    ) -> bool {
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
        for root in statement_expression_roots(program, state) {
            crate::queries::expression_traversal::collect_expression_nodes(
                program, root, &mut nodes, &mut seen,
            );
        }
        nodes.iter().any(|node| {
            crate::value_custody::recasts::reference_formation_source(program, *node).is_some_and(
                |formation| {
                    !formation.interpretation.is_access_view()
                        && formation.access.is_exclusive()
                        && borrow_root_symbol(program, formation.source) == Some(symbol)
                        && (observation == BindingObservation::WholeValue
                            || !selects_builtin_collection_contents(
                                program,
                                state,
                                formation.source,
                            ))
                },
            )
        })
    }

    fn selects_collection_contents(
        program: &dyn CheckingInput,
        mut expression: ExpressionHandle,
    ) -> bool {
        let mut visited = 0;
        while expression_table(program).expression_is_valid(expression) && visited < 128 {
            visited += 1;
            match expression_table(program).expression(expression) {
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(_) => return true,
                _ => return false,
            }
        }
        false
    }

    fn selects_builtin_collection_contents(
        program: &dyn CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        selects_collection_contents(program, expression)
            && program
                .machine_holding_state(state.symbol)
                .is_some_and(|machine| {
                    crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
                })
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum BindingObservation {
        WholeValue,
        Descriptor,
    }

    pub(crate) fn binding_is_written(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        observation: BindingObservation,
    ) -> bool {
        for statement in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
        {
            if let StatementNode::Assignment(assignment) = statement
                && borrow_root_symbol(program, assignment.target) == Some(symbol)
            {
                let selects_contents = observation == BindingObservation::Descriptor
                    && selects_builtin_collection_contents(program, state, assignment.target);
                if !selects_contents {
                    return true;
                }
            }
        }
        binding_has_write_effects(program, state, symbol, observation)
    }

    pub(crate) fn binding_has_write_effects(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
        observation: BindingObservation,
    ) -> bool {
        if program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::Call(call)
                if call.receiver_root_symbol == symbol
                    && call_receiver_is_mutable(program, call.target_symbol))
            })
        {
            return true;
        }
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
        for root in statement_expression_roots(program, state) {
            crate::queries::expression_traversal::collect_expression_nodes(
                program, root, &mut nodes, &mut seen,
            );
        }
        nodes
            .iter()
            .any(|node| match expression_table(program).expression(*node) {
                ExpressionNode::Call(call) => {
                    call.receiver.is_valid()
                        && borrow_root_symbol(program, call.receiver) == Some(symbol)
                        && call_receiver_is_mutable(program, call.target_symbol)
                }
                ExpressionNode::Atomic(atomic) => {
                    if let Some(carrier) =
                        crate::value_custody::atomic_operations::atomic_once_operands(
                            program, atomic,
                        )
                    {
                        return observation != BindingObservation::Descriptor
                            && borrow_root_symbol(program, carrier.place) == Some(symbol);
                    }
                    borrow_root_symbol(program, atomic.value) == Some(symbol)
                        || borrow_root_symbol(program, atomic.result) == Some(symbol)
                }
                _ => false,
            })
    }

    fn call_receiver_is_mutable(program: &dyn CheckingInput, target_symbol: SymbolHandle) -> bool {
        crate::machine_calls::calls::machine_state_by_symbol(program, target_symbol).is_none()
    }

    fn statement_expression_roots(
        program: &dyn CheckingInput,
        state: &State,
    ) -> Vec<ExpressionHandle> {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .flat_map(|statement| program.tables.expression_roots(statement))
            .map(|(_, root)| root)
            .collect()
    }

    fn borrow_root_symbol(
        program: &dyn CheckingInput,
        mut expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let mut visited = 0;
        while expression_table(program).expression_is_valid(expression) && visited < 128 {
            visited += 1;
            match expression_table(program).expression(expression) {
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                ExpressionNode::Name(path) => return Some(path.head_symbol),
                _ => return None,
            }
        }
        None
    }

    pub(crate) fn binding_is_unchanged_value(
        program: &dyn CheckingInput,
        state: &State,
        symbol: SymbolHandle,
    ) -> bool {
        !binding_is_written(program, state, symbol, BindingObservation::WholeValue)
            && !binding_has_exclusive_exposure(
                program,
                state,
                symbol,
                BindingObservation::WholeValue,
            )
    }

    pub fn place_has_immutable_denotation(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &State,
        before: usize,
        expression: ExpressionHandle,
    ) -> bool {
        if !crate::place_has_builtin_coordinates(program, machine, Some(state), expression) {
            return false;
        }
        if !immutable_read_place(program, state, before, expression) {
            return false;
        }
        let mut nodes = Vec::new();
        crate::queries::expression_traversal::collect_expression_tree(
            program, expression, &mut nodes,
        );
        nodes
            .into_iter()
            .all(|node| match expression_table(program).expression(node) {
                ExpressionNode::Name(path) => {
                    immutable_binding(program, state, before, path.head_symbol)
                }
                ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_) => true,
                _ => false,
            })
    }

    // Check that the values read by an expression cannot change. The caller
    // separately checks the operations; stable inputs alone do not make a call pure.
    pub fn value_reads_have_immutable_denotation(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &State,
        before: usize,
        expression: ExpressionHandle,
    ) -> bool {
        let mut nodes = Vec::new();
        crate::queries::expression_traversal::collect_expression_tree(
            program, expression, &mut nodes,
        );
        nodes
            .into_iter()
            .all(|node| match expression_table(program).expression(node) {
                ExpressionNode::Name(path) => {
                    crate::place_has_builtin_coordinates(program, machine, Some(state), node)
                        && immutable_read_place(program, state, before, node)
                        && immutable_binding(program, state, before, path.head_symbol)
                }
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                    crate::place_has_builtin_coordinates(program, machine, Some(state), node)
                        && immutable_read_place(program, state, before, node)
                }
                _ => true,
            })
    }

    fn immutable_binding(
        program: &dyn CheckingInput,
        state: &State,
        before: usize,
        symbol: SymbolHandle,
    ) -> bool {
        let reference = if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
            (!parameter.is_mutable).then_some(parameter.type_reference)
        } else {
            program
                .state_local_declaration(state, symbol)
                .and_then(|(position, local)| {
                    (position < before && !local.is_mutable).then_some(local.type_reference)
                })
        };
        reference.is_some_and(|reference| {
            crate::reference_shell(program, reference).is_none()
                && binding_is_unchanged_value(program, state, symbol)
        })
    }

    fn immutable_read_place(
        program: &dyn CheckingInput,
        state: &State,
        before: usize,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(place) = crate::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            before,
            expression,
        ) else {
            return false;
        };
        place.root.as_symbol().is_some()
            && (0..=place.segments.len()).all(|length| {
                crate::canonical_place_type_reference(
                    program,
                    state.symbol,
                    before,
                    &crate::CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..length].to_vec(),
                    },
                )
                .is_some_and(|reference| crate::reference_shell(program, reference).is_none())
            })
    }

    pub(crate) fn immutable_value_origin(
        program: &dyn CheckingInput,
        state: &State,
        mut before: usize,
        mut expression: ExpressionHandle,
    ) -> Option<(usize, ExpressionHandle)> {
        loop {
            if !expression_table(program).expression_is_valid(expression) {
                return None;
            }
            let Some(path) = expression_table(program).name_path_node(expression) else {
                return Some((before, expression));
            };
            if !path.symbol.is_valid() {
                return None;
            }
            if path.head_symbol != path.symbol
                || expression_table(program)
                    .name_path_members(path.members)
                    .len()
                    != 1
            {
                return Some((before, expression));
            }
            let Some((position, local)) = program.state_local_declaration(state, path.symbol)
            else {
                return Some((before, expression));
            };
            if position >= before
                || local.is_mutable
                || !local.initial_value.is_valid()
                || !binding_is_unchanged_value(program, state, local.symbol)
            {
                return None;
            }
            before = position;
            expression = local.initial_value;
        }
    }
}
pub(crate) mod captured_ownership {
    use super::canonical_projection::{CanonicalPlace, canonical_place_from_expression_in_state};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub fn ownership_place_from_expression_in_state(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let mut place = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        )?;
        let mut active = Vec::new();
        while let Some(anchor) = place.root.as_expression()
            && let ExpressionNode::SavedSubject(saved) = program.expression_table.expression(anchor)
        {
            if active.contains(&saved.capture) {
                return None;
            }
            active.push(saved.capture);
            let source = program.saved_value_source(saved)?;
            let (machine, state) = crate::find_state(program, state_symbol)?;
            let reference =
                crate::expression_result_type_reference(program, machine, state, source)?;
            if program.type_multiplicity(reference)
                == language_semantics::Multiplicity::Unrestricted
            {
                break;
            }
            let mut owner = canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                source,
            )?;
            owner.segments.extend(place.segments);
            place = owner;
        }
        Some(place)
    }
}
pub mod storage_path_relations {
    pub fn case_tag_segments_may_overlap(
        program: &dyn crate::CheckingInput,
        observed: &[PlaceSegment],
        written: &[PlaceSegment],
    ) -> bool {
        if written.len() > observed.len()
            && written
                .iter()
                .chain(observed)
                .all(|segment| !place_segment_has_unresolved_identity(*segment))
        {
            return false;
        }
        canonical_place_segments_may_overlap(program, observed, written)
    }

    pub fn place_segment_has_static_identity(segment: &PlaceSegment) -> bool {
        match segment {
            PlaceSegment::Field { symbol } => symbol.is_valid(),
            PlaceSegment::Case { variant } => variant.is_valid(),
            PlaceSegment::FixedIndex { .. } | PlaceSegment::FixedRange { .. } => true,
            PlaceSegment::Index { .. } | PlaceSegment::OpaqueRange => false,
        }
    }

    pub fn place_segment_has_unresolved_identity(segment: PlaceSegment) -> bool {
        match segment {
            PlaceSegment::Field { symbol } => !symbol.is_valid(),
            PlaceSegment::Case { variant } => !variant.is_valid(),
            _ => false,
        }
    }

    pub fn static_place_segment_pair_equal(
        left: PlaceSegment,
        right: PlaceSegment,
    ) -> Option<bool> {
        match (left, right) {
            (
                PlaceSegment::Field {
                    symbol: left_symbol,
                },
                PlaceSegment::Field {
                    symbol: right_symbol,
                },
            ) => Some(left_symbol == right_symbol),
            (
                PlaceSegment::Case {
                    variant: left_variant,
                },
                PlaceSegment::Case {
                    variant: right_variant,
                },
            ) => Some(left_variant == right_variant),
            (
                PlaceSegment::FixedIndex { index: left_index },
                PlaceSegment::FixedIndex { index: right_index },
            ) => Some(left_index == right_index),
            _ => None,
        }
    }

    pub fn canonical_place_segments_equal(
        program: &dyn crate::CheckingInput,
        left: PlaceSegment,
        right: PlaceSegment,
    ) -> bool {
        if place_segment_has_unresolved_identity(left)
            || place_segment_has_unresolved_identity(right)
        {
            return false;
        }
        static_place_segment_pair_equal(left, right).unwrap_or_else(|| match (left, right) {
            (
                PlaceSegment::FixedRange {
                    start: left_start,
                    end: left_end,
                },
                PlaceSegment::FixedRange {
                    start: right_start,
                    end: right_end,
                },
            ) => left_start == right_start && left_end == right_end,
            (PlaceSegment::FixedIndex { index }, PlaceSegment::Index { expression })
            | (PlaceSegment::Index { expression }, PlaceSegment::FixedIndex { index }) => {
                expression_static_index(program, expression).is_some_and(|value| value == index)
            }
            (
                PlaceSegment::Index {
                    expression: left_expression,
                },
                PlaceSegment::Index {
                    expression: right_expression,
                },
            ) => expression_table(program)
                .expressions_structurally_equal(left_expression, right_expression),
            _ => false,
        })
    }
    use crate::frozen_program::expression_table;
    use checked_trees::expression::ExpressionHandle;
    use facts::PlaceSegment;

    pub fn canonical_places_agree(
        program: &dyn crate::CheckingInput,
        left: &crate::CanonicalPlace,
        right: &crate::CanonicalPlace,
    ) -> bool {
        left.root == right.root
            && left.segments.len() == right.segments.len()
            && left
                .segments
                .iter()
                .zip(right.segments.iter())
                .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
    }

    /*
        How two places relate. A write at one place and a fact, loan or
        obligation at another ask this one question, so that every keep-or-kill
        decision gives the same answer for the same two places
        (wiki/drafts/designs/fact_transport.md).

        `Prefix` and `Extends` are containment by path: every shared segment is
        the same segment and one path is longer. A range that covers an index is
        `MayOverlap`, which a caller must treat as not contained.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PlaceRelation {
        Disjoint,
        Equal,
        /// The left place contains the right one.
        Prefix,
        /// The right place contains the left one.
        Extends,
        MayOverlap,
    }

    impl PlaceRelation {
        pub const fn may_overlap(self) -> bool {
            !matches!(self, Self::Disjoint)
        }
    }

    pub fn place_relation(
        program: &dyn crate::CheckingInput,
        left: &crate::CanonicalPlace,
        right: &crate::CanonicalPlace,
    ) -> PlaceRelation {
        if left.root != right.root {
            return PlaceRelation::Disjoint;
        }
        place_segments_relation(program, &left.segments, &right.segments)
    }

    pub fn place_segments_relation(
        program: &dyn crate::CheckingInput,
        left: &[PlaceSegment],
        right: &[PlaceSegment],
    ) -> PlaceRelation {
        if !canonical_place_segments_may_overlap(program, left, right) {
            return PlaceRelation::Disjoint;
        }
        if !left
            .iter()
            .zip(right)
            .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
        {
            return PlaceRelation::MayOverlap;
        }
        match left.len().cmp(&right.len()) {
            std::cmp::Ordering::Equal => PlaceRelation::Equal,
            std::cmp::Ordering::Less => PlaceRelation::Prefix,
            std::cmp::Ordering::Greater => PlaceRelation::Extends,
        }
    }

    pub fn canonical_places_may_overlap(
        program: &dyn crate::CheckingInput,
        left: &crate::CanonicalPlace,
        right: &crate::CanonicalPlace,
    ) -> bool {
        place_relation(program, left, right).may_overlap()
    }

    pub fn canonical_place_segments_may_overlap(
        program: &dyn crate::CheckingInput,
        left: &[PlaceSegment],
        right: &[PlaceSegment],
    ) -> bool {
        for (&left_segment, &right_segment) in left.iter().zip(right) {
            if place_segment_has_unresolved_identity(left_segment)
                || place_segment_has_unresolved_identity(right_segment)
            {
                return true;
            }
            if !canonical_place_segment_pair_may_overlap(program, left_segment, right_segment) {
                return false;
            }
        }
        true
    }

    pub fn canonical_place_joined_segments_may_overlap(
        program: &dyn crate::CheckingInput,
        prefix: &[PlaceSegment],
        suffix: &[PlaceSegment],
        right: &[PlaceSegment],
    ) -> bool {
        for (&left_segment, &right_segment) in prefix.iter().chain(suffix).zip(right) {
            if place_segment_has_unresolved_identity(left_segment)
                || place_segment_has_unresolved_identity(right_segment)
            {
                return true;
            }
            if !canonical_place_segment_pair_may_overlap(program, left_segment, right_segment) {
                return false;
            }
        }
        true
    }

    fn canonical_place_segment_pair_may_overlap(
        program: &dyn crate::CheckingInput,
        left: PlaceSegment,
        right: PlaceSegment,
    ) -> bool {
        static_place_segment_pair_equal(left, right).unwrap_or_else(|| match (left, right) {
            (PlaceSegment::OpaqueRange, _) | (_, PlaceSegment::OpaqueRange) => true,
            (
                PlaceSegment::FixedRange {
                    start: left_start,
                    end: left_end,
                },
                PlaceSegment::FixedRange {
                    start: right_start,
                    end: right_end,
                },
            ) => {
                left_start < left_end
                    && right_start < right_end
                    && left_start < right_end
                    && right_start < left_end
            }
            (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index })
            | (PlaceSegment::FixedIndex { index }, PlaceSegment::FixedRange { start, end }) => {
                fixed_range_contains(start, end, index)
            }
            (PlaceSegment::FixedRange { start, end }, PlaceSegment::Index { expression })
            | (PlaceSegment::Index { expression }, PlaceSegment::FixedRange { start, end }) => {
                expression_static_index(program, expression)
                    .is_none_or(|index| fixed_range_contains(start, end, index))
            }
            (PlaceSegment::FixedIndex { index }, PlaceSegment::Index { expression })
            | (PlaceSegment::Index { expression }, PlaceSegment::FixedIndex { index }) => {
                expression_static_index(program, expression).is_none_or(|value| value == index)
            }
            (
                PlaceSegment::Index {
                    expression: left_expression,
                },
                PlaceSegment::Index {
                    expression: right_expression,
                },
            ) => index_expressions_may_overlap(program, left_expression, right_expression),
            _ => false,
        })
    }

    fn fixed_range_contains(start: usize, end: usize, index: usize) -> bool {
        start < end && start <= index && index < end
    }

    fn expression_static_index(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        if !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        expression_table(program)
            .constant_integer_value(expression)
            .and_then(|value| usize::try_from(value).ok())
    }

    fn index_expressions_may_overlap(
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        if !expression_table(program).expression_is_valid(left)
            || !expression_table(program).expression_is_valid(right)
        {
            return true;
        }
        if left == right {
            return true;
        }

        match (
            expression_table(program).expression(left),
            expression_table(program).expression(right),
        ) {
            (
                checked_trees::expression::ExpressionNode::Integer(left_value),
                checked_trees::expression::ExpressionNode::Integer(right_value),
            ) => left_value == right_value,
            _ => true,
        }
    }

    pub fn binding_overwrite(
        program: &dyn crate::CheckingInput,
        place: &crate::CanonicalPlace,
        segments: &[PlaceSegment],
    ) -> bool {
        segments.len() <= place.segments.len() && contents_overlap(program, place, segments)
    }
    pub fn contents_overlap(
        program: &dyn crate::CheckingInput,
        place: &crate::CanonicalPlace,
        segments: &[PlaceSegment],
    ) -> bool {
        canonical_place_segments_may_overlap(program, &place.segments, segments)
    }
    #[cfg(test)]
    mod relation_tests {
        use super::*;

        #[test]
        fn relation_names_disjoint_equal_and_both_containments() {
            let program = typed_trees::TypedTrees::default();
            let at = |index| PlaceSegment::FixedIndex { index };
            let relation = |left: &[PlaceSegment], right: &[PlaceSegment]| {
                place_segments_relation(&program, left, right)
            };
            assert_eq!(relation(&[at(0)], &[at(1)]), PlaceRelation::Disjoint);
            assert_eq!(relation(&[at(0)], &[at(0)]), PlaceRelation::Equal);
            assert_eq!(relation(&[at(0)], &[at(0), at(1)]), PlaceRelation::Prefix);
            assert_eq!(relation(&[at(0), at(1)], &[at(0)]), PlaceRelation::Extends);
            // A range covering an index shares storage with it, but is not a
            // path prefix of it.
            let range = PlaceSegment::FixedRange { start: 0, end: 4 };
            assert_eq!(relation(&[range], &[at(2)]), PlaceRelation::MayOverlap);
            assert_eq!(relation(&[range], &[at(4)]), PlaceRelation::Disjoint);
            assert_eq!(
                relation(&[PlaceSegment::OpaqueRange], &[at(0)]),
                PlaceRelation::MayOverlap
            );
        }
    }
}
pub use captured_ownership::ownership_place_from_expression_in_state;
pub mod collection_operations {
    use language_semantics::declaration_selection::CollectionMeasure;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MeasureReceiver {
        Declared(TypeReferenceHandle),
        Window,
    }

    pub fn collection_measure_member(
        program: &dyn crate::CheckingInput,
        member: &typed_trees::expression::TableMemberExpression,
        receiver: MeasureReceiver,
    ) -> Option<CollectionMeasure> {
        if member.case_variant.is_some() {
            return None;
        }
        let receiver_is_collection = match receiver {
            MeasureReceiver::Declared(type_reference) => {
                type_reference_is_collection(program, type_reference)
            }
            MeasureReceiver::Window => true,
        };
        if !receiver_is_collection {
            return None;
        }
        CollectionMeasure::from_authored_spelling(member.member.as_str())
    }

    pub fn core_vector_measure_member(
        program: &dyn crate::CheckingInput,
        member: &typed_trees::expression::TableMemberExpression,
        receiver: TypeReferenceHandle,
    ) -> bool {
        if member.case_variant.is_some()
            || CollectionMeasure::from_authored_spelling(member.member.as_str()).is_none()
        {
            return false;
        }
        let Some(carrier) =
            crate::value_custody::reference_shells::carrier_beneath_borrows(program, receiver)
        else {
            return false;
        };
        let symbol = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            _ => return false,
        };
        crate::declarations::standard_declarations::is_core_vector(program, symbol)
    }

    pub fn type_reference_is_collection(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        crate::value_custody::reference_shells::carrier_beneath_borrows(program, type_reference)
            .is_some_and(|carrier| {
                matches!(
                    program.type_reference_table.type_reference(carrier),
                    TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
                )
            })
    }
}
pub(crate) mod declaration_types {
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn declaration_type_equations(
        program: &dyn crate::CheckingInput,
        mut owner: SymbolHandle,
    ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
        let mut substitutions = Vec::new();
        let mut visited = Vec::new();
        while owner.is_valid() && !visited.contains(&owner) {
            visited.push(owner);
            if let Some(machine) = program.machine_by_symbol(owner) {
                for equation in program
                    .declared_type_equations
                    .span_or_empty(machine.declared_type_equations)
                {
                    let TypeReferenceNode::Named { symbol, .. } = program
                        .type_reference_table
                        .type_reference(equation.parameter)
                    else {
                        continue;
                    };
                    if symbol.is_valid()
                        && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                    {
                        substitutions.push((*symbol, equation.definition));
                    }
                }
                closed_requirement_equations(program, machine, &mut substitutions);
            }
            owner = program.declaration_scope_parent(owner);
        }
        substitutions
    }

    fn closed_requirement_equations(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        substitutions: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) {
        let mut pending = program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(typed_trees::domain::ProofFact::as_expression)
            .copied()
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if let typed_trees::expression::ExpressionNode::Binary(binary) =
                program.expression_table.expression(expression)
                && binary.operator == typed_trees::expression::BinaryOperator::And
            {
                pending.extend([binary.left, binary.right]);
                continue;
            }
            let Some((left, right, false)) = program.static_type_predicate_operands(expression)
            else {
                continue;
            };
            for (binder, definition) in [(left, right), (right, left)] {
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(binder)
                    && symbol.is_valid()
                    && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                    && typed_trees::domain::index_argument_is_closed(program, definition)
                    && !substitutions.iter().any(|(existing, _)| existing == symbol)
                {
                    substitutions.push((*symbol, definition));
                }
            }
        }
    }

    pub fn state_root_type(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        root: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, root)
                    .map(|(_, local)| local.type_reference)
            })
    }

    pub fn declaration_type_reference(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let declaration = program.symbols.get(symbol);
        if declaration.kind == SymbolKind::Unknown {
            return None;
        }
        let parent = declaration.parent;
        let owner = program.symbols.get(parent);
        if declaration.kind == SymbolKind::Parameter && owner.kind == SymbolKind::Domain {
            let domain = typed_trees::domain::domain_by_symbol(program, parent)?;
            return (domain.subject_symbol == symbol).then_some(domain.target_type);
        }
        if owner.kind == SymbolKind::Data
            && let Some(definition) = program.data_definition_by_symbol(parent)
            && definition.predicate_subject_symbol == symbol
            && definition.predicate_subject_type.is_valid()
        {
            return Some(definition.predicate_subject_type);
        }
        if declaration.kind == SymbolKind::Const {
            return program.const_declarations().iter().find_map(|declaration| {
                (declaration.symbol == symbol).then_some(declaration.declared_type)
            });
        }
        let data_owner = if owner.kind == SymbolKind::Variant {
            owner.parent
        } else {
            parent
        };
        if let Some(data) = program.data_definition_by_symbol(data_owner) {
            if let Some(field) = program.data_field(data, symbol) {
                return Some(field.type_reference);
            }
            if owner.kind == SymbolKind::Variant
                && let Some(variant) = program.data_variant(data, parent)
                && let Some(field) = program.data_payload_field_by_symbol(variant, symbol)
            {
                return Some(field.type_reference);
            }
        }
        let machine_owner = if owner.kind == SymbolKind::State {
            owner.parent
        } else {
            parent
        };
        if let Some(machine) = program.machine_by_symbol(machine_owner) {
            if let Some(type_reference) =
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .find_map(|parameter| {
                        if parameter.symbol != symbol {
                            return None;
                        }
                        match parameter.kind {
                            TypeParameterKind::Value { type_reference }
                            | TypeParameterKind::Const { type_reference } => Some(type_reference),
                            _ => None,
                        }
                    })
            {
                return Some(type_reference);
            }
            if let Some(type_reference) = program
                .machine_owned_data(machine)
                .iter()
                .find_map(|owned| (owned.symbol == symbol).then_some(owned.type_reference))
            {
                return Some(type_reference);
            }
            for state in program
                .machine_states(machine)
                .iter()
                .filter(|state| state.symbol == parent)
            {
                if let Some(type_reference) = state_root_type(program, state, symbol) {
                    return Some(type_reference);
                }
            }
        }
        if let Some(parameters) = crate::call_target_parameters(program, parent)
            && let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        {
            return Some(parameter.type_reference);
        }
        for proposition in program
            .propositions()
            .iter()
            .filter(|proposition| proposition.symbol == parent)
        {
            if let Some(type_reference) = program
                .proposition_parameters(proposition)
                .iter()
                .find_map(|parameter| {
                    (parameter.symbol == symbol).then_some(parameter.type_reference)
                })
            {
                return Some(type_reference);
            }
        }
        None
    }
}
pub(crate) use declaration_types::declaration_type_equations;
pub use declaration_types::declaration_type_reference;
pub use declaration_types::state_root_type;

use crate::frozen_program::expression_table;
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::struct_literals::data_declares_field;
use access_plans::BorrowPolarity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn assignment_targets_parameter_referent(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    target: ExpressionHandle,
) -> bool {
    crate::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        state.statement_nodes.count() as usize,
        target,
    )
    .is_some_and(|place| {
        place.segments.is_empty()
            && place
                .root
                .as_symbol()
                .and_then(|symbol| program.state_parameter_by_symbol(state, symbol))
                .is_some_and(|parameter| {
                    crate::reference_shell(program, parameter.type_reference)
                        .is_some_and(|shell| shell.access.is_exclusive())
                })
    })
}

pub(crate) fn validate_assignment_target_handle(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    state_name: &str,
    receiver: BorrowPolarity,
) {
    let machine_name = machine.name.as_str();
    if !is_mutable_place_handle(program, target) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment target must be a named place"
        )));
        return;
    }

    if let Some(field_name) = direct_self_field_member(program, target)
        && let Some(data) = machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment: data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            data.name.as_str()
        )));
        return;
    }

    if let Some((container, member)) =
        first_unknown_nested_field(program, machine, current_state, target)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment: data `{container}` has \
             no field `{member}` (check the spelling of the field name)"
        )));
        return;
    }

    let Some(root_name) = expression_root_name_handle(program, target) else {
        return;
    };

    let target_is_bare_name = matches!(
        expression_table(program).expression(target),
        ExpressionNode::Name(_)
    );

    if receiver == BorrowPolarity::Shared {
        if !writable_roots.contains_for_shared_access(root_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` atomic operation cannot reach \
                 `{root_name}` because no such field or local exists in this state"
            )));
        }
        return;
    }

    let access = crate::value_custody::reference_access::assignment_storage_write_access(
        program,
        machine,
        current_state,
        writable_roots.statements,
        target,
    );
    let writes_borrowed_storage =
        access == Some(crate::value_custody::expression_types::StorageWriteAccess::BorrowedStorage);
    if !writable_roots.contains_for_write(root_name, target_is_bare_name)
        && !writes_borrowed_storage
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment cannot write `{root_name}` \
             because it is not mutable in this state (if `{root_name}` is undeclared -- a typo -- \
             no such field or local exists)"
        )));
    } else if access.is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{machine_name}` state `{state_name}` assignment requires mutable source access \
             through every reference crossed by its destination"
        )));
    }
}

fn is_mutable_place_handle(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match expression_table(program).expression(expression) {
        ExpressionNode::Indexed(indexed) => is_mutable_place_handle(program, indexed.collection),
        ExpressionNode::Member(member) => is_mutable_place_handle(program, member.receiver),
        ExpressionNode::Name(_) => true,
        _ => false,
    }
}

fn expression_root_name_handle(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<&str> {
    match expression_table(program).expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_name_handle(program, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            match expression_table(program).expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && expression_table(program)
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.is_self_receiver()) =>
                {
                    Some(member.member.as_str())
                }
                _ => expression_root_name_handle(program, member.receiver),
            }
        }
        ExpressionNode::Name(path) => expression_table(program)
            .name_path_members(path.members)
            .first()
            .map(|name| name.as_str()),
        _ => None,
    }
}

pub(crate) fn direct_self_field_member(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
) -> Option<&str> {
    match expression_table(program).expression(target) {
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let path = expression_table(program).name_path_node(member.receiver)?;
            let receiver = expression_table(program).name_path_members(path.members);
            (receiver.len() == 1 && receiver[0].is_self_receiver()).then(|| member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            let members = expression_table(program).name_path_members(path.members);
            (members.len() == 2 && members[0].is_self_receiver()).then(|| members[1].as_str())
        }
        _ => None,
    }
}

pub(crate) fn machine_attached_data<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<&'a DataDefinition> {
    program.unique_data_definition_by_symbol(machine.attached_data_symbol)
}

pub(crate) fn declared_place_type(
    program: &dyn crate::CheckingInput,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let raw = declared_place_type_raw(program, current_machine, current_state, argument)?;
    unwrapped_type_reference(program, raw)
}

pub fn assignment_value_type(
    program: &dyn crate::CheckingInput,
    destination: TypeReferenceHandle,
) -> TypeReferenceHandle {
    match program.type_reference_table.type_reference(destination) {
        TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => destination,
    }
}

pub fn declared_place_type_raw(
    program: &dyn crate::CheckingInput,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    if let Some(inner) = expression_table(program).borrow_expression(handle) {
        handle = inner.target;
    }

    if let ExpressionNode::SavedSubject(saved) = expression_table(program).expression(handle) {
        return program.saved_value_source(saved).and_then(|source| {
            crate::expression_result_type_reference(
                program,
                current_machine,
                current_state?,
                source,
            )
        });
    }

    if let Some(result) = crate::reserved_result_place(program, handle)
        && result.machine_symbol == current_machine.symbol
    {
        return Some(result.type_reference);
    }

    if let Some(path) = expression_table(program).name_path_node(handle)
        && expression_table(program).name_path_is_single_member(path)
        && let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol)
        && let typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
    {
        return Some(type_reference);
    }

    if let Some(path) = expression_table(program).name_path_node(handle)
        && path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1
        && expression_table(program).name_path_member_symbols(path.member_symbols) == [path.symbol]
        && let Some(state) = current_state
    {
        let mut matching = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| statement.as_local_data())
            .filter(|local| local.symbol == path.symbol)
            .map(|local| local.type_reference)
            .chain(
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| parameter.symbol == path.symbol)
                    .map(|parameter| parameter.type_reference),
            );
        if let Some(reference) = matching.next()
            && matching.next().is_none()
            && reference.is_valid()
        {
            return Some(reference);
        }
    }

    if let Some(path) = expression_table(program).name_path_node(handle)
        && path.head_symbol == path.symbol
        && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Field
        && expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1
    {
        return None;
    }

    if projected_members::contains_case_projection(program, handle) {
        return projected_members::declared_case_projection_type(
            program,
            current_machine,
            current_state,
            handle,
        );
    }

    if let Some(field) = exact_self_field(program, current_machine, handle) {
        return Some(field.type_reference);
    }

    if let Some(member) = expression_table(program).member_node(handle) {
        let receiver =
            declared_place_type_raw(program, current_machine, current_state, member.receiver)?;
        let symbol = member_resolution::effective_member_symbol_from_position(
            program,
            member.receiver,
            Some(member_resolution::MemberPosition::Reference(receiver)),
            member,
        );
        return project_type_reference_from_segments(
            program,
            receiver,
            &[facts::PlaceSegment::Field { symbol }],
        );
    }
    if let Some(members) = collect_member_path(program, handle) {
        return declared_member_path_type(program, current_machine, current_state, &members);
    }
    match expression_table(program).expression(handle) {
        ExpressionNode::ArrayLiteral(_) => crate::declared_constant_array_type(program, handle),
        ExpressionNode::Call(_) => crate::machine_calls::calls::call_result_type(program, handle),
        ExpressionNode::Indexed(_) => {
            declared_indexed_projection_type_raw(program, current_machine, current_state, handle)
        }
        _ => None,
    }
}

pub(crate) fn declared_member_field_symbol(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let member = expression_table(program).member_node(expression)?;
    if member.case_variant.is_some() {
        return None;
    }
    let receiver = declared_place_type_raw(program, machine, state, member.receiver)?;
    let data = data_definition_for_type(program, receiver)?;
    Some(data_field_or_payload(program, data, member.member.as_str())?.symbol)
}

pub(crate) fn bound_symbol_declared_type(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    let mut found = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol && found.replace(parameter.type_reference).is_some() {
                    return None;
                }
            }
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
            {
                if let Some(local) = statement.as_local_data()
                    && local.symbol == symbol
                    && found.replace(local.type_reference).is_some()
                {
                    return None;
                }
            }
        }
    }
    found
}

pub(crate) mod array_extents {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::state::State;
    use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, Debug)]
    pub enum ResolvedArrayExtent {
        Closed(BigInt),
        Bound {
            symbol: SymbolHandle,
            type_reference: TypeReferenceHandle,
        },
    }

    pub fn resolved_array_extent_at(
        program: &dyn crate::CheckingInput,
        state: &State,
        statement_index: usize,
        collection: ExpressionHandle,
    ) -> Result<ResolvedArrayExtent, &'static str> {
        let (machine, _) = crate::find_state(program, state.symbol)
            .ok_or("array collection has no resolved executable owner")?;
        let reference =
            crate::expression_result_type_reference(program, machine, state, collection)
                .ok_or("array collection has no checked result type")?;
        let source = super::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            collection,
        )
        .and_then(|place| {
            super::canonical_place_source_type_application(
                program,
                state.symbol,
                statement_index,
                &place,
            )
        });
        resolved_projected_array_extent(program, reference, source)
    }

    pub fn resolved_projected_array_extent(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        source: Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)>,
    ) -> Result<ResolvedArrayExtent, &'static str> {
        if matches!(
            super::fixed_array_type_extent(program, reference),
            Some(FixedArrayLength::Literal(_))
        ) {
            return resolved_array_extent(program, reference, &[]);
        }
        let (source, applications) =
            source.ok_or("open array extent has no supplying type applications")?;
        resolved_array_extent(program, source, &applications)
    }

    pub fn resolved_array_extent(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        applications: &[TypeReferenceHandle],
    ) -> Result<ResolvedArrayExtent, &'static str> {
        let length = super::fixed_array_type_extent(program, reference)
            .ok_or("collection is not an always-full fixed array")?;
        program
            .with_enclosing_data_identity_scopes(applications, |scopes| {
                let resolver = ExtentResolver { program, scopes };
                match length {
                    FixedArrayLength::Literal(value) => u64::try_from(*value)
                        .map(|value| ResolvedArrayExtent::Closed(BigInt::from_u64(value)))
                        .map_err(|_| "array extent is not representable"),
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        resolver.symbol(*symbol, scopes.len())
                    }
                    FixedArrayLength::Expression(reference) => {
                        resolver.reference(*reference, scopes.len())
                    }
                    _ => Err("array extent still requires selected constant evaluation"),
                }
            })
            .ok_or("array extent has an unauthenticated enclosing application")?
    }

    pub(crate) struct ExtentResolver<'program, 'scope, 'argument> {
        program: &'program dyn crate::CheckingInput,
        scopes: &'scope [AppliedTypeIdentityScope<'argument>],
    }

    impl ExtentResolver<'_, '_, '_> {
        fn symbol(
            &self,
            symbol: SymbolHandle,
            visible: usize,
        ) -> Result<ResolvedArrayExtent, &'static str> {
            if !symbol.is_valid() {
                return Err("array extent has no resolved declaration");
            }
            for (position, scope) in self.scopes[..visible].iter().enumerate().rev() {
                let mut bindings = scope
                    .arguments
                    .iter()
                    .filter(|(formal, _)| *formal == symbol);
                let Some((_, actual)) = bindings.next() else {
                    continue;
                };
                if bindings.next().is_some() {
                    return Err("array extent has ambiguous supplying arguments");
                }
                let parameter = self
                    .program
                    .data_type_parameter_by_symbol(symbol)
                    .ok_or("array extent argument has no declaration")?;
                if !matches!(
                    parameter.kind,
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                ) {
                    return Err("array extent argument is not a const or value parameter");
                }
                let IndexIdentityArgument::TypeReference(actual) = actual else {
                    return Err("array extent argument is not a static value operand");
                };
                return self.reference(*actual, position);
            }
            if let Some(parameter) = self.program.data_type_parameter_by_symbol(symbol) {
                let (TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference }) = parameter.kind
                else {
                    return Err("array extent names neither a const nor a value parameter");
                };
                if !self
                    .program
                    .primitive_type_reference(type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                {
                    return Err("array extent parameter has no fixed integer carrier");
                }
                return Ok(ResolvedArrayExtent::Bound {
                    symbol,
                    type_reference,
                });
            }
            let declaration = self
                .program
                .const_declaration_by_symbol(symbol)
                .ok_or("array extent names no const declaration or parameter")?;
            let encoding = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
                .ok_or("array extent constant has no evaluated value")?;
            self.decoded(CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding())
        }

        fn reference(
            &self,
            reference: TypeReferenceHandle,
            visible: usize,
        ) -> Result<ResolvedArrayExtent, &'static str> {
            if !self
                .program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return Err("array extent argument has no retained type operand");
            }
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    self.symbol(*symbol, visible)
                }
                TypeReferenceNode::Named { name, .. } => {
                    if let Ok(value) = name.as_str().parse::<i128>() {
                        return Ok(ResolvedArrayExtent::Closed(BigInt::from_i128(value)));
                    }
                    self.decoded(
                        CanonicalConstValue::from_atom(name.as_str())
                            .and_then(|value| value.decode_encoding()),
                    )
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    match self.program.expression_table.expression(*expression) {
                        ExpressionNode::Name(path) => self.symbol(path.symbol, visible),
                        ExpressionNode::Integer(literal) => literal
                            .value_bignum()
                            .map(ResolvedArrayExtent::Closed)
                            .ok_or("array extent literal is not an integer"),
                        _ => Err("array extent expression has not been reduced to a static value"),
                    }
                }
                _ => Err("array extent argument is not a retained integer value"),
            }
        }

        fn decoded(
            &self,
            value: Option<DecodedCanonicalConstValue>,
        ) -> Result<ResolvedArrayExtent, &'static str> {
            match value {
                Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                    Ok(ResolvedArrayExtent::Closed(BigInt::from_i128(value)))
                }
                _ => Err("array extent constant is not a canonical integer"),
            }
        }
    }
}
pub(crate) mod attached_storage {
    use super::{exact_self_field, single_member_receiver_field};
    use language_core::{SELF_RECEIVER, place_field, place_label};
    use symbols::SymbolHandle;
    use typed_trees::data::DataField;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;

    pub struct AttachedStorageRoot<'parameters> {
        pub name: &'parameters str,
        pub symbol: SymbolHandle,
    }

    pub fn attached_storage_root<'parameters>(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        parameters: &'parameters [StateParameter],
    ) -> Option<AttachedStorageRoot<'parameters>> {
        if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
            return None;
        }
        let parameter = program.data_binding_parameter(machine, parameters)?;

        parameter.symbol.is_valid().then(|| AttachedStorageRoot {
            name: parameter.name.as_str(),
            symbol: parameter.symbol,
        })
    }

    pub fn exact_attached_storage_field<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<&'program DataField> {
        if let Some(field) = exact_self_field(program, machine, expression) {
            return Some(field);
        }
        let root = attached_storage_root(program, machine, parameters)?;
        if root.name == SELF_RECEIVER {
            return None;
        }
        single_member_receiver_field(program, machine, expression, root.symbol, |only| {
            only.as_str() == root.name
        })
    }

    pub fn expression_names_attached_storage(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> bool {
        let Some(root) = attached_storage_root(program, machine, parameters) else {
            return false;
        };
        let mut current = expression;
        for _ in 0..128 {
            match program.expression_table.expression(current) {
                ExpressionNode::Member(member) if member.case_variant.is_none() => {
                    current = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => current = indexed.collection,
                ExpressionNode::Borrow(borrow) => current = borrow.target,
                ExpressionNode::Name(path) => {
                    return path.symbol == root.symbol
                        && path.head_symbol == root.symbol
                        && matches!(program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == root.name);
                }
                _ => return false,
            }
        }
        false
    }

    pub fn canonical_storage_label(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        parameters: &[StateParameter],
        label: &str,
    ) -> String {
        let Some(root) = attached_storage_root(program, machine, parameters) else {
            return label.to_owned();
        };
        if root.name == SELF_RECEIVER {
            return label.to_owned();
        }
        match place_field(root.name, label) {
            Some(field) => place_label(SELF_RECEIVER, field),
            None => label.to_owned(),
        }
    }

    pub fn declaration_storage_label(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        parameters: &[StateParameter],
        canonical: &str,
    ) -> Option<String> {
        let Some(field) = place_field(SELF_RECEIVER, canonical) else {
            return Some(canonical.to_owned());
        };
        let root = attached_storage_root(program, machine, parameters)?;
        Some(if root.name == SELF_RECEIVER {
            canonical.to_owned()
        } else {
            place_label(root.name, field)
        })
    }
}
pub use array_extents::{
    ResolvedArrayExtent, resolved_array_extent, resolved_array_extent_at,
    resolved_projected_array_extent,
};
pub(crate) mod builtin_coordinates {
    use super::{declared_indexed_projection_type, declared_place_type_raw};
    use crate::frozen_program::expression_table;
    use language_core::OperatorSpelling;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    pub fn collection_length_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let receiver = expression_table(program).authored_length_receiver(expression)?;
        let receiver_type = declared_place_type_raw(program, machine, state, receiver)?;
        let receiver_type = super::unwrapped_type_reference(program, receiver_type)?;
        matches!(
            program.type_reference_table.type_reference(receiver_type),
            typed_trees::types::TypeReferenceNode::Slice { .. }
                | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        )
        .then_some(receiver)
    }

    pub fn declared_collection_extent(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        receiver: ExpressionHandle,
    ) -> Option<usize> {
        let declared = declared_place_type_raw(program, machine, state, receiver)?;
        fixed_array_type_length(program, declared)
    }

    pub fn fixed_array_type_length(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<usize> {
        match fixed_array_type_extent(program, reference)? {
            typed_trees::types::FixedArrayLength::Literal(extent) => Some(*extent),
            _ => None,
        }
    }

    pub fn fixed_array_type_extent(
        program: &dyn crate::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<&typed_trees::types::FixedArrayLength> {
        loop {
            if crate::bounded_byte_buffer_capacity(program, reference).is_some() {
                return None;
            }
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::FixedArray { length, .. } => {
                    return Some(length);
                }
                typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                | typed_trees::types::TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                _ => return None,
            }
        }
    }

    pub fn subslice_expression(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let operation = match expression_table(program).expression(expression) {
            ExpressionNode::Borrow(borrow)
                if matches!(
                    borrow.access,
                    language_core::ReferenceAccess::Shared
                        | language_core::ReferenceAccess::Mutable
                ) =>
            {
                borrow.target
            }
            ExpressionNode::Borrow(_) => return None,
            _ => expression,
        };
        let indexed = expression_table(program).indexed_node(operation)?;
        matches!(
            expression_table(program).expression(indexed.index),
            ExpressionNode::Range(_)
        )
        .then_some(operation)
    }

    pub fn has_builtin_subslice_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(indexed) = expression_table(program).indexed_node(expression) else {
            return false;
        };
        if program.expression_table.range_node(indexed.index).is_none() {
            return false;
        }
        indexed_operation_has_builtin_meaning(
            program,
            machine,
            state,
            expression,
            declared_place_type_raw(program, machine, state, indexed.collection),
        )
    }

    pub fn range_operator_establishes_endpoint_extent(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(indexed) = expression_table(program).indexed_node(expression) else {
            return false;
        };
        let Some(range) = expression_table(program).range_node(indexed.index) else {
            return false;
        };
        let Some(collection) = declared_place_type_raw(program, machine, state, indexed.collection)
        else {
            return false;
        };
        if !reference_has_builtin_collection_geometry(program, collection) {
            return false;
        }
        let operands = [
            Some(collection),
            declared_place_type_raw(program, machine, state, range.start),
            declared_place_type_raw(program, machine, state, range.end),
        ];
        let candidates = typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            OperatorSpelling::Range,
            &operands,
            expression_table(program).source_span(expression),
        );
        let [candidate] = candidates.as_slice() else {
            return false;
        };
        if typed_trees::operator::selected_trait_operator_meaning_exists(
            program.typed_program(),
            machine.symbol,
            OperatorSpelling::Range,
            &operands,
        ) {
            return false;
        }
        operator_ensures_endpoint_extent(program, candidate.operator)
    }

    pub fn ensured_range_call_endpoints(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, ExpressionHandle)> {
        let call = expression_table(program).call_node(expression)?;
        let arguments = expression_table(program).expression_handles(call.arguments);
        let [_, start, end] = arguments else {
            return None;
        };
        call_target_ensures_endpoint_extent(program, call.target_symbol).then_some((*start, *end))
    }

    fn call_target_ensures_endpoint_extent(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> bool {
        if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol)
            && operator_ensures_endpoint_extent(program, operator)
        {
            return true;
        }
        if let Some(machine) = program.machine_by_symbol(symbol) {
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            let parameters = program.state_parameters(entry);
            return contracts_establish_endpoint_extent(
                program,
                machine.symbol,
                parameters,
                program.machine_contracts(machine),
            ) || contracts_establish_endpoint_extent(
                program,
                entry.symbol,
                parameters,
                program.state_contracts(entry),
            );
        }
        let Some(state) = program.state_by_symbol(symbol) else {
            return false;
        };
        let parameters = program.state_parameters(state);
        contracts_establish_endpoint_extent(
            program,
            state.symbol,
            parameters,
            program.state_contracts(state),
        ) || program
            .machine_holding_state(state.symbol)
            .is_some_and(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == state.symbol)
                    && contracts_establish_endpoint_extent(
                        program,
                        machine.symbol,
                        parameters,
                        program.machine_contracts(machine),
                    )
            })
    }

    fn operator_ensures_endpoint_extent(
        program: &dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> bool {
        operator.spelling == Some(OperatorSpelling::Range)
            && contracts_establish_endpoint_extent(
                program,
                operator.symbol,
                program.operator_parameters(operator),
                program.operator_contracts(operator),
            )
    }

    fn contracts_establish_endpoint_extent(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        contracts: &[typed_trees::signature::SignatureContract],
    ) -> bool {
        let [_, start, end] = parameters else {
            return false;
        };
        if ![start, end].into_iter().all(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some_and(|primitive| primitive.integer_sign_bits().is_some())
                && program.arithmetic_domain_for_type_reference(parameter.type_reference)
                    == numerics::arithmetic::ArithmeticDomain::Exact
        }) {
            return false;
        }
        contracts
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| {
                let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    return false;
                };
                expression_establishes_endpoint_extent(program, owner, parameters, *expression, 0)
            })
    }

    fn expression_establishes_endpoint_extent(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        if depth >= 128 {
            return false;
        }
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        if binary.operator == typed_trees::expression::BinaryOperator::And {
            return [binary.left, binary.right].into_iter().any(|part| {
                expression_establishes_endpoint_extent(program, owner, parameters, part, depth + 1)
            });
        }
        if binary.operator != typed_trees::expression::BinaryOperator::Equal
            || !crate::has_builtin_parameter_bound_expression_meaning(
                program, owner, parameters, expression,
            )
        {
            return false;
        }
        [(binary.left, binary.right), (binary.right, binary.left)]
            .into_iter()
            .any(|(length, difference)| {
                let Some(member) = expression_table(program).member_node(length) else {
                    return false;
                };
                let Some((result_owner, result_type)) =
                    crate::reserved_result_owner(program, member.receiver)
                else {
                    return false;
                };
                if member.case_variant.is_some()
                    || member.member.as_str() != "len"
                    || result_owner != owner
                    || !reference_has_builtin_collection_geometry(program, result_type)
                {
                    return false;
                }
                let Some(subtract) = expression_table(program).binary_node(difference) else {
                    return false;
                };
                subtract.operator == typed_trees::expression::BinaryOperator::Subtract
                    && expression_names_source_parameter(
                        program,
                        owner,
                        parameters,
                        subtract.left,
                        2,
                    )
                    && expression_names_source_parameter(
                        program,
                        owner,
                        parameters,
                        subtract.right,
                        1,
                    )
            })
    }

    fn expression_names_source_parameter(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        position: usize,
    ) -> bool {
        let Some(parameter) = parameters
            .get(position)
            .filter(|parameter| parameter.symbol.is_valid())
        else {
            return false;
        };
        if crate::proof_contracts::contract_results::source_contract_declaration_owner(
            program, expression,
        ) != Ok(owner)
        {
            return false;
        }
        let Some(source_parameters) =
            crate::proof_contracts::contract_results::source_contract_parameters(
                program, owner, expression,
            )
        else {
            return false;
        };
        if source_parameters.len() != parameters.len()
            || !source_parameters
                .iter()
                .zip(parameters)
                .all(|(source, parameter)| {
                    source.symbol == parameter.symbol
                        && source.type_reference == parameter.type_reference
                })
        {
            return false;
        }
        let Some(path) = expression_table(program).name_path_node(expression) else {
            return false;
        };
        let [name] = expression_table(program).name_path_members(path.members) else {
            return false;
        };
        name.as_str() == parameter.name.as_str()
            && source_parameters
                .iter()
                .filter(|source| source.name == parameter.name)
                .count()
                == 1
            && crate::queries::expression_traversal::single_segment_name_symbol(program, expression)
                .filter(|symbol| symbol.is_valid())
                .is_none_or(|symbol| symbol == parameter.symbol)
    }

    fn reference_has_builtin_collection_geometry(
        program: &dyn crate::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        for _ in 0..128 {
            if !reference.is_valid() {
                return false;
            }
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                | typed_trees::types::TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => {
                    reference = *referee;
                }
                typed_trees::types::TypeReferenceNode::FixedArray { .. }
                | typed_trees::types::TypeReferenceNode::Slice { .. } => return true,
                _ => return false,
            }
        }
        false
    }

    pub fn indexed_operation_has_builtin_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        collection: Option<typed_trees::types::TypeReferenceHandle>,
    ) -> bool {
        let Some(indexed) = expression_table(program).indexed_node(expression) else {
            return false;
        };
        let Some(collection) = collection else {
            return false;
        };
        let mut operands = [Some(collection), None, None];
        let (spelling, operand_count) =
            if let Some(range) = expression_table(program).range_node(indexed.index) {
                operands[1] = declared_place_type_raw(program, machine, state, range.start);
                operands[2] = declared_place_type_raw(program, machine, state, range.end);
                (OperatorSpelling::Range, 3)
            } else {
                operands[1] = declared_place_type_raw(program, machine, state, indexed.index);
                (OperatorSpelling::Index, 2)
            };
        let operands = &operands[..operand_count];
        if spelling == OperatorSpelling::Index {
            return typed_trees::operator::builtin_index_read_is_spelled(
                program,
                machine.symbol,
                expression,
                operands,
            );
        }
        typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            spelling,
            operands,
            expression_table(program).source_span(expression),
        )
        .is_empty()
            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                operands,
            )
    }

    pub fn whole_collection_view_source(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let operation = subslice_expression(program, expression)?;
        if !has_builtin_subslice_meaning(program, machine, state, operation) {
            return None;
        }
        let indexed = expression_table(program).indexed_node(operation)?;
        let range = expression_table(program).range_node(indexed.index)?;
        let integer = |expression| {
            if let Some(value) =
                program.closed_integer_expression_value_in(expression, machine.symbol)
            {
                value.to_u64()
            } else if let Some(state) = state {
                crate::captured_integer_value(program, machine, state, expression, |_, _| None)
                    .and_then(|value| value.to_u64())
            } else {
                None
            }
        };
        if range.start.is_valid() && integer(range.start) != Some(0) {
            return None;
        }
        if !range.end.is_valid() {
            return (!range.end_inclusive).then_some(indexed.collection);
        }
        if let Some(end) = integer(range.end)
            .and_then(|end| usize::try_from(end).ok())
            .and_then(|end| {
                if range.end_inclusive {
                    end.checked_add(1)
                } else {
                    Some(end)
                }
            })
            && declared_collection_extent(program, machine, state, indexed.collection) == Some(end)
        {
            return Some(indexed.collection);
        }
        None
    }

    pub fn assignment_target_selectors(
        program: &dyn crate::CheckingInput,
        target: ExpressionHandle,
    ) -> Vec<ExpressionHandle> {
        assignment_target_indexes(expression_table(program), target)
            .into_iter()
            .filter_map(
                |expression| match expression_table(program).expression(expression) {
                    ExpressionNode::Indexed(indexed) => Some(indexed.index),
                    _ => None,
                },
            )
            .collect()
    }

    pub fn assignment_target_indexes(
        expressions: &ExpressionTable,
        target: ExpressionHandle,
    ) -> Vec<ExpressionHandle> {
        let mut indexes = Vec::new();
        let mut cursor = target;
        for _ in 0..128 {
            if !expressions.expression_is_valid(cursor) {
                break;
            }
            match expressions.expression(cursor) {
                ExpressionNode::Indexed(indexed) => {
                    if !matches!(
                        expressions.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) {
                        indexes.push(cursor);
                    }
                    cursor = indexed.collection;
                }
                ExpressionNode::Member(member) => cursor = member.receiver,
                _ => break,
            }
        }
        indexes
    }

    pub fn place_has_builtin_coordinates(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        builtin_coordinates(program, machine, state, expression, 0)
    }

    pub fn value_element_has_builtin_coordinate(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        occurrence: ExpressionHandle,
        collection: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let Some(indexed) = expression_table(program).indexed_node(occurrence) else {
            return false;
        };
        if program.expression_table.range_node(indexed.index).is_some() {
            return false;
        }
        indexed_operation_has_builtin_meaning(program, machine, state, occurrence, Some(collection))
            && crate::has_builtin_bound_expression_meaning(program, machine, state, indexed.index)
    }

    pub fn value_has_builtin_coordinates(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        value_has_builtin_coordinates_with_types(program, machine, state, expression, |source| {
            crate::expression_result_type_reference(program, machine, state, source)
        })
    }

    pub fn value_has_builtin_coordinates_with_types(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        mut expression: ExpressionHandle,
        collection_type: impl Fn(ExpressionHandle) -> Option<typed_trees::types::TypeReferenceHandle>,
    ) -> bool {
        for depth in 0..program.expression_table.expression_count() {
            let Some(node) = expression_table(program).expression_at_depth(expression, depth)
            else {
                return false;
            };
            match node {
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => {
                    let collection = collection_type(indexed.collection);
                    if !indexed_operation_has_builtin_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                        collection,
                    ) {
                        return false;
                    }
                    let bounds = expression_table(program)
                        .range_node(indexed.index)
                        .map_or([indexed.index, ExpressionHandle::invalid()], |range| {
                            [range.start, range.end]
                        });
                    if bounds.into_iter().any(|bound| {
                        bound.is_valid()
                            && !crate::has_builtin_bound_expression_meaning(
                                program,
                                machine,
                                Some(state),
                                bound,
                            )
                    }) {
                        return false;
                    }
                    expression = indexed.collection;
                }
                _ => return true,
            }
        }
        false
    }

    pub fn mutable_byte_slice_parameter_element(
        program: &dyn crate::CheckingInput,
        state: &State,
        symbol: symbols::SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        use typed_trees::types::TypeReferenceNode;
        let parameter = program.state_parameter_by_symbol(state, symbol)?;
        let TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Mutable,
            ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        (program.primitive_type_reference(*element_type)
            == Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*element_type)
    }

    fn builtin_coordinates(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        let Some(node) = expression_table(program).expression_at_depth(expression, depth) else {
            return false;
        };
        match node {
            ExpressionNode::Name(_) => true,
            ExpressionNode::SavedSubject(saved) => program.saved_value_source(saved).is_some(),
            ExpressionNode::Borrow(borrow) => {
                builtin_coordinates(program, machine, state, borrow.target, depth + 1)
            }
            ExpressionNode::Member(member) => {
                builtin_coordinates(program, machine, state, member.receiver, depth + 1)
            }
            ExpressionNode::Indexed(indexed) => {
                if !builtin_coordinates(program, machine, state, indexed.collection, depth + 1) {
                    return false;
                }
                if let Some(range) = expression_table(program).range_node(indexed.index) {
                    let collection =
                        declared_place_type_raw(program, machine, state, indexed.collection)
                            .and_then(|reference| {
                                super::unwrapped_type_reference(program, reference)
                            });
                    return collection.is_some_and(|reference| {
                        matches!(
                            program.type_reference_table.type_reference(reference),
                            typed_trees::types::TypeReferenceNode::Slice { .. }
                                | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                        )
                    }) && has_builtin_subslice_meaning(program, machine, state, expression)
                        && [range.start, range.end].into_iter().all(|bound| {
                            !bound.is_valid()
                                || crate::has_builtin_bound_expression_meaning(
                                    program, machine, state, bound,
                                )
                        });
                }
                if declared_indexed_projection_type(program, machine, state, expression).is_none() {
                    return false;
                }
                let operands = [
                    declared_place_type_raw(program, machine, state, indexed.collection).or_else(
                        || {
                            declared_indexed_projection_type(
                                program,
                                machine,
                                state,
                                indexed.collection,
                            )
                        },
                    ),
                    declared_place_type_raw(program, machine, state, indexed.index),
                ];
                typed_trees::operator::builtin_index_read_is_spelled(
                    program,
                    machine.symbol,
                    expression,
                    &operands,
                ) && crate::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    state,
                    indexed.index,
                )
            }
            _ => false,
        }
    }
}
pub(crate) mod exact_self_field {
    use super::exact_data_member_field;
    use symbols::SymbolHandle;
    use typed_trees::data::DataField;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;

    pub fn single_member_receiver_field<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
        receiver_symbol: SymbolHandle,
        member_matches: impl Fn(&typed_trees::name::Identifier) -> bool,
    ) -> Option<&'program DataField> {
        let member = program.expression_table.member_node(expression)?;
        if member.case_variant.is_some() {
            return None;
        }
        let receiver = program.expression_table.name_path_node(member.receiver)?;
        if receiver.symbol != receiver_symbol
            || receiver.head_symbol != receiver_symbol
            || !matches!(program.expression_table.name_path_members(receiver.members),
            [only] if member_matches(only))
        {
            return None;
        }
        exact_attached_field(
            program,
            machine,
            member.member_symbol,
            member.member.as_str(),
        )
    }

    pub fn exact_self_field<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> Option<&'program DataField> {
        if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
            return None;
        }
        single_member_receiver_field(
            program,
            machine,
            expression,
            machine.symbol,
            typed_trees::name::Identifier::is_self_receiver,
        )
    }

    pub fn exact_attached_field<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        field_symbol: SymbolHandle,
        field_name: &str,
    ) -> Option<&'program DataField> {
        if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
            return None;
        }
        let owner = super::unique_data_definition_by_symbol(program, machine.attached_data_symbol)?;
        exact_data_member_field(program, owner, field_symbol, field_name, None)
    }
}
pub(crate) mod structural_scalar_field {
    use crate::frozen_program::expression_table;
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::CheckedUnitStructuralPathSegment;
    use language_semantics::ReferenceAccess;
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::data::DataMember;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{
        PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StructuralScalarField {
        pub root: facts::PlaceRoot,
        pub receiver_value: ExpressionHandle,
        pub statement_ordinal: u32,
        pub type_reference: TypeReferenceHandle,
        pub carrier_type_reference: TypeReferenceHandle,
        pub result_type_reference: TypeReferenceHandle,
        pub path: Vec<CheckedUnitStructuralPathSegment>,
        pub field: SymbolHandle,
        pub field_identity: CheckedMemberIdentity,
        pub primitive_type: PrimitiveType,
        pub domain: ArithmeticDomain,
    }

    pub fn structural_scalar_field_in_parameters(
        program: &dyn crate::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        bindings: &[(super::CanonicalPlace, super::CanonicalPlace)],
        expression: ExpressionHandle,
    ) -> Option<StructuralScalarField> {
        let coordinate = super::canonical_place_from_expression(program, expression)?;
        let mut rebased = None;
        for (formal, actual) in bindings {
            if formal.root != coordinate.root {
                continue;
            }
            let Some(suffix) = coordinate.segments.strip_prefix(formal.segments.as_slice()) else {
                continue;
            };
            let mut segments = actual.segments.clone();
            segments.extend_from_slice(suffix);
            let actual = super::CanonicalPlace {
                root: actual.root,
                segments,
            };
            if rebased.as_ref().is_some_and(|selected| selected != &actual) {
                return None;
            }
            rebased = Some(actual);
        }
        let coordinate = rebased.unwrap_or(coordinate);
        let facts::PlaceRoot::Symbol(parameter) = coordinate.root else {
            return None;
        };
        structural_scalar_field_at_coordinate(program, parameters, parameter, &coordinate.segments)
    }

    pub(crate) mod coordinates {
        use super::*;
        use typed_trees::signature::StateParameter;

        #[derive(Clone)]
        pub enum ScalarCoordinateProjection {
            Storage(checked_trees::CheckedUnitStructuralPathSegment),
            Element {
                selector: checked_trees::CheckedEntryIndexSelector,
                collection_path: Vec<facts::PlaceSegment>,
                source_step: usize,
                collection_type: TypeReferenceHandle,
                element_type: TypeReferenceHandle,
                extent: Option<usize>,
            },
        }

        pub struct StructuralCoordinate {
            pub root: facts::PlaceRoot,
            pub type_reference: TypeReferenceHandle,
            pub result_type_reference: TypeReferenceHandle,
            pub projections: Vec<ScalarCoordinateProjection>,
            pub source_requirements: Vec<Vec<facts::PlaceSegment>>,
            endpoint: Endpoint,
            stored_crossing: bool,
        }

        pub struct StructuralScalarCoordinate {
            pub root: facts::PlaceRoot,
            pub type_reference: TypeReferenceHandle,
            pub result_type_reference: TypeReferenceHandle,
            pub primitive_type: PrimitiveType,
            pub domain: ArithmeticDomain,
            pub projections: Vec<ScalarCoordinateProjection>,
            pub source_requirements: Vec<Vec<facts::PlaceSegment>>,
            endpoint: Endpoint,
        }

        pub(crate) enum Endpoint {
            Field {
                carrier: TypeReferenceHandle,
                symbol: SymbolHandle,
                identity: checked_trees::CheckedMemberIdentity,
                position: usize,
            },
            Element {
                carrier: TypeReferenceHandle,
                position: usize,
            },
            Root,
        }

        fn ordinary_carrier(
            program: &dyn crate::CheckingInput,
            carrier: TypeReferenceHandle,
        ) -> Option<()> {
            let declared = program
                .type_reference_table
                .unconstrained_type_reference(carrier);
            if program
                .access_view_plan_for_type_reference(declared)
                .is_some()
                || program
                    .access_view_field_plan_for_type_reference(declared)
                    .is_some()
                || matches!(program.type_reference_table.type_reference(declared),
            TypeReferenceNode::Named { symbol, .. } if program.symbols.builtin_type_atom(*symbol).is_some_and(|atom| atom.is_atomic()))
            {
                return None;
            }
            Some(())
        }

        pub fn structural_scalar_coordinate(
            program: &dyn crate::CheckingInput,
            parameters: &[StateParameter],
            parameter: SymbolHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralScalarCoordinate> {
            let mut inputs = parameters.iter().filter(|input| {
                input.symbol == parameter && !input.is_const && !input.relevance.is_erased()
            });
            let input = inputs.next()?;
            if inputs.next().is_some() {
                return None;
            }
            structural_scalar_coordinate_in_type(
                program,
                facts::PlaceRoot::Symbol(parameter),
                input.type_reference,
                coordinate,
            )
        }

        pub fn structural_scalar_coordinate_in_type(
            program: &dyn crate::CheckingInput,
            root: facts::PlaceRoot,
            source_type: TypeReferenceHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralScalarCoordinate> {
            let selected = structural_coordinate_in_type(program, root, source_type, coordinate)?;
            let StructuralCoordinate {
                root,
                type_reference: source_type,
                result_type_reference,
                mut projections,
                mut source_requirements,
                endpoint,
                stored_crossing,
            } = selected;
            let mut crossings = Vec::new();
            let readable_leaf =
                readable_projection_carrier(program, result_type_reference, &mut crossings)?;
            ordinary_carrier(program, readable_leaf)?;
            let primitive_type = realized_scalar_carrier(program, result_type_reference)?;
            if !stored_crossing && !crossings.is_empty() {
                source_requirements[0].clear();
                source_requirements[0].extend_from_slice(coordinate);
            }
            projections.extend(
                crossings
                    .into_iter()
                    .map(ScalarCoordinateProjection::Storage),
            );
            Some(StructuralScalarCoordinate {
                root,
                type_reference: source_type,
                result_type_reference,
                primitive_type,
                domain: program.arithmetic_domain_for_type_reference(readable_leaf),
                projections,
                source_requirements,
                endpoint,
            })
        }

        pub fn structural_coordinate(
            program: &dyn crate::CheckingInput,
            parameters: &[StateParameter],
            parameter: SymbolHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralCoordinate> {
            let mut inputs = parameters.iter().filter(|input| {
                input.symbol == parameter && !input.is_const && !input.relevance.is_erased()
            });
            let input = inputs.next()?;
            if inputs.next().is_some() {
                return None;
            }
            structural_coordinate_in_type(
                program,
                facts::PlaceRoot::Symbol(parameter),
                input.type_reference,
                coordinate,
            )
        }

        pub fn structural_coordinate_in_type(
            program: &dyn crate::CheckingInput,
            root: facts::PlaceRoot,
            source_type: TypeReferenceHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralCoordinate> {
            let source_type = match program
                .type_reference_table
                .type_reference(crate::peel_declared_constraints(program, source_type))
            {
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if access.is_readable() => *referee,
                TypeReferenceNode::Reference { .. } => return None,
                _ => source_type,
            };
            let mut carrier = source_type;
            let mut projections = Vec::new();
            let mut selected_case = SymbolHandle::invalid();
            let mut endpoint = Endpoint::Root;
            let mut stored_crossing = false;
            for (source_step, segment) in coordinate.iter().enumerate() {
                let mut crossings = Vec::new();
                carrier = readable_projection_carrier(program, carrier, &mut crossings)?;
                stored_crossing |= !crossings.is_empty();
                projections.extend(
                    crossings
                        .into_iter()
                        .map(ScalarCoordinateProjection::Storage),
                );
                ordinary_carrier(program, carrier)?;
                match segment {
                    facts::PlaceSegment::Case { variant } => {
                        if selected_case.is_valid() {
                            return None;
                        }
                        let record =
                            crate::field_domains::readable_nominal_definition(program, carrier)?;
                        let mut cases = program
                            .data_members(record)
                            .iter()
                            .filter_map(|member| member.as_variant())
                            .filter(|case| case.symbol == *variant);
                        let case = cases.next()?;
                        if cases.next().is_some() {
                            return None;
                        }
                        selected_case = case.symbol;
                        projections.push(ScalarCoordinateProjection::Storage(
                            checked_trees::CheckedUnitStructuralPathSegment::Case(
                                checked_trees::CheckedMemberIdentity::of_member(
                                    case.identity,
                                    case.name.clone(),
                                ),
                            ),
                        ));
                        endpoint = Endpoint::Root;
                    }
                    facts::PlaceSegment::Field { symbol } => {
                        let record =
                            crate::field_domains::readable_nominal_definition(program, carrier)?;
                        let fields = if selected_case.is_valid() {
                            let case = program
                                .data_members(record)
                                .iter()
                                .filter_map(|member| member.as_variant())
                                .find(|case| case.symbol == selected_case)?;
                            program.data_payload_fields(case).iter().collect::<Vec<_>>()
                        } else {
                            program
                                .data_members(record)
                                .iter()
                                .filter_map(|member| {
                                    if let DataMember::Field(field) = member {
                                        Some(field)
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        };
                        let mut fields = fields.into_iter().filter(|field| field.symbol == *symbol);
                        let field = fields.next()?;
                        if fields.next().is_some()
                            || field.relevance.is_erased()
                            || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
                            || program.symbols.get(field.symbol).parent
                                != if selected_case.is_valid() {
                                    selected_case
                                } else {
                                    record.symbol
                                }
                        {
                            return None;
                        }
                        let identity = checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        );
                        endpoint = Endpoint::Field {
                            carrier,
                            symbol: field.symbol,
                            identity: identity.clone(),
                            position: projections.len(),
                        };
                        projections.push(ScalarCoordinateProjection::Storage(
                            checked_trees::CheckedUnitStructuralPathSegment::Field(identity),
                        ));
                        carrier = field.type_reference;
                        selected_case = SymbolHandle::invalid();
                    }
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
                        if !selected_case.is_valid() =>
                    {
                        let (element, extent) = if let Some((element, extent)) =
                            crate::value_custody::field_domains::readable_fixed_array_elements(
                                program, carrier,
                            ) {
                            (element, Some(extent))
                        } else {
                            (
                                crate::value_custody::field_domains::readable_slice_element(
                                    program, carrier,
                                )?,
                                None,
                            )
                        };
                        endpoint = Endpoint::Element {
                            carrier,
                            position: projections.len() + 1,
                        };
                        match segment {
                            facts::PlaceSegment::FixedIndex { index } if extent.is_some() => {
                                if *index >= extent? {
                                    return None;
                                }
                                projections.push(ScalarCoordinateProjection::Storage(
                                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                        u64::try_from(*index).ok()?,
                                    ),
                                ));
                            }
                            _ => {
                                let selector = match segment {
                                    facts::PlaceSegment::FixedIndex { index } => {
                                        checked_trees::CheckedEntryIndexSelector::Constant(
                                            u64::try_from(*index).ok()?,
                                        )
                                    }
                                    facts::PlaceSegment::Index { expression }
                                        if expression.is_valid() =>
                                    {
                                        checked_trees::CheckedEntryIndexSelector::Expression(
                                            *expression,
                                        )
                                    }
                                    _ => return None,
                                };
                                projections.push(ScalarCoordinateProjection::Element {
                                    selector,
                                    collection_path: coordinate[..source_step].to_vec(),
                                    source_step,
                                    collection_type: carrier,
                                    element_type: element,
                                    extent,
                                });
                            }
                        }
                        carrier = element;
                    }
                    _ => return None,
                }
            }
            if selected_case.is_valid() {
                return None;
            }
            let source_requirements =
                projection_source_requirements(coordinate, &projections, stored_crossing);
            Some(StructuralCoordinate {
                root,
                type_reference: source_type,
                result_type_reference: carrier,
                projections,
                source_requirements,
                endpoint,
                stored_crossing,
            })
        }

        fn projection_source_requirements(
            coordinate: &[facts::PlaceSegment],
            projections: &[ScalarCoordinateProjection],
            stored_crossing: bool,
        ) -> Vec<Vec<facts::PlaceSegment>> {
            let prefix = if stored_crossing {
                coordinate
            } else {
                let end = projections
                    .iter()
                    .find_map(|step| match step {
                        ScalarCoordinateProjection::Element { source_step, .. } => {
                            Some(*source_step)
                        }
                        _ => None,
                    })
                    .unwrap_or(coordinate.len());
                &coordinate[..end]
            };
            vec![prefix.to_vec()]
        }

        pub fn structural_scalar_field_at_coordinate(
            program: &dyn crate::CheckingInput,
            parameters: &[StateParameter],
            parameter: SymbolHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralScalarField> {
            let selected =
                structural_scalar_coordinate(program, parameters, parameter, coordinate)?;
            let Endpoint::Field {
                carrier,
                symbol,
                identity,
                position,
            } = selected.endpoint
            else {
                return None;
            };
            let path = selected
                .projections
                .into_iter()
                .take(position)
                .map(|step| {
                    if let ScalarCoordinateProjection::Storage(step) = step {
                        Some(step)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(StructuralScalarField {
                root: selected.root,
                receiver_value: ExpressionHandle::invalid(),
                statement_ordinal: 0,
                type_reference: selected.type_reference,
                carrier_type_reference: carrier,
                result_type_reference: selected.result_type_reference,
                path,
                field: symbol,
                field_identity: identity,
                primitive_type: selected.primitive_type,
                domain: selected.domain,
            })
        }

        pub fn structural_scalar_field_at_element_coordinate(
            program: &dyn crate::CheckingInput,
            state: SymbolHandle,
            parameter: SymbolHandle,
            coordinate: &[facts::PlaceSegment],
        ) -> Option<StructuralScalarField> {
            let state = program.state_by_symbol(state)?;
            let reference = program
                .state_parameter_by_symbol(state, parameter)
                .filter(|input| !input.is_const && !input.relevance.is_erased())
                .map(|input| input.type_reference)
                .or_else(|| {
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter_map(|statement| statement.as_local_data())
                        .find(|local| local.symbol == parameter)
                        .map(|local| local.type_reference)
                })?;
            let selected = structural_scalar_coordinate_in_type(
                program,
                facts::PlaceRoot::Symbol(parameter),
                reference,
                coordinate,
            )?;
            let Endpoint::Element { carrier, position } = selected.endpoint else {
                return None;
            };
            let path = selected
                .projections
                .into_iter()
                .take(position)
                .map(|step| {
                    if let ScalarCoordinateProjection::Storage(step) = step {
                        Some(step)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(StructuralScalarField {
                root: selected.root,
                receiver_value: ExpressionHandle::invalid(),
                statement_ordinal: 0,
                type_reference: selected.type_reference,
                carrier_type_reference: carrier,
                result_type_reference: selected.result_type_reference,
                path,
                field: SymbolHandle::invalid(),
                field_identity: Default::default(),
                primitive_type: selected.primitive_type,
                domain: selected.domain,
            })
        }
    }
    pub use coordinates::{
        ScalarCoordinateProjection, StructuralCoordinate, StructuralScalarCoordinate,
        structural_coordinate, structural_coordinate_in_type, structural_scalar_coordinate,
        structural_scalar_coordinate_in_type, structural_scalar_field_at_coordinate,
        structural_scalar_field_at_element_coordinate,
    };

    pub fn structural_scalar_field(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        bindings: &[(SymbolHandle, ExpressionHandle)],
        source: impl FnMut(ExpressionHandle) -> Option<StructuralSource>,
    ) -> Option<StructuralScalarField> {
        structural_field_observation(
            program,
            machine,
            state,
            expression,
            bindings,
            ScalarObservationAccess::Ordinary,
            None,
            source,
        )
    }

    pub fn structural_scalar_field_of_evaluated_receiver(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        receiver: ExpressionHandle,
        source: StructuralSource,
    ) -> Option<StructuralScalarField> {
        structural_field_observation(
            program,
            machine,
            state,
            expression,
            &[],
            ScalarObservationAccess::Ordinary,
            Some((receiver, source)),
            |_| None,
        )
    }

    pub fn structural_atomic_field(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        bindings: &[(SymbolHandle, ExpressionHandle)],
        evaluated_receiver: Option<(ExpressionHandle, StructuralSource)>,
        source: impl FnMut(ExpressionHandle) -> Option<StructuralSource>,
    ) -> Option<StructuralScalarField> {
        structural_field_observation(
            program,
            machine,
            state,
            expression,
            bindings,
            ScalarObservationAccess::Atomic,
            evaluated_receiver,
            source,
        )
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ScalarObservationAccess {
        Ordinary,
        Atomic,
    }

    fn structural_field_observation(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        expression: ExpressionHandle,
        bindings: &[(SymbolHandle, ExpressionHandle)],
        access: ScalarObservationAccess,
        evaluated_receiver: Option<(ExpressionHandle, StructuralSource)>,
        mut source: impl FnMut(ExpressionHandle) -> Option<StructuralSource>,
    ) -> Option<StructuralScalarField> {
        let mut steps = Vec::new();
        let mut receiver = expression;
        let mut visits = 0;
        let local = loop {
            visits += 1;
            if visits > expression_table(program).expression_count() {
                return None;
            }
            if !steps.is_empty()
                && let Some((evaluated, source)) = evaluated_receiver
                && receiver == evaluated
            {
                break source;
            }
            match expression_table(program).expression(receiver) {
                ExpressionNode::Name(name)
                    if name.head_symbol == name.symbol
                        && expression_table(program)
                            .name_path_members(name.members)
                            .len()
                            == 1
                        && bindings.iter().any(|(symbol, _)| *symbol == name.symbol) =>
                {
                    let mut selected = bindings.iter().filter(|(symbol, _)| *symbol == name.symbol);
                    let (_, actual) = selected.next()?;
                    if selected.next().is_some() || *actual == receiver {
                        return None;
                    }
                    receiver = *actual;
                    if steps.len() + 1 >= expression_table(program).expression_count() {
                        return None;
                    }
                }
                ExpressionNode::Member(member) => {
                    steps.push(Step::Member(member));
                    receiver = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    steps.push(Step::Element {
                        occurrence: receiver,
                        index: indexed.index,
                    });
                    receiver = indexed.collection;
                }
                _ => break source(receiver)?,
            }
        };
        if steps.is_empty() && access == ScalarObservationAccess::Atomic {
            let mut path = Vec::new();
            let carrier = readable_projection_carrier(program, local.type_reference, &mut path)?;
            let primitive_type = atomic_scalar_carrier(program, carrier)?;
            return Some(StructuralScalarField {
                root: local.root,
                receiver_value: local.receiver_value,
                statement_ordinal: local.statement_ordinal,
                type_reference: local.type_reference,
                carrier_type_reference: carrier,
                result_type_reference: carrier,
                path,
                field: SymbolHandle::invalid(),
                field_identity: CheckedMemberIdentity::default(),
                primitive_type,
                domain: program.arithmetic_domain_for_type_reference(carrier),
            });
        }
        let leaf = steps.first().copied()?;
        let fresh = local.receiver_value.is_valid();
        let scope = scope(program, machine, state);
        if !fresh
            && steps
                .iter()
                .any(|step| matches!(step, Step::Element { .. }))
            && !scope.is_some_and(|(machine, state)| {
                super::place_has_builtin_coordinates(program, machine, Some(state), expression)
            })
        {
            return None;
        }
        let value_coordinate = |occurrence, collection| {
            !fresh
                || scope.is_some_and(|(machine, state)| {
                    super::value_element_has_builtin_coordinate(
                        program,
                        machine,
                        Some(state),
                        occurrence,
                        collection,
                    )
                })
        };
        let mut carrier = local.type_reference;
        let mut path = Vec::new();
        for step in steps.iter().rev().take(steps.len() - 1) {
            carrier = readable_projection_carrier(program, carrier, &mut path)?;
            let member = match step {
                Step::Member(member) => member,
                Step::Element { occurrence, index } => {
                    let TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } = program.type_reference_table.type_reference(carrier)
                    else {
                        return None;
                    };
                    if !value_coordinate(*occurrence, carrier) {
                        return None;
                    }
                    let index = expression_table(program)
                        .constant_integer_value(*index)
                        .and_then(|index| u64::try_from(index).ok())?;
                    if usize::try_from(index).ok()? >= *length {
                        return None;
                    }
                    path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                    carrier = *element_type;
                    continue;
                }
            };
            let declaration_carrier = crate::peel_declared_constraints(program, carrier);
            let record = program.closed_data_definition(declaration_carrier)?;
            let field =
                selected_member_field(program, declaration_carrier, record, member, &mut path)?;
            path.push(CheckedUnitStructuralPathSegment::Field(
                CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
            ));
            carrier = field.type_reference;
        }
        carrier = readable_projection_carrier(program, carrier, &mut path)?;
        let leaf = match leaf {
            Step::Member(leaf) => leaf,
            Step::Element { occurrence, index } => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                if !value_coordinate(occurrence, carrier) {
                    return None;
                }
                let index = expression_table(program)
                    .constant_integer_value(index)
                    .and_then(|index| u64::try_from(index).ok())?;
                if usize::try_from(index).ok()? >= *length {
                    return None;
                }
                let primitive_type = match access {
                    ScalarObservationAccess::Ordinary => {
                        observable_scalar(realized_scalar_carrier(program, *element_type)?)?
                    }
                    ScalarObservationAccess::Atomic => {
                        atomic_scalar_carrier(program, *element_type)?
                    }
                };
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                return Some(StructuralScalarField {
                    root: local.root,
                    receiver_value: local.receiver_value,
                    statement_ordinal: local.statement_ordinal,
                    type_reference: local.type_reference,
                    carrier_type_reference: carrier,
                    result_type_reference: *element_type,
                    path,
                    field: SymbolHandle::invalid(),
                    field_identity: CheckedMemberIdentity::default(),
                    primitive_type,
                    domain: program.arithmetic_domain_for_type_reference(
                        program
                            .type_reference_table
                            .dereferenced_type_reference(*element_type),
                    ),
                });
            }
        };
        let declaration_carrier = crate::peel_declared_constraints(program, carrier);
        let record = program.closed_data_definition(declaration_carrier)?;
        let field = selected_member_field(program, declaration_carrier, record, leaf, &mut path)?;
        let primitive_type = match access {
            ScalarObservationAccess::Ordinary => {
                observable_scalar(realized_scalar_carrier(program, field.type_reference)?)?
            }
            ScalarObservationAccess::Atomic => {
                atomic_scalar_carrier(program, field.type_reference)?
            }
        };
        Some(StructuralScalarField {
            root: local.root,
            receiver_value: local.receiver_value,
            statement_ordinal: local.statement_ordinal,
            type_reference: local.type_reference,
            carrier_type_reference: carrier,
            result_type_reference: field.type_reference,
            path,
            field: field.symbol,
            field_identity: CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
            primitive_type,
            domain: program.arithmetic_domain_for_type_reference(
                program
                    .type_reference_table
                    .dereferenced_type_reference(field.type_reference),
            ),
        })
    }

    fn atomic_scalar_carrier(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        if !crate::is_atomic_type(program, reference) {
            return None;
        }
        observable_scalar(
            crate::value_custody::recasts::exact_scalar_representation_type(program, reference)?,
        )
    }

    fn observable_scalar(primitive_type: PrimitiveType) -> Option<PrimitiveType> {
        matches!(
            primitive_type,
            PrimitiveType::Bool
                | PrimitiveType::Addr
                | PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::F32
                | PrimitiveType::F64
                | PrimitiveType::Selector
        )
        .then_some(primitive_type)
    }

    fn readable_projection_carrier(
        program: &dyn crate::CheckingInput,
        mut carrier: TypeReferenceHandle,
        path: &mut Vec<CheckedUnitStructuralPathSegment>,
    ) -> Option<TypeReferenceHandle> {
        for _ in 0..program.type_reference_table.type_reference_count() {
            match program
                .type_reference_table
                .type_reference(crate::peel_declared_constraints(program, carrier))
            {
                TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::Shared | ReferenceAccess::Mutable,
                    ..
                } => {
                    if crate::stores_reference_shape(program, carrier, &[]) {
                        path.push(CheckedUnitStructuralPathSegment::Referent);
                    }
                    carrier = *referee;
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => return Some(carrier),
            }
        }
        None
    }

    fn selected_member_field<'program>(
        program: &'program dyn crate::CheckingInput,
        carrier: TypeReferenceHandle,
        record: &'program typed_trees::data::DataDefinition,
        member: &typed_trees::expression::TableMemberExpression,
        path: &mut Vec<CheckedUnitStructuralPathSegment>,
    ) -> Option<&'program typed_trees::data::DataField> {
        let field = super::projected_members::applied_data_member_field(program, carrier, member)?;
        if field.relevance.is_erased()
            || program.symbols.get(field.symbol).kind != SymbolKind::Field
        {
            return None;
        }
        let selected_case = member.case_variant.as_ref().map(|name| name.as_str());
        let mut payload_owners = program.data_members(record).iter().filter_map(|member| {
            let variant = member.as_variant()?;
            program
                .data_payload_fields(variant)
                .iter()
                .any(|candidate| candidate.symbol == field.symbol)
                .then_some(variant)
        });
        let payload_owner = payload_owners.next();
        if payload_owners.next().is_some() {
            return None;
        }
        let owner = if selected_case.is_some() || payload_owner.is_some() {
            let mut variants = program.data_members(record).iter().filter_map(|member| {
                let variant = member.as_variant()?;
                (selected_case.map_or_else(
                    || payload_owner.is_some_and(|owner| owner.symbol == variant.symbol),
                    |name| variant.name.as_str() == name,
                ))
                .then_some(variant)
            });
            let variant = variants.next()?;
            if variants.next().is_some()
                || !variant.symbol.is_valid()
                || program.symbols.get(variant.symbol).kind != SymbolKind::Variant
                || program.symbols.get(variant.symbol).parent != record.symbol
                || program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|candidate| candidate.symbol == field.symbol)
                    .count()
                    != 1
            {
                return None;
            }
            path.push(CheckedUnitStructuralPathSegment::Case(
                CheckedMemberIdentity::of_member(variant.identity, variant.name.clone()),
            ));
            variant.symbol
        } else {
            if program
            .data_members(record)
            .iter()
            .filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol))
            .count()
            != 1
        {
            return None;
        }
            record.symbol
        };
        (program.symbols.get(field.symbol).parent == owner).then_some(field)
    }

    #[derive(Clone, Copy)]
    pub(crate) enum Step<'a> {
        Member(&'a typed_trees::expression::TableMemberExpression),
        Element {
            occurrence: ExpressionHandle,
            index: ExpressionHandle,
        },
    }

    fn scope(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
        let mut machines = program
            .machines()
            .iter()
            .filter(|owner| owner.symbol == machine);
        let (Some(machine), None) = (machines.next(), machines.next()) else {
            return None;
        };
        let mut states = program
            .machine_states(machine)
            .iter()
            .filter(|owner| owner.symbol == state);
        let (Some(state), None) = (states.next(), states.next()) else {
            return None;
        };
        Some((machine, state))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StructuralSource {
        pub root: facts::PlaceRoot,
        pub receiver_value: ExpressionHandle,
        pub statement_ordinal: u32,
        pub type_reference: TypeReferenceHandle,
    }

    fn expression_at_statement(
        program: &dyn crate::CheckingInput,
        statement: &StatementNode,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        let mut pending =
            crate::machine_calls::calls::statement_value_expression_roots(program, statement);
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if handle == expression {
                return true;
            }
            if !program.expression_table.expression_is_valid(handle) || visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            crate::queries::expression_traversal::children(
                program,
                expression_table(program).expression(handle),
                |child| pending.push(child),
            );
        }
        false
    }

    pub fn saved_dispatch_subject(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<(usize, ExpressionHandle)> {
        let table = expression_table(program);
        if !table.expression_is_valid(expression)
            || !matches!(table.expression(expression), ExpressionNode::Call(_))
        {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let statement = statements.get(statement_index)?;
        if !expression_at_statement(program, statement, expression) {
            return None;
        }
        let same_subject = |subject| same_authored_call(program, subject, expression);
        if !same_subject(dispatch_case_subject(program, statement)?) {
            return None;
        }
        statements[..=statement_index]
            .iter()
            .enumerate()
            .find_map(|(ordinal, statement)| {
                let subject = dispatch_case_subject(program, statement)?;
                same_subject(subject).then_some((ordinal, subject))
            })
    }

    pub fn saved_dispatch_call(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<(usize, ExpressionHandle)> {
        let table = expression_table(program);
        if !table.expression_is_valid(expression)
            || !matches!(table.expression(expression), ExpressionNode::Call(_))
        {
            return None;
        }
        let statement = program
            .statement_table
            .statement_at(state, statement_index)?;
        let mut pending =
            crate::machine_calls::calls::statement_value_expression_roots(program, statement)
                .into_iter()
                .map(|node| (node, None))
                .collect::<Vec<_>>();
        let mut visited = Vec::new();
        let mut found = None;
        while let Some((node, inherited)) = pending.pop() {
            if node == ExpressionHandle::invalid() {
                if inherited
                    .is_some_and(|(_, counterpart)| counterpart != ExpressionHandle::invalid())
                {
                    return None;
                }
                continue;
            }
            if !table.expression_is_valid(node) {
                return None;
            }
            let canonical =
                inherited.or_else(|| saved_dispatch_subject(program, state, statement_index, node));
            if visited.contains(&(node, canonical)) {
                continue;
            }
            visited.push((node, canonical));
            if let Some((_, counterpart)) = canonical
                && (!table.expression_is_valid(counterpart)
                    || !table.expressions_structurally_equal(node, counterpart)
                    || (matches!(table.expression(node), ExpressionNode::Call(_))
                        && !same_authored_call(program, node, counterpart)))
            {
                return None;
            }
            if node == expression {
                let canonical = canonical?;
                if found.is_some_and(|prior| prior != canonical) {
                    return None;
                }
                found = Some(canonical);
                continue;
            }
            let mut children = Vec::new();
            crate::queries::expression_traversal::children(
                program,
                table.expression(node),
                |child| {
                    children.push(child);
                },
            );
            if let Some((ordinal, counterpart)) = canonical {
                let mut counterparts = Vec::new();
                crate::queries::expression_traversal::children(
                    program,
                    table.expression(counterpart),
                    |child| {
                        counterparts.push(child);
                    },
                );
                if children.len() != counterparts.len() {
                    return None;
                }
                pending.extend(
                    children
                        .into_iter()
                        .zip(counterparts)
                        .map(|(child, counterpart)| (child, Some((ordinal, counterpart)))),
                );
            } else {
                pending.extend(children.into_iter().map(|child| (child, None)));
            }
        }
        found
    }

    fn same_authored_call(
        program: &dyn crate::CheckingInput,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let table = expression_table(program);
        if !matches!(table.expression(left), ExpressionNode::Call(_))
            || !matches!(table.expression(right), ExpressionNode::Call(_))
            || !table.expressions_structurally_equal(left, right)
        {
            return false;
        }
        let mut occurrences = table.authored_selection_occurrences(left).peekable();
        occurrences.peek().is_some() && occurrences.eq(table.authored_selection_occurrences(right))
    }

    fn dispatch_case_subject(
        program: &dyn crate::CheckingInput,
        statement: &StatementNode,
    ) -> Option<ExpressionHandle> {
        use typed_trees::expression::BinaryOperator;
        use typed_trees::statement::TransitionGuardNode;
        let transition = statement.as_transition()?;
        let TransitionGuardNode::When(mut guard) = transition.guard else {
            return None;
        };
        let table = expression_table(program);
        for _ in 0..table.expression_count() {
            let binary = table.binary_node(guard)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            ) {
                return None;
            }
            if binary.operator == BinaryOperator::Equal {
                if matches!(table.expression(binary.left), ExpressionNode::Boolean(true)) {
                    guard = binary.right;
                    continue;
                }
                if matches!(
                    table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) {
                    guard = binary.left;
                    continue;
                }
            }
            if crate::exact_case_reference_owner(program, binary.right).is_some() {
                return Some(binary.left);
            }
            if binary.operator == BinaryOperator::Equal
                && crate::exact_case_reference_owner(program, binary.left).is_some()
            {
                return Some(binary.right);
            }
            return None;
        }
        None
    }

    pub fn structural_source(
        program: &dyn crate::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_ordinal: u32,
        expression: ExpressionHandle,
        receiver_expression: ExpressionHandle,
    ) -> Option<StructuralSource> {
        if !machine.is_valid()
            || !state.is_valid()
            || !expression_table(program).expression_is_valid(expression)
        {
            return None;
        }
        let mut machines = program
            .machines()
            .iter()
            .filter(|owner| owner.symbol == machine);
        let machine = machines.next()?;
        if machines.next().is_some() {
            return None;
        }
        let mut states = program
            .machine_states(machine)
            .iter()
            .filter(|owner| owner.symbol == state);
        let state = states.next()?;
        if states.next().is_some() {
            return None;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        if !expression_at_statement(
            program,
            statements.get(statement_ordinal as usize)?,
            expression,
        ) {
            return None;
        }
        let (root, receiver_value, ordinal, source_type) =
            if let ExpressionNode::SavedSubject(saved) =
                expression_table(program).expression(receiver_expression)
                && matches!(
                    expression_table(program).value_capture(saved.capture).owner,
                    typed_trees::expression::ValueCaptureOwner::Match
                )
            {
                let source = program.saved_value_source(saved)?;
                let reference =
                    crate::expression_result_type_reference(program, machine, state, source)?;
                (
                    facts::PlaceRoot::Expression(
                        expression_table(program)
                            .value_capture(saved.capture)
                            .value_anchor,
                    ),
                    source,
                    statement_ordinal as usize,
                    reference,
                )
            } else if let Some(source) = program.saved_argument_source(receiver_expression) {
                let reference =
                    crate::expression_result_type_reference(program, machine, state, source)?;
                (
                    facts::PlaceRoot::Expression(source),
                    ExpressionHandle::invalid(),
                    statement_ordinal as usize,
                    reference,
                )
            } else {
                match expression_table(program).expression(receiver_expression) {
                    ExpressionNode::Name(receiver) => {
                        if !receiver.symbol.is_valid()
                            || receiver.head_symbol != receiver.symbol
                            || expression_table(program)
                                .name_path_members(receiver.members)
                                .len()
                                != 1
                            || expression_table(program)
                                .name_path_member_symbols(receiver.member_symbols)
                                != [receiver.symbol]
                        {
                            return None;
                        }
                        let parameter = program.state_parameters(state).iter().find(|parameter| {
                            parameter.symbol == receiver.symbol
                                && !parameter.is_const
                                && !parameter.relevance.is_erased()
                        });
                        let local = program.state_local_declaration(state, receiver.symbol);
                        let (ordinal, source_type) = match (parameter, local) {
                            (Some(parameter), None) => (0, parameter.type_reference),
                            (None, Some((ordinal, local))) => {
                                if ordinal >= statement_ordinal as usize {
                                    return None;
                                }
                                (ordinal, local.type_reference)
                            }
                            _ => return None,
                        };
                        (
                            facts::PlaceRoot::Symbol(receiver.symbol),
                            ExpressionHandle::invalid(),
                            ordinal,
                            source_type,
                        )
                    }
                    _ => {
                        if let Some((ordinal, subject)) = saved_dispatch_subject(
                            program,
                            state,
                            statement_ordinal as usize,
                            receiver_expression,
                        ) {
                            let result_type = crate::expression_result_type_reference(
                                program, machine, state, subject,
                            )?;
                            (
                                facts::PlaceRoot::Expression(subject),
                                ExpressionHandle::invalid(),
                                ordinal,
                                result_type,
                            )
                        } else {
                            let result_type = crate::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                receiver_expression,
                            )?;
                            (
                                facts::PlaceRoot::Expression(receiver_expression),
                                receiver_expression,
                                statement_ordinal as usize,
                                result_type,
                            )
                        }
                    }
                }
            };
        let carrier = match program
            .type_reference_table
            .type_reference(crate::peel_declared_constraints(program, source_type))
        {
            TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::Shared | ReferenceAccess::Mutable,
                ..
            } if matches!(root, facts::PlaceRoot::Symbol(_)) => *referee,
            _ => source_type,
        };
        Some(StructuralSource {
            root,
            receiver_value,
            statement_ordinal: u32::try_from(ordinal).ok()?,
            type_reference: carrier,
        })
    }

    fn realized_scalar_carrier(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        for _ in 0..program.type_reference_table.type_reference_count() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::Shared | ReferenceAccess::Mutable,
                    ..
                } => reference = *referee,
                TypeReferenceNode::Reference { .. } => return None,
                _ => break,
            }
        }
        if crate::is_arithmetic_policy_only_integer(program, reference) {
            return program.primitive_type_reference(reference);
        }
        let primitive = program.primitive_type_reference(reference)?;
        let mut bounded = false;
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            let rows = program.type_reference_table.constraints(*constraints);
            if rows.is_empty() || rows.len() != constraints.count() as usize {
                return None;
            }
            for constraint in rows {
                match constraint {
                    TypeConstraintNode::Domain(domain) => {
                        if let Some((minimum, maximum)) =
                            crate::exact_declared_domain_carrier_interval(
                                program, primitive, domain,
                            )
                        {
                            if minimum > maximum {
                                return None;
                            }
                            bounded = true;
                        } else {
                            crate::scalar_domain_value_predicate(
                                program,
                                domain,
                                checked_trees::CheckedScalarExpression::Parameter {
                                    position: 0,
                                    primitive_type: primitive,
                                },
                            )?;
                        }
                    }
                    _ => return None,
                }
            }
            reference = *base_type;
        }
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::DynamicTrait { .. }
        ) {
            return (!bounded).then_some(PrimitiveType::Selector);
        }
        let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
        (!bounded || primitive.is_fixed_integer()).then_some(primitive)
    }
}
pub use structural_scalar_field::{
    ScalarCoordinateProjection, StructuralCoordinate, StructuralScalarCoordinate,
    StructuralScalarField, StructuralSource, saved_dispatch_call, saved_dispatch_subject,
    structural_atomic_field, structural_coordinate, structural_coordinate_in_type,
    structural_scalar_coordinate, structural_scalar_coordinate_in_type, structural_scalar_field,
    structural_scalar_field_at_coordinate, structural_scalar_field_at_element_coordinate,
    structural_scalar_field_in_parameters, structural_scalar_field_of_evaluated_receiver,
    structural_source,
};
pub(crate) mod member_paths {
    use super::{declared_place_type_raw, unwrapped_type_reference};
    use crate::frozen_program::expression_table;
    use language_core::is_self_receiver;
    use typed_trees::data::DataMember;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn declared_member_path_type(
        program: &dyn crate::CheckingInput,
        current_machine: &typed_trees::machine::Machine,
        current_state: Option<&typed_trees::state::State>,
        members: &[String],
    ) -> Option<TypeReferenceHandle> {
        match members {
            [name] => local_or_parameter_type(program, current_state, name),
            [root, field_name] if is_self_receiver(root) => {
                let data = super::machine_attached_data(program, current_machine)?;
                let field = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.name.as_str() == field_name => {
                            Some(field)
                        }
                        _ => None,
                    })?;
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            [root, field_name] => {
                let receiver_type = local_or_parameter_type(program, current_state, root)?;
                let data = data_definition_for_type(program, receiver_type)?;
                data_field_or_payload_type(program, data, field_name)
            }
            path if path.len() >= 3 => {
                resolve_nested_member_path(program, current_machine, current_state, path)
            }
            _ => None,
        }
    }

    pub(crate) fn collect_member_path(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) => Some(
                expression_table(program)
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect(),
            ),
            ExpressionNode::Member(member) => {
                let mut names = collect_member_path(program, member.receiver)?;
                names.push(member.member.as_str().to_owned());
                Some(names)
            }
            _ => None,
        }
    }

    pub(crate) fn first_unknown_nested_field(
        program: &dyn crate::CheckingInput,
        current_machine: &Machine,
        current_state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
    ) -> Option<(String, String)> {
        if let Some(member) = expression_table(program).member_node(expression) {
            if let Some(missing) =
                first_unknown_nested_field(program, current_machine, current_state, member.receiver)
            {
                return Some(missing);
            }
            let receiver_type =
                declared_place_type_raw(program, current_machine, current_state, member.receiver)?;
            let container = data_definition_for_type(program, receiver_type)?;
            let field_exists = match member.case_variant.as_ref() {
                Some(_) => super::projected_members::applied_data_member_field(
                    program,
                    receiver_type,
                    member,
                )
                .is_some(),
                None => data_field_or_payload(program, container, member.member.as_str()).is_some(),
            };
            return (!field_exists).then(|| {
                (
                    container.name.as_str().to_owned(),
                    member.member.as_str().to_owned(),
                )
            });
        }
        let path = collect_member_path(program, expression)?;
        if path.len() < 2 {
            return None;
        }
        let (root, rest) = path.split_first()?;
        if is_self_receiver(root) && path.len() < 3 {
            return None;
        }
        let mut current_data = if is_self_receiver(root) {
            let first_hop = rest.first()?;
            let is_machine_owned_data = program
                .machine_owned_data(current_machine)
                .iter()
                .any(|owned| owned.name.as_str() == *first_hop);
            if is_machine_owned_data {
                return None;
            }
            super::machine_attached_data(program, current_machine)?
        } else {
            let receiver_type = local_or_parameter_type(program, current_state, root)?;
            data_definition_for_type(program, receiver_type)?
        };
        for (position, hop) in rest.iter().enumerate() {
            let Some(hop_type) = data_field_or_payload_type(program, current_data, hop) else {
                return Some((current_data.name.as_str().to_owned(), hop.clone()));
            };
            if position + 1 == rest.len() {
                return None;
            }
            current_data = data_definition_for_type(program, hop_type)?;
        }
        None
    }

    fn resolve_nested_member_path(
        program: &dyn crate::CheckingInput,
        current_machine: &typed_trees::machine::Machine,
        current_state: Option<&typed_trees::state::State>,
        path: &[String],
    ) -> Option<TypeReferenceHandle> {
        let (root, rest) = path.split_first()?;
        let mut current_data = if is_self_receiver(root) {
            super::machine_attached_data(program, current_machine)?
        } else {
            let receiver_type = local_or_parameter_type(program, current_state, root)?;
            data_definition_for_type(program, receiver_type)?
        };
        let (last, intermediates) = rest.split_last()?;
        for hop in intermediates {
            let hop_type = data_field_or_payload_type(program, current_data, hop)?;
            current_data = data_definition_for_type(program, hop_type)?;
        }
        data_field_or_payload_type(program, current_data, last)
    }

    pub(crate) fn data_definition_for_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let unwrapped = unwrapped_type_reference(program, type_reference)?;
        if let Some(instance) = program.data_field_definition(unwrapped) {
            return Some(instance);
        }
        let symbol = match program.type_reference_table.type_reference(unwrapped) {
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            _ => return None,
        };
        super::unique_data_definition_by_symbol(program, symbol)
    }

    fn local_or_parameter_type(
        program: &dyn crate::CheckingInput,
        current_state: Option<&typed_trees::state::State>,
        name: &str,
    ) -> Option<TypeReferenceHandle> {
        let state = current_state?;
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data()
                && local.name.as_str() == name
            {
                return local
                    .type_reference
                    .is_valid()
                    .then_some(local.type_reference);
            }
        }
        for parameter in program.state_parameters(state) {
            if parameter.name.as_str() == name {
                return parameter
                    .type_reference
                    .is_valid()
                    .then_some(parameter.type_reference);
            }
        }
        None
    }

    pub(crate) fn data_field_or_payload_type(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        field_name: &str,
    ) -> Option<TypeReferenceHandle> {
        data_field_or_payload(program, data, field_name).map(|field| field.type_reference)
    }

    pub(crate) fn data_field_or_payload<'program>(
        program: &'program dyn crate::CheckingInput,
        data: &'program typed_trees::data::DataDefinition,
        field_name: &str,
    ) -> Option<&'program typed_trees::data::DataField> {
        for member in program.data_members(data) {
            if let Some(field) = member.as_field()
                && field.name.as_str() == field_name
                && field.type_reference.is_valid()
            {
                return Some(field);
            }
            if let Some(variant) = member.as_variant() {
                for field in program.data_payload_fields(variant) {
                    if field.name.as_str() == field_name && field.type_reference.is_valid() {
                        return Some(field);
                    }
                }
            }
        }
        None
    }
}
pub(crate) mod parameter_scoped {
    use super::unwrapped_type_reference;
    use crate::frozen_program::expression_table;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn parameter_scoped_type_reference(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        expression_type(program, owner, parameters, expression, 0)
    }

    pub fn parameter_source_type_application(
        program: &dyn crate::CheckingInput,
        callable: symbols::SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)> {
        if !callable.is_valid() {
            return None;
        }
        let place = super::canonical_place_from_expression(program, expression)?;
        if place.segments.is_empty() {
            let reference = crate::parameter_expression_result_type_reference(
                program, callable, parameters, expression,
            )?;
            return Some((reference, Vec::new()));
        }
        if !place.segments.is_empty() {
            parameter_scoped_type_reference(program, callable, parameters, expression)?;
        }
        let reference = match place.root {
            facts::PlaceRoot::Symbol(symbol) => {
                let mut candidates = parameters
                    .iter()
                    .filter(|parameter| parameter.symbol == symbol);
                let parameter = candidates.next()?;
                if candidates.next().is_some() {
                    return None;
                }
                parameter.type_reference
            }
            facts::PlaceRoot::Expression(source) => {
                crate::parameter_expression_result_type_reference(
                    program, callable, parameters, source,
                )?
            }
            _ => return None,
        };
        super::source_type_application_from_segments(program, reference, &place.segments)
    }

    fn expression_type(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        let node = expression_table(program).expression_at_depth(expression, depth)?;
        let child_type =
            |expression| expression_type(program, owner, parameters, expression, depth + 1);
        let reference = match node {
            ExpressionNode::SavedSubject(saved) => child_type(program.saved_value_source(saved)?)?,
            ExpressionNode::Name(path) => {
                let [_] = expression_table(program).name_path_members(path.members) else {
                    return None;
                };
                if !path.symbol.is_valid()
                    || path.head_symbol != path.symbol
                    || expression_table(program).name_path_member_symbols(path.member_symbols)
                        != [path.symbol]
                {
                    return None;
                }
                let mut candidates = parameters
                    .iter()
                    .filter(|parameter| parameter.symbol == path.symbol);
                let parameter = candidates.next()?;
                if candidates.next().is_some() {
                    return None;
                }
                parameter.type_reference
            }
            ExpressionNode::Borrow(borrow) => child_type(borrow.target)?,
            ExpressionNode::Member(member) => {
                let receiver = unwrapped_type_reference(program, child_type(member.receiver)?)?;
                super::projected_members::applied_data_member_field(program, receiver, member)?
                    .type_reference
            }
            ExpressionNode::Indexed(indexed) => {
                if expression_table(program).indexed_on_range(indexed) {
                    return None;
                }
                let collection_type = child_type(indexed.collection)?;
                let operands = [Some(collection_type), child_type(indexed.index)];
                if !typed_trees::operator::builtin_index_read_is_spelled(
                    program, owner, expression, &operands,
                ) {
                    return None;
                }
                super::collection_element_type_reference(program, collection_type)?
            }
            ExpressionNode::Call(_) => {
                crate::machine_calls::calls::call_result_type(program, expression)?
            }
            ExpressionNode::Cast(cast) => cast.target_type,
            ExpressionNode::ZeroValue(reference) => *reference,
            _ => return None,
        };
        program
            .type_reference_table
            .contains_type_reference(reference)
            .then_some(reference)
    }

    pub(crate) fn has_retained_builtin_index_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
        use typed_trees::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};

        let mut occurrences = expression_table(program).authored_selection_occurrences(expression);
        let Some(occurrence) = occurrences.next() else {
            return false;
        };
        occurrences.next().is_none()
            && program
                .authored_declaration_selections()
                .get(occurrence)
                .is_some_and(|selection| {
                    selection.kind() == AuthoredDeclarationSelectionKind::Operator
                        && selection.target()
                            == AuthoredDeclarationSelectionTarget::Intrinsic(
                                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                            )
                })
    }
}
pub(crate) use parameter_scoped::has_retained_builtin_index_meaning;
pub(crate) use parameter_scoped::parameter_scoped_type_reference;
pub use parameter_scoped::parameter_source_type_application;
pub(crate) mod projected_members {
    use super::declared_place_type_raw;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;

    pub fn applied_data_member_field<'program>(
        program: &'program dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<&'program typed_trees::data::DataField> {
        applied_data_field(
            program,
            reference,
            member.member_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|variant| variant.as_str()),
        )
    }

    pub(crate) fn applied_data_field<'program>(
        program: &'program dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        retained: SymbolHandle,
        member_name: &str,
        case_variant: Option<&str>,
    ) -> Option<&'program typed_trees::data::DataField> {
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        let closed = program.closed_data_definition(reference);
        let data = match closed {
            Some(data) => data,
            None => program
                .data_application_definition(reference)
                .or_else(|| super::data_definition_for_type(program, reference))?,
        };
        let selected = if closed.is_some() && retained.is_valid() {
            program
                .closed_data_constructor_field(reference, data.symbol, retained)?
                .symbol
        } else {
            retained
        };
        exact_data_member_field(program, data, selected, member_name, case_variant)
    }

    pub(crate) fn contains_case_projection(
        program: &dyn crate::CheckingInput,
        mut expression: ExpressionHandle,
    ) -> bool {
        loop {
            expression = match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) if member.case_variant.is_some() => return true,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => return false,
            };
        }
    }

    pub(crate) fn declared_case_projection_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if let Some(borrow) = program.expression_table.borrow_expression(expression) {
            return declared_place_type_raw(program, machine, state, borrow.target);
        }
        declared_case_projection_field(program, machine, state, expression)
            .map(|field| field.type_reference)
    }

    pub fn declared_case_projection_field<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<&'program typed_trees::data::DataField> {
        let member = match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => member,
            _ => return None,
        };
        let receiver_symbol = match program.expression_table.expression(member.receiver) {
            ExpressionNode::Name(name)
                if name.symbol == machine.symbol
                    && matches!(
                        program.expression_table.name_path_members(name.members),
                        [only] if only.is_self_receiver()
                    ) =>
            {
                machine.attached_data_symbol
            }
            _ => {
                let receiver_type =
                    declared_place_type_raw(program, machine, state, member.receiver)?;
                return applied_data_member_field(program, receiver_type, member);
            }
        };
        let data = program.data_definition_by_symbol(receiver_symbol)?;
        let field_symbol = if member.case_variant.is_some() {
            member.member_symbol
        } else {
            SymbolHandle::invalid()
        };
        exact_data_member_field(
            program,
            data,
            field_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|variant| variant.as_str()),
        )
    }

    pub fn exact_data_member_field<'program>(
        program: &'program dyn crate::CheckingInput,
        data: &'program typed_trees::data::DataDefinition,
        member_symbol: SymbolHandle,
        member_name: &str,
        case_variant: Option<&str>,
    ) -> Option<&'program typed_trees::data::DataField> {
        if let Some(case_variant) = case_variant {
            let mut matches = program.data_members(data).iter().filter_map(|member| {
                let variant = member.as_variant()?;
                (variant.name.as_str() == case_variant).then_some(variant)
            });
            let variant = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            let mut fields = program.data_payload_fields(variant).iter().filter(|field| {
                field.name.as_str() == member_name
                    && (!member_symbol.is_valid() || field.symbol == member_symbol)
                    && field.symbol.is_valid()
                    && field.type_reference.is_valid()
            });
            let field = fields.next()?;
            return fields.next().is_none().then_some(field);
        }

        let mut selected = None;
        for member in program.data_members(data) {
            let fields = match member {
                typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                typed_trees::data::DataMember::Variant(variant) => {
                    program.data_payload_fields(variant)
                }
            };
            for field in fields {
                if field.name.as_str() != member_name
                    || (member_symbol.is_valid() && field.symbol != member_symbol)
                    || !field.symbol.is_valid()
                    || !field.type_reference.is_valid()
                {
                    continue;
                }
                if selected.is_some() {
                    return None;
                }
                selected = Some(field);
            }
        }
        selected
    }
}
pub(crate) mod result_shape {
    use super::{
        data_definition_for_type, data_field_or_payload_type, declared_place_type_raw,
        unwrapped_type_reference,
    };
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, Copy)]
    pub(crate) enum Projection<'expression> {
        Field(&'expression str),
        Element,
    }

    pub(crate) fn expression_result_is_reference(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let mut pending = vec![(expression, Vec::new())];
        let mut saw_result = false;
        let mut saw_reference = false;
        while let Some((expression, mut projections)) = pending.pop() {
            if projections.is_empty()
                && super::collection_length_receiver(program, machine, Some(state), expression)
                    .is_some()
            {
                saw_result = true;
                continue;
            }
            if !matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Borrow(_) | ExpressionNode::Cast(_)
            ) && let Some(reference) =
                declared_place_type_raw(program, machine, Some(state), expression)
            {
                saw_reference |= projected_type_is_reference(program, reference, &projections)?;
                saw_result = true;
                continue;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => {
                    projections.push(Projection::Field(member.member.as_str()));
                    pending.push((member.receiver, projections));
                }
                ExpressionNode::Indexed(indexed) => {
                    if program.expression_table.indexed_on_range(indexed) {
                        return None;
                    }
                    projections.push(Projection::Element);
                    pending.push((indexed.collection, projections));
                }
                ExpressionNode::StructLiteral(literal) if !projections.is_empty() => {
                    let Projection::Field(field) = projections.pop()? else {
                        return None;
                    };
                    let definition = if literal.type_symbol.is_valid() {
                        program.unique_data_definition_by_symbol(literal.type_symbol)
                    } else {
                        let mut definitions = program
                            .data_definitions()
                            .iter()
                            .filter(|definition| definition.name == literal.type_name);
                        let definition = definitions.next()?;
                        if definitions.next().is_some() {
                            return None;
                        }
                        Some(definition)
                    }?;
                    let reference = crate::value_custody::struct_literals::construction_field_type(
                        program,
                        definition,
                        literal.case_name.as_ref().map(|case| case.as_str()),
                        field,
                    )?;
                    saw_reference |= projected_type_is_reference(program, reference, &projections)?;
                    saw_result = true;
                }
                ExpressionNode::ArrayLiteral(elements) if !projections.is_empty() => {
                    let Projection::Element = projections.pop()? else {
                        return None;
                    };
                    let elements = program.expression_table.expression_handles(*elements);
                    if elements.is_empty() {
                        return None;
                    }
                    pending.extend(
                        elements
                            .iter()
                            .map(|element| (*element, projections.clone())),
                    );
                }
                ExpressionNode::Borrow(borrow) if !projections.is_empty() => {
                    pending.push((borrow.target, projections));
                }
                ExpressionNode::Borrow(_) => {
                    saw_reference = true;
                    saw_result = true;
                }
                ExpressionNode::Cast(cast) => {
                    saw_reference |= (cast.form.is_recast() && projections.is_empty())
                        || projected_type_is_reference(program, cast.target_type, &projections)?;
                    saw_result = true;
                }
                ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::String(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::Binary(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::ArrayLiteral(_)
                    if projections.is_empty() =>
                {
                    saw_result = true;
                }
                _ => return None,
            }
        }
        saw_result.then_some(saw_reference)
    }

    fn projected_type_is_reference(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        projections: &[Projection<'_>],
    ) -> Option<bool> {
        for projection in projections.iter().rev() {
            reference = unwrapped_type_reference(program, reference)?;
            reference = match projection {
                Projection::Field(field) => data_field_or_payload_type(
                    program,
                    data_definition_for_type(program, reference)?,
                    field,
                )?,
                Projection::Element => match program.type_reference_table.type_reference(reference)
                {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => return None,
                },
            };
        }
        reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        reference.is_valid().then(|| {
            matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Reference { .. }
            )
        })
    }
}
pub use attached_storage::{
    AttachedStorageRoot, attached_storage_root, canonical_storage_label, declaration_storage_label,
    exact_attached_storage_field, expression_names_attached_storage,
};
pub use builtin_coordinates::{
    assignment_target_indexes, assignment_target_selectors, collection_length_receiver,
    declared_collection_extent, ensured_range_call_endpoints, fixed_array_type_extent,
    fixed_array_type_length, has_builtin_subslice_meaning, indexed_operation_has_builtin_meaning,
    mutable_byte_slice_parameter_element, range_operator_establishes_endpoint_extent,
    subslice_expression, whole_collection_view_source,
};
pub use builtin_coordinates::{
    place_has_builtin_coordinates, value_element_has_builtin_coordinate,
    value_has_builtin_coordinates, value_has_builtin_coordinates_with_types,
};
pub use exact_self_field::exact_attached_field;
pub use exact_self_field::exact_self_field;
pub(crate) use exact_self_field::single_member_receiver_field;
use member_paths::{collect_member_path, data_field_or_payload, data_field_or_payload_type};
pub(crate) use member_paths::{
    data_definition_for_type, declared_member_path_type, first_unknown_nested_field,
};
pub use projected_members::applied_data_member_field;
pub use projected_members::declared_case_projection_field;
pub use projected_members::exact_data_member_field;
pub(crate) use result_shape::expression_result_is_reference;

pub(crate) fn declared_indexed_projection_type(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    declared_indexed_projection_type_raw(program, current_machine, current_state, target)
        .and_then(|element_type| unwrapped_type_reference(program, element_type))
}

pub(crate) fn declared_indexed_projection_type_raw(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&typed_trees::state::State>,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = target;
    let mut projected_fields = Vec::new();
    loop {
        match expression_table(program).expression(handle) {
            ExpressionNode::Borrow(inner) => handle = inner.target,
            ExpressionNode::Member(member) => {
                projected_fields.push(member.member.as_str());
                handle = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if expression_table(program).indexed_on_range(indexed) {
                    return None;
                }
                let collection_type = declared_place_type_raw(
                    program,
                    current_machine,
                    current_state,
                    indexed.collection,
                )?;
                let mut leaf = collection_element_type_reference(program, collection_type)?;
                for field_name in projected_fields.iter().rev() {
                    let data = data_definition_for_type(program, leaf)?;
                    leaf = data_field_or_payload_type(program, data, field_name)?;
                }
                return Some(leaf);
            }
            _ => return None,
        }
    }
}

pub fn unwrapped_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    program.unwrapped_type_reference(type_reference)
}

pub fn peel_declared_constraints(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .unconstrained_type_reference(type_reference)
}

pub fn collection_element_type(
    program: &dyn crate::CheckingInput,
    collection: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(collection) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

pub fn declared_reference_access(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<language_core::ReferenceAccess> {
    crate::value_custody::reference_shells::reference_shell(program, type_reference)
        .map(|shell| shell.access)
}

pub(crate) mod declared_carriers {
    use super::peel_declared_constraints;
    use crate::value_custody::type_references::substituted_identity;
    use symbols::SymbolHandle;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone, Copy)]
    pub(crate) enum Substitutions<'bindings> {
        Exact,
        DeclarationEquations(&'bindings [(SymbolHandle, TypeReferenceHandle)]),
        Application(&'bindings [(SymbolHandle, TypeReferenceHandle)]),
    }

    pub(crate) fn under_equations(
        program: &dyn crate::CheckingInput,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        equations: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        let substitutions = if equations.is_empty() {
            Substitutions::Exact
        } else {
            Substitutions::DeclarationEquations(equations)
        };
        compare(program, left, substitutions, right, substitutions)
    }

    pub(crate) fn under_application(
        program: &dyn crate::CheckingInput,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        bindings: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        compare(
            program,
            left,
            Substitutions::Exact,
            right,
            Substitutions::Application(bindings),
        )
    }

    fn normalize_root<'bindings>(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        substitutions: Substitutions<'bindings>,
    ) -> Option<(TypeReferenceHandle, Substitutions<'bindings>)> {
        if !reference.is_valid() {
            return None;
        }
        match substitutions {
            Substitutions::DeclarationEquations(equations) => {
                substituted_declared_carrier(program, reference, equations)
                    .map(|reference| (reference, substitutions))
            }
            Substitutions::Exact => {
                Some((peel_declared_constraints(program, reference), substitutions))
            }
            Substitutions::Application(bindings) => {
                if bindings.is_empty() {
                    return normalize_root(program, reference, Substitutions::Exact);
                }
                let reference = peel_declared_constraints(program, reference);
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                    && let Some((_, actual)) = bindings.iter().find(|(formal, _)| formal == symbol)
                {
                    normalize_root(program, *actual, Substitutions::Exact)
                } else {
                    Some((reference, substitutions))
                }
            }
        }
    }

    fn compare(
        program: &dyn crate::CheckingInput,
        left: TypeReferenceHandle,
        left_substitutions: Substitutions<'_>,
        right: TypeReferenceHandle,
        right_substitutions: Substitutions<'_>,
    ) -> bool {
        let (Some((left, left_substitutions)), Some((right, right_substitutions))) = (
            normalize_root(program, left, left_substitutions),
            normalize_root(program, right, right_substitutions),
        ) else {
            return false;
        };
        let compare_children = |left, right| {
            compare(
                program,
                left,
                left_substitutions,
                right,
                right_substitutions,
            )
        };
        match (
            program.type_reference_table.type_reference(left),
            program.type_reference_table.type_reference(right),
        ) {
            (
                TypeReferenceNode::Reference {
                    referee: left,
                    access: left_access,
                    ..
                },
                TypeReferenceNode::Reference {
                    referee: right,
                    access: right_access,
                    ..
                },
            ) => left_access == right_access && compare_children(*left, *right),
            (
                TypeReferenceNode::Slice { element_type: left },
                TypeReferenceNode::Slice {
                    element_type: right,
                },
            ) => compare_children(*left, *right),
            (
                TypeReferenceNode::FixedArray {
                    element_type: left,
                    length: left_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right,
                    length: right_length,
                },
            ) => {
                let extent = |length, substitutions| match substitutions {
                    Substitutions::Exact => program.fixed_array_length_identity(length, &[], &[]),
                    Substitutions::DeclarationEquations(bindings) => {
                        program.fixed_array_length_identity(length, &[], bindings)
                    }
                    Substitutions::Application(bindings) => program
                        .fixed_array_length_identity_with_simultaneous_substitutions(
                            length, bindings,
                        ),
                };
                extent(left_length, left_substitutions)
                    .zip(extent(right_length, right_substitutions))
                    .is_some_and(|(left, right)| left == right)
                    && compare_children(*left, *right)
            }
            (
                TypeReferenceNode::Generic {
                    base_symbol: left_symbol,
                    arguments: left_arguments,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_symbol,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                let left = program
                    .type_reference_table
                    .type_reference_handles(*left_arguments);
                let right = program
                    .type_reference_table
                    .type_reference_handles(*right_arguments);
                // Reused declarations can retain different symbol handles for
                // the same nominal owner. Its canonical identity retains source
                // ownership and instantiated schema identity; arguments still
                // compare recursively in their original substitution scopes.
                let same_owner = left_symbol == right_symbol
                    || program
                        .normalized_nominal_type_identity(*left_symbol)
                        .zip(program.normalized_nominal_type_identity(*right_symbol))
                        .is_some_and(|(left, right)| left.is_valid() && left == right);
                same_owner
                    && left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| compare_children(*left, *right))
            }
            _ => {
                let identity = |reference, substitutions| match substitutions {
                    Substitutions::Exact => program.normalized_type_identity(reference),
                    Substitutions::DeclarationEquations(bindings) => {
                        substituted_identity(program, reference, bindings)
                    }
                    Substitutions::Application(bindings) => program
                        .type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                            substitutions: bindings,
                            ..TypeIdentityRequest::ordinary(reference)
                        }),
                };
                let left = identity(left, left_substitutions);
                let right = identity(right, right_substitutions);
                left.is_valid() && left == right
            }
        }
    }

    pub(crate) fn substituted_declared_carrier(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<TypeReferenceHandle> {
        let mut visited = Vec::new();
        loop {
            if !reference.is_valid() {
                return None;
            }
            reference = peel_declared_constraints(program, reference);
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some(reference);
            };
            let Some((_, replacement)) = substitutions.iter().find(|(binder, _)| binder == symbol)
            else {
                return Some(reference);
            };
            if visited.contains(symbol) {
                return None;
            }
            visited.push(*symbol);
            reference = *replacement;
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use language_core::ReferenceAccess;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{TypedTrees, name::Identifier, types::FixedArrayLength};

        fn fixture() -> (TypedTrees, [SymbolHandle; 2], [TypeReferenceHandle; 3]) {
            let mut symbols = SymbolTableBuilder::default();
            let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
            let binders = symbols.insert_children(
                owner,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("Value")),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("Alias")),
                ],
            );
            let parameters = [binders.handle_at(0), binders.handle_at(1)];
            let cell = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Cell"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let references = [parameters[0], parameters[1], cell].map(|symbol| {
                let name = Identifier::from(program.symbols.name(symbol));
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named { symbol, name })
            });
            (program, parameters, references)
        }

        #[test]
        fn equations_are_transitive_but_application_actuals_keep_caller_scope() {
            let (program, [value, alias], [value_type, alias_type, cell]) = fixture();
            let equations = [(value, alias_type), (alias, cell)];
            assert!(under_equations(&program, cell, value_type, &equations));
            assert!(under_application(
                &program, alias_type, value_type, &equations
            ));
            assert!(!under_application(&program, cell, value_type, &equations));
            assert!(under_application(&program, cell, alias_type, &equations));
            let cycle = [(value, alias_type), (alias, value_type)];
            assert!(!under_equations(&program, cell, value_type, &cycle));
        }

        #[test]
        fn declaration_equations_do_not_erase_reference_access_or_array_extent() {
            let (mut program, [value, _], [value_type, _, cell]) = fixture();
            let equations = [(value, cell)];
            let reference = |program: &mut TypedTrees, referee, access| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: None,
                    })
            };
            let mutable = reference(&mut program, value_type, ReferenceAccess::Mutable);
            let shared = reference(&mut program, cell, ReferenceAccess::Shared);
            let write_only = reference(&mut program, cell, ReferenceAccess::WriteOnly);
            assert!(!under_equations(&program, shared, mutable, &equations));
            assert!(!under_equations(&program, write_only, mutable, &equations));
            let array = |program: &mut TypedTrees, element_type, extent| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(extent),
                    })
            };
            let open = array(&mut program, value_type, 1);
            let same = array(&mut program, cell, 1);
            let other = array(&mut program, cell, 2);
            assert!(under_equations(&program, same, open, &equations));
            assert!(!under_equations(&program, other, open, &equations));
        }
    }
}
pub(crate) use declared_carriers::substituted_declared_carrier;

pub fn declared_carriers_match(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    declared_carriers_match_under_application(program, left, right, &[])
}

pub(crate) fn declared_carriers_match_with_substitutions(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    declared_carriers::under_equations(program, left, right, substitutions)
}

pub(crate) fn declared_carriers_match_under_application(
    program: &dyn crate::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    declared_carriers::under_application(program, left, right, substitutions)
}

pub fn zeroed_structural_parts(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    let mut pending = vec![type_reference];
    while let Some(current) = pending.pop() {
        if !current.is_valid() {
            return false;
        }
        let reference = peel_declared_constraints(program, current);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        ) {
            return false;
        }
        if program.primitive_type_reference(reference).is_some() {
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                if *length != 0 {
                    pending.push(*element_type);
                }
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let Some(data) = program.closed_data_definition(reference) else {
                    return false;
                };
                let members = program.data_members(data);
                let mut variants = members.iter().filter_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant) => Some(variant),
                    _ => None,
                });
                let Some(first_case) = variants.next() else {
                    for member in members {
                        let Some(field) = member.as_field() else {
                            return false;
                        };
                        if field.relevance.is_erased() {
                            return false;
                        }
                        pending.push(field.type_reference);
                    }
                    continue;
                };
                for member in members {
                    if let Some(field) = member.as_field() {
                        if field.relevance.is_erased() {
                            return false;
                        }
                        pending.push(field.type_reference);
                    }
                }
                for field in program.data_payload_fields(first_case) {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    pending.push(field.type_reference);
                }
            }
            _ => return false,
        }
    }
    true
}

pub(crate) mod declared_symbols {
    use ::symbols::SymbolHandle;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::data::DataMember;
    use typed_trees::statement::StatementNode;

    pub fn machine_symbol_from_type_reference_handle(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> SymbolHandle {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                machine_symbol_from_type_reference_handle(program, *referee)
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                machine_symbol_from_type_reference_handle(program, *base_type)
            }
            typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            | typed_trees::types::TypeReferenceNode::Named {
                symbol: base_symbol,
                ..
            } => *base_symbol,
            typed_trees::types::TypeReferenceNode::DynamicTrait {
                symbol: trait_symbol,
                ..
            } => *trait_symbol,
            typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
        }
    }

    pub fn expression_root_symbol(
        expression: ExpressionHandle,
        expressions: &typed_trees::expression::ExpressionTable,
        machine_symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        match expressions.expression(expression) {
            ExpressionNode::Indexed(indexed) => {
                expression_root_symbol(indexed.collection, expressions, machine_symbol)
            }
            ExpressionNode::Borrow(inner) => {
                expression_root_symbol(inner.target, expressions, machine_symbol)
            }
            ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && path.symbol.is_valid()
                        && path.symbol == machine_symbol =>
                {
                    member
                        .member_symbol
                        .is_valid()
                        .then_some(member.member_symbol)
                }
                _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
            },
            ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
            _ => None,
        }
    }

    pub fn first_valid_name_path_symbol(
        path: &typed_trees::expression::TableNamePath,
        expressions: &typed_trees::expression::ExpressionTable,
    ) -> Option<SymbolHandle> {
        expressions
            .name_path_member_symbols(path.member_symbols)
            .first()
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
            .or_else(|| path.symbol.is_valid().then_some(path.symbol))
    }

    pub struct RuntimeFieldSubject {
        pub receiver: SymbolHandle,
        pub field: SymbolHandle,
    }

    pub fn runtime_field_subject(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        term: &typed_trees::name::Identifier,
        member: &typed_trees::name::Identifier,
    ) -> Option<RuntimeFieldSubject> {
        let receiver =
            nearest_scoped_local(program, state, usize::MAX, std::slice::from_ref(term))?;
        let carrier = scoped_binding_type(program, state, receiver)?;
        let data = declared_data_type(program, carrier)?;
        let field = program.data_members(data).iter().find_map(|row| {
            let DataMember::Field(field) = row else {
                return None;
            };
            (field.name.as_str() == member.as_str()).then_some(field.symbol)
        })?;
        Some(RuntimeFieldSubject { receiver, field })
    }

    fn scoped_binding_type(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            return Some(parameter.type_reference);
        }
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .rev()
            .find_map(|statement| {
                let local = statement.as_local_data()?;
                (local.symbol == symbol).then_some(local.type_reference)
            })
    }

    pub fn nearest_scoped_local(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        scope_limit: usize,
        path: &[typed_trees::name::Identifier],
    ) -> Option<SymbolHandle> {
        let [name] = path else {
            return None;
        };
        let mut resolved = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name.as_str() == name.as_str())
            .map(|parameter| parameter.symbol);
        for statement in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(scope_limit)
        {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if local.name.as_str() == name.as_str() {
                resolved = Some(local.symbol);
            }
        }
        resolved
    }

    pub fn structural_member_identity(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<checked_trees::CheckedMemberIdentity> {
        program
            .any_field_owner(symbol)
            .map(|(_, field)| field.member_identity())
            .or_else(|| {
                program
                    .variant_owner(symbol)
                    .map(|(_, variant)| variant.member_identity())
            })
    }

    pub fn data_member_symbol_by_name(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        member_name: &str,
    ) -> Option<SymbolHandle> {
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) if field.name.as_str() == member_name => {
                    return Some(field.symbol);
                }
                DataMember::Variant(variant) if variant.name.as_str() == member_name => {
                    return Some(variant.symbol);
                }
                DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.name.as_str() == member_name)
                    {
                        return Some(field.symbol);
                    }
                }
                DataMember::Field(_) => {}
            }
        }
        None
    }

    pub fn declared_data_type(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => declared_data_type(program, *referee),
            typed_trees::types::TypeReferenceNode::Named { symbol, name }
                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
            {
                program
                    .data_definition_by_symbol(*symbol)
                    .filter(|definition| definition.name == *name)
            }
            _ => None,
        }
    }

    pub fn state_scoped_name_symbol(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        name: &str,
        statement_index: usize,
    ) -> Option<SymbolHandle> {
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name.as_str() == name)
        {
            return Some(parameter.symbol);
        }

        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(statement_index)
            .find_map(|statement| match statement {
                StatementNode::LocalData(local_data) if local_data.name.as_str() == name => {
                    Some(local_data.symbol)
                }
                _ => None,
            })
    }
}
pub use declared_symbols::{
    RuntimeFieldSubject, data_member_symbol_by_name, declared_data_type, expression_root_symbol,
    first_valid_name_path_symbol, machine_symbol_from_type_reference_handle, nearest_scoped_local,
    runtime_field_subject, state_scoped_name_symbol, structural_member_identity,
};
pub(crate) mod canonical_projection {
    pub(crate) use syntax_queries::ProjectionSyntaxQueries;
    use syntax_queries::{literal_arm_bindings, states_naming};

    use super::contextual_projection::contextual_canonical_place_from_expression;
    use super::member_resolution::effective_member_symbol;
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use typed_trees::types::TypeReferenceNode;

    use super::declared_symbols::first_valid_name_path_symbol;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::{SymbolHandle, SymbolKind};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct CanonicalPlace {
        pub root: facts::PlaceRoot,
        pub segments: Vec<facts::PlaceSegment>,
    }

    impl CanonicalPlace {
        pub fn compare_identity(&self, other: &Self) -> std::cmp::Ordering {
            use facts::{PlaceRoot, PlaceSegment};
            let root_key = |root| match root {
                PlaceRoot::Unknown => (0, 0, 0, 0, 0),
                PlaceRoot::InvocationOutput(_) => (5, 0, 0, 0, 0),
                PlaceRoot::Symbol(handle) => (1, handle.arena_index(), handle.generation(), 0, 0),
                PlaceRoot::Expression(handle) => {
                    (2, handle.arena_index(), handle.generation(), 0, 0)
                }
                PlaceRoot::TypeReference(handle) => {
                    (3, handle.arena_index(), handle.generation(), 0, 0)
                }
                PlaceRoot::InvocationReferent {
                    parameter,
                    type_reference,
                } => (
                    4,
                    parameter.arena_index(),
                    parameter.generation(),
                    type_reference.arena_index(),
                    type_reference.generation(),
                ),
            };
            let segment_key = |segment| match segment {
                PlaceSegment::Field { symbol } => {
                    (0, symbol.arena_index(), symbol.generation(), 0, 0)
                }
                PlaceSegment::Case { variant } => {
                    (1, variant.arena_index(), variant.generation(), 0, 0)
                }
                PlaceSegment::FixedIndex { index } => (2, 0, 0, index, 0),
                PlaceSegment::FixedRange { start, end } => (3, 0, 0, start, end),
                PlaceSegment::OpaqueRange => (4, 0, 0, 0, 0),
                PlaceSegment::Index { expression } => {
                    (5, expression.arena_index(), expression.generation(), 0, 0)
                }
            };
            root_key(self.root)
                .cmp(&root_key(other.root))
                .then_with(|| match (self.root, other.root) {
                    (PlaceRoot::InvocationOutput(left), PlaceRoot::InvocationOutput(right)) => {
                        left.compare_identity(right)
                    }
                    _ => std::cmp::Ordering::Equal,
                })
                .then_with(|| {
                    self.segments
                        .iter()
                        .copied()
                        .map(segment_key)
                        .cmp(other.segments.iter().copied().map(segment_key))
                })
        }

        pub fn extend_segments(&mut self, segments: &[facts::PlaceSegment]) {
            self.segments.extend(segments.iter().copied());
        }

        pub fn has_only_positional_segments(&self) -> bool {
            !self.segments.is_empty()
                && self.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                    )
                })
        }

        pub fn label(&self, program: &dyn CheckingInput) -> String {
            facts::canonical_place_label_from_parts(program, self.root, &self.segments)
        }
    }

    pub fn canonical_collection_element_place(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        collection: ExpressionHandle,
        element_index: usize,
    ) -> Option<CanonicalPlace> {
        if !super::place_has_builtin_coordinates(program, machine, state, collection) {
            return None;
        }
        if super::subslice_expression(program, collection).is_none() {
            let collection_type =
                super::declared_place_type_raw(program, machine, state, collection)?;
            if !super::collection_operations::type_reference_is_collection(program, collection_type)
            {
                return None;
            }
        }
        let mut place = canonical_place_from_expression(program, collection)?;
        place.segments.push(facts::PlaceSegment::FixedIndex {
            index: element_index,
        });
        let segments = place
            .segments
            .into_iter()
            .map(|segment| match segment {
                facts::PlaceSegment::FixedRange { start, end } => Some(WindowedSegment::Window {
                    start,
                    end: Some(end),
                }),
                facts::PlaceSegment::Index { expression } => {
                    let range = expression_table(program).range_node(expression)?;
                    let ordinal = |bound| {
                        program
                            .closed_integer_expression_value_in(bound, machine.symbol)?
                            .to_u64()
                            .and_then(|value| usize::try_from(value).ok())
                    };
                    let start = if range.start.is_valid() {
                        ordinal(range.start)?
                    } else {
                        0
                    };
                    let end = if range.end.is_valid() {
                        let end = ordinal(range.end)?;
                        Some(if range.end_inclusive {
                            end.checked_add(1)?
                        } else {
                            end
                        })
                    } else {
                        None
                    };
                    Some(WindowedSegment::Window { start, end })
                }
                segment => Some(WindowedSegment::Segment(segment)),
            })
            .collect::<Option<Vec<_>>>()?;
        let normalized = fold_fixed_element_windows(segments)?;
        place.segments = normalized;
        Some(place)
    }

    /// One step of a place path while fixed collection windows are folded:
    /// either an ordinary segment or a window `[start..end]` (`end` is `None`
    /// for an open-ended window such as `[start..]`).
    pub enum WindowedSegment {
        Window { start: usize, end: Option<usize> },
        Segment(facts::PlaceSegment),
    }

    /// A fixed window `[s..e]` followed by element `[i]` names the single
    /// element `[s + i]` of the window's storage. Place identity must not
    /// depend on which view spelled that element, so canonical element and
    /// write places fold every such pair (nested windows compose first) into
    /// one absolute `FixedIndex`. Folding is exact or absent: it returns `None`
    /// when a window is followed by anything but a fixed element, the element
    /// lies outside its window, nested windows exceed their parent, or the path
    /// ends inside a window. Callers keep their refusal in those cases.
    pub fn fold_fixed_element_windows(
        segments: impl IntoIterator<Item = WindowedSegment>,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let mut normalized = Vec::new();
        let mut window: Option<(usize, Option<usize>)> = None;
        for segment in segments {
            let segment = match segment {
                WindowedSegment::Window { start, end } => {
                    if end.is_some_and(|end| start > end) {
                        return None;
                    }
                    window = Some(match window {
                        None => (start, end),
                        Some((base, limit)) => {
                            let start = base.checked_add(start)?;
                            let end = match end {
                                Some(end) => Some(base.checked_add(end)?),
                                None => None,
                            };
                            if limit.is_some_and(|limit| start > limit)
                                || limit.zip(end).is_some_and(|(limit, end)| end > limit)
                            {
                                return None;
                            }
                            (start, end.or(limit))
                        }
                    });
                    continue;
                }
                WindowedSegment::Segment(segment) => segment,
            };
            let segment = match (window.take(), segment) {
                (Some((base, limit)), facts::PlaceSegment::FixedIndex { index }) => {
                    let index = base.checked_add(index)?;
                    if limit.is_some_and(|limit| index >= limit) {
                        return None;
                    }
                    facts::PlaceSegment::FixedIndex { index }
                }
                (Some(_), _) => return None,
                (None, segment) => segment,
            };
            normalized.push(segment);
        }
        if window.is_some() {
            return None;
        }
        Some(normalized)
    }

    pub fn index_place_segment(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> facts::PlaceSegment {
        if let ExpressionNode::Range(range) = expression_table(program).expression(expression)
            && let Some((start, end)) = fixed_half_open_range(program, range)
        {
            return facts::PlaceSegment::FixedRange { start, end };
        }
        element_place_segment(program, expression)
    }

    pub fn element_place_segment(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> facts::PlaceSegment {
        if program
            .closed_integer_expression_value(expression)
            .is_none()
        {
            let mut nodes = Vec::new();
            crate::queries::expression_traversal::collect_expression_tree(
                program, expression, &mut nodes,
            );
            if nodes.iter().any(|node| {
                matches!(
                    expression_table(program).expression(*node),
                    ExpressionNode::Call(_)
                )
            }) {
                return facts::PlaceSegment::Index { expression };
            }
        }
        exact_fixed_element(program, expression, 0)
            .map(|index| facts::PlaceSegment::FixedIndex { index })
            .unwrap_or(facts::PlaceSegment::Index { expression })
    }

    fn exact_fixed_element(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<usize> {
        if depth >= 64 || !expression.is_valid() {
            return None;
        }
        if let Some(value) = program.closed_integer_expression_value(expression) {
            return value.to_u64().and_then(|value| usize::try_from(value).ok());
        }
        if let Some(value) =
            crate::value_custody::literals::closed_record_integer_projection(program, expression)
        {
            return value
                .value
                .to_i64()
                .and_then(|element| usize::try_from(element).ok());
        }
        let bindings = match_arm_bindings(program, expression).unwrap_or_default();
        if let Some(value) = folded_coordinate(program, expression, &bindings) {
            return usize::try_from(value).ok();
        }
        if let Some(value) = standing_fixed_element(program, expression) {
            return usize::try_from(value).ok();
        }
        if let Some(initializer) = immutable_name_initializer(program, expression) {
            if let Some(call) = expression_table(program).call_node(initializer) {
                let declaration = program
                    .symbols
                    .get(expression_name_symbol(program, expression)?);
                let caller = program.symbols.get(declaration.parent).parent;
                let (Some(low), Some(high)) =
                    crate::normal_return_integer_bounds(program, caller, initializer, call)?
                else {
                    return None;
                };
                return (low == high)
                    .then_some(low)
                    .and_then(|value| usize::try_from(value).ok());
            }
            return exact_fixed_element(program, initializer, depth + 1);
        }
        if let ExpressionNode::Match(dispatch) = expression_table(program).expression(expression) {
            let mut arms = expression_table(program).match_arms(dispatch.arms).iter();
            let first = exact_fixed_element(program, arms.next()?.value, depth + 1)?;
            return arms
                .all(|arm| exact_fixed_element(program, arm.value, depth + 1) == Some(first))
                .then_some(first);
        }
        let ExpressionNode::Member(member) = expression_table(program).expression(expression)
        else {
            return None;
        };
        let initializer = immutable_name_initializer(program, member.receiver)?;
        let literal = expression_table(program).struct_literal_node(initializer)?;
        let field = expression_table(program)
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.field_symbol == member.member_symbol)?;
        exact_fixed_element(program, field.value, depth + 1)
    }

    fn standing_fixed_element(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        let states = states_naming(program, expression);
        if states.is_empty() {
            return None;
        }
        let mut element = None;
        for state_symbol in states.iter().copied() {
            let machine_symbol = program.symbols.get(state_symbol).parent;
            let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
            let (lower, upper) =
                crate::standing_integer_bounds(program, machine, state, expression)?;
            let lower = lower?;
            if upper != Some(lower) {
                return None;
            }
            if element.is_some_and(|prior| prior != lower) {
                return None;
            }
            element = Some(lower);
        }
        element
    }

    fn expression_name_symbol(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let ExpressionNode::Name(path) = expression_table(program).expression(expression) else {
            return None;
        };
        Some(path.symbol)
    }

    fn immutable_name_initializer(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let ExpressionNode::Name(path) = expression_table(program).expression(expression) else {
            return None;
        };
        if !path.symbol.is_valid() || path.head_symbol != path.symbol {
            return None;
        }
        let declaration = program.symbols.get(path.symbol);
        if declaration.kind != SymbolKind::Local {
            return None;
        }
        let state_symbol = declaration.parent;
        let machine_symbol = program.symbols.get(state_symbol).parent;
        let (_, state) = program.machine_state(machine_symbol, state_symbol)?;
        let (_, local) = program.state_local_declaration(state, path.symbol)?;
        if local.is_mutable || !local.initial_value.is_valid() {
            return None;
        }
        Some(local.initial_value)
    }

    fn fixed_half_open_range(
        program: &dyn CheckingInput,
        range: &typed_trees::expression::TableRangeExpression,
    ) -> Option<(usize, usize)> {
        let start = if range.start.is_valid() {
            window_coordinate(program, range.start)?
        } else {
            0
        };
        if !range.end.is_valid() {
            return None;
        }
        let mut end = window_coordinate(program, range.end)?;
        if range.end_inclusive {
            end = end.checked_add(1)?;
        }
        Some((start, end))
    }

    pub fn window_coordinate(
        program: &dyn CheckingInput,
        endpoint: ExpressionHandle,
    ) -> Option<usize> {
        exact_fixed_element(program, endpoint, 0)
    }

    fn match_arm_bindings(
        program: &dyn CheckingInput,
        endpoint: ExpressionHandle,
    ) -> Option<Vec<(ExpressionHandle, i64)>> {
        let bindings = literal_arm_bindings(program, endpoint);
        let mut consistent = Vec::new();
        for (subject, value) in bindings.iter().copied() {
            if let Some((_, prior)) = consistent
                .iter()
                .find(|(existing, _)| same_match_subject(program, *existing, subject))
            {
                if *prior != value {
                    return None;
                }
                continue;
            }
            consistent.push((subject, value));
        }
        Some(consistent)
    }

    fn folded_coordinate(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        bindings: &[(ExpressionHandle, i64)],
    ) -> Option<i64> {
        if !expression.is_valid() {
            return None;
        }
        let states = states_naming(program, expression);
        let mut result = None;
        for state_symbol in states
            .iter()
            .copied()
            .chain(states.is_empty().then_some(SymbolHandle::invalid()))
        {
            let machine_symbol = if state_symbol.is_valid() {
                program.symbols.get(state_symbol).parent
            } else {
                SymbolHandle::invalid()
            };
            let value = program
                .integer_expression_value_with_bindings(
                    expression,
                    machine_symbol,
                    &mut |operand| {
                        let mut matched = bindings
                            .iter()
                            .filter(|(subject, _)| same_match_subject(program, *subject, operand))
                            .map(|(_, value)| *value);
                        let value = if let Some(first) = matched.next() {
                            matched.all(|value| value == first).then_some(first)?
                        } else {
                            let call = expression_table(program).call_node(operand)?;
                            let (Some(low), Some(high)) = crate::normal_return_integer_bounds(
                                program,
                                machine_symbol,
                                operand,
                                call,
                            )?
                            else {
                                return None;
                            };
                            (low == high).then_some(low)?
                        };
                        let type_reference = if state_symbol.is_valid() {
                            let (machine, state) =
                                program.machine_state(machine_symbol, state_symbol)?;
                            crate::expression_result_type_reference(
                                program, machine, state, operand,
                            )?
                        } else {
                            let call = expression_table(program).call_node(operand)?;
                            crate::call_target_return_type(
                                program,
                                crate::call_target_symbol(program, call),
                            )?
                        };
                        Some((numerics::bignum::BigInt::from_i64(value), type_reference))
                    },
                )?
                .to_i64()?;
            if result.is_some_and(|prior| prior != value) {
                return None;
            }
            result = Some(value);
        }
        result
    }

    fn same_match_subject(
        program: &dyn CheckingInput,
        subject: ExpressionHandle,
        expression: ExpressionHandle,
    ) -> bool {
        if subject == expression {
            return true;
        }
        let Some(subject_place) = canonical_place_from_expression(program, subject) else {
            return false;
        };
        canonical_place_from_expression(program, expression)
            .is_some_and(|place| place == subject_place)
    }

    pub fn push_field_place_segments(
        program: &dyn CheckingInput,
        segments: &mut Vec<facts::PlaceSegment>,
        symbol: SymbolHandle,
    ) {
        if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
            segments.push(facts::PlaceSegment::Case { variant });
        }
        segments.push(facts::PlaceSegment::Field { symbol });
    }

    pub fn canonical_place_from_expression(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        if !expression.is_valid() {
            return None;
        }

        match expression_table(program).expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                program.saved_value_source(saved)?;
                let anchor = expression_table(program)
                    .value_capture(saved.capture)
                    .value_anchor;
                if !expression_table(program).expression_is_valid(anchor) {
                    return None;
                }
                Some(CanonicalPlace {
                    root: facts::PlaceRoot::Expression(anchor),
                    segments: Vec::new(),
                })
            }
            ExpressionNode::Borrow(inner) => {
                canonical_place_from_expression(program, storage_operand(program, inner.target))
            }
            ExpressionNode::Cast(cast) if cast_preserves_place(program, cast) => {
                canonical_place_from_expression(program, cast.value)
            }
            ExpressionNode::Name(path) => {
                if name_materializes_value(program, expression) {
                    return Some(CanonicalPlace {
                        root: facts::PlaceRoot::Expression(expression),
                        segments: Vec::new(),
                    });
                }
                let root_symbol = first_valid_name_path_symbol(path, expression_table(program))?;
                let mut place = CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root_symbol),
                    segments: Vec::new(),
                };
                let members = expression_table(program).name_path_members(path.members);
                for (offset, retained) in expression_table(program)
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .skip(1)
                    .copied()
                    .enumerate()
                {
                    let symbol = members
                        .get(offset + 1)
                        .and_then(|member| {
                            super::contextual_projection::resolve_member_symbol_from_place(
                                program,
                                &place,
                                member.as_str(),
                                None,
                                retained,
                            )
                        })
                        .unwrap_or(retained);
                    push_field_place_segments(program, &mut place.segments, symbol);
                }
                Some(place)
            }
            ExpressionNode::Member(member) => {
                let mut place = canonical_place_from_expression(
                    program,
                    storage_operand(program, member.receiver),
                )?;
                let symbol = effective_member_symbol(program, member.receiver, member);
                push_field_place_segments(program, &mut place.segments, symbol);
                Some(place)
            }
            ExpressionNode::Indexed(indexed) => {
                let mut place = canonical_place_from_expression(
                    program,
                    storage_operand(program, indexed.collection),
                )?;
                place
                    .segments
                    .push(index_place_segment(program, indexed.index));
                Some(place)
            }
            _ => Some(CanonicalPlace {
                root: facts::PlaceRoot::Expression(expression),
                segments: Vec::new(),
            }),
        }
    }

    pub(crate) fn cast_preserves_place(
        program: &dyn CheckingInput,
        cast: &checked_trees::expression::TableCastExpression,
    ) -> bool {
        cast_shares_storage(program, cast)
            && (cast.form.is_recast()
                || !super::member_resolution::expression_type_position(program, cast.value)
                    .is_some_and(|position| {
                        let super::member_resolution::MemberPosition::Reference(operand) = position
                        else {
                            return false;
                        };
                        if crate::reference_shell(program, operand).is_some() {
                            return false;
                        }
                        crate::field_domains::domain_constraint_identities(program, operand)
                            .into_iter()
                            .any(|(symbol, identity)| {
                                crate::field_domains::cast_erases_membership(
                                    program, cast, symbol, identity,
                                )
                            })
                    }))
    }

    fn cast_shares_storage(
        program: &dyn CheckingInput,
        cast: &checked_trees::expression::TableCastExpression,
    ) -> bool {
        if cast.form.is_recast() {
            return true;
        }
        let target = if cast.result_type.is_valid() {
            cast.result_type
        } else {
            cast.target_type
        };
        if !target.is_valid() {
            return false;
        }
        let Some(target_carrier) = program.unwrapped_type_reference(target) else {
            return false;
        };
        let Some(operand_pos) =
            super::member_resolution::expression_type_position(program, cast.value)
        else {
            return false;
        };
        match operand_pos {
            super::member_resolution::MemberPosition::Reference(operand_ref) => {
                carriers_match(program, operand_ref, target_carrier)
            }
            super::member_resolution::MemberPosition::Declaration(symbol) => {
                match program.type_reference_table.type_reference(target_carrier) {
                    TypeReferenceNode::Named { symbol: s, .. } => *s == symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol == symbol,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn carriers_match(
        program: &dyn CheckingInput,
        left: typed_trees::types::TypeReferenceHandle,
        right: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let Some(left) = program.unwrapped_type_reference(left) else {
            return false;
        };
        let Some(right) = program.unwrapped_type_reference(right) else {
            return false;
        };
        if left == right {
            return true;
        }
        match (
            program.type_reference_table.type_reference(left),
            program.type_reference_table.type_reference(right),
        ) {
            (
                TypeReferenceNode::Named {
                    symbol: s1,
                    name: n1,
                },
                TypeReferenceNode::Named {
                    symbol: s2,
                    name: n2,
                },
            ) => {
                if s1.is_valid() && s2.is_valid() {
                    s1 == s2
                } else {
                    n1 == n2
                }
            }
            (
                TypeReferenceNode::Generic {
                    base_symbol: s1,
                    base_name: n1,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: s2,
                    base_name: n2,
                    ..
                },
            ) => {
                if s1.is_valid() && s2.is_valid() {
                    s1 == s2
                } else {
                    n1 == n2
                }
            }
            (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
            _ => false,
        }
    }

    pub fn place_root_may_be_symbol(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        let mut leaf = expression;
        loop {
            if !leaf.is_valid() {
                return false;
            }
            leaf = match expression_table(program).expression(leaf) {
                ExpressionNode::Borrow(inner) => inner.target,
                ExpressionNode::Cast(cast) if cast_shares_storage(program, cast) => cast.value,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed) => indexed.collection,
                ExpressionNode::Name(path) => {
                    return !name_materializes_value(program, leaf)
                        && first_valid_name_path_symbol(path, expression_table(program))
                            .is_none_or(|root| root == symbol);
                }
                _ => return false,
            };
        }
    }

    pub(crate) fn name_materializes_value(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        crate::exact_case_reference_owner(program, expression).is_some()
            || matches!(
                expression_table(program).expression(expression),
                ExpressionNode::Name(path)
                    if path.symbol.is_valid()
                        && program.symbols.get(path.symbol).kind == SymbolKind::Const
            )
    }

    pub fn storage_place_from_expression_in_state(
        program: &dyn CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            storage_operand(program, expression),
        )
    }

    pub fn storage_operand(
        program: &dyn CheckingInput,
        mut expression: ExpressionHandle,
    ) -> ExpressionHandle {
        while let Some(cast) = expression_table(program).cast_node(expression)
            && cast_shares_storage(program, cast)
        {
            expression = cast.value;
        }
        expression
    }

    pub fn canonical_place_from_expression_in_state(
        program: &dyn CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let mut place = contextual_canonical_place_from_expression(
            program,
            state_symbol,
            statement_index,
            expression,
        )
        .or_else(|| canonical_place_from_expression(program, expression))?;
        if let Some(receiver) = place.root.as_expression()
            && let Some(state) = program.state_by_symbol(state_symbol)
            && let Some((_, saved)) = super::structural_scalar_field::saved_dispatch_subject(
                program,
                state,
                statement_index,
                receiver,
            )
        {
            place.root = facts::PlaceRoot::Expression(saved);
        }
        Some(place)
    }

    pub(crate) mod syntax_queries {
        //! Flow sweeps revisit selectors without changing their source associations.
        //! Retain those associations for the frozen input, including empty results.
        //! These rows select no executable arm and establish no numeric or flow fact:
        //! subject consistency, standing bounds and completed-call judgments remain
        //! with the projection query that consumes them.

        use crate::CheckingInput;
        use crate::frozen_program::{expression_table, frozen_memo, once_lock_rows};
        use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
        use std::sync::{Arc, OnceLock};
        use symbols::{SymbolHandle, SymbolKind};

        #[derive(Default)]
        struct EndpointAssociations {
            naming_states: OnceLock<Arc<[SymbolHandle]>>,
            literal_arm_bindings: OnceLock<Arc<[(ExpressionHandle, i64)]>>,
        }

        pub(crate) struct ProjectionSyntaxQueries {
            endpoints: Vec<(ExpressionHandle, EndpointAssociations)>,
            state_matches: Vec<(SymbolHandle, OnceLock<Arc<[ExpressionHandle]>>)>,
        }

        impl ProjectionSyntaxQueries {
            fn of(program: &dyn CheckingInput) -> Self {
                let mut endpoints: Vec<_> = expression_table(program)
                    .iter_backing_expressions()
                    .map(|(handle, _)| (handle, EndpointAssociations::default()))
                    .collect();
                endpoints.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
                let state_matches = once_lock_rows(program.machines().iter().flat_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .map(|state| state.symbol)
                }));
                Self {
                    endpoints,
                    state_matches,
                }
            }

            fn endpoint(&self, endpoint: ExpressionHandle) -> Option<&EndpointAssociations> {
                self.endpoints
                    .binary_search_by_key(
                        &(endpoint.arena_index(), endpoint.generation()),
                        |(handle, _)| (handle.arena_index(), handle.generation()),
                    )
                    .ok()
                    .map(|position| &self.endpoints[position].1)
            }

            fn matches(
                &self,
                program: &dyn CheckingInput,
                state: SymbolHandle,
            ) -> Arc<[ExpressionHandle]> {
                let compute = || state_match_nodes(program, state);
                match self.state_matches.binary_search_by_key(
                    &(state.arena_index(), state.generation()),
                    |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                ) {
                    Ok(position) => self.state_matches[position].1.get_or_init(compute).clone(),
                    Err(_) => compute(),
                }
            }
        }

        fn state_match_nodes(
            program: &dyn CheckingInput,
            state: SymbolHandle,
        ) -> Arc<[ExpressionHandle]> {
            let machine = program.symbols.get(state).parent;
            let Some((_, state)) = program.machine_state(machine, state) else {
                return Arc::from([]);
            };
            let mut matches = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, root) in program.tables.expression_roots(statement) {
                    let mut nodes = Vec::new();
                    crate::queries::expression_traversal::collect_expression_tree(
                        program, root, &mut nodes,
                    );
                    // Each root owns its traversal. A shared handle in another root
                    // remains another candidate, in the original order.
                    matches.extend(nodes.into_iter().filter(|node| {
                        matches!(
                            expression_table(program).expression(*node),
                            ExpressionNode::Match(_)
                        )
                    }));
                }
            }
            Arc::from(matches)
        }

        fn queries(program: &dyn CheckingInput) -> Arc<ProjectionSyntaxQueries> {
            frozen_memo(
                program,
                |memos| &memos.projection_syntax,
                || ProjectionSyntaxQueries::of(program),
            )
        }

        pub(super) fn states_naming(
            program: &dyn CheckingInput,
            endpoint: ExpressionHandle,
        ) -> Arc<[SymbolHandle]> {
            let compute = || {
                let mut nodes = Vec::new();
                crate::queries::expression_traversal::collect_expression_tree(
                    program, endpoint, &mut nodes,
                );
                let mut states = Vec::new();
                for node in nodes {
                    let ExpressionNode::Name(path) = expression_table(program).expression(node)
                    else {
                        continue;
                    };
                    if !path.head_symbol.is_valid() {
                        continue;
                    }
                    let declaration = program.symbols.get(path.head_symbol);
                    if matches!(declaration.kind, SymbolKind::Parameter | SymbolKind::Local)
                        && !states.contains(&declaration.parent)
                    {
                        states.push(declaration.parent);
                    }
                }
                Arc::from(states)
            };
            if program.checking_memos().is_none() {
                return compute();
            }
            let queries = queries(program);
            match queries.endpoint(endpoint) {
                Some(row) => row.naming_states.get_or_init(compute).clone(),
                None => compute(),
            }
        }

        pub(super) fn literal_arm_bindings(
            program: &dyn CheckingInput,
            endpoint: ExpressionHandle,
        ) -> Arc<[(ExpressionHandle, i64)]> {
            let queries = program.checking_memos().map(|_| queries(program));
            let compute = || {
                let mut bindings = Vec::new();
                for state in states_naming(program, endpoint).iter().copied() {
                    let matches = match &queries {
                        Some(queries) => queries.matches(program, state),
                        None => state_match_nodes(program, state),
                    };
                    for node in matches.iter().copied() {
                        let ExpressionNode::Match(dispatch) =
                            expression_table(program).expression(node)
                        else {
                            continue;
                        };
                        let mut patterns = Vec::new();
                        for arm in expression_table(program).match_arms(dispatch.arms) {
                            let MatchPattern::Value(pattern) = arm.pattern else {
                                continue;
                            };
                            patterns.push(pattern);
                            let Some(value) = expression_table(program)
                                .integer_node(pattern)
                                .and_then(|literal| literal.value_i64())
                            else {
                                continue;
                            };
                            // Keep every preceding pattern and every executable
                            // occurrence. A failed effectful pattern cannot grant the
                            // captured subject's equality to a later storage read.
                            if crate::queries::expression_traversal::expression_has_inert_prefix(
                                program, &patterns, arm.value, endpoint,
                            ) {
                                bindings.push((dispatch.subject, value));
                            }
                        }
                    }
                }
                Arc::from(bindings)
            };
            match queries
                .as_ref()
                .and_then(|queries| queries.endpoint(endpoint))
            {
                Some(row) => row.literal_arm_bindings.get_or_init(compute).clone(),
                None => compute(),
            }
        }
    }
}
pub(crate) use canonical_projection::ProjectionSyntaxQueries;
pub use canonical_projection::{
    CanonicalPlace, WindowedSegment, canonical_collection_element_place,
    canonical_place_from_expression, canonical_place_from_expression_in_state,
    element_place_segment, fold_fixed_element_windows, index_place_segment,
    place_root_may_be_symbol, push_field_place_segments, storage_operand,
    storage_place_from_expression_in_state, window_coordinate,
};
pub(crate) mod contextual_projection {
    use super::canonical_projection::CanonicalPlace;
    use super::canonical_projection::index_place_segment;
    use super::canonical_projection::push_field_place_segments;
    use super::member_resolution;
    use super::member_resolution::resolve_member_symbol_from_type_symbol;
    use crate::first_valid_name_path_symbol;
    use crate::frozen_program::expression_table;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use facts::PlaceRoot;
    use member_resolution::MemberPosition;
    use symbols::SymbolHandle;

    pub fn contextual_canonical_place_from_expression(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        if !expression.is_valid() {
            return None;
        }

        match expression_table(program).expression(expression) {
            ExpressionNode::SavedSubject(_) => {
                super::canonical_projection::canonical_place_from_expression(program, expression)
            }
            ExpressionNode::Borrow(inner) => contextual_canonical_place_from_expression(
                program,
                state_symbol,
                statement_index,
                super::canonical_projection::storage_operand(program, inner.target),
            ),
            ExpressionNode::Cast(cast)
                if cast.form.is_recast()
                    || super::canonical_projection::cast_preserves_place(program, cast) =>
            {
                contextual_canonical_place_from_expression(
                    program,
                    state_symbol,
                    statement_index,
                    cast.value,
                )
            }
            ExpressionNode::Name(path) => {
                if super::canonical_projection::name_materializes_value(program, expression) {
                    return super::canonical_projection::canonical_place_from_expression(
                        program, expression,
                    );
                }
                let root_symbol = resolve_contextual_name_path_root(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                    path,
                )?;
                let mut place = CanonicalPlace {
                    root: PlaceRoot::Symbol(root_symbol),
                    segments: Vec::new(),
                };
                let members = expression_table(program).name_path_members(path.members);
                let member_symbols =
                    expression_table(program).name_path_member_symbols(path.member_symbols);
                let start_index = usize::from(
                    members
                        .first()
                        .is_some_and(|member| member.is_self_receiver()),
                );
                for (offset, member_name) in members.iter().skip(start_index + 1).enumerate() {
                    let retained = member_symbols
                        .get(offset + start_index + 1)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid);
                    let symbol = resolve_member_symbol_from_place(
                        program,
                        &place,
                        member_name.as_str(),
                        None,
                        retained,
                    )
                    .unwrap_or(retained);
                    push_field_place_segments(program, &mut place.segments, symbol);
                }
                Some(place)
            }
            ExpressionNode::Member(member) => {
                let mut place = contextual_canonical_place_from_expression(
                    program,
                    state_symbol,
                    statement_index,
                    super::canonical_projection::storage_operand(program, member.receiver),
                )?;
                let symbol = {
                    let symbol = member_resolution::effective_member_symbol_from_position(
                        program,
                        member.receiver,
                        member_resolution::expression_type_position(program, member.receiver),
                        member,
                    );
                    if symbol.is_valid() {
                        symbol
                    } else {
                        resolve_member_symbol_from_place(
                            program,
                            &place,
                            member.member.as_str(),
                            member.case_variant.as_ref().map(|variant| variant.as_str()),
                            member.member_symbol,
                        )
                        .unwrap_or_else(SymbolHandle::invalid)
                    }
                };
                push_field_place_segments(program, &mut place.segments, symbol);
                Some(place)
            }
            ExpressionNode::Indexed(indexed) => {
                let mut place = contextual_canonical_place_from_expression(
                    program,
                    state_symbol,
                    statement_index,
                    super::canonical_projection::storage_operand(program, indexed.collection),
                )?;
                if let Some((machine, state)) = crate::find_state(program, state_symbol)
                    && super::builtin_coordinates::whole_collection_view_source(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                    .is_some()
                {
                    return Some(place);
                }
                place
                    .segments
                    .push(index_place_segment(program, indexed.index));
                Some(place)
            }
            _ => super::canonical_projection::canonical_place_from_expression(program, expression),
        }
    }

    fn resolve_contextual_name_path_root(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        path: &typed_trees::expression::TableNamePath,
    ) -> Option<SymbolHandle> {
        let name = expression_table(program).display_name(expression);
        let state = program.state_by_symbol(state_symbol)?;
        if let Some(symbol) = first_valid_name_path_symbol(path, expression_table(program)) {
            return Some(symbol);
        }

        crate::state_scoped_name_symbol(program, state, &name, statement_index)
    }

    pub fn resolve_member_symbol_from_place(
        program: &dyn crate::CheckingInput,
        place: &CanonicalPlace,
        member_name: &str,
        case_variant: Option<&str>,
        retained: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let position = match place.root {
            PlaceRoot::Symbol(symbol) => member_resolution::symbol_type_position(program, symbol)?,
            PlaceRoot::Expression(expression) => {
                member_resolution::expression_type_position(program, expression)?
            }
            PlaceRoot::TypeReference(reference) => MemberPosition::Reference(reference),
            PlaceRoot::Unknown
            | PlaceRoot::InvocationReferent { .. }
            | PlaceRoot::InvocationOutput(_) => return None,
        };

        let position =
            member_resolution::member_position_after_segments(program, position, &place.segments)?;

        if let Some(symbol) = member_resolution::closed_generic_member_symbol(
            program,
            Some(position),
            member_name,
            case_variant,
            retained,
        ) {
            return Some(symbol);
        }
        if case_variant.is_none() && retained.is_valid() {
            return Some(retained);
        }
        let current = member_resolution::position_leaf_symbol(program, position);
        match case_variant {
            Some(variant) => member_resolution::resolve_case_member_symbol_from_type_symbol(
                program,
                current,
                variant,
                member_name,
            ),
            None => resolve_member_symbol_from_type_symbol(program, current, member_name),
        }
    }
}
pub use contextual_projection::{
    contextual_canonical_place_from_expression, resolve_member_symbol_from_place,
};
pub(crate) mod member_resolution {

    use super::declared_symbols::{data_member_symbol_by_name, first_valid_name_path_symbol};
    use crate::frozen_program::expression_table;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use facts::PlaceSegment;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::data::DataMember;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum MemberPosition {
        Reference(typed_trees::types::TypeReferenceHandle),
        Declaration(SymbolHandle),
        Sliced(typed_trees::types::TypeReferenceHandle),
    }

    pub(crate) fn position_leaf_symbol(
        program: &dyn crate::CheckingInput,
        position: MemberPosition,
    ) -> SymbolHandle {
        match position {
            MemberPosition::Reference(reference) => closed_generic_owner(program, reference)
                .map(|owner| owner.symbol)
                .unwrap_or_else(|| program.type_reference_table.type_symbol(reference)),
            MemberPosition::Declaration(symbol) => symbol,
            MemberPosition::Sliced(_) => SymbolHandle::invalid(),
        }
    }

    fn closed_generic_owner(
        program: &dyn crate::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        while let typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = program.type_reference_table.type_reference(reference)
        {
            reference = *referee;
        }
        let owner = program.closed_data_definition(reference)?;
        owner.generic_instance.is_some().then_some(owner)
    }

    pub(crate) fn closed_generic_member_symbol(
        program: &dyn crate::CheckingInput,
        position: Option<MemberPosition>,
        member_name: &str,
        case_variant: Option<&str>,
        retained: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let MemberPosition::Reference(reference) = position? else {
            return None;
        };
        closed_generic_owner(program, reference)?;
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        let selected = super::projected_members::applied_data_field(
            program,
            reference,
            retained,
            member_name,
            case_variant,
        )
        .map(|field| field.symbol);
        Some(selected.unwrap_or_else(SymbolHandle::invalid))
    }

    fn member_type_position(
        program: &dyn crate::CheckingInput,
        receiver_position: Option<MemberPosition>,
        member_symbol: SymbolHandle,
    ) -> Option<MemberPosition> {
        if let Some(MemberPosition::Reference(reference)) = receiver_position
            && let Some(projected) = super::projection_types::project_type_reference_from_segments(
                program,
                reference,
                &[PlaceSegment::Field {
                    symbol: member_symbol,
                }],
            )
        {
            return Some(MemberPosition::Reference(projected));
        }
        if matches!(receiver_position, Some(MemberPosition::Reference(reference))
        if closed_generic_owner(program, reference).is_some())
        {
            return None;
        }
        symbol_type_position(program, member_symbol)
    }

    pub(crate) enum ContainerOutcome {
        Position(MemberPosition),
        MappedMiss,
        Unmapped,
    }

    fn container_member_type_position(
        program: &dyn crate::CheckingInput,
        parent: SymbolHandle,
        symbol: SymbolHandle,
    ) -> ContainerOutcome {
        if !parent.is_valid() {
            return ContainerOutcome::Unmapped;
        }
        let grandparent = program.symbols.get(parent).parent;
        match program.symbols.get(parent).kind {
            SymbolKind::Machine => {
                let Some(machine) = program.machine_by_symbol(parent) else {
                    return ContainerOutcome::Unmapped;
                };
                if let Some(field) = crate::exact_attached_field(
                    program,
                    machine,
                    symbol,
                    program.symbols.name(symbol),
                ) {
                    return ContainerOutcome::Position(MemberPosition::Reference(
                        field.type_reference,
                    ));
                }
                program
                    .machine_owned_data_by_symbol(machine, symbol)
                    .map(|owned| {
                        ContainerOutcome::Position(MemberPosition::Reference(owned.type_reference))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::Data => {
                let Some(data) = program.data_definition_by_symbol(parent) else {
                    return ContainerOutcome::Unmapped;
                };
                program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.symbol == symbol => {
                            Some(ContainerOutcome::Position(MemberPosition::Reference(
                                field.type_reference,
                            )))
                        }
                        _ => None,
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::Variant => {
                if !grandparent.is_valid() {
                    return ContainerOutcome::Unmapped;
                }
                let Some(data) = program.data_definition_by_symbol(grandparent) else {
                    return ContainerOutcome::Unmapped;
                };
                let Some(variant) =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == parent => {
                                Some(variant)
                            }
                            _ => None,
                        })
                else {
                    return ContainerOutcome::Unmapped;
                };
                program
                    .data_payload_field_by_symbol(variant, symbol)
                    .map(|field| {
                        ContainerOutcome::Position(MemberPosition::Reference(field.type_reference))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::State
                if grandparent.is_valid()
                    && program.symbols.get(grandparent).kind == SymbolKind::Trait =>
            {
                let Some(trait_definition) = program.trait_definition_by_symbol(grandparent) else {
                    return ContainerOutcome::Unmapped;
                };
                let Some(signature) =
                    program.trait_machine_signature_by_symbol(trait_definition, parent)
                else {
                    return ContainerOutcome::Unmapped;
                };
                program
                    .state_signature_parameters(signature)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .map(|parameter| {
                        ContainerOutcome::Position(MemberPosition::Reference(
                            parameter.type_reference,
                        ))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::State => {
                let Some(state) = program.state_by_symbol(parent) else {
                    return ContainerOutcome::Unmapped;
                };
                if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                    return ContainerOutcome::Position(MemberPosition::Reference(
                        parameter.type_reference,
                    ));
                }
                program
                    .state_local_declaration(state, symbol)
                    .map(|(_, local_data)| {
                        ContainerOutcome::Position(MemberPosition::Reference(
                            local_data.type_reference,
                        ))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::Operator
                if grandparent.is_valid()
                    && program.symbols.get(grandparent).kind == SymbolKind::Domain =>
            {
                let Some(domain) = program.domain_definition(grandparent) else {
                    return ContainerOutcome::Unmapped;
                };
                let Some(operator) = program
                    .domain_operators(domain)
                    .iter()
                    .find(|operator| operator.symbol == parent)
                else {
                    return ContainerOutcome::Unmapped;
                };
                program
                    .operator_parameters(operator)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .map(|parameter| {
                        ContainerOutcome::Position(MemberPosition::Reference(
                            parameter.type_reference,
                        ))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            SymbolKind::Operator => {
                let Some(operator) = program.operator_definition_by_symbol(parent) else {
                    return ContainerOutcome::Unmapped;
                };
                program
                    .operator_parameters(operator)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .map(|parameter| {
                        ContainerOutcome::Position(MemberPosition::Reference(
                            parameter.type_reference,
                        ))
                    })
                    .unwrap_or(ContainerOutcome::MappedMiss)
            }
            _ => ContainerOutcome::Unmapped,
        }
    }

    pub(crate) fn symbol_type_position(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<MemberPosition> {
        if !symbol.is_valid() {
            return None;
        }

        let parent = program.symbols.get(symbol).parent;
        match container_member_type_position(program, parent, symbol) {
            ContainerOutcome::Position(position) => return Some(position),
            ContainerOutcome::MappedMiss => return None,
            ContainerOutcome::Unmapped => {}
        }
        match program.symbols.get(symbol).kind {
            SymbolKind::Machine => {
                let machine = program.machine_by_symbol(symbol)?;
                if machine.attached_data_application.is_valid() {
                    return Some(MemberPosition::Reference(machine.attached_data_application));
                }
                return program
                    .attached_data_definition(machine)
                    .map(|data| MemberPosition::Declaration(data.symbol));
            }
            SymbolKind::Parameter | SymbolKind::Local | SymbolKind::Field => {}
            _ => return None,
        }
        whole_program_member_type_position(program, symbol)
    }

    fn whole_program_member_type_position(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<MemberPosition> {
        for machine in program.machines() {
            if machine.symbol == symbol {
                if machine.attached_data_application.is_valid() {
                    return Some(MemberPosition::Reference(machine.attached_data_application));
                }
                if let Some(data) = program.attached_data_definition(machine) {
                    return Some(MemberPosition::Declaration(data.symbol));
                }
            }
            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    if parameter.symbol == symbol {
                        return Some(MemberPosition::Reference(parameter.type_reference));
                    }
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(local_data) = statement.as_local_data()
                        && local_data.symbol == symbol
                    {
                        return Some(MemberPosition::Reference(local_data.type_reference));
                    }
                }
            }
            for owned in program.machine_owned_data(machine) {
                if owned.symbol == symbol {
                    return Some(MemberPosition::Reference(owned.type_reference));
                }
            }
        }

        for definition in program.traits() {
            for signature in program.trait_machine_signatures(definition) {
                for parameter in program.state_signature_parameters(signature) {
                    if parameter.symbol == symbol {
                        return Some(MemberPosition::Reference(parameter.type_reference));
                    }
                }
            }
        }

        for operator in program.all_operators() {
            for parameter in program.operator_parameters(operator) {
                if parameter.symbol == symbol {
                    return Some(MemberPosition::Reference(parameter.type_reference));
                }
            }
        }

        if let Some((_, field)) = program.any_field_owner(symbol) {
            return Some(MemberPosition::Reference(field.type_reference));
        }

        None
    }

    pub(crate) fn expression_type_position(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<MemberPosition> {
        if !expression.is_valid() {
            return None;
        }
        if let Some(declaration) = program.expression_const_declaration(expression)
            && declaration.declared_type.is_valid()
        {
            return Some(MemberPosition::Reference(declaration.declared_type));
        }

        match expression_table(program).expression(expression) {
            ExpressionNode::Borrow(inner) => expression_type_position(program, inner.target),
            ExpressionNode::Atomic(atomic) => expression_type_position(program, atomic.value),
            ExpressionNode::Call(call) => {
                let retained =
                    expression_table(program).constructed_value_type_reference(expression);
                retained
                    .is_valid()
                    .then_some(retained)
                    .or_else(|| crate::call_target_return_type(program, call.target_symbol))
                    .map(MemberPosition::Reference)
            }
            ExpressionNode::Name(path) => {
                let symbol = first_valid_name_path_symbol(path, expression_table(program))?;
                symbol_type_position(program, symbol)
            }
            ExpressionNode::Indexed(indexed) => {
                let segment =
                    super::canonical_projection::index_place_segment(program, indexed.index);
                match expression_type_position(program, indexed.collection) {
                    Some(MemberPosition::Reference(reference)) => match segment {
                        PlaceSegment::FixedRange { .. } => {
                            super::projection_types::collection_element_type_reference(
                                program, reference,
                            )
                            .map(MemberPosition::Sliced)
                        }
                        _ => super::projection_types::project_type_reference_from_segments(
                            program,
                            reference,
                            &[segment],
                        )
                        .map(MemberPosition::Reference),
                    },
                    Some(MemberPosition::Sliced(element)) => match segment {
                        PlaceSegment::FixedRange { .. } => Some(MemberPosition::Sliced(element)),
                        _ => Some(MemberPosition::Reference(element)),
                    },
                    position => position,
                }
            }
            ExpressionNode::Member(member) => {
                let receiver_position = expression_type_position(program, member.receiver);
                let symbol = effective_member_symbol_from_position(
                    program,
                    member.receiver,
                    receiver_position,
                    member,
                );
                member_type_position(program, receiver_position, symbol)
            }
            ExpressionNode::StructLiteral(literal) => {
                let reference =
                    expression_table(program).constructed_value_type_reference(expression);
                if reference.is_valid() {
                    Some(MemberPosition::Reference(reference))
                } else {
                    literal
                        .type_symbol
                        .is_valid()
                        .then_some(MemberPosition::Declaration(literal.type_symbol))
                }
            }
            ExpressionNode::Cast(cast) => cast
                .result_type
                .is_valid()
                .then_some(MemberPosition::Reference(cast.result_type)),
            ExpressionNode::ZeroValue(reference) => reference
                .is_valid()
                .then_some(MemberPosition::Reference(*reference)),
            ExpressionNode::ArrayLiteral(elements) => {
                let retained =
                    expression_table(program).constructed_value_type_reference(expression);
                if retained.is_valid() {
                    Some(MemberPosition::Reference(retained))
                } else {
                    array_literal_position(
                        program,
                        expression_table(program).expression_handles(*elements),
                    )
                }
            }
            ExpressionNode::SavedSubject(saved) => program
                .saved_value_source(saved)
                .and_then(|subject| expression_type_position(program, subject)),
            ExpressionNode::Match(dispatch) => match_result_position(program, dispatch),
            _ => None,
        }
    }

    fn array_literal_position(
        program: &dyn crate::CheckingInput,
        elements: &[ExpressionHandle],
    ) -> Option<MemberPosition> {
        let mut positions = elements
            .iter()
            .map(|element| expression_type_position(program, *element));
        let Some(Some(first)) = positions.next() else {
            return None;
        };
        let MemberPosition::Reference(element) = first else {
            return None;
        };
        positions
            .all(|position| position == Some(first))
            .then_some(MemberPosition::Sliced(element))
    }

    pub(crate) fn member_position_after_segments(
        program: &dyn crate::CheckingInput,
        position: MemberPosition,
        segments: &[PlaceSegment],
    ) -> Option<MemberPosition> {
        let mut position = position;
        for segment in segments {
            position = match segment {
                PlaceSegment::Case { .. } => position,
                PlaceSegment::Field { symbol } => match position {
                    MemberPosition::Reference(reference) => {
                        match super::projection_types::project_type_reference_from_segments(
                            program,
                            reference,
                            std::slice::from_ref(segment),
                        ) {
                            Some(projected) => MemberPosition::Reference(projected),
                            None if closed_generic_owner(program, reference).is_some() => {
                                return None;
                            }
                            None => symbol_type_position(program, *symbol)?,
                        }
                    }
                    MemberPosition::Declaration(_) => symbol_type_position(program, *symbol)?,
                    MemberPosition::Sliced(_) => return None,
                },
                PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. } => match position {
                    MemberPosition::Reference(reference) => MemberPosition::Reference(
                        super::projection_types::project_type_reference_from_segments(
                            program,
                            reference,
                            std::slice::from_ref(segment),
                        )?,
                    ),
                    MemberPosition::Sliced(element) => MemberPosition::Reference(element),
                    MemberPosition::Declaration(_) => return None,
                },
                PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => match position {
                    MemberPosition::Reference(reference) => MemberPosition::Sliced(
                        super::projection_types::collection_element_type_reference(
                            program, reference,
                        )?,
                    ),
                    MemberPosition::Sliced(element) => MemberPosition::Sliced(element),
                    MemberPosition::Declaration(_) => return None,
                },
            };
        }
        Some(position)
    }

    pub fn expression_place_type_reference(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        segments: &[PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        expression_place_type_reference_observing(program, expression, segments, &mut |_| Some(()))
    }

    pub(crate) fn expression_place_type_reference_observing(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        segments: &[PlaceSegment],
        observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let mut position = expression_type_position(program, expression)?;
        observe_prefix(member_position_type_reference(program, position))?;
        for segment in segments {
            position =
                member_position_after_segments(program, position, std::slice::from_ref(segment))?;
            observe_prefix(member_position_type_reference(program, position))?;
        }
        member_position_type_reference(program, position)
    }

    fn member_position_type_reference(
        program: &dyn crate::CheckingInput,
        position: MemberPosition,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        match position {
            MemberPosition::Reference(reference) => Some(reference),
            MemberPosition::Declaration(symbol) => program
                .type_reference_table
                .find_named_type_reference(symbol),
            MemberPosition::Sliced(_) => None,
        }
    }

    fn match_result_position(
        program: &dyn crate::CheckingInput,
        dispatch: &typed_trees::expression::TableMatchExpression,
    ) -> Option<MemberPosition> {
        let mut positions = expression_table(program)
            .match_arms(dispatch.arms)
            .iter()
            .map(|arm| expression_type_position(program, arm.value));
        let Some(Some(first)) = positions.next() else {
            return None;
        };
        positions
            .all(|position| position == Some(first))
            .then_some(first)
    }

    pub fn effective_member_symbol(
        program: &dyn crate::CheckingInput,
        receiver: ExpressionHandle,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> SymbolHandle {
        effective_member_symbol_from_position(
            program,
            receiver,
            expression_type_position(program, receiver),
            member,
        )
    }

    pub(crate) fn effective_member_symbol_from_position(
        program: &dyn crate::CheckingInput,
        receiver: ExpressionHandle,
        receiver_position: Option<MemberPosition>,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> SymbolHandle {
        if program.pattern_component(receiver) != Some(language_core::PatternComponent::Fields) {
            return SymbolHandle::invalid();
        }
        if let ExpressionNode::SavedSubject(saved) = expression_table(program).expression(receiver)
        {
            return saved_subject_member_symbol(program, saved, receiver_position, member)
                .unwrap_or_else(SymbolHandle::invalid);
        }
        if let Some(symbol) = closed_generic_member_symbol(
            program,
            receiver_position,
            member.member.as_str(),
            member.case_variant.as_ref().map(|variant| variant.as_str()),
            member.member_symbol,
        ) {
            return symbol;
        }
        if let Some(case_name) = &member.case_variant {
            let selected = receiver_position
                .map(|position| position_leaf_symbol(program, position))
                .and_then(|type_symbol| {
                    let declaration = program.data_definition_by_symbol(type_symbol)?;
                    let variant =
                        program
                            .data_members(declaration)
                            .iter()
                            .find_map(|row| match row {
                                DataMember::Variant(variant) if variant.name == *case_name => {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.name == member.member)
                        .map(|field| field.symbol)
                });
            return selected
                .filter(|symbol| {
                    !member.member_symbol.is_valid() || member.member_symbol == *symbol
                })
                .unwrap_or_else(SymbolHandle::invalid);
        }
        if let ExpressionNode::StructLiteral(literal) =
            expression_table(program).expression(receiver)
            && let Some(field) = expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name == member.member)
            && field.field_symbol.is_valid()
        {
            return field.field_symbol;
        }

        if let Some(symbol) =
            resolve_member_symbol_from_position(program, receiver_position, member.member.as_str())
        {
            return symbol;
        }

        if member.member_symbol.is_valid() {
            return member.member_symbol;
        }

        SymbolHandle::invalid()
    }

    fn saved_subject_member_symbol(
        program: &dyn crate::CheckingInput,
        saved: &typed_trees::expression::TableSavedSubject,
        position: Option<MemberPosition>,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<SymbolHandle> {
        program.saved_value_source(saved)?;
        let MemberPosition::Reference(reference) = position? else {
            return None;
        };
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        let data = super::data_definition_for_type(program, reference)?;
        let selected = if let Some(head) =
            expression_table(program).name_path_node(saved.pattern_head)
            && head.symbol.is_valid()
            && program.symbols.get(head.symbol).kind == SymbolKind::Variant
        {
            if program.closed_data_definition(reference).is_some() {
                program
                    .closed_data_constructor_case(reference, head.symbol)?
                    .symbol
            } else {
                head.symbol
            }
        } else {
            SymbolHandle::invalid()
        };
        let retained = if member.member_symbol.is_valid()
            && program.closed_data_definition(reference).is_some()
        {
            program
                .closed_data_constructor_field(reference, data.symbol, member.member_symbol)?
                .symbol
        } else {
            member.member_symbol
        };
        let mut fields = program
            .data_members(data)
            .iter()
            .flat_map(|entry| match entry {
                DataMember::Field(field) if !selected.is_valid() => std::slice::from_ref(field),
                DataMember::Variant(variant) if variant.symbol == selected => {
                    program.data_payload_fields(variant)
                }
                _ => &[],
            })
            .filter(|field| {
                field.name == member.member && (!retained.is_valid() || field.symbol == retained)
            });
        let field = fields.next()?;
        fields.next().is_none().then_some(field.symbol)
    }

    pub(crate) fn resolve_case_member_symbol_from_type_symbol(
        program: &dyn crate::CheckingInput,
        type_symbol: SymbolHandle,
        case_name: &str,
        member_name: &str,
    ) -> Option<SymbolHandle> {
        let declaration = program.data_definition_by_symbol(type_symbol)?;
        let variant = program
            .data_members(declaration)
            .iter()
            .find_map(|row| match row {
                DataMember::Variant(variant) if variant.name.as_str() == case_name => Some(variant),
                _ => None,
            })?;
        program
            .data_payload_fields(variant)
            .iter()
            .find(|field| field.name.as_str() == member_name)
            .map(|field| field.symbol)
    }

    pub fn resolve_member_symbol_from_type_symbol(
        program: &dyn crate::CheckingInput,
        type_symbol: SymbolHandle,
        member_name: &str,
    ) -> Option<SymbolHandle> {
        if let Some(data) = program.data_definition_by_symbol(type_symbol)
            && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
        {
            return Some(symbol);
        }

        if let Some(machine) = program.machine_by_symbol(type_symbol) {
            if let Some(data) = program.attached_data_definition(machine)
                && let Some(symbol) = data_member_symbol_by_name(program, data, member_name)
            {
                return Some(symbol);
            }
            for owned in program.machine_owned_data(machine) {
                if owned.name.as_str() == member_name {
                    return Some(owned.symbol);
                }
            }
        }

        None
    }

    fn resolve_member_symbol_from_position(
        program: &dyn crate::CheckingInput,
        receiver_position: Option<MemberPosition>,
        member_name: &str,
    ) -> Option<SymbolHandle> {
        let type_symbol =
            receiver_position.map(|position| position_leaf_symbol(program, position))?;
        resolve_member_symbol_from_type_symbol(program, type_symbol, member_name)
    }

    pub fn expression_type_symbol(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        expression_type_position(program, expression)
            .map(|position| position_leaf_symbol(program, position))
    }

    pub fn symbol_type_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        symbol_type_position(program, symbol)
            .map(|position| position_leaf_symbol(program, position))
    }
}
pub use member_resolution::{
    effective_member_symbol, expression_place_type_reference, expression_type_symbol,
    resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
pub(crate) mod projection_types {
    use super::canonical_projection::CanonicalPlace;
    use super::canonical_projection::canonical_place_from_expression_in_state;
    use crate::value_custody::type_references::substituted_identity;
    use checked_trees::expression::ExpressionHandle;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::TypeReferenceHandle;

    pub fn expression_type_reference_in_state(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if !expression.is_valid() {
            return None;
        }

        let place = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        )?;
        canonical_place_type_reference(program, state_symbol, statement_index, &place)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StorageObservationTypePathError {
        pub segment_ordinal: usize,
        pub segment: Option<checked_trees::CheckedUnitStructuralPathSegment>,
        pub source_type: typed_trees::types::TypeReferenceHandle,
        pub current_type: typed_trees::types::TypeReferenceHandle,
        pub current_type_identity: String,
        pub reason: &'static str,
    }

    #[derive(Debug, Clone)]
    pub struct StorageObservationTypePath {
        entries: Vec<(typed_trees::types::TypeReferenceHandle, usize)>,
        substitutions: Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
    }

    impl StorageObservationTypePath {
        pub fn entries(
            &self,
        ) -> impl Iterator<
            Item = (
                typed_trees::types::TypeReferenceHandle,
                &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            ),
        > {
            self.entries
                .iter()
                .map(|(reference, end)| (*reference, &self.substitutions[..*end]))
        }
    }

    pub fn observed_storage_type_references(
        program: &dyn crate::CheckingInput,
        source_type: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        field: SymbolHandle,
        field_identity: &checked_trees::CheckedMemberIdentity,
        atomic: bool,
    ) -> Result<StorageObservationTypePath, StorageObservationTypePathError> {
        use checked_trees::CheckedUnitStructuralPathSegment as Segment;
        use typed_trees::types::TypeReferenceNode;

        let final_field = field
            .is_valid()
            .then(|| Segment::Field(field_identity.clone()));
        let mut cursor = ObservationTypeCursor::new(source_type);
        cursor
            .projection
            .substitutions
            .extend_from_slice(substitutions);
        let mut references = vec![(source_type, substitutions.len())];
        for (ordinal, segment) in path.iter().chain(final_field.iter()).enumerate() {
            let current = cursor.projection.normalize(program);
            let identity = substituted_identity(program, current, &cursor.projection.substitutions);
            let _ = cursor.advance(program, segment).map_err(|reason| {
                StorageObservationTypePathError {
                    segment_ordinal: ordinal,
                    segment: Some(segment.clone()),
                    source_type,
                    current_type: current,
                    current_type_identity: program.type_identity_text(identity).unwrap_or_default(),
                    reason,
                }
            })?;
            if !matches!(segment, Segment::Case(_)) {
                references.push((
                    cursor.projection.normalize(program),
                    cursor.projection.substitutions.len(),
                ));
            }
        }
        let current = cursor.projection.normalize(program);
        let failure = |reason| StorageObservationTypePathError {
            segment_ordinal: path.len() + usize::from(field.is_valid()),
            segment: None,
            source_type,
            current_type: current,
            current_type_identity: observation_type_identity_text(
                program,
                current,
                &cursor.projection.substitutions,
            ),
            reason,
        };
        if cursor.selected_case.is_some() {
            return Err(failure("field observation ends in an unprojected case"));
        }
        if atomic {
            let leaf = crate::peel_declared_constraints(program, current);
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(leaf)
            else {
                return Err(failure("atomic observation has no atomic leaf declaration"));
            };
            if !program
                .symbols
                .builtin_type_atom(*symbol)
                .is_some_and(|atom| atom.is_atomic())
            {
                return Err(failure(
                    "atomic observation ends at an ordinary scalar leaf",
                ));
            }
            if field.is_valid() {
                references.pop();
            }
        }
        Ok(StorageObservationTypePath {
            entries: references,
            substitutions: cursor.projection.substitutions,
        })
    }

    pub(crate) mod applied_members {
        use facts::PlaceSegment;
        use typed_trees::types::TypeReferenceHandle;

        pub fn corresponding_loan_owner_path(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            path: &[checked_trees::BorrowLoanOwnerSegment],
        ) -> Option<Vec<checked_trees::BorrowLoanOwnerSegment>> {
            use checked_trees::BorrowLoanOwnerSegment as Owner;
            let segments = path
                .iter()
                .map(|segment| match *segment {
                    Owner::Field(symbol) => PlaceSegment::Field { symbol },
                    Owner::Case(variant) => PlaceSegment::Case { variant },
                    Owner::FixedIndex(index) => PlaceSegment::FixedIndex { index },
                    Owner::DynamicIndex => PlaceSegment::Index {
                        expression: Default::default(),
                    },
                })
                .collect::<Vec<_>>();
            let selected = corresponding_place_projection(program, reference, &segments)?;
            if selected.len() != path.len() {
                return None;
            }
            selected
                .into_iter()
                .zip(path)
                .map(|(selected, original)| match (*original, selected) {
                    (Owner::Field(_), PlaceSegment::Field { symbol }) => Some(Owner::Field(symbol)),
                    (Owner::Case(_), PlaceSegment::Case { variant }) => Some(Owner::Case(variant)),
                    (Owner::FixedIndex(index), PlaceSegment::FixedIndex { index: selected })
                        if index == selected =>
                    {
                        Some(*original)
                    }
                    (Owner::DynamicIndex, PlaceSegment::Index { .. }) => Some(*original),
                    _ => None,
                })
                .collect()
        }

        pub fn corresponding_place_projection(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> Option<Vec<PlaceSegment>> {
            if !path.iter().any(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
                )
            }) || (!path
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                && crate::project_type_reference_from_segments(program, reference, path).is_some())
            {
                return Some(path.to_vec());
            }
            let mut selected = Vec::with_capacity(path.len());
            let mut case = None;
            for segment in path {
                let current =
                    crate::project_type_reference_from_segments(program, reference, &selected)?;
                let current = crate::unwrapped_type_reference(program, current)?;
                let segment = match *segment {
                    PlaceSegment::Case { variant } => {
                        if !variant.is_valid() || case.is_some() {
                            return None;
                        }
                        let owner = program.symbols.get(variant).parent;
                        let variant = program
                            .data_constructor_case(current, owner, variant)?
                            .symbol;
                        case = Some(variant);
                        PlaceSegment::Case { variant }
                    }
                    PlaceSegment::Field { symbol } => {
                        if !symbol.is_valid() {
                            return None;
                        }
                        let parent = program.symbols.get(symbol).parent;
                        if !parent.is_valid() {
                            return None;
                        }
                        let owner =
                            if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                                program.symbols.get(parent).parent
                            } else {
                                parent
                            };
                        let symbol = program
                            .data_constructor_field(current, owner, symbol)?
                            .symbol;
                        if let Some(case) = case.take() {
                            let (_, case) = program.variant_owner(case)?;
                            if !program
                                .data_payload_fields(case)
                                .iter()
                                .any(|field| field.symbol == symbol)
                            {
                                return None;
                            }
                        }
                        PlaceSegment::Field { symbol }
                    }
                    segment => {
                        if case.is_some() {
                            return None;
                        }
                        segment
                    }
                };
                selected.push(segment);
            }
            Some(selected)
        }
    }
    pub use applied_members::{corresponding_loan_owner_path, corresponding_place_projection};

    pub(crate) mod qualification_paths {
        use super::{ObservationTypeCursor, TypeReferenceHandle};

        pub fn qualification_storage_path(
            program: &dyn crate::CheckingInput,
            root_type: TypeReferenceHandle,
            segments: &[facts::PlaceSegment],
        ) -> Result<Vec<checked_trees::CheckedUnitStructuralPathSegment>, &'static str> {
            use checked_trees::CheckedUnitStructuralPathSegment as Segment;
            use typed_trees::types::TypeReferenceNode;

            if !root_type.is_valid() {
                return Err("qualification storage has no exact root type");
            }
            let mut cursor = ObservationTypeCursor::new(root_type);
            let mut path = Vec::with_capacity(segments.len());
            for segment in segments {
                let mut crossed = Vec::new();
                loop {
                    let carrier = cursor.carrier(program)?;
                    if !matches!(
                        program.type_reference_table.type_reference(carrier),
                        TypeReferenceNode::Reference { .. }
                    ) {
                        break;
                    }
                    if crossed.contains(&carrier) {
                        return Err("qualification storage crosses a cyclic reference carrier");
                    }
                    crossed.push(carrier);
                    let stored_pointer = crate::stores_reference_shape(
                        program,
                        carrier,
                        &cursor.projection.substitutions,
                    );
                    let _ = cursor.advance(program, &Segment::Referent)?;
                    if stored_pointer {
                        path.push(Segment::Referent);
                    }
                }
                let (checked, symbol) = match segment {
                    facts::PlaceSegment::Field { symbol } => (
                        Segment::Field(
                            super::super::structural_member_identity(program, *symbol)
                                .ok_or("qualification field has no exact member identity")?,
                        ),
                        Some(*symbol),
                    ),
                    facts::PlaceSegment::Case { variant } => (
                        Segment::Case(
                            super::super::structural_member_identity(program, *variant)
                                .ok_or("qualification case has no exact member identity")?,
                        ),
                        Some(*variant),
                    ),
                    facts::PlaceSegment::FixedIndex { index } => (
                        Segment::FixedIndex(
                            u64::try_from(*index).map_err(|_| "qualification index exceeds u64")?,
                        ),
                        None,
                    ),
                    facts::PlaceSegment::Index { .. } => {
                        return Err("qualification storage requires a statically captured index");
                    }
                    extent if *extent == crate::field_domains::WHOLE_ELEMENT_EXTENT => {
                        (Segment::AllElements, None)
                    }
                    facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                        return Err("qualification storage has an unsupported collection window");
                    }
                };
                let member = if matches!(checked, Segment::AllElements) {
                    cursor.advance_element(program)?
                } else {
                    cursor.advance(program, &checked)?
                };
                if member != symbol {
                    return Err("qualification member differs from its exact typed owner");
                }
                path.push(checked);
            }
            if cursor.selected_case.is_some() {
                return Err("qualification storage ends in an unprojected case");
            }
            Ok(path)
        }

        pub fn canonical_qualification_path(
            program: &dyn crate::CheckingInput,
            root_type: TypeReferenceHandle,
            path: &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<Vec<facts::PlaceSegment>, &'static str> {
            use checked_trees::CheckedUnitStructuralPathSegment as Segment;
            use typed_trees::types::TypeReferenceNode;
            if !root_type.is_valid() {
                return Err("qualification source has no exact carrier");
            }
            let mut cursor = ObservationTypeCursor::new(root_type);
            let mut result = Vec::with_capacity(path.len());
            for segment in path {
                if !matches!(segment, Segment::Referent) {
                    let mut crossed = Vec::new();
                    loop {
                        let carrier = cursor.carrier(program)?;
                        if !matches!(
                            program.type_reference_table.type_reference(carrier),
                            TypeReferenceNode::Reference { .. }
                        ) {
                            break;
                        }
                        if crossed.contains(&carrier) {
                            return Err("qualification source crosses a cyclic reference");
                        }
                        crossed.push(carrier);
                        cursor.advance(program, &Segment::Referent)?;
                    }
                }
                let member = if matches!(segment, Segment::AllElements) {
                    cursor.advance_element(program)?
                } else {
                    cursor.advance(program, segment)?
                };
                match segment {
                    Segment::Field(_) => result.push(facts::PlaceSegment::Field {
                        symbol: member.ok_or("qualification field lost its selected symbol")?,
                    }),
                    Segment::Case(_) => result.push(facts::PlaceSegment::Case {
                        variant: member.ok_or("qualification case lost its selected symbol")?,
                    }),
                    Segment::FixedIndex(index) => result.push(facts::PlaceSegment::FixedIndex {
                        index: usize::try_from(*index)
                            .map_err(|_| "qualification index exceeds usize")?,
                    }),
                    Segment::AllElements => result.push(crate::field_domains::WHOLE_ELEMENT_EXTENT),
                    Segment::Referent => {}
                    _ => return Err("qualification coordinate has no static source projection"),
                }
            }
            if cursor.selected_case.is_some() {
                return Err("qualification source ends in an unprojected case");
            }
            Ok(result)
        }
    }
    pub use qualification_paths::{canonical_qualification_path, qualification_storage_path};

    pub(crate) struct ObservationTypeCursor<'program> {
        projection: ProjectionTypeCursor,
        selected_case: Option<&'program typed_trees::data::DataVariant>,
    }

    impl<'program> ObservationTypeCursor<'program> {
        fn new(reference: TypeReferenceHandle) -> Self {
            Self {
                projection: ProjectionTypeCursor::new(reference),
                selected_case: None,
            }
        }

        fn carrier(
            &mut self,
            program: &dyn crate::CheckingInput,
        ) -> Result<TypeReferenceHandle, &'static str> {
            let current = self.projection.normalize(program);
            crate::value_custody::reference_shells::peel_carrier_shells(
                program,
                current,
                &self.projection.substitutions,
            )
            .ok_or("observation carrier has no finite supplying application")
        }

        fn advance(
            &mut self,
            program: &'program dyn crate::CheckingInput,
            segment: &checked_trees::CheckedUnitStructuralPathSegment,
        ) -> Result<Option<SymbolHandle>, &'static str> {
            use checked_trees::CheckedUnitStructuralPathSegment as Segment;
            use typed_trees::{data::DataMember, types::TypeReferenceNode};

            let current = self.projection.normalize(program);
            let carrier = self.carrier(program)?;
            let mut member = None;
            self.projection.current = match segment {
                Segment::Referent => {
                    if self.selected_case.is_some() {
                        return Err("reference crossing interrupts a selected case");
                    }
                    let TypeReferenceNode::Reference {
                        referee, access, ..
                    } = program.type_reference_table.type_reference(carrier)
                    else {
                        return Err("reference crossing has no reference carrier");
                    };
                    if !access.is_readable() {
                        return Err("field observation crosses a write-only reference");
                    }
                    *referee
                }
                Segment::Case(identity) => {
                    if self.selected_case.is_some() {
                        return Err("case selection interrupts another selected case");
                    }
                    let data = observation_data_declaration(
                        program,
                        carrier,
                        &self.projection.substitutions,
                    )
                    .ok_or("case carrier has no exact data declaration")?;
                    let mut matching = program.data_members(data).iter().filter_map(|value| {
                        let DataMember::Variant(case) = value else {
                            return None;
                        };
                        (case.member_identity() == *identity).then_some(case)
                    });
                    let case = matching
                        .next()
                        .ok_or("case identity is absent from its carrier")?;
                    if matching.next().is_some() {
                        return Err("case identity is ambiguous in its carrier");
                    }
                    self.selected_case = Some(case);
                    return Ok(Some(case.symbol));
                }
                Segment::Field(identity) => {
                    let data = observation_data_declaration(
                        program,
                        carrier,
                        &self.projection.substitutions,
                    )
                    .ok_or("field carrier has no exact data declaration")?;
                    let case = self.selected_case.take();
                    let payloads = case.map_or(&[][..], |case| program.data_payload_fields(case));
                    let fields =
                        program
                            .data_members(data)
                            .iter()
                            .filter_map(|value| match value {
                                DataMember::Field(field) if case.is_none() => Some(field),
                                _ => None,
                            });
                    let mut matching = payloads
                        .iter()
                        .chain(fields)
                        .filter(|field| field.member_identity() == *identity);
                    let selected = matching
                        .next()
                        .ok_or("field identity is absent from its carrier")?;
                    if matching.next().is_some() {
                        return Err("field identity is ambiguous in its carrier");
                    }
                    if selected.relevance.is_erased() {
                        return Err("field observation traverses an erased field");
                    }
                    member = Some(selected.symbol);
                    field_type_reference(
                        program,
                        current,
                        selected.symbol,
                        &mut self.projection.substitutions,
                    )
                    .ok_or("field type lost its supplying application")?
                }
                Segment::FixedIndex(index) => {
                    if self.selected_case.is_some() {
                        return Err("index interrupts a selected case payload");
                    }
                    if let Some(length) = crate::fixed_array_type_length(program, carrier)
                        && usize::try_from(*index).map_or(true, |index| index >= length)
                    {
                        return Err("fixed index exceeds its declared collection");
                    }
                    if matches!(
                        program.type_reference_table.type_reference(carrier),
                        TypeReferenceNode::Reference { .. }
                    ) {
                        return Err("index requires its explicit reference crossing");
                    }
                    indexed_element_type_reference(program, current, &self.projection.substitutions)
                        .ok_or("index carrier has no collection element")?
                }
                Segment::RuntimeIndex(_) | Segment::ComputedIndex(_) => {
                    return self.advance_element(program);
                }
                _ => return Err("field observation has a non-field projection segment"),
            };
            self.projection.normalize(program);
            Ok(member)
        }

        fn advance_element(
            &mut self,
            program: &dyn crate::CheckingInput,
        ) -> Result<Option<SymbolHandle>, &'static str> {
            let current = self.projection.normalize(program);
            let carrier = self.carrier(program)?;
            if self.selected_case.is_some() {
                return Err("index interrupts a selected case payload");
            }
            if matches!(
                program.type_reference_table.type_reference(carrier),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            ) {
                return Err("index requires its explicit reference crossing");
            }
            self.projection.current =
                indexed_element_type_reference(program, current, &self.projection.substitutions)
                    .ok_or("index carrier has no collection element")?;
            self.projection.normalize(program);
            Ok(None)
        }
    }

    fn observation_type_identity_text(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> String {
        program
            .type_identity_text(program.type_identity(TypeIdentityRequest {
                substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            }))
            .unwrap_or_default()
    }

    fn observation_data_declaration<'program>(
        program: &'program dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        use typed_trees::types::TypeReferenceNode;
        let exact_application = substitutions.is_empty()
            || program.normalized_type_identity(reference)
                == substituted_identity(program, reference, substitutions);
        program
            .data_application_definition(reference)
            .filter(|_| exact_application)
            .or_else(|| {
                program
                    .closed_data_definition(reference)
                    .filter(|_| exact_application)
            })
            .or_else(|| {
                let symbol = match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return None,
                };
                unique_data_definition_by_symbol(program, symbol)
            })
    }

    pub fn canonical_place_type_reference(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        canonical_place_type_reference_observing(
            program,
            state_symbol,
            statement_index,
            place,
            &mut |_| Some(()),
        )
    }

    pub fn canonical_owned_place_type_reference(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        canonical_place_type_reference_observing(
            program,
            state_symbol,
            statement_index,
            place,
            &mut |reference| {
                let reference = reference?;
                crate::reference_shell(program, reference)
                    .is_none()
                    .then_some(())
            },
        )
    }

    fn canonical_place_type_reference_observing(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if let facts::PlaceRoot::InvocationOutput(identity) = place.root {
            if !identity.is_valid() {
                return None;
            }
            return project_type_reference_from_segment_iter_observing(
                program,
                crate::invocation_output_parameter(program, identity)?.type_reference,
                &place.segments,
                observe_prefix,
            );
        }
        if let facts::PlaceRoot::InvocationReferent { type_reference, .. } = place.root {
            return project_type_reference_from_segment_iter_observing(
                program,
                type_reference,
                &place.segments,
                observe_prefix,
            );
        }
        if let Some(expression) = place.root.as_expression() {
            return super::member_resolution::expression_place_type_reference_observing(
                program,
                expression,
                &place.segments,
                observe_prefix,
            );
        }
        if let Some(reference) = place.root.as_type_reference() {
            return project_type_reference_from_segment_iter_observing(
                program,
                reference,
                &place.segments,
                observe_prefix,
            );
        }
        let root_symbol = place.root.as_symbol()?;

        if let Some((machine, _)) = crate::find_state(program, state_symbol)
            && machine.symbol == root_symbol
            && let Some((facts::PlaceSegment::Field { symbol }, remaining)) =
                place.segments.split_first()
        {
            observe_prefix(canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            ))?;
            if machine.attached_data_application.is_valid() {
                let mut projection = ProjectionTypeCursor::new(machine.attached_data_application);
                projection.project(program, &place.segments[0])?;
                return projection.walk(program, remaining, observe_prefix);
            }
            let current = attached_data_field_type_reference(program, machine, *symbol)?;
            return project_type_reference_from_segment_iter_observing(
                program,
                current,
                remaining,
                observe_prefix,
            );
        }

        if let Some((machine, _)) = crate::find_state(program, state_symbol)
            && machine.attached_data_application.is_valid()
            && let Some(field) = crate::exact_attached_field(
                program,
                machine,
                root_symbol,
                program.symbols.name(root_symbol),
            )
        {
            let root_field = facts::PlaceSegment::Field {
                symbol: field.symbol,
            };
            let mut projection = ProjectionTypeCursor::new(machine.attached_data_application);
            projection.project(program, &root_field)?;
            return projection.walk(program, &place.segments, observe_prefix);
        }

        let current =
            symbol_type_reference_in_state(program, state_symbol, statement_index, root_symbol)?;
        project_type_reference_from_segment_iter_observing(
            program,
            current,
            &place.segments,
            observe_prefix,
        )
    }

    pub fn canonical_place_source_type_application(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        Vec<typed_trees::types::TypeReferenceHandle>,
    )> {
        let reference = canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        source_type_application_from_segments(program, reference, &place.segments)
    }

    pub fn source_type_application_from_segments(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)> {
        let mut applications = Vec::new();
        for segment in segments {
            let (projected, frames) =
                program.project_enclosing_data_type_reference(reference, &applications)?;
            reference = projected;
            applications.truncate(frames);
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let parent = crate::unwrapped_type_reference(program, reference)?;
                    let (parent, frames) =
                        program.project_enclosing_data_type_reference(parent, &applications)?;
                    applications.truncate(frames);
                    let owner = program
                        .type_reference_table
                        .type_reference(parent)
                        .type_symbol(&program.type_reference_table);
                    let declaration = program.unique_data_definition_by_symbol(owner)?;
                    applications.push(parent);
                    reference = data_field_type_reference(program, declaration, *symbol)?;
                }
                facts::PlaceSegment::Case { .. } => {}
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                    reference = collection_element_type_reference(program, reference)?;
                }
                extent if *extent == crate::field_domains::WHOLE_ELEMENT_EXTENT => {
                    reference = collection_element_type_reference(program, reference)?;
                }
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                    return None;
                }
            }
        }
        let (reference, frames) =
            program.project_enclosing_data_type_reference(reference, &applications)?;
        applications.truncate(frames);
        Some((reference, applications))
    }

    pub fn project_type_reference_from_segments(
        program: &dyn crate::CheckingInput,
        current: typed_trees::types::TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        project_type_reference_from_segment_iter(program, current, segments)
    }

    pub fn qualification_storage_type_reference(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<TypeReferenceHandle> {
        if !root.is_valid() {
            return None;
        }
        let mut projection = ProjectionTypeCursor::new(root);
        for (ordinal, segment) in segments.iter().enumerate() {
            match segment {
                facts::PlaceSegment::FixedRange { start, end }
                    if *segment != crate::field_domains::WHOLE_ELEMENT_EXTENT =>
                {
                    let current = projection.normalize(program);
                    let length = crate::fixed_array_type_length(program, current)?;
                    if ordinal + 1 == segments.len() || start > end || *end > length {
                        return None;
                    }
                    projection.current = indexed_element_type_reference(
                        program,
                        current,
                        &projection.substitutions,
                    )?;
                }
                facts::PlaceSegment::OpaqueRange => return None,
                facts::PlaceSegment::Index { expression }
                    if !expression.is_valid()
                        || matches!(
                            program.expression_table.expression(*expression),
                            typed_trees::expression::ExpressionNode::Range(_)
                        ) =>
                {
                    return None;
                }
                _ => projection.advance(program, segment)?,
            }
        }
        Some(projection.normalize(program))
    }

    pub fn owned_projection_type_reference(
        program: &dyn crate::CheckingInput,
        current: typed_trees::types::TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let mut projection = ProjectionTypeCursor::new(current);
        for segment in segments {
            if crate::reference_shell(program, projection.normalize(program)).is_some() {
                return None;
            }
            projection.advance(program, segment)?;
        }
        Some(projection.normalize(program))
    }

    pub fn loan_carrier_has_owned_storage(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        owner_symbol: SymbolHandle,
        owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    ) -> bool {
        let Some(mut current) =
            symbol_type_reference_in_state(program, state_symbol, statement_index, owner_symbol)
        else {
            return false;
        };
        let mut substitutions = Vec::new();
        for segment in owner_path {
            current = substituted_type_reference(program, current, &substitutions);
            if crate::reference_shell(program, current).is_some() {
                return false;
            }
            let projected = match segment {
                checked_trees::BorrowLoanOwnerSegment::Case(_) => Some(current),
                checked_trees::BorrowLoanOwnerSegment::Field(symbol) => {
                    field_type_reference(program, current, *symbol, &mut substitutions)
                }
                checked_trees::BorrowLoanOwnerSegment::FixedIndex(_)
                | checked_trees::BorrowLoanOwnerSegment::DynamicIndex => {
                    indexed_element_type_reference(program, current, &substitutions)
                }
            };
            let Some(projected) = projected else {
                return false;
            };
            current = projected;
        }
        current.is_valid()
    }

    pub fn collection_element_type_reference(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        indexed_element_type_reference(program, type_reference, &[])
    }

    fn project_type_reference_from_segment_iter<'segment>(
        program: &dyn crate::CheckingInput,
        current: typed_trees::types::TypeReferenceHandle,
        segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        project_type_reference_from_segment_iter_observing(program, current, segments, &mut |_| {
            Some(())
        })
    }

    fn project_type_reference_from_segment_iter_observing<'segment>(
        program: &dyn crate::CheckingInput,
        current: typed_trees::types::TypeReferenceHandle,
        segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
        observe_prefix: &mut impl FnMut(Option<typed_trees::types::TypeReferenceHandle>) -> Option<()>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        ProjectionTypeCursor::new(current).walk(program, segments, observe_prefix)
    }

    pub(crate) struct ProjectionTypeCursor {
        current: typed_trees::types::TypeReferenceHandle,
        substitutions: Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
    }

    impl ProjectionTypeCursor {
        fn new(current: typed_trees::types::TypeReferenceHandle) -> Self {
            Self {
                current,
                substitutions: Vec::new(),
            }
        }

        fn normalize(
            &mut self,
            program: &dyn crate::CheckingInput,
        ) -> typed_trees::types::TypeReferenceHandle {
            self.current = substituted_type_reference(program, self.current, &self.substitutions);
            self.current
        }

        fn project(
            &mut self,
            program: &dyn crate::CheckingInput,
            segment: &facts::PlaceSegment,
        ) -> Option<()> {
            self.normalize(program);
            self.advance(program, segment)
        }

        fn advance(
            &mut self,
            program: &dyn crate::CheckingInput,
            segment: &facts::PlaceSegment,
        ) -> Option<()> {
            self.current = match segment {
                facts::PlaceSegment::Case { .. } => self.current,
                facts::PlaceSegment::Field { symbol } => {
                    field_type_reference(program, self.current, *symbol, &mut self.substitutions)?
                }
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                    indexed_element_type_reference(program, self.current, &self.substitutions)?
                }
                extent if *extent == crate::field_domains::WHOLE_ELEMENT_EXTENT => {
                    indexed_element_type_reference(program, self.current, &self.substitutions)?
                }
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                    return None;
                }
            };
            Some(())
        }

        fn walk<'segment>(
            mut self,
            program: &dyn crate::CheckingInput,
            segments: impl IntoIterator<Item = &'segment facts::PlaceSegment>,
            observe_prefix: &mut impl FnMut(
                Option<typed_trees::types::TypeReferenceHandle>,
            ) -> Option<()>,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            let mut reference = self.normalize(program);
            observe_prefix(Some(reference))?;
            for segment in segments {
                self.advance(program, segment)?;
                reference = self.normalize(program);
                observe_prefix(Some(reference))?;
            }
            Some(reference)
        }
    }

    fn symbol_type_reference_in_state(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let state = program.state_by_symbol(state_symbol)?;

        let local_candidate = !symbol.is_valid()
            || program.symbols.get(symbol).parent == state_symbol
            || !program.symbols.get(symbol).parent.is_valid();

        if local_candidate {
            program
                .state_parameter_by_symbol(state, symbol)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    program
                        .state_local_declaration(state, symbol)
                        .filter(|&(ordinal, _)| ordinal < statement_index)
                        .map(|(_, local_data)| local_data.type_reference)
                })
                .or_else(|| machine_member_type_reference(program, state_symbol, symbol))
        } else {
            machine_member_type_reference(program, state_symbol, symbol)
        }
    }

    fn machine_member_type_reference(
        program: &dyn crate::CheckingInput,
        state_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        crate::find_state(program, state_symbol).and_then(|(machine, _)| {
            program
                .machine_owned_data_by_symbol(machine, symbol)
                .map(|owned| owned.type_reference)
                .or_else(|| attached_data_field_type_reference(program, machine, symbol))
        })
    }

    fn attached_data_field_type_reference(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if machine.attached_data_application.is_valid() {
            return project_type_reference_from_segments(
                program,
                machine.attached_data_application,
                &[facts::PlaceSegment::Field { symbol }],
            );
        }
        let data = unique_data_definition_by_symbol(program, machine.attached_data_symbol)?;
        data_field_type_reference(program, data, symbol)
    }

    fn field_type_reference(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        field_symbol: SymbolHandle,
        substitutions: &mut Vec<(SymbolHandle, typed_trees::types::TypeReferenceHandle)>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let type_reference = substituted_type_reference(program, type_reference, substitutions);
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => field_type_reference(program, *referee, field_symbol, substitutions),
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                let exact_application = substitutions.is_empty()
                    || program.normalized_type_identity(type_reference)
                        == substituted_identity(program, type_reference, substitutions);
                if let Some(instance) = program
                    .data_application_definition(type_reference)
                    .filter(|_| exact_application)
                    .or_else(|| program.closed_data_definition(type_reference))
                {
                    return data_field_type_reference(program, instance, field_symbol);
                }
                let data = crate::field_domains::data_definition_by_symbol_or_name(
                    program,
                    *base_symbol,
                    base_name,
                )?;
                let bindings: Vec<_> = program
                    .data_type_parameters(data)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    )
                    .filter(|&(parameter, _argument)| {
                        matches!(parameter.kind, TypeParameterKind::Type)
                    })
                    .map(|(parameter, argument)| {
                        (
                            parameter.symbol,
                            substituted_type_reference(program, *argument, substitutions),
                        )
                    })
                    .collect();
                substitutions.extend(bindings);
                data_field_type_reference(program, data, field_symbol)
            }
            typed_trees::types::TypeReferenceNode::Named {
                symbol: base_symbol,
                name: base_name,
            } => {
                if let Some(machine) = program.machine(*base_symbol) {
                    attached_data_field_type_reference(program, machine, field_symbol)
                } else {
                    crate::field_domains::data_definition_by_symbol_or_name(
                        program,
                        *base_symbol,
                        base_name,
                    )
                    .and_then(|data| data_field_type_reference(program, data, field_symbol))
                }
            }
            typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::Unit => None,
        }
    }

    fn indexed_element_type_reference(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let type_reference = substituted_type_reference(program, type_reference, substitutions);
        let element = match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => indexed_element_type_reference(program, *referee, substitutions),
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
            | typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
            typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | typed_trees::types::TypeReferenceNode::Generic { .. }
            | typed_trees::types::TypeReferenceNode::Named { .. }
            | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            | typed_trees::types::TypeReferenceNode::Unit => None,
        }?;
        let collection_policy = program.arithmetic_domain_for_type_reference(type_reference);
        if program
            .type_reference_table
            .element_inherits_collection_arithmetic_policy(element, &program.symbols)
            && collection_policy != numerics::arithmetic::ArithmeticDomain::Exact
            && program.arithmetic_domain_for_type_reference(element)
                == numerics::arithmetic::ArithmeticDomain::Exact
        {
            program
                .type_reference_table
                .find_policy_qualified_type_reference(element, collection_policy)
        } else {
            Some(element)
        }
    }

    fn substituted_type_reference(
        program: &dyn crate::CheckingInput,
        mut type_reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> typed_trees::types::TypeReferenceHandle {
        let mut remaining = substitutions.len().saturating_add(1);
        while remaining > 0 {
            remaining -= 1;
            let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                break;
            };
            let Some(replacement) =
                substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, replacement)| {
                        (*parameter == *symbol).then_some(*replacement)
                    })
            else {
                break;
            };
            if replacement == type_reference {
                break;
            }
            type_reference = replacement;
        }
        type_reference
    }

    pub fn unique_data_definition_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        program.unique_data_definition_by_symbol(symbol)
    }

    fn data_field_type_reference(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        field_symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if !field_symbol.is_valid() {
            return None;
        }

        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.type_reference)
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.type_reference)
                    }),
            })
    }
}
pub use projection_types::{
    StorageObservationTypePath, StorageObservationTypePathError,
    canonical_owned_place_type_reference, canonical_place_source_type_application,
    canonical_place_type_reference, canonical_qualification_path,
    collection_element_type_reference, corresponding_loan_owner_path,
    corresponding_place_projection, expression_type_reference_in_state,
    loan_carrier_has_owned_storage, observed_storage_type_references,
    owned_projection_type_reference, project_type_reference_from_segments,
    qualification_storage_path, qualification_storage_type_reference,
    source_type_application_from_segments, unique_data_definition_by_symbol,
};
pub mod receiver_projections {
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::{
        expression::{ExpressionHandle, ExpressionNode},
        name::Identifier,
        state::State,
        types::TypeReferenceHandle,
    };

    pub struct Projection {
        pub root_symbol: SymbolHandle,
        pub symbol: SymbolHandle,
        pub type_reference: TypeReferenceHandle,
    }

    pub fn root(
        program: &dyn crate::CheckingInput,
        state: &State,
        before: usize,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Option<Projection> {
        if !symbol.is_valid() {
            return None;
        }
        let type_reference = program
            .state_parameter_by_symbol(state, symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, local)| ordinal < before && local.name == *name)
                    .map(|(_, local)| local.type_reference)
            })?;
        Some(Projection {
            root_symbol: symbol,
            symbol,
            type_reference,
        })
    }

    pub fn field(
        program: &dyn crate::CheckingInput,
        receiver: Projection,
        name: &Identifier,
    ) -> Option<Projection> {
        let nominal =
            crate::machine_symbol_from_type_reference_handle(program, receiver.type_reference);
        let definition = program.data_definition_by_symbol(nominal)?;
        let selected = super::exact_data_member_field(
            program,
            definition,
            SymbolHandle::invalid(),
            name.as_str(),
            None,
        )?;
        Some(Projection {
            root_symbol: receiver.root_symbol,
            symbol: selected.symbol,
            type_reference: selected.type_reference,
        })
    }

    pub fn expression(
        program: &dyn crate::CheckingInput,
        state: &State,
        before: usize,
        handle: ExpressionHandle,
        visited: &mut Vec<ExpressionHandle>,
    ) -> Result<Option<Projection>, Vec<Diagnostic>> {
        if !handle.is_valid() || visited.contains(&handle) {
            return Ok(None);
        }
        visited.push(handle);
        let retained = match expression_table(program).expression(handle) {
            ExpressionNode::Borrow(borrow) => {
                expression(program, state, before, borrow.target, visited)?
            }
            ExpressionNode::Name(path) => {
                let members = expression_table(program).name_path_members(path.members);
                let Some(first) = members.first() else {
                    return Ok(None);
                };
                let Some(mut selected) = root(program, state, before, path.head_symbol, first)
                else {
                    return Ok(None);
                };
                let symbols =
                    expression_table(program).name_path_member_symbols(path.member_symbols);
                for (index, member) in members.iter().enumerate() {
                    if index != 0 {
                        let Some(projected) = field(program, selected, member) else {
                            return Ok(None);
                        };
                        selected = projected;
                    }
                    check_symbol(
                        program,
                        handle,
                        symbols
                            .get(index)
                            .copied()
                            .unwrap_or_else(SymbolHandle::invalid),
                        &selected,
                    )?;
                }
                check_symbol(program, handle, path.symbol, &selected)?;
                Some(selected)
            }
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                let Some(receiver) = expression(program, state, before, member.receiver, visited)?
                else {
                    return Ok(None);
                };
                let Some(selected) = field(program, receiver, &member.member) else {
                    return Ok(None);
                };
                check_symbol(program, handle, member.member_symbol, &selected)?;
                Some(selected)
            }
            ExpressionNode::Indexed(indexed) => {
                let Some(receiver) =
                    expression(program, state, before, indexed.collection, visited)?
                else {
                    return Ok(None);
                };
                let Some(machine) = crate::find_machine(program, state.symbol) else {
                    return Ok(None);
                };
                if !crate::place_has_builtin_coordinates(program, machine, Some(state), handle) {
                    return Ok(None);
                }
                if matches!(
                    expression_table(program).expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return Ok(None);
                }
                let Some(element_type) =
                    super::collection_element_type_reference(program, receiver.type_reference)
                else {
                    return Ok(None);
                };
                Some(Projection {
                    root_symbol: receiver.root_symbol,
                    symbol: SymbolHandle::invalid(),
                    type_reference: element_type,
                })
            }
            _ => None,
        };
        Ok(retained)
    }

    fn check_symbol(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        authored: SymbolHandle,
        selected: &Projection,
    ) -> Result<(), Vec<Diagnostic>> {
        if authored.is_valid() && authored != selected.symbol {
            return Err(vec![
                Diagnostic::error(
                    "projected call receiver disagrees with its exact declared field",
                )
                .with_source_span(expression_table(program).source_span(expression)),
            ]);
        }
        Ok(())
    }

    pub fn projected_statement_receiver_place(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        before: usize,
        call: &typed_trees::statement::TableCall,
    ) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
        let members = program.statement_table.name_path_members(call.receiver);
        let mut receiver = root(
            program,
            state,
            before,
            call.receiver_root_symbol,
            members.first()?,
        )?;
        let root = receiver.symbol;
        let mut segments = Vec::new();
        for member in members.iter().skip(1) {
            receiver = field(program, receiver, member)?;
            segments.push(facts::PlaceSegment::Field {
                symbol: receiver.symbol,
            });
        }
        (receiver.symbol == call.receiver_symbol).then_some((root, segments))
    }

    pub fn projected_statement_receiver_type_reference(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        before: usize,
        call: &typed_trees::statement::TableCall,
    ) -> Option<TypeReferenceHandle> {
        let (root, segments) = projected_statement_receiver_place(program, state, before, call)?;
        super::canonical_place_type_reference(
            program,
            state.symbol,
            before,
            &super::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            },
        )
    }
}
pub use receiver_projections::{
    projected_statement_receiver_place, projected_statement_receiver_type_reference,
};
