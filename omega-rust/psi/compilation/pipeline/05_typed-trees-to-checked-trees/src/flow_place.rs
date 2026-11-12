/*
    Flow-dependent place judgments and semantic-fact translation.
    Exact typed-program projection lives in validation's value-custody owner;
    these children narrow live selectors, compare paths and check case access.
*/

mod canonicalization {
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use facts::FactPlan;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use validation::{CanonicalPlace, canonical_place_from_expression, effective_member_symbol};

    /*
        Narrow each runtime `Index` segment whose selector value is pinned in the
        live contexts to the `FixedIndex` it names at this program point.

        A selector the scalar facts cannot pin stays an `Index` segment, so the
        overlap predicates keep their conservative may-alias treatment. This is
        the same narrowing `transfers::write_target` applies to a write target:
        sharing it keeps the invalidation view of a runtime index identical to the
        establishment view, instead of the write landing on a fixed element while
        retiring facts at every element.
    */
    pub(crate) fn narrow_pinned_index_segments(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        state_symbol: SymbolHandle,
        statement_index: usize,
        context_handles: &[facts::FactContextHandle],
        segments: &mut [facts::PlaceSegment],
    ) {
        narrow_pinned_index_segments_with_values(
            program,
            state_symbol,
            statement_index,
            segments,
            |selector| {
                crate::values::scalar_value_at_place(
                    program,
                    semantic,
                    context_handles
                        .iter()
                        .map(|handle| semantic.contexts.get(*handle)),
                    selector,
                )
            },
        );
    }

    pub(crate) fn narrow_pinned_index_segments_with_values(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        segments: &mut [facts::PlaceSegment],
        mut scalar_at_place: impl FnMut(&CanonicalPlace) -> Option<facts::ScalarValue>,
    ) {
        for segment in segments.iter_mut() {
            let facts::PlaceSegment::Index { expression } = *segment else {
                continue;
            };
            let enclosed = program
                .expression_interval(expression)
                .and_then(|interval| {
                    let low = interval.low?;
                    (interval.high == Some(low)).then_some(low)
                });
            let index = match enclosed {
                Some(index) => u64::try_from(index).ok(),
                None => {
                    let Some(selector) =
                        place_at(program, state_symbol, statement_index, expression)
                    else {
                        continue;
                    };
                    let Some(facts::ScalarValue::Integer(index)) = scalar_at_place(&selector)
                    else {
                        continue;
                    };
                    index.to_u64()
                }
            };
            if let Some(index) = index.and_then(|index| usize::try_from(index).ok()) {
                *segment = facts::PlaceSegment::FixedIndex { index };
            }
        }
    }

    pub(crate) fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
        symbol.is_valid().then_some(CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        })
    }

    pub(crate) use validation::membership_evidence::canonical_place_from_semantic_place;

    /*
        The contract `field_actuals::checked_place` states for a requires leaf:
        the receiver chain must stay acyclic and every retained member selection
        must still be the member its expression resolves to, or the place it
        yields would read evidence the source did not name.
    */
    pub(crate) fn projected_leaf_place(
        program: &dyn ::validation::CheckingInput,
        leaf: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let mut current = leaf;
        let mut visited = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(current) || visited.contains(&current)
            {
                return None;
            }
            visited.push(current);
            current = match program.expression_table.expression(current) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Name(name) => {
                    if !name.symbol.is_valid()
                        || !name.head_symbol.is_valid()
                        || expressions(program)
                            .name_path_member_symbols(name.member_symbols)
                            .first()
                            .is_some_and(|symbol| *symbol != name.head_symbol)
                        || matches!(
                            program.expression_table.name_path_members(name.members),
                            [_]
                        ) && name.symbol != name.head_symbol
                    {
                        return None;
                    }
                    break;
                }
                _ => break,
            };
        }
        for expression in visited.into_iter().rev() {
            if let Some(member) = program.expression_table.member_node(expression)
                && (!member.member_symbol.is_valid()
                    || effective_member_symbol(program, member.receiver, member)
                        != member.member_symbol)
            {
                return None;
            }
        }
        let place = canonical_place_from_expression(program, leaf)?;
        (!place
            .segments
            .iter()
            .any(|segment| super::place_segment_has_unresolved_identity(*segment)))
        .then_some(place)
    }

    /*
        The canonical place a source-independent expression names: a valid symbol
        root, and every segment a statically known field, case, or literal index —
        computed indices keep the place source-dependent.
    */
    pub(crate) fn source_independent_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let place = canonical_place_from_expression(program, expression)?;
        if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
            || !place.segments.iter().all(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                facts::PlaceSegment::Case { variant } => variant.is_valid(),
                facts::PlaceSegment::FixedIndex { .. } => true,
                facts::PlaceSegment::FixedRange { .. }
                | facts::PlaceSegment::OpaqueRange
                | facts::PlaceSegment::Index { .. } => false,
            })
        {
            return None;
        }
        Some(place)
    }

    /*
        The referent place a reference initializer captures: explicit borrows and
        whole-place recasts unwrap to their target, and a plain place expression
        names the place the new local aliases.
    */
    pub(crate) fn reference_source_place(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        initializer: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let mut expression = initializer;
        loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => expression = borrow.target,
                ExpressionNode::Cast(cast) if cast.form.is_recast() => expression = cast.value,
                _ => break,
            }
        }
        place_at(program, state_symbol, statement_index, expression)
    }
}
mod case_access {
    /*
        Case payload access consumes live tag evidence for the exact saved subject.
        Conditional field contracts may move with a whole sum; extracting a payload
        must discharge every case crossed by the source place. Prepare observations
        once for a batch, so each conditional membership does not rescan all facts
        and resolve the same guard operations. The batch owns scratch observations
        from one immutable scope; it never survives a write, join or context change.
    */
    use facts::{FactContextHandle, FactPlan, PlaceRoot, PlaceSegment};
    use symbols::SymbolHandle;
    use validation::CanonicalPlace;

    use super::canonical_place_from_semantic_place;
    mod observations {
        /*
            Authenticate the tag observations already live in one consumer scope.
            Preparation does not inspect initializers or recover retired observations.
            Each Boolean leaf keeps its selected operation, exact place and polarity.
        */
        use crate::flow::expressions;
        use crate::published::places::place_at;
        use facts::{FactContextHandle, FactPayload, FactPlan};
        use symbols::SymbolHandle;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use validation::CanonicalPlace;

        pub(super) struct CaseObservation {
            pub place: NormalizedPlace,
            pub variant: SymbolHandle,
            pub selected: bool,
            pub stable_index_subject: bool,
        }

        /*
            Zero formation of an owned array, as its one `ZeroCaseElements` fact
            states it: `array[index].element` has `variant` selected for every
            index in `first..end`. It answers for each such element exactly as a
            selected observation at that element's own place would.
        */
        pub(super) struct ElementCaseObservation {
            pub array: CanonicalPlace,
            pub first: usize,
            pub end: usize,
            pub element: Vec<facts::PlaceSegment>,
            pub variant: SymbolHandle,
        }

        impl ElementCaseObservation {
            pub(super) fn covers(&self, subject: &CanonicalPlace) -> bool {
                let array = self.array.segments.len();
                subject.root == self.array.root
                    && subject.segments.len() == array + 1 + self.element.len()
                    && subject.segments[..array] == self.array.segments[..]
                    && matches!(
                        subject.segments[array],
                        facts::PlaceSegment::FixedIndex { index }
                            if self.first <= index && index < self.end
                    )
                    && subject.segments[array + 1..] == self.element[..]
            }
        }

        /* A tag observation as the facts spell it, before normalization. */
        struct RawObservation {
            place: CanonicalPlace,
            variant: SymbolHandle,
            selected: bool,
        }

        /*
            A place in one evidence batch's normalized coordinates. Only
            `normalize_place` constructs one, so a holder never needs normalizing
            again: the batch borrows an observation's own place as a query subject.
        */
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct NormalizedPlace(CanonicalPlace);

        impl std::ops::Deref for NormalizedPlace {
            type Target = CanonicalPlace;

            fn deref(&self) -> &CanonicalPlace {
                &self.0
            }
        }

        pub(super) fn collect(
            program: &dyn validation::CheckingInput,
            semantic: &FactPlan,
            contexts: &[FactContextHandle],
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
        ) -> (Vec<CaseObservation>, Vec<ElementCaseObservation>) {
            let observed = arena::unique_handles_in_order(
                semantic
                    .context_fact_handles(contexts.iter().copied())
                    .filter(|handle| {
                        matches!(
                            semantic.facts.get(*handle).payload,
                            FactPayload::BooleanValue { .. }
                                | FactPayload::BooleanExpression(_)
                                | FactPayload::ContractBooleanExpression { .. }
                                | FactPayload::CaseValue { .. }
                                | FactPayload::ZeroCaseElements { .. }
                        )
                    }),
            );
            let mut elements = Vec::new();
            let mut observer = Observer {
                program,
                machine,
                state,
                statement_index,
                rows: Vec::with_capacity(observed.len()),
            };
            for handle in observed {
                let fact = semantic.facts.get(handle);
                match fact.payload {
                    FactPayload::BooleanValue { expression, value } => {
                        observer.predicate(expression, value, 0)
                    }
                    FactPayload::BooleanExpression(expression)
                    | FactPayload::ContractBooleanExpression { expression, .. } => {
                        observer.predicate(expression, true, 0)
                    }
                    FactPayload::CaseValue { variant, value } => {
                        if !variant.is_valid()
                            || program.symbols.get(variant).kind != symbols::SymbolKind::Variant
                        {
                            continue;
                        }
                        let Some(place) = fact.place.as_place().and_then(|place| {
                            super::canonical_place_from_semantic_place(
                                program,
                                semantic,
                                semantic.places.get(place),
                            )
                        }) else {
                            continue;
                        };
                        observer.rows.push(RawObservation {
                            place,
                            variant,
                            selected: value,
                        });
                    }
                    FactPayload::ZeroCaseElements {
                        first,
                        end,
                        element,
                        variant,
                    } => {
                        if !variant.is_valid()
                            || program.symbols.get(variant).kind != symbols::SymbolKind::Variant
                        {
                            continue;
                        }
                        let Some(array) = fact.place.as_place().and_then(|place| {
                            super::canonical_place_from_semantic_place(
                                program,
                                semantic,
                                semantic.places.get(place),
                            )
                        }) else {
                            continue;
                        };
                        elements.push(ElementCaseObservation {
                            array,
                            first,
                            end,
                            element: semantic.place_segments.span_or_empty(element).to_vec(),
                            variant,
                        });
                    }
                    _ => {}
                }
            }
            let mut rows = observer
                .rows
                .into_iter()
                .map(|row| CaseObservation {
                    stable_index_subject: has_index(&row.place)
                        && super::stable_subject(
                            program,
                            machine,
                            state,
                            statement_index,
                            &row.place,
                        ),
                    place: normalize_place(
                        program,
                        semantic,
                        contexts,
                        state.symbol,
                        statement_index,
                        row.place,
                    ),
                    variant: row.variant,
                    selected: row.selected,
                })
                .collect::<Vec<_>>();
            rows.sort_by(|left, right| left.place.compare_identity(&right.place));
            (rows, elements)
        }

        /*
            Give observations and queries the same coordinate representation. The
            shared equality already compares Index with FixedIndex only when this same
            expression-table constant query succeeds. Normalize those constants too,
            so an unresolved Index can match only another unresolved Index.
        */
        pub(super) fn normalize_place(
            program: &dyn validation::CheckingInput,
            semantic: &FactPlan,
            contexts: &[FactContextHandle],
            state_symbol: SymbolHandle,
            statement_index: usize,
            mut place: CanonicalPlace,
        ) -> NormalizedPlace {
            super::super::narrow_pinned_index_segments(
                program,
                semantic,
                state_symbol,
                statement_index,
                contexts,
                &mut place.segments,
            );
            for segment in &mut place.segments {
                let facts::PlaceSegment::Index { expression } = *segment else {
                    continue;
                };
                if !program.expression_table.expression_is_valid(expression) {
                    continue;
                }
                if let Some(index) = expressions(program)
                    .constant_integer_value(expression)
                    .and_then(|value| usize::try_from(value).ok())
                {
                    *segment = facts::PlaceSegment::FixedIndex { index };
                }
            }
            NormalizedPlace(place)
        }

        pub(super) fn has_index(place: &CanonicalPlace) -> bool {
            place
                .segments
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        }

        struct Observer<'program> {
            program: &'program dyn validation::CheckingInput,
            machine: &'program typed_trees::machine::Machine,
            state: &'program typed_trees::state::State,
            statement_index: usize,
            rows: Vec<RawObservation>,
        }

        impl Observer<'_> {
            fn predicate(&mut self, expression: ExpressionHandle, value: bool, depth: usize) {
                if self
                    .program
                    .expression_table
                    .expression_at_depth(expression, depth)
                    .is_none()
                {
                    return;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        self.predicate(unary.operand, !value, depth + 1);
                    }
                    ExpressionNode::Binary(binary)
                        if (binary.operator == BinaryOperator::And && value)
                            || (binary.operator == BinaryOperator::Or && !value) =>
                    {
                        self.predicate(binary.left, value, depth + 1);
                        self.predicate(binary.right, value, depth + 1);
                    }
                    ExpressionNode::Binary(binary)
                        if binary.operator == BinaryOperator::Equal
                            && matches!(
                                self.program.expression_table.expression(binary.right),
                                ExpressionNode::Boolean(true)
                            )
                            && validation::has_builtin_decomposed_guard_meaning(
                                self.program,
                                self.machine,
                                Some(self.state),
                                expression,
                            ) =>
                    {
                        self.predicate(binary.left, value, depth + 1);
                    }
                    ExpressionNode::Binary(binary)
                        if binary.operator == BinaryOperator::Equal
                            && matches!(
                                self.program.expression_table.expression(binary.left),
                                ExpressionNode::Boolean(true)
                            )
                            && validation::has_builtin_decomposed_guard_meaning(
                                self.program,
                                self.machine,
                                Some(self.state),
                                expression,
                            ) =>
                    {
                        self.predicate(binary.right, value, depth + 1);
                    }
                    ExpressionNode::Binary(binary) => {
                        let Some(variant) = validation::applied_case_membership_variant(
                            self.program,
                            self.machine,
                            Some(self.state),
                            expression,
                            binary,
                        ) else {
                            return;
                        };
                        let Some(place) = place_at(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            binary.left,
                        ) else {
                            return;
                        };
                        self.rows.push(RawObservation {
                            place,
                            variant,
                            selected: value,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    use observations::CaseObservation;

    pub(crate) struct CaseEvidence<'program> {
        program: &'program dyn validation::CheckingInput,
        semantic: &'program FactPlan,
        contexts: Vec<FactContextHandle>,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
        observations: Vec<CaseObservation>,
        element_observations: Vec<observations::ElementCaseObservation>,
        indexed_observations: Vec<usize>,
        scalar_values: Option<crate::checks::current_scalar_values::CurrentScalarValues<'program>>,
    }

    /*
        Coordinates prepared for repeated variant queries in one immutable evidence
        batch. This is local query scratch, never retained across a context change.
    */
    struct PreparedCaseSubject<'subject> {
        place: std::borrow::Cow<'subject, observations::NormalizedPlace>,
        exact_start: usize,
        indexed_start: usize,
    }

    impl<'program> CaseEvidence<'program> {
        pub(crate) fn prepare(
            program: &'program dyn validation::CheckingInput,
            semantic: &'program FactPlan,
            contexts: &[FactContextHandle],
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
        ) -> Option<Self> {
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.state_in_machine(machine, state_symbol)?;
            let (observations, element_observations) =
                observations::collect(program, semantic, contexts, machine, state, statement_index);
            let indexed_observations: Vec<_> = observations
                .iter()
                .enumerate()
                .filter_map(|(position, row)| {
                    observations::has_index(&row.place).then_some(position)
                })
                .collect();
            let scalar_values = indexed_observations
                .iter()
                .any(|position| observations[*position].stable_index_subject)
                .then(|| {
                    crate::checks::current_scalar_values::CurrentScalarValues::at(
                        program,
                        machine,
                        state,
                        statement_index,
                        Default::default(),
                        None,
                    )
                })
                .flatten();
            Some(Self {
                program,
                semantic,
                contexts: contexts.to_vec(),
                machine,
                state,
                statement_index,
                observations,
                element_observations,
                indexed_observations,
                scalar_values,
            })
        }

        /*
            Resolve only a declared case of this exact applied sum. An enclosing
            inactive payload is not a readable sum, and missing evidence is unknown.
        */
        pub(crate) fn case_truth(
            &self,
            subject: &CanonicalPlace,
            reference: typed_trees::types::TypeReferenceHandle,
            case: &typed_trees::data::DataMemberIdentity,
        ) -> Option<bool> {
            if !self.cases_are_selected(subject) {
                return None;
            }
            let reference =
                validation::field_domains::readable_type_reference(self.program, reference)?;
            let data = self.program.closed_data_definition(reference)?;
            let variant = self.program.data_members(data).iter().find_map(|member| {
                let typed_trees::data::DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.member_identity() == *case).then_some(variant.symbol)
            })?;
            let selected = self.has_value(subject, variant, true);
            let excluded = self.has_value(subject, variant, false);
            match (selected, excluded) {
                (true, false) => Some(true),
                (false, true) => Some(false),
                _ => None,
            }
        }

        pub(crate) fn cases_are_selected(&self, subject: &CanonicalPlace) -> bool {
            self.cases_are_selected_after(subject, 0)
        }

        fn cases_are_selected_after(&self, subject: &CanonicalPlace, prefix_length: usize) -> bool {
            if prefix_length > subject.segments.len() {
                return false;
            }
            subject
                .segments
                .iter()
                .enumerate()
                .skip(prefix_length)
                .all(|(position, segment)| {
                    let PlaceSegment::Case { variant } = segment else {
                        return true;
                    };
                    self.has_value(
                        &CanonicalPlace {
                            root: subject.root,
                            segments: subject.segments[..position].to_vec(),
                        },
                        *variant,
                        true,
                    )
                })
        }

        pub(crate) fn path_is_inactive(&self, subject: &CanonicalPlace) -> bool {
            subject
                .segments
                .iter()
                .enumerate()
                .any(|(position, segment)| {
                    let PlaceSegment::Case { variant } = segment else {
                        return false;
                    };
                    self.has_value(
                        &CanonicalPlace {
                            root: subject.root,
                            segments: subject.segments[..position].to_vec(),
                        },
                        *variant,
                        false,
                    )
                })
        }

        /*
            Exact owned payload namespaces excluded in this immutable scope. The
            same batch supplies linear availability and affine residual geometry;
            a saved reference descriptor never supplies ownership of its referent.
        */
        pub(crate) fn for_each_inactive_owned_case(&self, mut visit: impl FnMut(&CanonicalPlace)) {
            let mut path = CanonicalPlace {
                root: PlaceRoot::Unknown,
                segments: Vec::new(),
            };
            let mut previous = None;
            let mut previous_root = None;
            let mut root_is_plain_without_cleanup = false;
            let mut subject_cleanup_types = Vec::new();
            for observation in &self.observations {
                let subject = &observation.place;
                if previous_root != Some(&subject.root) {
                    let root = CanonicalPlace {
                        root: subject.root,
                        segments: Vec::new(),
                    };
                    root_is_plain_without_cleanup =
                        validation::canonical_owned_place_type_reference(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            &root,
                        )
                        .is_some_and(|reference| {
                            validation::has_plain_contents_without_nominal_cleanup(
                                self.program,
                                reference,
                            )
                        });
                    previous_root = Some(&subject.root);
                }
                if previous == Some(subject) {
                    continue;
                }
                previous = Some(subject);
                if !stable_subject(
                    self.program,
                    self.machine,
                    self.state,
                    self.statement_index,
                    subject,
                ) {
                    continue;
                }
                let Some(reference) = validation::canonical_owned_place_type_reference(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    subject,
                ) else {
                    continue;
                };
                if root_is_plain_without_cleanup {
                    let no_owned_cleanup = if let Some((_, answer)) = subject_cleanup_types
                        .iter()
                        .find(|(retained, _)| *retained == reference)
                    {
                        *answer
                    } else {
                        let answer =
                            validation::has_no_owned_cleanup_obligations(self.program, reference);
                        subject_cleanup_types.push((reference, answer));
                        answer
                    };
                    if no_owned_cleanup {
                        continue;
                    }
                }
                let reference = validation::peel_declared_constraints(self.program, reference);
                let Some(data) = self.program.closed_data_definition(reference).or_else(|| {
                    self.program
                        .typed_program()
                        .data_application_definition(reference)
                }) else {
                    continue;
                };
                let prepared = self.locate_normalized_subject(std::borrow::Cow::Borrowed(subject));
                for member in self.program.data_members(data) {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        continue;
                    };
                    if self.has_value_for_subject(&prepared, variant.symbol, false) {
                        path.root = subject.root;
                        path.segments.clone_from(&subject.segments);
                        path.segments.push(PlaceSegment::Case {
                            variant: variant.symbol,
                        });
                        visit(&path);
                    }
                }
            }
            self.for_each_inactive_element_case(&mut visit);
        }

        /*
            The same exclusions for zero-formed array elements. Every element of
            one `ZeroCaseElements` range has the same type, the same stability and
            the same excluded alternatives, so decide those once on the first
            element and then name each element's namespace.
        */
        fn for_each_inactive_element_case(&self, visit: &mut impl FnMut(&CanonicalPlace)) {
            for row in &self.element_observations {
                if row.first >= row.end {
                    continue;
                }
                let index_position = row.array.segments.len();
                let mut subject = CanonicalPlace {
                    root: row.array.root,
                    segments: row
                        .array
                        .segments
                        .iter()
                        .copied()
                        .chain([PlaceSegment::FixedIndex { index: row.first }])
                        .chain(row.element.iter().copied())
                        .collect(),
                };
                if !stable_subject(
                    self.program,
                    self.machine,
                    self.state,
                    self.statement_index,
                    &subject,
                ) {
                    continue;
                }
                let Some(reference) = validation::canonical_owned_place_type_reference(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    &subject,
                ) else {
                    continue;
                };
                let root_is_plain_without_cleanup =
                    validation::canonical_owned_place_type_reference(
                        self.program,
                        self.state.symbol,
                        self.statement_index,
                        &CanonicalPlace {
                            root: subject.root,
                            segments: Vec::new(),
                        },
                    )
                    .is_some_and(|reference| {
                        validation::has_plain_contents_without_nominal_cleanup(
                            self.program,
                            reference,
                        )
                    });
                if root_is_plain_without_cleanup
                    && validation::has_no_owned_cleanup_obligations(self.program, reference)
                {
                    continue;
                }
                let reference = validation::peel_declared_constraints(self.program, reference);
                let Some(data) = self.program.closed_data_definition(reference).or_else(|| {
                    self.program
                        .typed_program()
                        .data_application_definition(reference)
                }) else {
                    continue;
                };
                let excluded: Vec<SymbolHandle> = self
                    .program
                    .data_members(data)
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol != row.variant
                                && self.program.symbols.get(variant.symbol).parent
                                    == self.program.symbols.get(row.variant).parent =>
                        {
                            Some(variant.symbol)
                        }
                        _ => None,
                    })
                    .collect();
                if excluded.is_empty() {
                    continue;
                }
                let case_position = subject.segments.len();
                subject.segments.push(PlaceSegment::Case {
                    variant: row.variant,
                });
                /*
                    One exclusion names the case for the whole range. Every
                    reader of these paths accepts a range where one element's
                    index would stand, so no element needs its own copy.
                */
                subject.segments[index_position] = PlaceSegment::FixedRange {
                    start: row.first,
                    end: row.end,
                };
                for variant in &excluded {
                    subject.segments[case_position] = PlaceSegment::Case { variant: *variant };
                    visit(&subject);
                }
            }
        }

        fn has_value(&self, subject: &CanonicalPlace, case: SymbolHandle, required: bool) -> bool {
            let stable = stable_subject(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
                subject,
            );
            let mut selected = stable
                && case.is_valid()
                && self.program.symbols.get(case).kind == symbols::SymbolKind::Variant
                && self.has_value_for_subject(
                    &self.prepare_stable_subject(subject),
                    case,
                    required,
                );
            if !selected && stable && case.is_valid() {
                let prepared = self.prepare_stable_subject(subject);
                selected = self.dispatch_copy_carries_case(&prepared.place, case, required);
            }
            if !selected && ::validation::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE current-case machine={:?} state={:?} statement={} subject={subject:?} stable={stable} case={case:?} required={required} observations={:?}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    self.observations
                        .iter()
                        .map(|row| (&row.place, row.variant, row.selected))
                        .collect::<Vec<_>>()
                );
            }
            selected
        }

        /*
            The caller authenticated stability at this evidence batch's exact cut.
            Only the variant changes in this emission walk, so normalize the subject
            and locate its observation ranges once before querying variants.
        */
        fn prepare_stable_subject(&self, subject: &CanonicalPlace) -> PreparedCaseSubject<'static> {
            let subject = observations::normalize_place(
                self.program,
                self.semantic,
                &self.contexts,
                self.state.symbol,
                self.statement_index,
                subject.clone(),
            );
            self.locate_normalized_subject(std::borrow::Cow::Owned(subject))
        }

        /*
            Locate the observation ranges of a subject already in this batch's
            normalized coordinates, such as an observation's own place. Only
            `normalize_place` makes a `NormalizedPlace`, so an observation place is
            borrowed as it is rather than copied and normalized again.
        */
        fn locate_normalized_subject<'subject>(
            &self,
            subject: std::borrow::Cow<'subject, observations::NormalizedPlace>,
        ) -> PreparedCaseSubject<'subject> {
            let exact_start = self
                .observations
                .partition_point(|row| row.place.compare_identity(&subject).is_lt());
            let indexed_start = if observations::has_index(&subject) {
                let root = CanonicalPlace {
                    root: subject.root,
                    segments: Vec::new(),
                };
                self.indexed_observations.partition_point(|position| {
                    CanonicalPlace {
                        root: self.observations[*position].place.root,
                        segments: Vec::new(),
                    }
                    .compare_identity(&root)
                    .is_lt()
                })
            } else {
                self.indexed_observations.len()
            };
            PreparedCaseSubject {
                place: subject,
                exact_start,
                indexed_start,
            }
        }

        fn has_value_for_subject(
            &self,
            prepared: &PreparedCaseSubject<'_>,
            case: SymbolHandle,
            required: bool,
        ) -> bool {
            if !case.is_valid()
                || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant
            {
                return false;
            }
            let subject: &CanonicalPlace = &prepared.place;
            let has_case_evidence = |case: SymbolHandle, required: bool| {
                self.observations[prepared.exact_start..]
                    .iter()
                    .take_while(|row| *row.place == *subject)
                    .chain(
                        self.indexed_observations[prepared.indexed_start..]
                            .iter()
                            .map(|position| &self.observations[*position])
                            .take_while(|row| row.place.root == subject.root)
                            .filter(|row| {
                                *row.place != *subject
                                    && (super::canonical_places_agree(
                                        self.program,
                                        &row.place,
                                        subject,
                                    ) || (row.stable_index_subject
                                        && self.scalar_values.as_ref().is_some_and(|values| {
                                            values.same_scalar_projection(
                                                &row.place.segments,
                                                &subject.segments,
                                            )
                                        })))
                            }),
                    )
                    .any(|row| {
                        self.program.symbols.get(row.variant).parent
                            == self.program.symbols.get(case).parent
                            && if row.selected {
                                (row.variant == case) == required
                            } else {
                                row.variant == case && !required
                            }
                    })
                    || self.element_observations.iter().any(|row| {
                        self.program.symbols.get(row.variant).parent
                            == self.program.symbols.get(case).parent
                            && (row.variant == case) == required
                            && row.covers(subject)
                    })
            };
            self.case_evidence_proves(case, required, has_case_evidence)
        }

        pub(crate) fn has_value_for_call_projection(
            &self,
            prefix: &CanonicalPlace,
            projection: &crate::checks::current_scalar_values::CallArgumentProjection,
            case: SymbolHandle,
            required: bool,
        ) -> bool {
            if !stable_subject(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
                prefix,
            ) {
                if validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE call-case-match stage=unstable-prefix state={:?} statement={}",
                        self.state.symbol, self.statement_index
                    );
                }
                return false;
            }
            let prepared_values = self
                .scalar_values
                .is_none()
                .then(|| {
                    crate::checks::current_scalar_values::CurrentScalarValues::at(
                        self.program,
                        self.machine,
                        self.state,
                        self.statement_index,
                        typed_trees::statement::TransitionTargetHandle::invalid(),
                        None,
                    )
                })
                .flatten();
            let Some(values) = self.scalar_values.as_ref().or(prepared_values.as_ref()) else {
                if validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE call-case-match stage=scalar-inputs state={:?} statement={}",
                        self.state.symbol, self.statement_index
                    );
                }
                return false;
            };
            let mut root_rows = 0_u32;
            let mut stable_rows = 0_u32;
            let mut matched_rows = 0_u32;
            let proven = self.case_evidence_proves(case, required, |case, required| {
                self.observations.iter().any(|row| {
                    row.place.root == prefix.root
                        && {
                            root_rows = root_rows.saturating_add(1);
                            true
                        }
                        && (!observations::has_index(&row.place) || row.stable_index_subject)
                        && {
                            stable_rows = stable_rows.saturating_add(1);
                            true
                        }
                        && values.matches_call_argument_projection(
                            &row.place.segments,
                            &prefix.segments,
                            projection,
                        )
                        && {
                            matched_rows = matched_rows.saturating_add(1);
                            true
                        }
                        && self.program.symbols.get(row.variant).parent
                            == self.program.symbols.get(case).parent
                        && if row.selected {
                            (row.variant == case) == required
                        } else {
                            row.variant == case && !required
                        }
                }) || self.element_observations.iter().any(|row| {
                    /*
                        The projection reads one element. Name the covered
                        element it selects and ask the same question of that
                        element's tagged place.
                    */
                    row.array.root == prefix.root
                        && self.program.symbols.get(row.variant).parent
                            == self.program.symbols.get(case).parent
                        && (row.variant == case) == required
                        && values
                            .call_argument_projection_index(
                                &prefix.segments,
                                projection,
                                row.array.segments.len(),
                            )
                            .filter(|index| row.first <= *index && *index < row.end)
                            .is_some_and(|index| {
                                let tagged: Vec<PlaceSegment> = row
                                    .array
                                    .segments
                                    .iter()
                                    .copied()
                                    .chain([PlaceSegment::FixedIndex { index }])
                                    .chain(row.element.iter().copied())
                                    .collect();
                                values.matches_call_argument_projection(
                                    &tagged,
                                    &prefix.segments,
                                    projection,
                                )
                            })
                })
            });
            if !proven && validation::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE call-case-match machine={:?} state={:?} statement={} root={:?} case={:?} root_rows={} stable_rows={} matched_rows={}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    prefix.root,
                    case,
                    root_rows,
                    stable_rows,
                    matched_rows,
                );
            }
            proven
        }

        fn case_evidence_proves(
            &self,
            case: SymbolHandle,
            required: bool,
            mut has_case_evidence: impl FnMut(SymbolHandle, bool) -> bool,
        ) -> bool {
            if !case.is_valid()
                || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant
            {
                return false;
            }
            if has_case_evidence(case, required) {
                return true;
            }
            if !required || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant {
                return false;
            }
            let Some(definition) = self
                .program
                .data_definition_by_symbol(self.program.symbols.get(case).parent)
            else {
                return false;
            };
            let mut declared = false;
            let mut excluded_alternative = false;
            for member in self.program.data_members(definition) {
                let typed_trees::data::DataMember::Variant(alternative) = member else {
                    continue;
                };
                if alternative.symbol == case {
                    declared = true;
                } else {
                    if !has_case_evidence(alternative.symbol, false) {
                        return false;
                    }
                    excluded_alternative = true;
                }
            }
            declared && excluded_alternative
        }

        /*
            A transition evaluates an owned copy, then proves the case on that copy.
            The payload read still names the initializer. The case carries back when
            the initializer is an immutable binding: nothing can change it after the
            copy, so the copy's tag is the source's tag.
        */
        fn dispatch_copy_carries_case(
            &self,
            subject: &CanonicalPlace,
            case: SymbolHandle,
            required: bool,
        ) -> bool {
            if self.program.symbols.get(case).kind != symbols::SymbolKind::Variant {
                return false;
            }
            let statements = self
                .program
                .statement_table
                .statements(self.state.statement_nodes);
            self.observations.iter().any(|row| {
                if row.variant != case
                    || row.selected != required
                    || row.place.segments != subject.segments
                {
                    return false;
                }
                let PlaceRoot::Symbol(copy) = row.place.root else {
                    return false;
                };
                let Some((local_index, initializer)) =
                    statements
                        .iter()
                        .enumerate()
                        .find_map(|(index, statement)| {
                            if index > self.statement_index {
                                return None;
                            }
                            let local = statement.as_local_data()?;
                            if local.symbol != copy
                                || local.is_mutable
                                || !local.role.has_ordinary_initializer()
                            {
                                return None;
                            }
                            local
                                .ordinary_value_initializer()
                                .map(|value| (index, value))
                        })
                else {
                    return false;
                };
                let Some(source) = crate::published::places::place_at(
                    self.program,
                    self.state.symbol,
                    local_index,
                    initializer,
                ) else {
                    return false;
                };
                if !self.places_name_same_value(local_index, subject, &source) {
                    return false;
                }
                self.place_root_is_immutable(&source)
            })
        }

        fn places_name_same_value(
            &self,
            at: usize,
            subject: &CanonicalPlace,
            source: &CanonicalPlace,
        ) -> bool {
            if super::canonical_places_agree(self.program, subject, source) {
                return true;
            }
            let PlaceRoot::Expression(expression) = subject.root else {
                return false;
            };
            let Some(named) =
                crate::published::places::place_at(self.program, self.state.symbol, at, expression)
            else {
                return false;
            };
            let mut resolved = named;
            resolved.segments.extend_from_slice(&subject.segments);
            super::canonical_places_agree(self.program, &resolved, source)
        }

        fn place_root_is_immutable(&self, place: &CanonicalPlace) -> bool {
            let PlaceRoot::Symbol(symbol) = place.root else {
                return false;
            };
            if self
                .program
                .state_parameters(self.state)
                .iter()
                .any(|parameter| parameter.symbol == symbol && !parameter.is_mutable)
            {
                return true;
            }
            self.program
                .statement_table
                .statements(self.state.statement_nodes)
                .iter()
                .any(|statement| {
                    statement.as_local_data().is_some_and(|local| {
                        local.symbol == symbol
                            && !local.is_mutable
                            && local.role.has_ordinary_initializer()
                    })
                })
        }
    }

    /*
        An inactive case of a zero-formed array is observed once for a range of
        elements: its path spells that range where one element's path spells its
        index. Such a path stands for the same case in every element of the range,
        so it is a prefix of an element's path when the index lies inside it.
        Every reader of inactive-case paths compares through these two.
    */
    fn observed_segment_admits(observed: PlaceSegment, queried: PlaceSegment) -> bool {
        observed == queried
            || matches!(
                (observed, queried),
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedIndex { index },
                ) if start <= index && index < end
            )
    }

    /* The observed case path, or a path below it. */
    pub(crate) fn lies_under_observed_case(
        observed: &[PlaceSegment],
        path: &[PlaceSegment],
    ) -> bool {
        observed.len() <= path.len()
            && observed
                .iter()
                .zip(path)
                .all(|(observed, queried)| observed_segment_admits(*observed, *queried))
    }

    /* A path that contains the observed case path. */
    pub(crate) fn contains_observed_case(path: &[PlaceSegment], observed: &[PlaceSegment]) -> bool {
        path.len() <= observed.len()
            && observed
                .iter()
                .zip(path)
                .all(|(observed, queried)| observed_segment_admits(*observed, *queried))
    }

    /*
        One `ZeroCaseElements` fact read below a storage prefix, for the transfers
        that carry every observation under a moved or copied value. The fact
        stands for a selected case at `array[index].element` for each covered
        index, so a prefix that holds the whole array carries the fact itself and
        a prefix inside one covered element carries that element's single tag.
    */
    pub(crate) enum ZeroCaseBelow {
        /* The array's path below the prefix. */
        Elements { array: Vec<PlaceSegment> },
        /* The tagged place's path below the prefix, inside one element. */
        Element { place: Vec<PlaceSegment> },
    }

    pub(crate) fn zero_case_below(
        program: &dyn validation::CheckingInput,
        array: &[PlaceSegment],
        elements: std::ops::Range<usize>,
        element: &[PlaceSegment],
        prefix: &[PlaceSegment],
    ) -> Option<ZeroCaseBelow> {
        if let Some(array) = array.strip_prefix(prefix) {
            return Some(ZeroCaseBelow::Elements {
                array: array.to_vec(),
            });
        }
        let (selector, inside) = prefix.strip_prefix(array)?.split_first()?;
        let index = match *selector {
            PlaceSegment::FixedIndex { index } => index,
            PlaceSegment::Index { expression }
                if program.expression_table.expression_is_valid(expression) =>
            {
                crate::flow::expressions(program)
                    .constant_integer_value(expression)
                    .and_then(|value| usize::try_from(value).ok())?
            }
            _ => return None,
        };
        if !elements.contains(&index) {
            return None;
        }
        Some(ZeroCaseBelow::Element {
            place: element.strip_prefix(inside)?.to_vec(),
        })
    }

    /*
        The case a `ZeroCaseElements` fact selects at exactly `subject`, for the
        readers that look for one place's own tag observation.
    */
    pub(crate) fn zero_case_at(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        fact: &facts::Fact,
        subject: &CanonicalPlace,
    ) -> Option<SymbolHandle> {
        let facts::FactPayload::ZeroCaseElements {
            first,
            end,
            element,
            variant,
        } = fact.payload
        else {
            return None;
        };
        let array = canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(fact.place.as_place()?),
        )?;
        if array.root != subject.root {
            return None;
        }
        match zero_case_below(
            program,
            &array.segments,
            first..end,
            semantic.place_segments.span_or_empty(element),
            &subject.segments,
        )? {
            ZeroCaseBelow::Element { place } if place.is_empty() => Some(variant),
            _ => None,
        }
    }

    pub(crate) fn place_cases_are_selected(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        subject: &CanonicalPlace,
    ) -> bool {
        place_cases_are_selected_after(
            program,
            semantic,
            contexts,
            machine_symbol,
            state_symbol,
            statement_index,
            subject,
            0,
        )
    }

    /*
        A saved descriptor authenticates only its own namespace prefix. Case
        segments reached below that descriptor still require live tag evidence.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_cases_are_selected_after(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        subject: &CanonicalPlace,
        prefix_length: usize,
    ) -> bool {
        if prefix_length > subject.segments.len() {
            return false;
        }
        if !subject.segments[prefix_length..]
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        {
            return true;
        }
        CaseEvidence::prepare(
            program,
            semantic,
            contexts,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        .is_some_and(|evidence| evidence.cases_are_selected_after(subject, prefix_length))
    }

    /*
        A whole value owes a payload invariant only when its enclosing cases are
        active. Contrary tag evidence discharges the conditional obligation, but
        never authorizes reading that payload.
    */
    pub(crate) fn place_case_path_is_inactive(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        subject: &CanonicalPlace,
    ) -> bool {
        if !subject
            .segments
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        {
            return false;
        }
        CaseEvidence::prepare(
            program,
            semantic,
            contexts,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        .is_some_and(|evidence| evidence.path_is_inactive(subject))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_case_has_value(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        subject: &CanonicalPlace,
        case: SymbolHandle,
        required: bool,
    ) -> bool {
        CaseEvidence::prepare(
            program,
            semantic,
            contexts,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        .is_some_and(|evidence| evidence.has_value(subject, case, required))
    }

    pub(crate) fn stable_subject(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        subject: &CanonicalPlace,
    ) -> bool {
        let scalar_values = std::cell::OnceCell::new();
        let stable_root = match subject.root {
            PlaceRoot::Symbol(symbol) => symbol.is_valid(),
            PlaceRoot::Expression(expression) => expression.is_valid(),
            PlaceRoot::InvocationReferent { parameter, .. } => {
                super::super::entry_origins::invocation_referent(program, machine, parameter)
                    == Some(subject.root)
            }
            _ => false,
        };
        stable_root
            && subject.segments.iter().all(|segment| {
                matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                    || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                    || matches!(segment, PlaceSegment::FixedIndex { .. })
                    || matches!(segment, PlaceSegment::Index { expression }
                    if validation::place_has_immutable_denotation(
                        program, machine, state, statement_index, *expression,
                    ) || scalar_values.get_or_init(|| {
                        crate::checks::current_scalar_values::CurrentScalarValues::at(
                            program, machine, state, statement_index, Default::default(), None,
                        )
                    }).as_ref().is_some_and(|values| {
                        values.selector_has_immutable_denotation(*expression)
                    }))
            })
    }

    #[cfg(test)]
    mod zero_case_below_tests {
        use super::{ZeroCaseBelow, zero_case_below};
        use facts::PlaceSegment;

        fn field(index: u32) -> PlaceSegment {
            PlaceSegment::Field {
                symbol: symbols::SymbolHandle::from_parts(index, 1),
            }
        }

        fn at(index: usize) -> PlaceSegment {
            PlaceSegment::FixedIndex { index }
        }

        /* `owner.table[2..6]` tagged at `.slot.kind` inside each element. */
        fn below(prefix: &[PlaceSegment]) -> Option<(bool, Vec<PlaceSegment>)> {
            let program = typed_trees::TypedTrees::default();
            zero_case_below(
                &program,
                &[field(1), field(2)],
                2..6,
                &[field(3), field(4)],
                prefix,
            )
            .map(|below| match below {
                ZeroCaseBelow::Elements { array } => (true, array),
                ZeroCaseBelow::Element { place } => (false, place),
            })
        }

        #[test]
        fn a_prefix_holding_the_array_carries_the_whole_fact() {
            assert_eq!(below(&[]), Some((true, vec![field(1), field(2)])));
            assert_eq!(below(&[field(1)]), Some((true, vec![field(2)])));
            assert_eq!(below(&[field(1), field(2)]), Some((true, vec![])));
        }

        #[test]
        fn a_prefix_inside_one_covered_element_carries_that_tag() {
            assert_eq!(
                below(&[field(1), field(2), at(2)]),
                Some((false, vec![field(3), field(4)]))
            );
            assert_eq!(
                below(&[field(1), field(2), at(5), field(3)]),
                Some((false, vec![field(4)]))
            );
            assert_eq!(
                below(&[field(1), field(2), at(5), field(3), field(4)]),
                Some((false, vec![]))
            );
        }

        #[test]
        fn other_storage_carries_nothing() {
            assert_eq!(below(&[field(1), field(2), at(6)]), None);
            assert_eq!(below(&[field(1), field(2), at(1)]), None);
            assert_eq!(below(&[field(1), field(2), at(3), field(9)]), None);
            assert_eq!(below(&[field(1), field(9)]), None);
            assert_eq!(
                below(&[
                    field(1),
                    field(2),
                    PlaceSegment::FixedRange { start: 2, end: 6 }
                ]),
                None
            );
            assert_eq!(
                below(&[field(1), field(2), at(3), field(3), field(4), field(5)]),
                None
            );
        }
    }
}
mod coverage {
    /*
        Shared structural extent coverage.
    */
    pub(crate) use validation::membership_evidence::segments_cover_subject;
}
mod domain_membership {
    /*
        Shared immutable live membership evidence.
    */
    pub(crate) use validation::membership_evidence::MembershipEvidence;
}
mod literal_projection {
    /*
        Project a callee access route through constructors, without replacing a
        caller binding by its captured storage origin. Constructors have no caller
        address; their actual field/element expressions supply the access routes.
        Common fields use Field; selected payload fields use Case/Field. Both
        belong to one authored constructor roster, and resolved symbols join each
        operand to its declaration without letting a common field select a case.
    */
    use crate::published::places::place_at;
    use facts::PlaceSegment;
    use facts::payload_variant_for_field;
    use validation::CanonicalPlace;

    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::DataMember;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn literal_argument_access_places(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        segments: &[PlaceSegment],
    ) -> Option<Vec<CanonicalPlace>> {
        literal_projections(
            program,
            expression,
            reference,
            segments,
            ProjectionMode::Access,
        )?
        .into_iter()
        .map(|projection| {
            let mut place = place_at(
                program,
                state_symbol,
                statement_index,
                projection.expression()?,
            )?;
            place.segments.extend_from_slice(&projection.remaining);
            Some(place)
        })
        .collect()
    }

    mod zero_values {
        /*
            Exact owned zero formation under an actually omitted constructor field.
            Type paths select zero storage, never a referent or an unchosen case.
        */
        use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

        pub(super) fn selected_type(
            program: &dyn ::validation::CheckingInput,
            mut reference: TypeReferenceHandle,
            segments: &[facts::PlaceSegment],
        ) -> Option<TypeReferenceHandle> {
            for segment in segments {
                reference = validation::peel_declared_constraints(program, reference);
                if matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Reference { .. }
                ) {
                    return None;
                }
                reference = match (
                    segment,
                    program.type_reference_table.type_reference(reference),
                ) {
                    (facts::PlaceSegment::Field { symbol }, _) => {
                        let definition = validation::field_domains::data_definition_for_field_type(
                            program, reference,
                        )?;
                        if facts::payload_variant_for_field(program, *symbol).is_some() {
                            return None;
                        }
                        program
                            .closed_data_constructor_field(reference, definition.symbol, *symbol)?
                            .type_reference
                    }
                    (
                        facts::PlaceSegment::FixedIndex { index },
                        TypeReferenceNode::FixedArray {
                            element_type,
                            length: FixedArrayLength::Literal(length),
                        },
                    ) if *index < *length => *element_type,
                    _ => return None,
                };
            }
            let carrier = validation::peel_declared_constraints(program, reference);
            if matches!(
                program.type_reference_table.type_reference(carrier),
                TypeReferenceNode::Reference { .. }
            ) {
                return None;
            }
            Some(reference)
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum LiteralValueSource {
        Expression(ExpressionHandle),
        OmittedZero {
            type_reference: TypeReferenceHandle,
            /*
                The evaluated constructor owns this inline zero value. It is not
                a separately authored operand and supplies no borrowed address.
            */
            constructor: ExpressionHandle,
            constructor_path_length: usize,
        },
    }

    #[derive(Clone)]
    pub(crate) struct LiteralValueProjection {
        pub source: LiteralValueSource,
        pub remaining: Vec<PlaceSegment>,
        pub destination: Vec<PlaceSegment>,
    }

    impl LiteralValueProjection {
        fn expression(&self) -> Option<ExpressionHandle> {
            match self.source {
                LiteralValueSource::Expression(expression) => Some(expression),
                LiteralValueSource::OmittedZero { .. } => None,
            }
        }
    }

    /*
        Authored operands have evaluation frontiers and may name storage. Implicit
        zero formation has neither; expression/custody clients must fail closed.
    */
    #[derive(Clone)]
    pub(crate) struct LiteralExpressionProjection {
        pub expression: ExpressionHandle,
        pub remaining: Vec<PlaceSegment>,
        pub destination: Vec<PlaceSegment>,
    }

    pub(crate) fn literal_expression_projections(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        segments: &[PlaceSegment],
        enumerate: bool,
    ) -> Option<Vec<LiteralExpressionProjection>> {
        literal_value_projections(program, expression, reference, segments, enumerate)?
            .into_iter()
            .map(|projection| {
                Some(LiteralExpressionProjection {
                    expression: projection.expression()?,
                    remaining: projection.remaining,
                    destination: projection.destination,
                })
            })
            .collect()
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ProjectionMode {
        Access,
        SelectedValue,
        ConstructedValues,
    }

    /*
        Only an exact constructed alternative makes a payload contract inapplicable.
        Unknown stored values and malformed projections remain obligations. Resolve
        nested fields/array elements through the same projection owner as accesses;
        never replay a local initializer to discover its former tag.
    */
    pub(crate) fn literal_value_path_is_inactive(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        segments: &[PlaceSegment],
    ) -> bool {
        segments.iter().enumerate().any(|(position, segment)| {
            let PlaceSegment::Case { variant } = segment else {
                return false;
            };
            let Some(projections) = literal_expression_projections(
                program,
                expression,
                reference,
                &segments[..position],
                false,
            ) else {
                return false;
            };
            let receiving = validation::project_type_reference_from_segments(
                program,
                reference,
                &segments[..position],
            );
            let selected = |owner: SymbolHandle, authored: SymbolHandle| {
                receiving
                    .and_then(|receiving| program.data_constructor_case(receiving, owner, authored))
                    .map_or(authored, |case| case.symbol)
            };
            !projections.is_empty()
                && projections.iter().all(|projection| {
                    if !projection.remaining.is_empty() {
                        return false;
                    }
                    let actual = match program.expression_table.expression(projection.expression) {
                        ExpressionNode::StructLiteral(literal) => literal
                            .case_symbol
                            .filter(|symbol| {
                                symbol.is_valid()
                                    && program.symbols.get(*symbol).parent == literal.type_symbol
                            })
                            .map(|symbol| selected(literal.type_symbol, symbol)),
                        ExpressionNode::Name(name)
                            if program.symbols.get(name.symbol).kind == SymbolKind::Variant =>
                        {
                            Some(selected(
                                program.symbols.get(name.symbol).parent,
                                name.symbol,
                            ))
                        }
                        _ => None,
                    };
                    actual.is_some_and(|actual| {
                        actual != *variant
                            && variant.is_valid()
                            && program.symbols.get(actual).kind == SymbolKind::Variant
                            && program.symbols.get(*variant).kind == SymbolKind::Variant
                            && program.symbols.get(actual).parent
                                == program.symbols.get(*variant).parent
                    })
                })
        })
    }

    pub(crate) fn literal_value_projections(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        segments: &[PlaceSegment],
        enumerate: bool,
    ) -> Option<Vec<LiteralValueProjection>> {
        literal_projections(
            program,
            expression,
            reference,
            segments,
            if enumerate {
                ProjectionMode::ConstructedValues
            } else {
                ProjectionMode::SelectedValue
            },
        )
    }

    fn literal_projections(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        segments: &[PlaceSegment],
        mode: ProjectionMode,
    ) -> Option<Vec<LiteralValueProjection>> {
        let mut pending = vec![(expression, reference, segments, Vec::new())];
        let mut places = Vec::new();
        while let Some((expression, reference, segments, destination)) = pending.pop() {
            if !reference.is_valid() {
                return None;
            }
            if let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                pending.push((expression, *base_type, segments, destination));
                continue;
            }
            if mode == ProjectionMode::SelectedValue && segments.is_empty() {
                places.push(LiteralValueProjection {
                    source: LiteralValueSource::Expression(expression),
                    remaining: Vec::new(),
                    destination,
                });
                continue;
            }
            if program.primitive_type_reference(reference).is_some()
                || matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Unit
                )
            {
                if !segments.is_empty() {
                    return None;
                }
                if mode != ProjectionMode::Access {
                    places.push(LiteralValueProjection {
                        source: LiteralValueSource::Expression(expression),
                        remaining: Vec::new(),
                        destination,
                    });
                }
                continue;
            }
            if mode == ProjectionMode::ConstructedValues
                && segments.is_empty()
                && matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::ArrayLiteral(_)
                )
                && crate::values::is_assignment_literal(program, expression)
            {
                places.push(LiteralValueProjection {
                    source: LiteralValueSource::Expression(expression),
                    remaining: Vec::new(),
                    destination: destination.clone(),
                });
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => {
                    let definition =
                        program.data_constructor_definition(reference, literal.type_symbol)?;
                    let owner = definition.symbol;
                    let members = program.data_members(definition);
                    let variant = match literal.case_symbol {
                        Some(symbol) => Some(members.iter().find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.symbol == symbol
                                    && symbol.is_valid()
                                    && program.symbols.get(symbol).kind == SymbolKind::Variant
                                    && program.symbols.get(symbol).parent == owner =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })?),
                        None if members
                            .iter()
                            .any(|member| matches!(member, DataMember::Variant(_))) =>
                        {
                            return None;
                        }
                        None => None,
                    };
                    let fields = program.data_construction_fields(members, literal.case_symbol);
                    let (selected_variant, segments) = match segments.split_first() {
                        Some((PlaceSegment::Case { variant: selected }, remaining)) => {
                            if variant.is_none_or(|variant| variant.symbol != *selected) {
                                return None;
                            }
                            (Some(*selected), remaining)
                        }
                        _ => (None, segments),
                    };
                    let actuals = program.expression_table.struct_fields(literal.fields);
                    let selected = match segments.split_first() {
                        Some((PlaceSegment::Field { symbol }, remaining)) => {
                            Some((*symbol, remaining))
                        }
                        None => None,
                        _ => return None,
                    };
                    if selected.is_some_and(|(symbol, _)| {
                        !fields.iter().any(|field| field.symbol == symbol)
                            || payload_variant_for_field(program, symbol) != selected_variant
                    }) {
                        return None;
                    }
                    let mut mapped_actuals = Vec::with_capacity(actuals.len());
                    for actual in actuals {
                        let field = program.data_constructor_field(
                            reference,
                            literal.type_symbol,
                            actual.field_symbol,
                        )?;
                        if !fields
                            .iter()
                            .any(|candidate| candidate.symbol == field.symbol)
                            || mapped_actuals
                                .iter()
                                .any(|(symbol, _)| *symbol == field.symbol)
                            || program.symbols.get(field.symbol).kind != SymbolKind::Field
                        {
                            return None;
                        }
                        mapped_actuals.push((field.symbol, actual));
                    }
                    for field in fields {
                        if selected_variant.is_some_and(|variant| {
                            payload_variant_for_field(program, field.symbol) != Some(variant)
                        }) {
                            continue;
                        }
                        if selected.is_some_and(|(symbol, _)| symbol != field.symbol) {
                            continue;
                        }
                        let Some((_, actual)) = mapped_actuals
                            .iter()
                            .find(|(symbol, _)| *symbol == field.symbol)
                        else {
                            if mode == ProjectionMode::SelectedValue && selected.is_some() {
                                let remaining =
                                    selected.map_or(&[][..], |(_, remaining)| remaining);
                                let type_reference = zero_values::selected_type(
                                    program,
                                    field.type_reference,
                                    remaining,
                                )?;
                                let constructor_path_length = destination.len();
                                let mut destination = destination.clone();
                                if let Some(variant) =
                                    payload_variant_for_field(program, field.symbol)
                                {
                                    destination.push(PlaceSegment::Case { variant });
                                }
                                destination.push(PlaceSegment::Field {
                                    symbol: field.symbol,
                                });
                                destination.extend_from_slice(remaining);
                                places.push(LiteralValueProjection {
                                    source: LiteralValueSource::OmittedZero {
                                        type_reference,
                                        constructor: expression,
                                        constructor_path_length,
                                    },
                                    remaining: Vec::new(),
                                    destination,
                                });
                                continue;
                            }
                            if mode == ProjectionMode::ConstructedValues && selected.is_none() {
                                continue;
                            }
                            if program
                                .primitive_type_reference(field.type_reference)
                                .is_some()
                                && selected.is_none_or(|(_, remaining)| remaining.is_empty())
                            {
                                continue;
                            }
                            return None;
                        };
                        let mut destination = destination.clone();
                        if let Some(variant) = payload_variant_for_field(program, field.symbol) {
                            destination.push(PlaceSegment::Case { variant });
                        }
                        destination.push(PlaceSegment::Field {
                            symbol: field.symbol,
                        });
                        pending.push((
                            actual.value,
                            field.type_reference,
                            selected.map_or(&[][..], |(_, remaining)| remaining),
                            destination,
                        ));
                    }
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    let TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    } = program.type_reference_table.type_reference(reference)
                    else {
                        return None;
                    };
                    let elements = program.expression_table.expression_handles(*elements);
                    if elements.len() != *length {
                        return None;
                    }
                    match segments.split_first() {
                        None => {
                            for (index, element) in elements.iter().enumerate() {
                                let mut destination = destination.clone();
                                destination.push(PlaceSegment::FixedIndex { index });
                                pending.push((*element, *element_type, &[], destination));
                            }
                        }
                        Some((PlaceSegment::FixedIndex { index }, remaining)) => {
                            let mut destination = destination;
                            destination.push(PlaceSegment::FixedIndex { index: *index });
                            pending.push((
                                *elements.get(*index)?,
                                *element_type,
                                remaining,
                                destination,
                            ));
                        }
                        Some((PlaceSegment::Index { .. }, remaining)) => {
                            for (index, element) in elements.iter().enumerate() {
                                let mut destination = destination.clone();
                                destination.push(PlaceSegment::FixedIndex { index });
                                pending.push((*element, *element_type, remaining, destination));
                            }
                        }
                        Some((PlaceSegment::FixedRange { start, end }, remaining))
                            if start <= end && *end <= elements.len() =>
                        {
                            for (offset, element) in elements[*start..*end].iter().enumerate() {
                                let mut destination = destination.clone();
                                destination.push(PlaceSegment::FixedIndex {
                                    index: start + offset,
                                });
                                pending.push((*element, *element_type, remaining, destination));
                            }
                        }
                        _ => return None,
                    }
                }
                _ => {
                    places.push(LiteralValueProjection {
                        source: LiteralValueSource::Expression(expression),
                        remaining: segments.to_vec(),
                        destination,
                    });
                }
            }
        }
        Some(places)
    }
}

pub(crate) use coverage::segments_cover_subject;
pub(crate) use domain_membership::MembershipEvidence;
use facts::PlaceSegment;
pub(crate) use facts::compose_captured_view_segments;

pub(crate) use canonicalization::{
    canonical_place_from_semantic_place, canonical_place_from_symbol, narrow_pinned_index_segments,
    narrow_pinned_index_segments_with_values, projected_leaf_place, reference_source_place,
    source_independent_place,
};
pub(crate) use case_access::{
    CaseEvidence, ZeroCaseBelow, contains_observed_case, lies_under_observed_case,
    place_case_has_value, place_case_path_is_inactive, place_cases_are_selected,
    place_cases_are_selected_after, stable_subject as stable_case_subject, zero_case_at,
    zero_case_below,
};
pub(crate) use literal_projection::{
    LiteralExpressionProjection, LiteralValueSource, literal_argument_access_places,
    literal_expression_projections, literal_value_path_is_inactive, literal_value_projections,
};
pub(crate) use validation::storage_path_relations::{
    canonical_place_joined_segments_may_overlap, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, canonical_places_agree, case_tag_segments_may_overlap,
    place_segment_has_static_identity, place_segment_has_unresolved_identity,
    static_place_segment_pair_equal,
};

/*
    A segment that selects a fixed coordinate: a field, a selected case, or a
    statically known fixed-array element. A runtime `Index`, a window, and an
    opaque selector stay variable, so a path through them names no single
    storage.
*/
pub(crate) fn fixed_selection_segment(segment: &PlaceSegment) -> bool {
    matches!(
        segment,
        PlaceSegment::Field { .. } | PlaceSegment::Case { .. } | PlaceSegment::FixedIndex { .. }
    )
}
