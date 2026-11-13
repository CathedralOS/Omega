/*
    Value facts and scalar plans for the check pass.

    `build_value_facts` records a `CheckedValueFact` (type reference, primitive
    type, and an integer range where one is known) for each expression it visits:
    each executable occurrence visited by validation. That shared semantic owner
    chooses expression roles and traversal order without producing proof facts;
    this owner attaches the checked type and range evidence.

    `scalar` builds the checked scalar expression plans, which the check pass
    builds before flow, and the value computation plans, which it builds after
    flow. `evaluation` evaluates selected scalar operations, `snapshots` reads
    the current assigned value, literal, or integer bounds at a place, and
    `place_intervals` serves those live facts to validation's interval domain
    (`validation::selected_scalar_interval`), which owns the arithmetic.
*/

use checked_trees::{
    CheckedIntegerRange, CheckedValueFact, CheckedValueFacts, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use proof::obligations::ProofPlan;

mod captured_call_intervals {
    /*
        Check integer arguments using their values when they were evaluated.
        Copies keep the bounds known at that time; references use the storage
        observations valid at the call. Every possible value contributes to the range.
    */
    use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
    use facts::{FactPlan, IntegerRange};
    use symbols::SymbolHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::PrimitiveType;

    pub(crate) struct CapturedCallIntervals<'a> {
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
        parameters: &'a [StateParameter],
        arguments: &'a [FlowOperandFact],
        epoch: &'a FlowOperandFact,
    }

    impl<'a> CapturedCallIntervals<'a> {
        pub(crate) fn new(
            program: &'a dyn validation::CheckingInput,
            operators: &'a checked_trees::CheckedOperatorFacts,
            semantic: &'a FactPlan,
            flow: &'a FlowFacts,
            state: &'a FlowStateFact,
            call: &'a FlowCallFact,
            parameters: &'a [StateParameter],
        ) -> Option<Self> {
            if !call.authored_source_custody_valid {
                return None;
            }
            let (operands, receiver_offset) =
                crate::flow::captured_call_operands(program, flow.view(), state, call)?;
            let arguments = crate::shared::call_formals::arguments_aligned_to_formals(
                program,
                call.target_symbol,
                parameters.len(),
                operands.get(receiver_offset..)?,
            );
            if arguments.len() != parameters.len() {
                return None;
            }
            Some(Self {
                program,
                operators,
                semantic,
                flow,
                state,
                call,
                parameters,
                arguments,
                epoch: operands.last()?,
            })
        }

        fn parameter_bounds(
            &self,
            ordinal: usize,
            path: &[facts::PlaceSegment],
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            let parameter = self.parameters.get(ordinal)?;
            let operand = self.arguments.get(ordinal)?;
            super::CapturedIntegerIntervals::new(
                self.program,
                self.operators,
                self.semantic,
                self.flow,
                self.state,
            )
            .bounds(
                crate::flow::CapturedOperand::unrecorded(operand),
                parameter.type_reference,
                path,
                self.call.entry_constraints,
                self.epoch,
                primitive,
            )
        }
    }

    impl validation::SelectedScalarIntervalSource for CapturedCallIntervals<'_> {
        fn binding(&mut self, position: usize, primitive: PrimitiveType) -> Option<IntegerRange> {
            let (ordinal, parameter) = self
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| super::occupies_scalar_position(self.program, parameter))
                .nth(position)?;
            if self
                .program
                .primitive_type_reference(parameter.type_reference)
                != Some(primitive)
            {
                return None;
            }
            self.parameter_bounds(ordinal, &[], primitive)
        }

        fn storage(
            &mut self,
            _symbol: SymbolHandle,
            _primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            None
        }

        fn structural_field(
            &mut self,
            position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Option<IntegerRange> {
            let (_, segments, reference, _) = super::resolve_structural_parameter_path(
                self.program,
                self.parameters,
                position,
                path,
            )?;
            let primitive = self.program.primitive_type_reference(reference)?;
            self.parameter_bounds(position as usize, &segments, primitive)
        }

        fn indexed_read(
            &mut self,
            _root: checked_trees::CheckedStorageRoot,
            _path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            _index: &checked_trees::CheckedScalarSelector,
            _element_path: &[checked_trees::CheckedScalarReadProjection],
            _primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            None
        }
    }
}
mod captured_intervals {
    /*
        Integer consequences of one completed operand and all its retained alternatives.
        The value's own observation and producer epochs remain separate; neither
        rereads a source initializer or imports facts established after completion.
    */
    use checked_trees::{FlowConstraintRef, FlowFacts, FlowOperandFact, FlowStateFact};
    use facts::{FactPlan, IntegerRange};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    pub(crate) struct CapturedIntegerIntervals<'a> {
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
        state: &'a FlowStateFact,
    }

    impl<'a> CapturedIntegerIntervals<'a> {
        pub(crate) fn new(
            program: &'a dyn validation::CheckingInput,
            operators: &'a checked_trees::CheckedOperatorFacts,
            semantic: &'a FactPlan,
            flow: &'a FlowFacts,
            state: &'a FlowStateFact,
        ) -> Self {
            Self {
                program,
                operators,
                semantic,
                flow,
                state,
            }
        }

        pub(crate) fn bounds(
            &self,
            operand: crate::flow::CapturedOperand<'_>,
            reference: TypeReferenceHandle,
            path: &[facts::PlaceSegment],
            invocation: arena::HandleSpan<FlowConstraintRef>,
            epoch: &FlowOperandFact,
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            let values = crate::flow::captured_operand_values(
                self.program,
                self.flow.view(),
                self.semantic,
                self.state,
                operand,
                reference,
                path,
            )?;
            let values = crate::flow::observe_captured_values(
                self.program,
                self.flow,
                self.semantic,
                self.state,
                values,
                epoch,
            );
            let mut hull: Option<IntegerRange> = None;
            for value in values {
                let mut known: Option<IntegerRange> = None;
                for (capture, reference, projection, places, invocation) in [
                    (
                        value.observation,
                        value.observation_type,
                        &value.observation_projection,
                        crate::flow::captured_observation_places(
                            self.program,
                            self.flow.view(),
                            &value,
                        ),
                        invocation,
                    ),
                    (
                        value.operand,
                        value.type_reference,
                        &value.projection,
                        crate::flow::captured_value_places(self.program, self.flow.view(), &value),
                        value.producer_invocation(invocation),
                    ),
                ] {
                    let contexts = crate::flow::operand_contexts(
                        self.program,
                        &self.flow.contexts,
                        &self.flow.control,
                        self.semantic,
                        capture.row(),
                        reference,
                        invocation,
                    );
                    let literal = projection
                        .remaining
                        .is_empty()
                        .then(|| {
                            let expression = capture.row().expression;
                            let integer =
                                match self.program.expression_table.expression(expression) {
                                    typed_trees::expression::ExpressionNode::Integer(literal) => {
                                        literal.value_bignum()
                                    }
                                    _ => None,
                                }
                                .or_else(|| {
                                    validation::completed_integer_constant(self.program, expression)
                                        .map(|constant| constant.value)
                                });
                            integer.map(|value| IntegerRange {
                                minimum: value.clone(),
                                maximum: value,
                            })
                        })
                        .flatten();
                    let bounds = literal.or_else(|| {
                        let places = places?;
                        if places.is_empty() {
                            return None;
                        }
                        let mut current = super::CurrentIntegerBounds::at_statement(
                            self.program,
                            self.operators,
                            self.semantic,
                            &contexts,
                            capture.row().state_symbol,
                            capture.row().statement_index,
                        )?;
                        let mut range: Option<IntegerRange> = None;
                        for place in places {
                            let next = current.at_place(&place.projected, primitive)?;
                            range = Some(match range {
                                Some(previous) => IntegerRange {
                                    minimum: previous.minimum.min(next.minimum),
                                    maximum: previous.maximum.max(next.maximum),
                                },
                                None => next,
                            });
                        }
                        range
                    });
                    if let Some(bounds) = bounds {
                        known = Some(match known {
                            Some(previous) => IntegerRange {
                                minimum: previous.minimum.max(bounds.minimum),
                                maximum: previous.maximum.min(bounds.maximum),
                            },
                            None => bounds,
                        });
                    }
                }
                let bounds = known?;
                if bounds.minimum > bounds.maximum {
                    return None;
                }
                hull = Some(match hull {
                    Some(previous) => IntegerRange {
                        minimum: previous.minimum.min(bounds.minimum),
                        maximum: previous.maximum.max(bounds.maximum),
                    },
                    None => bounds,
                });
            }
            hull
        }
    }
}
mod current_integer_bounds {
    /*
        Contents bounds at one completed read cut. Geometry comes from the declared
        collection, never from whichever element facts survived. Every possible
        element contributes to the hull; a missing element contributes its carrier.
        These consequences do not grant initialization, index formation or access.
    */
    use super::scalar::structural_fields::resolve_structural_path;
    use super::snapshots::RootAssignmentObservations;
    use crate::published::facts::boolean_predicates::{self, BoundPredicate};
    use facts::{FactContextHandle, FactPlan, IntegerRange, PlaceSegment, ProgramPoint};
    use symbols::SymbolHandle;
    use typed_trees::types::PrimitiveType;
    use validation::CanonicalPlace;

    pub(crate) struct CurrentIntegerBounds<'a> {
        program: &'a dyn validation::CheckingInput,
        semantic: &'a FactPlan,
        contexts: &'a [FactContextHandle],
        state: SymbolHandle,
        operators: &'a checked_trees::CheckedOperatorFacts,
        point: ProgramPoint,
        /*
            The retained rows each queried root consults, built on its first query:
            a collection's per-element membership rows are not rebuilt for a query
            about an unrelated scalar, nor for a bounds object that is never asked.
        */
        predicates: Vec<RootPredicates>,
        /*
            Repeated element and predicate reads share the exact selected root's
            assignments. The borrowed fact plan cannot change within this read.
        */
        assignments: RootAssignmentObservations<'a>,
    }

    /*
        One root's retained predicates, indexed by the places they bind. A
        predicate with no binding at a queried place yields that place's carrier or
        nothing, so neither narrows its bounds; an element read of a large
        collection consults only the predicates naming that element.
    */
    struct RootPredicates {
        root: facts::PlaceRoot,
        rows: Vec<BoundPredicate>,
        /* `(predicate, binding)` positions ordered by the binding's place. */
        by_place: Vec<(usize, usize)>,
    }

    pub(crate) enum IntegerReadStep {
        Static(checked_trees::CheckedStructuralPredicatePathSegment),
        Index(Option<IntegerRange>),
    }

    impl<'a> CurrentIntegerBounds<'a> {
        pub(crate) fn at_statement(
            program: &'a dyn validation::CheckingInput,
            operators: &'a checked_trees::CheckedOperatorFacts,
            semantic: &'a FactPlan,
            contexts: &'a [FactContextHandle],
            state: SymbolHandle,
            statement_index: usize,
        ) -> Option<Self> {
            let (machine, _) = validation::find_state(program, state)?;
            Some(Self::new(
                program,
                operators,
                semantic,
                contexts,
                state,
                ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: state,
                    statement_index,
                },
            ))
        }
        pub(crate) fn selected_read(
            &mut self,
            root: SymbolHandle,
            reference: typed_trees::types::TypeReferenceHandle,
            mutable: bool,
            steps: &[IntegerReadStep],
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            self.selected_path(
                root,
                reference,
                mutable,
                steps,
                &mut Vec::new(),
                primitive,
                &mut 4096,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn selected_path(
            &mut self,
            root: SymbolHandle,
            reference: typed_trees::types::TypeReferenceHandle,
            mutable: bool,
            steps: &[IntegerReadStep],
            path: &mut Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
            primitive: PrimitiveType,
            budget: &mut usize,
        ) -> Option<IntegerRange> {
            *budget = budget.checked_sub(1)?;
            let Some((step, tail)) = steps.split_first() else {
                let (segments, selected, _) = resolve_structural_path(
                    self.program,
                    reference,
                    mutable,
                    path,
                    &mut |_, _| Some(()),
                )?;
                if self.program.primitive_type_reference(selected) != Some(primitive) {
                    return None;
                }
                return self.exact(
                    &CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    },
                    primitive,
                );
            };
            match step {
                IntegerReadStep::Static(step) => {
                    path.push(step.clone());
                    let result =
                        self.selected_path(root, reference, mutable, tail, path, primitive, budget);
                    path.pop();
                    result
                }
                IntegerReadStep::Index(selector) => {
                    let (segments, _, _) = resolve_structural_path(
                        self.program,
                        reference,
                        mutable,
                        path,
                        &mut |_, _| Some(()),
                    )?;
                    let collection = CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    };
                    let extent = validation::closed_collection_length_at_place(
                        self.program,
                        self.state,
                        &collection,
                    )?;
                    let last = extent.checked_sub(1)?;
                    let zero = numerics::bignum::BigInt::from_u64(0);
                    let last_value = numerics::bignum::BigInt::from_u64(last);
                    let (first, last) = match selector {
                        Some(selector) => (
                            selector.minimum.clone().max(zero).to_u64()?,
                            selector.maximum.clone().min(last_value).to_u64()?,
                        ),
                        None => (0, last),
                    };
                    if first > last || usize::try_from(last - first + 1).ok()? > *budget {
                        return None;
                    }
                    let mut hull: Option<IntegerRange> = None;
                    for index in first..=last {
                        path.push(
                            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index),
                        );
                        let bounds = self
                            .selected_path(root, reference, mutable, tail, path, primitive, budget);
                        path.pop();
                        let bounds = bounds?;
                        hull = Some(match hull {
                            Some(previous) => IntegerRange {
                                minimum: previous.minimum.min(bounds.minimum),
                                maximum: previous.maximum.max(bounds.maximum),
                            },
                            None => bounds,
                        });
                    }
                    hull
                }
            }
        }
        pub(crate) fn new(
            program: &'a dyn validation::CheckingInput,
            operators: &'a checked_trees::CheckedOperatorFacts,
            semantic: &'a FactPlan,
            contexts: &'a [FactContextHandle],
            state: SymbolHandle,
            point: ProgramPoint,
        ) -> Self {
            Self {
                program,
                semantic,
                contexts,
                state,
                operators,
                point,
                predicates: Vec::new(),
                assignments: RootAssignmentObservations::new(program, semantic, contexts),
            }
        }

        /* Where `root`'s rows sit in `predicates`, building them on first use. */
        fn rows_position(&mut self, root: facts::PlaceRoot) -> usize {
            if let Some(position) = self.predicates.iter().position(|known| known.root == root) {
                return position;
            }
            let rows = boolean_predicates::retained_at_root(
                self.program,
                self.operators,
                self.semantic,
                self.contexts,
                self.point,
                root,
            );
            let mut by_place = rows
                .iter()
                .enumerate()
                .flat_map(|(predicate, row)| {
                    (0..row.bindings.len()).map(move |binding| (predicate, binding))
                })
                .collect::<Vec<_>>();
            by_place.sort_by(|&(left, left_binding), &(right, right_binding)| {
                rows[left].bindings[left_binding]
                    .place
                    .compare_identity(&rows[right].bindings[right_binding].place)
                    .then(left.cmp(&right))
            });
            self.predicates.push(RootPredicates {
                root,
                rows,
                by_place,
            });
            self.predicates.len() - 1
        }

        pub(crate) fn at_place(
            &mut self,
            place: &CanonicalPlace,
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            let exact = self.exact(place, primitive);
            if !place
                .segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
            {
                return exact;
            }
            let projected = self.projected(place, primitive, &mut 4096);
            match (exact, projected) {
                (Some(exact), Some(projected)) => {
                    let minimum = exact.minimum.max(projected.minimum);
                    let maximum = exact.maximum.min(projected.maximum);
                    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
                }
                (exact, projected) => exact.or(projected),
            }
        }

        fn projected(
            &mut self,
            place: &CanonicalPlace,
            primitive: PrimitiveType,
            budget: &mut usize,
        ) -> Option<IntegerRange> {
            *budget = budget.checked_sub(1)?;
            let Some(position) = place
                .segments
                .iter()
                .position(|segment| matches!(segment, PlaceSegment::Index { .. }))
            else {
                return self.exact(place, primitive);
            };
            let mut collection = place.clone();
            collection.segments.truncate(position);
            let extent = validation::closed_collection_length_at_place(
                self.program,
                self.state,
                &collection,
            )?;
            if extent == 0 || usize::try_from(extent).ok()? > *budget {
                return None;
            }
            let mut hull: Option<IntegerRange> = None;
            for index in 0..extent {
                let mut element = place.clone();
                element.segments[position] = PlaceSegment::FixedIndex {
                    index: usize::try_from(index).ok()?,
                };
                let bounds = self.projected(&element, primitive, budget)?;
                hull = Some(match hull {
                    Some(previous) => IntegerRange {
                        minimum: previous.minimum.min(bounds.minimum),
                        maximum: previous.maximum.max(bounds.maximum),
                    },
                    None => bounds,
                });
            }
            hull
        }

        fn exact(
            &mut self,
            place: &CanonicalPlace,
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            let rows = self.rows_position(place.root);
            let mut bounds = validation::carrier_integer_range(primitive)?;
            if let Some(snapshot) = self.assignments.integer_bounds(place) {
                bounds.minimum = bounds.minimum.max(snapshot.minimum);
                bounds.maximum = bounds.maximum.min(snapshot.maximum);
            }
            let indexed = &self.predicates[rows];
            let first = indexed.by_place.partition_point(|&(predicate, binding)| {
                indexed.rows[predicate].bindings[binding]
                    .place
                    .compare_identity(place)
                    .is_lt()
            });
            let mut binding_predicates = indexed.by_place[first..]
                .iter()
                .take_while(|&&(predicate, binding)| {
                    indexed.rows[predicate].bindings[binding]
                        .place
                        .compare_identity(place)
                        .is_eq()
                })
                .filter(|&&(predicate, binding)| {
                    indexed.rows[predicate].bindings[binding].place == *place
                })
                .map(|&(predicate, _)| predicate)
                .collect::<Vec<_>>();
            binding_predicates.sort_unstable();
            binding_predicates.dedup();
            for predicate in binding_predicates {
                let predicate = &self.predicates[rows].rows[predicate];
                if let Some(consequence) =
                    predicate.integer_interval_at_place(place, primitive, |subject| {
                        self.assignments.scalar_value(subject)
                    })
                {
                    bounds.minimum = bounds.minimum.max(consequence.minimum);
                    bounds.maximum = bounds.maximum.min(consequence.maximum);
                }
            }
            (bounds.minimum <= bounds.maximum).then_some(bounds)
        }
    }
}
mod evaluation {
    /*
        Evaluate already-selected scalar operations over caller-owned value facts.
        This adapter supplies no source binding, initializer, call, or range premise.
    */
    use checked_trees::{
        CheckedBooleanExpression, CheckedIntegerBinaryKind as BinaryKind,
        CheckedIntegerComparisonKind as ComparisonKind, CheckedScalarExpression,
    };
    use numerics::{bignum::BigInt, literals::LandedIntegerType};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use typed_trees::types::PrimitiveType;

    use facts::ScalarValue;

    mod sources {
        use facts::ScalarValue;
        use symbols::SymbolHandle;

        pub(crate) trait ScalarValueSource {
            fn binding(&mut self, position: usize) -> Option<ScalarValue>;

            fn storage(&mut self, _symbol: SymbolHandle) -> Option<ScalarValue> {
                None
            }

            /* Current storage below an authored parameter, not a dense scalar binding. */
            fn structural_field(
                &mut self,
                _observation: typed_trees::expression::ExpressionHandle,
                _parameter_position: u32,
                _path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            ) -> Option<ScalarValue> {
                None
            }
        }

        impl<Resolve: FnMut(usize) -> Option<ScalarValue>> ScalarValueSource for Resolve {
            fn binding(&mut self, position: usize) -> Option<ScalarValue> {
                self(position)
            }
        }

        /*
            Exact immutable binding identities and separately named current storage
            reads. Both resolve only against the caller's facts at this program point.
        */
        pub(crate) struct BoundScalarValues<'a, Resolve> {
            pub symbols: &'a [SymbolHandle],
            pub value_at_symbol: Resolve,
        }

        impl<Resolve: FnMut(SymbolHandle) -> Option<ScalarValue>> ScalarValueSource
            for BoundScalarValues<'_, Resolve>
        {
            fn binding(&mut self, position: usize) -> Option<ScalarValue> {
                (self.value_at_symbol)(*self.symbols.get(position)?)
            }

            fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
                (self.value_at_symbol)(symbol)
            }
        }
    }
    pub(crate) use sources::{BoundScalarValues, ScalarValueSource};

    pub(crate) fn evaluate(
        expression: &CheckedScalarExpression,
        resolve_binding: &mut impl ScalarValueSource,
    ) -> Option<ScalarValue> {
        match expression {
            CheckedScalarExpression::StructuralParameterField {
                observation,
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_position,
                    },
                path,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(PrimitiveType::Bool),
                ..
            } => resolve_binding
                .structural_field(*observation, *parameter_position, path)
                .filter(|value| matches!(value, ScalarValue::Boolean(_))),
            CheckedScalarExpression::StorageRead {
                symbol,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(PrimitiveType::Bool),
            } => resolve_binding
                .storage(*symbol)
                .filter(|value| matches!(value, ScalarValue::Boolean(_))),
            CheckedScalarExpression::Boolean(expression) => {
                boolean(expression, resolve_binding).map(ScalarValue::Boolean)
            }
            CheckedScalarExpression::Parameter {
                position,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(PrimitiveType::Bool),
            }
            | CheckedScalarExpression::Local {
                position,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(PrimitiveType::Bool),
            } => binding_boolean(*position, resolve_binding).map(ScalarValue::Boolean),
            _ => integer(expression, resolve_binding)
                .map(|(_, value)| ScalarValue::Integer(integer_magnitude(value))),
        }
    }

    fn integer(
        expression: &CheckedScalarExpression,
        resolve_binding: &mut impl ScalarValueSource,
    ) -> Option<(IntegerType, IntegerValue)> {
        match expression {
            CheckedScalarExpression::StorageRead {
                scalar_type: typed_trees::callable_values::ScalarValueType::Callable(_),
                ..
            }
            | CheckedScalarExpression::Parameter {
                scalar_type: typed_trees::callable_values::ScalarValueType::Callable(_),
                ..
            }
            | CheckedScalarExpression::Local {
                scalar_type: typed_trees::callable_values::ScalarValueType::Callable(_),
                ..
            }
            | CheckedScalarExpression::StructuralParameterField {
                scalar_type: typed_trees::callable_values::ScalarValueType::Callable(_),
                ..
            } => None,
            CheckedScalarExpression::StructuralParameterIndexedRead { .. }
            | CheckedScalarExpression::StructuralParameterField {
                root:
                    checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                    | checked_trees::CheckedStorageRoot::ViewLocal { .. },
                ..
            }
            | CheckedScalarExpression::StructuralSequenceLength { .. } => None,
            CheckedScalarExpression::StructuralParameterField {
                observation,
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_position,
                    },
                path,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(primitive_type),
                ..
            } => {
                let scalar_type = integer_type(*primitive_type)?;
                let ScalarValue::Integer(value) =
                    resolve_binding.structural_field(*observation, *parameter_position, path)?
                else {
                    return None;
                };
                Some((scalar_type, admitted_integer(scalar_type, &value)?))
            }
            CheckedScalarExpression::StorageRead {
                symbol,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(primitive_type),
            } => {
                let scalar_type = integer_type(*primitive_type)?;
                let ScalarValue::Integer(value) = resolve_binding.storage(*symbol)? else {
                    return None;
                };
                Some((scalar_type, admitted_integer(scalar_type, &value)?))
            }
            CheckedScalarExpression::Parameter {
                position,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(primitive_type),
            }
            | CheckedScalarExpression::Local {
                position,
                scalar_type:
                    typed_trees::callable_values::ScalarValueType::Primitive(primitive_type),
            } => {
                let scalar_type = integer_type(*primitive_type)?;
                let ScalarValue::Integer(value) = resolve_binding.binding(*position)? else {
                    return None;
                };
                Some((scalar_type, admitted_integer(scalar_type, &value)?))
            }
            CheckedScalarExpression::ErasedParameter { .. } => None,
            CheckedScalarExpression::IntegerLiteral { literal } => {
                let landed = literal.landing()?.landed_type;
                if landed == LandedIntegerType::Addr {
                    return None;
                }
                let scalar_type = IntegerType::new(
                    if landed.is_signed() {
                        IntegerSign::Signed
                    } else {
                        IntegerSign::Unsigned
                    },
                    u16::try_from(landed.bit_width()).ok()?,
                )
                .ok()?;
                Some((
                    scalar_type,
                    admitted_integer(scalar_type, &literal.value_bignum()?)?,
                ))
            }
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                ..
            } => {
                let scalar_type = integer_type(*primitive_type)?;
                let (left_type, left) = integer(left, resolve_binding)?;
                let (right_type, right) = integer(right, resolve_binding)?;
                if left_type != scalar_type {
                    return None;
                }
                let shift = matches!(
                    kind,
                    BinaryKind::WrappingShiftLeft
                        | BinaryKind::WrappingShiftRight
                        | BinaryKind::ExactShiftLeft
                        | BinaryKind::ExactShiftRight
                        | BinaryKind::SaturatingShiftLeft
                        | BinaryKind::SaturatingShiftRight
                        | BinaryKind::TrappingShiftLeft
                        | BinaryKind::TrappingShiftRight
                );
                if !shift && right_type != scalar_type {
                    return None;
                }
                Some((
                    scalar_type,
                    binary(*kind, scalar_type, right_type, left, right)?,
                ))
            }
            CheckedScalarExpression::IntegerBitwiseNot {
                primitive_type,
                operand,
            } => {
                let scalar_type = integer_type(*primitive_type)?;
                let (operand_type, value) = integer(operand, resolve_binding)?;
                if operand_type != scalar_type {
                    return None;
                }
                Some((scalar_type, scalar_type.bitwise_not(value)?))
            }
            CheckedScalarExpression::IntegerWiden {
                primitive_type,
                operand,
            } => {
                let target = integer_type(*primitive_type)?;
                let (source, value) = integer(operand, resolve_binding)?;
                Some((target, source.widen_value_to(target, value)?))
            }
            CheckedScalarExpression::RepresentationView {
                primitive_type,
                operand,
            } => {
                let target = integer_type(*primitive_type)?;
                let (_, value) = integer(operand, resolve_binding)?;
                Some((target, representation_view_value(target, value)?))
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type,
                operand,
                range,
                ..
            } => {
                let target = integer_type(*primitive_type)?;
                let (source, value) = integer(operand, resolve_binding)?;
                let magnitude = integer_magnitude(value);
                if magnitude < range.minimum || magnitude > range.maximum {
                    return None;
                }
                Some((target, source.exact_cast_value_to(target, value)?))
            }
            CheckedScalarExpression::ConstantApplication { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. }
            | CheckedScalarExpression::ImplementationSelection { .. }
            | CheckedScalarExpression::Boolean(_) => None,
        }
    }

    fn binary(
        kind: BinaryKind,
        scalar_type: IntegerType,
        count_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        match kind {
            BinaryKind::ExactAdd => scalar_type.exact_add(left, right),
            BinaryKind::ExactSubtract => scalar_type.exact_sub(left, right),
            BinaryKind::ExactMultiply => scalar_type.exact_mul(left, right),
            BinaryKind::ExactDivide => scalar_type.exact_div(left, right),
            BinaryKind::ExactRemainder => scalar_type.exact_rem(left, right),
            BinaryKind::WrappingAdd => scalar_type.wrapping_add(left, right),
            BinaryKind::WrappingSubtract => scalar_type.wrapping_sub(left, right),
            BinaryKind::WrappingMultiply => scalar_type.wrapping_mul(left, right),
            BinaryKind::WrappingDivide => scalar_type.wrapping_div(left, right),
            BinaryKind::WrappingRemainder => scalar_type.wrapping_rem(left, right),
            BinaryKind::SaturatingAdd => scalar_type.saturating_add(left, right),
            BinaryKind::SaturatingSubtract => scalar_type.saturating_sub(left, right),
            BinaryKind::SaturatingMultiply => scalar_type.saturating_mul(left, right),
            BinaryKind::SaturatingDivide => scalar_type.saturating_div(left, right),
            BinaryKind::SaturatingRemainder => scalar_type.saturating_rem(left, right),
            BinaryKind::BitwiseAnd => scalar_type.bitwise_and(left, right),
            BinaryKind::BitwiseOr => scalar_type.bitwise_or(left, right),
            BinaryKind::BitwiseXor => scalar_type.bitwise_xor(left, right),
            BinaryKind::WrappingShiftLeft => {
                scalar_type.wrapping_shift_left(left, count_type, right)
            }
            BinaryKind::WrappingShiftRight => {
                scalar_type.wrapping_shift_right(left, count_type, right)
            }
            BinaryKind::ExactShiftLeft => scalar_type.exact_shift_left(left, count_type, right),
            BinaryKind::ExactShiftRight => scalar_type.exact_shift_right(left, count_type, right),
            BinaryKind::SaturatingShiftRight => {
                scalar_type.exact_shift_right(left, count_type, right)
            }
            BinaryKind::SaturatingShiftLeft => {
                scalar_type.exact_shift_right(left, count_type, right)?;
                Some(
                    scalar_type
                        .exact_shift_left(left, count_type, right)
                        .unwrap_or_else(|| {
                            if matches!(left, IntegerValue::Signed(value) if value < 0) {
                                scalar_type.minimum_value()
                            } else {
                                scalar_type.maximum_value()
                            }
                        }),
                )
            }
            BinaryKind::TrappingShiftLeft => scalar_type.exact_shift_left(left, count_type, right),
            BinaryKind::TrappingShiftRight => {
                scalar_type.exact_shift_right(left, count_type, right)
            }
            BinaryKind::TrappingAdd => scalar_type.exact_add(left, right),
            BinaryKind::TrappingSubtract => scalar_type.exact_sub(left, right),
            BinaryKind::TrappingMultiply => scalar_type.exact_mul(left, right),
            BinaryKind::TrappingDivide => scalar_type.exact_div(left, right),
            BinaryKind::TrappingRemainder => scalar_type.exact_rem(left, right),
        }
    }

    pub(crate) fn boolean(
        expression: &CheckedBooleanExpression,
        resolve_binding: &mut impl ScalarValueSource,
    ) -> Option<bool> {
        match expression {
            CheckedBooleanExpression::StructuralParameterField {
                observation,
                parameter_position,
                path,
                ..
            } => match resolve_binding.structural_field(*observation, *parameter_position, path)? {
                ScalarValue::Boolean(value) => Some(value),
                _ => None,
            },
            CheckedBooleanExpression::StorageRead { symbol } => {
                match resolve_binding.storage(*symbol)? {
                    ScalarValue::Boolean(value) => Some(value),
                    _ => None,
                }
            }
            CheckedBooleanExpression::Constant(value) => Some(*value),
            CheckedBooleanExpression::StructuralIdentity { left, right } => {
                (left == right).then_some(true)
            }
            CheckedBooleanExpression::Parameter { position }
            | CheckedBooleanExpression::Local { position } => {
                binding_boolean(*position, resolve_binding)
            }
            CheckedBooleanExpression::ErasedParameter { .. } => None,
            CheckedBooleanExpression::Not(operand) => Some(!boolean(operand, resolve_binding)?),
            CheckedBooleanExpression::Equal { left, right } => {
                Some(boolean(left, resolve_binding)? == boolean(right, resolve_binding)?)
            }
            CheckedBooleanExpression::And { left, right } => {
                match (
                    boolean(left, resolve_binding),
                    boolean(right, resolve_binding),
                ) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                }
            }
            CheckedBooleanExpression::Or { left, right } => {
                match (
                    boolean(left, resolve_binding),
                    boolean(right, resolve_binding),
                ) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                }
            }
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let (left_type, left) = integer(left, resolve_binding)?;
                let (right_type, right) = integer(right, resolve_binding)?;
                if left_type != right_type {
                    return None;
                }
                let ordering = left_type.compare(left, right)?;
                Some(match kind {
                    ComparisonKind::Equal => ordering.is_eq(),
                    ComparisonKind::LessThan => ordering.is_lt(),
                    ComparisonKind::LessOrEqual => !ordering.is_gt(),
                })
            }
            CheckedBooleanExpression::IntegerMathComparison { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::ByteSequencePredicate { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => None,
        }
    }

    fn binding_boolean(
        position: usize,
        resolve_binding: &mut impl ScalarValueSource,
    ) -> Option<bool> {
        match resolve_binding.binding(position)? {
            ScalarValue::Boolean(value) => Some(value),
            ScalarValue::Integer(_) | ScalarValue::Unknown => None,
        }
    }

    pub(crate) fn integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
        let (sign, bits) = primitive.integer_sign_bits()?;
        IntegerType::new(sign, bits).ok()
    }

    /*
        An admitted representation view interprets the source bits with the target
        signedness. Its caller has already established the carrier relation.
    */
    pub(crate) fn representation_view_value(
        target: IntegerType,
        value: IntegerValue,
    ) -> Option<IntegerValue> {
        let bits = match value {
            IntegerValue::Signed(value) => value as u128,
            IntegerValue::Unsigned(value) => value,
        };
        Some(match (target.sign(), target.bits()) {
            (IntegerSign::Signed, 8) => IntegerValue::Signed(i128::from(bits as i8)),
            (IntegerSign::Signed, 16) => IntegerValue::Signed(i128::from(bits as i16)),
            (IntegerSign::Signed, 32) => IntegerValue::Signed(i128::from(bits as i32)),
            (IntegerSign::Signed, 64) => IntegerValue::Signed(i128::from(bits as i64)),
            (IntegerSign::Unsigned, 8) => IntegerValue::Unsigned(u128::from(bits as u8)),
            (IntegerSign::Unsigned, 16) => IntegerValue::Unsigned(u128::from(bits as u16)),
            (IntegerSign::Unsigned, 32) => IntegerValue::Unsigned(u128::from(bits as u32)),
            (IntegerSign::Unsigned, 64) => IntegerValue::Unsigned(u128::from(bits as u64)),
            _ => return None,
        })
    }

    pub(crate) fn admitted_integer(
        scalar_type: IntegerType,
        value: &BigInt,
    ) -> Option<IntegerValue> {
        let value = match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(value.to_i64()?)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value.to_u64()?)),
        };
        scalar_type.admits(value).then_some(value)
    }

    pub(crate) fn integer_magnitude(value: IntegerValue) -> BigInt {
        match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        }
    }
}
pub(crate) use captured_call_intervals::CapturedCallIntervals;
pub(crate) use captured_intervals::CapturedIntegerIntervals;
pub(crate) use current_integer_bounds::CurrentIntegerBounds;
pub(crate) mod float_predicates {
    /*
        Normalize positive IEEE comparisons into observations of one stable place.
        Selection and sealed-source checks happen before assigning any order law.
        The resulting predicate contains landed bits, not a source expression that
        could read another value after a mutation or a state-parameter transfer.
    */
    use crate::flow::place_segment_has_static_identity;
    use crate::published::places::place_at;
    use facts::FloatPredicate;
    use numerics::bignum::{BigRational, ExactFloat, IeeeRounding};
    use numerics::literals::FloatFormat;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::operator::primitive_float_binary_semantics;
    use typed_trees::state::State;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
    use validation::expression_type_reference_in_state;
    use validation::find_machine_by_entry_state;
    use validation::has_builtin_decomposed_guard_meaning;
    use validation::has_builtin_or_sealed_float_comparison_meaning;

    macro_rules! keep {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
            collect_true($a, $b, $c, $d, $e, $f)
        };
    }

    pub(crate) fn collect_true(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
        observations: &mut Vec<(ExpressionHandle, FloatPredicate)>,
    ) {
        if let Some(borrow) = program.expression_table.borrow_expression(expression) {
            keep!(program => machine => state => statement_index => borrow.target => observations);
            return;
        }
        let Some(binary) = comparison(program, expression) else {
            return;
        };
        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Equal)
            && has_builtin_decomposed_guard_meaning(program, machine, Some(state), expression)
        {
            if binary.operator == BinaryOperator::And {
                keep!(program => machine => state => statement_index => binary.left => observations);
                keep!(program => machine => state => statement_index => binary.right => observations);
                return;
            }
            for (operand, wrapper) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if matches!(
                    program.expression_table.expression(wrapper),
                    ExpressionNode::Boolean(true)
                ) {
                    keep!(program => machine => state => statement_index => operand => observations);
                    return;
                }
            }
        }
        for (subject, bound, operator) in [
            (binary.left, binary.right, binary.operator),
            (binary.right, binary.left, reverse(binary.operator)),
        ] {
            let Some(reference) =
                expression_type_reference_in_state(program, state.symbol, statement_index, subject)
            else {
                continue;
            };
            let format = match program.type_reference_table.primitive_type(reference) {
                Some(PrimitiveType::F32) => FloatFormat::F32,
                Some(PrimitiveType::F64) => FloatFormat::F64,
                _ => continue,
            };
            if !has_ieee_meaning(
                program,
                machine.symbol,
                expression,
                binary.operator,
                reference,
            ) {
                continue;
            }
            let Some(place) = place_at(program, state.symbol, statement_index, subject) else {
                continue;
            };
            if !place.segments.iter().all(place_segment_has_static_identity) {
                continue;
            }
            let predicate = if operator == BinaryOperator::Equal
                && place_at(program, state.symbol, statement_index, bound)
                    .is_some_and(|other| other == place)
            {
                Some(FloatPredicate::non_nan(format))
            } else {
                literal(program, bound, format)
                    .and_then(|bound| FloatPredicate::comparison(format, operator, &bound))
            };
            if let Some(predicate) = predicate {
                observations.push((subject, predicate));
                return;
            }
        }
    }

    pub(crate) fn literal(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        format: FloatFormat,
    ) -> Option<ExactFloat> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Float(value) => Some(match format {
                FloatFormat::F32 => ExactFloat::from_f32(value.value_f32()),
                FloatFormat::F64 => ExactFloat::from_f64(value.value_f64()),
            }),
            ExpressionNode::Integer(value) => {
                let rational = BigRational::from_integer(value.value_bignum()?);
                Some(match format {
                    FloatFormat::F32 => ExactFloat::from_f32(
                        rational.to_f32_with_rounding(IeeeRounding::NearestTiesToEven),
                    ),
                    FloatFormat::F64 => ExactFloat::from_f64(
                        rational.to_f64_with_rounding(IeeeRounding::NearestTiesToEven),
                    ),
                })
            }
            ExpressionNode::Borrow(borrow) => literal(program, borrow.target, format),
            _ => None,
        }
    }

    pub(crate) fn has_ieee_meaning(
        program: &dyn ::validation::CheckingInput,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        operator: BinaryOperator,
        reference: TypeReferenceHandle,
    ) -> bool {
        owner.is_valid()
            && has_builtin_or_sealed_float_comparison_meaning(
                program,
                owner,
                expression,
                operator,
                &[Some(reference), Some(reference)],
            )
    }

    /*
        Reading a token-lowered call's operation does not authorize its laws.
        Consumers must check the exact selection with `has_ieee_meaning`.
    */
    pub(crate) fn comparison(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TableBinaryExpression> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => Some(*binary),
            ExpressionNode::Call(call)
                if !call.receiver.is_valid()
                    && !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
                    && call.machine_arguments.is_empty()
                    && call.evidence_arguments.is_empty() =>
            {
                let (machine, _) = find_machine_by_entry_state(program, call.target_symbol)?;
                let binding = program.machine_token_binding_by_symbol(machine.symbol)?;
                let (operator, _) = primitive_float_binary_semantics(program, binding)?;
                let [left, right] = program.expression_table.expression_handles(call.arguments)
                else {
                    return None;
                };
                Some(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    operator,
                    left: *left,
                    right: *right,
                })
            }
            _ => None,
        }
    }

    pub(crate) fn reverse(operator: BinaryOperator) -> BinaryOperator {
        operator.mirrored()
    }
}
mod place_intervals {
    /*
        The live place facts a selected scalar plan reads during flow and at exits.

        This is the interval source behind `validation::selected_scalar_interval`:
        it answers each operand position from the live snapshot at its place, the
        standing invariants of its storage (declared ranges, an immutable formal's
        arrival contract) and completed byte literals. It computes no arithmetic;
        the interval domain is validation's.
    */
    use crate::flow::canonical_place_from_symbol;
    use crate::values::exclusive_reference;
    use crate::values::literal_at_place;
    use crate::values::resolve_structural_parameter_path;
    use crate::values::snapshots::integer_bounds_at_place;
    use checked_trees::CheckedStructuralPredicatePathSegment;
    use facts::{FactContextHandle, FactPlan, IntegerRange};
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::{
        signature::StateParameter,
        types::{PrimitiveType, TypeReferenceHandle},
    };
    use validation::{CanonicalPlace, SelectedScalarIntervalSource};

    pub(crate) struct LivePlaceIntervals<'a> {
        pub program: &'a dyn ::validation::CheckingInput,
        pub semantic: &'a FactPlan,
        pub contexts: &'a [FactContextHandle],
        pub parameters: &'a [StateParameter],
        pub symbols: &'a [SymbolHandle],
        /*
            The state whose declarations own `symbols`; used to recover the declared
            type of nonlocal storage reads (owned formals and mutable locals).
        */
        pub state: SymbolHandle,
        pub contents: super::CurrentIntegerBounds<'a>,
    }

    impl LivePlaceIntervals<'_> {
        fn snapshot(&self, place: &CanonicalPlace) -> Option<IntegerRange> {
            integer_bounds_at_place(
                self.program,
                self.semantic,
                self.contexts
                    .iter()
                    .map(|context| self.semantic.contexts.get(*context)),
                place,
            )
        }

        /*
            Bounds for one resolved operand place. Live snapshots and standing
            invariants both hold at this read; neither replays an earlier initializer.
        */
        pub(crate) fn interval_at_place(
            &mut self,
            place: &CanonicalPlace,
            primitive_type: PrimitiveType,
        ) -> Option<IntegerRange> {
            let standing = match place.root {
                facts::PlaceRoot::Symbol(symbol) if place.segments.is_empty() => {
                    self.standing_interval(symbol, primitive_type)
                }
                _ => validation::carrier_integer_range(primitive_type),
            };
            match (
                self.contents
                    .at_place(place, primitive_type)
                    .or_else(|| self.snapshot(place)),
                standing,
            ) {
                (Some(snapshot), Some(standing)) => {
                    let minimum = snapshot.minimum.max(standing.minimum);
                    let maximum = snapshot.maximum.min(standing.maximum);
                    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
                }
                (snapshot, standing) => snapshot.or(standing),
            }
        }

        fn standing_interval(
            &self,
            symbol: SymbolHandle,
            primitive_type: PrimitiveType,
        ) -> Option<IntegerRange> {
            let mut bounds = validation::carrier_integer_range(primitive_type)?;
            if self.parameters.iter().any(|parameter| {
                parameter.symbol == symbol
                    && !parameter.is_mutable
                    && !exclusive_reference(self.program, parameter.type_reference)
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        == Some(primitive_type)
            }) && let Some((machine, state)) = validation::find_state(self.program, self.state)
                && let Some((minimum, maximum)) = validation::state_parameter_integer_interval(
                    self.program,
                    machine,
                    state,
                    symbol,
                )
            {
                if let Some(minimum) = minimum {
                    bounds.minimum = bounds.minimum.max(BigInt::from_i64(minimum));
                }
                if let Some(maximum) = maximum {
                    bounds.maximum = bounds.maximum.min(BigInt::from_i64(maximum));
                }
            }
            (bounds.minimum <= bounds.maximum).then_some(bounds)
        }

        /*
            The resolved place, the field's declared type, and whether the whole
            path is frozen (immutable end to end). Declared constraints only hold
            as read invariants on frozen storage: a mutable formal or exclusive
            reference can be reborrowed through a pointee type that drops them.
        */
        fn field(
            &self,
            position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<(CanonicalPlace, TypeReferenceHandle, bool)> {
            let (symbol, segments, reference, frozen) =
                resolve_structural_parameter_path(self.program, self.parameters, position, path)?;
            let mut place = canonical_place_from_symbol(symbol)?;
            place.segments = segments;
            Some((place, reference, frozen))
        }
    }

    impl SelectedScalarIntervalSource for LivePlaceIntervals<'_> {
        fn binding(
            &mut self,
            position: usize,
            primitive_type: PrimitiveType,
        ) -> Option<IntegerRange> {
            self.storage(*self.symbols.get(position)?, primitive_type)
        }

        fn storage(
            &mut self,
            symbol: SymbolHandle,
            primitive_type: PrimitiveType,
        ) -> Option<IntegerRange> {
            let place = canonical_place_from_symbol(symbol)?;
            self.interval_at_place(&place, primitive_type)
        }

        fn structural_field(
            &mut self,
            position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<IntegerRange> {
            let (place, reference, _frozen) = self.field(position, path)?;
            let declared = (|| {
                if let Some((CheckedStructuralPredicatePathSegment::FixedIndex(index), parent)) =
                    path.split_last()
                {
                    let index = IntegerRange {
                        minimum: BigInt::from_u64(*index),
                        maximum: BigInt::from_u64(*index),
                    };
                    if let Some(bounds) = self.literal_indexed_field(position, parent, Some(&index))
                    {
                        return Some(bounds);
                    }
                }
                let primitive_type = self.program.primitive_type_reference(reference)?;
                validation::carrier_integer_range(primitive_type)
            })();
            let primitive = self.program.primitive_type_reference(reference)?;
            let current = self.contents.at_place(&place, primitive);
            match (current, declared) {
                (Some(current), Some(declared)) => {
                    let minimum = current.minimum.max(declared.minimum);
                    let maximum = current.maximum.min(declared.maximum);
                    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
                }
                (current, declared) => current.or(declared),
            }
        }

        fn byte_length(
            &mut self,
            position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<IntegerRange> {
            let length = self.field(position, path).and_then(|(place, _, _)| {
                let literal = literal_at_place(
                    self.program,
                    self.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.semantic.contexts.get(*context)),
                    &place,
                )?;
                let bytes = typed_trees::byte_predicates::literal_bytes(self.program, literal)?;
                u64::try_from(bytes.len()).ok().map(BigInt::from_u64)
            });
            length
                .map(|length| IntegerRange {
                    minimum: length.clone(),
                    maximum: length,
                })
                .or_else(|| validation::carrier_integer_range(PrimitiveType::U64))
        }

        fn indexed_read(
            &mut self,
            root: checked_trees::CheckedStorageRoot,
            path: &[CheckedStructuralPredicatePathSegment],
            index: &checked_trees::CheckedScalarSelector,
            element_path: &[checked_trees::CheckedScalarReadProjection],
            primitive: PrimitiveType,
        ) -> Option<IntegerRange> {
            use super::current_integer_bounds::IntegerReadStep;
            let first = validation::selected_scalar_interval(&index.expression, self);
            let mut steps = path
                .iter()
                .cloned()
                .map(IntegerReadStep::Static)
                .collect::<Vec<_>>();
            steps.push(IntegerReadStep::Index(first.clone()));
            for step in element_path {
                steps.push(match step {
                    checked_trees::CheckedScalarReadProjection::Static(step) => {
                        IntegerReadStep::Static(step.clone())
                    }
                    checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                        IntegerReadStep::Index(validation::selected_scalar_interval(
                            &selector.expression,
                            self,
                        ))
                    }
                });
            }
            let (symbol, reference, mutable) = match root {
                checked_trees::CheckedStorageRoot::Parameter { index } => {
                    let parameter = self.parameters.get(usize::try_from(index).ok()?)?;
                    (
                        parameter.symbol,
                        parameter.type_reference,
                        parameter.is_mutable,
                    )
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol }
                | checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                    let state = self.program.state_by_symbol(self.state)?;
                    let (_, local) = self.program.state_local_declaration(state, symbol)?;
                    (symbol, local.type_reference, local.is_mutable)
                }
            };
            let current = self
                .contents
                .selected_read(symbol, reference, mutable, &steps, primitive)
                .or_else(|| validation::carrier_integer_range(primitive))?;
            let literal = root
                .parameter()
                .filter(|_| element_path.is_empty() && primitive == PrimitiveType::U8)
                .and_then(|position| self.literal_indexed_field(position, path, first.as_ref()));
            match literal {
                Some(literal) => {
                    let minimum = current.minimum.max(literal.minimum);
                    let maximum = current.maximum.min(literal.maximum);
                    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
                }
                None => Some(current),
            }
        }
    }

    impl LivePlaceIntervals<'_> {
        fn literal_indexed_field(
            &mut self,
            position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
            index: Option<&IntegerRange>,
        ) -> Option<IntegerRange> {
            let (place, mut reference, _frozen) = self.field(position, path)?;
            loop {
                use typed_trees::types::TypeReferenceNode;
                match self.program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. }
                    | TypeReferenceNode::Constrained {
                        base_type: referee, ..
                    } => reference = *referee,
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type }
                        if self.program.primitive_type_reference(*element_type)
                            == Some(PrimitiveType::U8) =>
                    {
                        break;
                    }
                    _ => return None,
                }
            }
            let literal_bounds = literal_at_place(
                self.program,
                self.semantic,
                self.contexts
                    .iter()
                    .map(|context| self.semantic.contexts.get(*context)),
                &place,
            )
            .and_then(|literal| {
                let bytes = typed_trees::byte_predicates::literal_bytes(self.program, literal)?;
                let bytes =
                    if let Some(index) = index.filter(|index| index.minimum == index.maximum) {
                        let position = usize::try_from(index.minimum.to_u64()?).ok()?;
                        std::slice::from_ref(bytes.get(position)?)
                    } else {
                        &bytes[..]
                    };
                Some(IntegerRange {
                    minimum: BigInt::from_u64(u64::from(*bytes.iter().min()?)),
                    maximum: BigInt::from_u64(u64::from(*bytes.iter().max()?)),
                })
            });
            literal_bounds.or_else(|| validation::carrier_integer_range(PrimitiveType::U8))
        }
    }
}
mod projection_evaluations {
    /*
        Executable selectors along one authored place spine, independent of its
        proven storage coordinates. Borrow/recast wrappers preserve that spine;
        a call is its own value producer and owns its ordinary argument schedule.
    */
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn authored_projection_selectors(
        program: &dyn validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<Vec<ExpressionHandle>> {
        authored_projection_selectors_below(program, expression, ExpressionHandle::invalid())
    }

    /*
        Selectors below an already completed receiver run at this observation.
        The receiver's own selectors belong to its producer, not to this read.
    */
    pub(crate) fn authored_projection_selectors_below(
        program: &dyn validation::CheckingInput,
        expression: ExpressionHandle,
        receiver: ExpressionHandle,
    ) -> Option<Vec<ExpressionHandle>> {
        let mut cursor = expression;
        let mut visited = Vec::new();
        let mut selectors = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
                return None;
            }
            if cursor == receiver {
                break;
            }
            visited.push(cursor);
            cursor = match program.expression_table.expression(cursor) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Cast(cast) if cast.form.is_recast() => cast.value,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed) => {
                    if !matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) {
                        selectors.push(indexed.index);
                    }
                    indexed.collection
                }
                _ if !receiver.is_valid() => break,
                _ => return None,
            };
        }
        selectors.reverse();
        Some(selectors)
    }
}
pub(crate) use projection_evaluations::{
    authored_projection_selectors, authored_projection_selectors_below,
};
pub(crate) mod reference_sources {

    /*
        Reference formation publishes a storage producer, not an erased alias.

        A parent's address is captured at its declaration, before later selector
        mutations. Children project through that retained carrier; they do not
        rediscover a parent's initializer. Recasts retain the actual backing and
        the source-selected storage interpretation separately from the logical
        referee. Borrow admission still owns access, lineage and lifetime.
    */
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use checked_trees::{
        CheckedReferenceStorage, CheckedStructuralAccess as StructuralAccess,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
    };
    use typed_trees::types::TypeReferenceHandle;

    use crate::execution::terminal_unit::calls;
    use crate::execution::terminal_unit::types::field_requires_place_observation;
    use crate::published::places::place_at;

    pub(crate) struct ReferenceCapture {
        pub(crate) source: CheckedUnitStructuralArgumentPlan,
        pub(crate) storage: CheckedReferenceStorage,
        pub(crate) backing_type: TypeReferenceHandle,
    }

    /*
        A computed pointer value carries its source-selected reference type.
        Named homes retain their existing projected-loan route; registering them
        as new values would duplicate their descriptors and custody. This query
        grants no receiving access and does not evaluate the producer.
    */
    pub(crate) fn transported_value_type(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if validation::reference_formation_source(program, expression).is_some() {
            return None;
        }
        if place_at(program, state.symbol, statement_index, expression)
            .is_some_and(|place| place.root.as_symbol().is_some())
        {
            return None;
        }
        let actual =
            validation::expression_result_type_reference(program, machine, state, expression)?;
        validation::reference_result_custody::stored_loan_parts(program, actual)?;
        Some(actual)
    }

    /*
        A loan of a pointer slot must retain that slot rather than its pointee.
        Preserve the exact endpoint selected by source admission; otherwise form
        the source-derived reference before the consumer attenuates its access.
        Receiving type correspondence grants neither storage nor access permission.
    */
    pub(crate) fn formed_reference_type(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
        receiving: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let formation = validation::reference_formation_source(program, expression)?;
        let reference = if validation::borrow_selects_reference_storage(
            program,
            formation.source,
            receiving,
            &[],
        ) {
            receiving
        } else {
            validation::expression_result_type_reference(program, machine, state, expression)?
        };
        let shell = validation::reference_shell(program, reference)?;
        let referee = shell.referee;
        let access = StructuralAccess::from(shell.access);
        (access == StructuralAccess::from(formation.access)
            && (!formation.view_type.is_valid()
                || program.normalized_type_identity(referee)
                    == program.normalized_type_identity(formation.view_type)))
        .then_some(reference)
    }

    pub(crate) fn capture(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        reference: TypeReferenceHandle,
        selector: &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> Option<ReferenceCapture> {
        capture_source(
            program,
            machine,
            state,
            statement_index,
            validation::reference_formation_source(program, expression)?,
            reference,
            selector,
        )
    }

    /*
        Geometry for an admitted reference capture, explicit or implicit. The
        caller selects custody from checked evidence; this operation retains the
        actual runtime source and every descriptor crossing, not ultimate backing.
    */
    pub(crate) fn capture_source(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        formation: validation::ReferenceFormationSource,
        reference: TypeReferenceHandle,
        selector: &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> Option<ReferenceCapture> {
        let shell = validation::reference_shell(program, reference)?;
        let referent = shell.referee;
        let access = StructuralAccess::from(shell.access);
        if StructuralAccess::from(formation.access) != access {
            return None;
        }
        let is_view = formation.view_type.is_valid();
        let is_access_view = formation.interpretation.is_access_view();
        if is_view
            && program.normalized_type_identity(formation.view_type)
                != program.normalized_type_identity(referent)
        {
            return None;
        }
        let target = formation.source;
        let place = place_at(program, state.symbol, statement_index, target)?;
        let ProjectedArgumentPath {
            declared_type,
            storage_type: backing_type,
            mut path,
        } = match calls::projected_evaluated_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
            Some(selector),
        ) {
            Some(path) => path,
            None => {
                if !matches!(place.root, facts::PlaceRoot::Expression(_))
                    || !place.segments.is_empty()
                {
                    return None;
                }
                ProjectedArgumentPath {
                    declared_type: referent,
                    storage_type: referent,
                    path: Vec::new(),
                }
            }
        };
        let expression_root_type = if matches!(place.root, facts::PlaceRoot::Expression(_)) {
            validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &validation::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )
        } else {
            None
        };
        if matches!(place.root, facts::PlaceRoot::Expression(_)) && !place.segments.is_empty() {
            // A completed expression stores its reference descriptor. Projection
            // begins by crossing that exact carrier, before any case or field.
            // A whole reference slot keeps the endpoint loop below instead, so
            // borrowing a descriptor never becomes borrowing its referent.
            let root_reference = expression_root_type?;
            let mut prefix = calls::stored_reference_prefix(program, root_reference, |supplied| {
                access_can_supply(supplied.into(), access)
            })?;
            prefix.append(&mut path);
            path = prefix;
        }
        // An exclusive reference cannot erase a field's receiving restriction,
        // and an atomic cell cannot become ordinary scalar storage. Shared scalar
        // loans can attenuate to the declared carrier while retaining their exact
        // captured source; every write still belongs to the owning record. Keep
        // their selectors in this ordinary value producer rather than replaying
        // mutable selector storage through a silent place alias.
        if let Some(facts::PlaceSegment::Field { symbol }) = place.segments.last()
            && field_requires_place_observation(program, *symbol, backing_type, access)
        {
            return None;
        }
        let mut endpoint = backing_type;
        let mut storage_contract = declared_type;
        if place.segments.is_empty()
            && let facts::PlaceRoot::Symbol(root) = place.root
            && program.state_parameter_by_symbol(state, root).is_some()
            && let Some(shell) = validation::reference_shell(program, declared_type)
            && !access_can_supply(shell.access.into(), access)
        {
            return None;
        }
        let mut crossings = 0;
        let requested_storage =
            validation::stored_structural_type_identity(program, referent, &[], &[]);
        if !requested_storage.is_valid() {
            return None;
        }
        let mut remaining = program.type_reference_table.type_reference_count();
        loop {
            let selected_storage =
                validation::stored_structural_type_identity(program, endpoint, &[], &[]);
            if !selected_storage.is_valid() {
                return None;
            }
            if !is_view && selected_storage == requested_storage {
                break;
            }
            let Some(shell) = validation::reference_shell(program, endpoint) else {
                break;
            };
            remaining = remaining.checked_sub(1)?;
            if !access_can_supply(shell.access.into(), access) {
                return None;
            }
            crossings += 1;
            storage_contract = endpoint;
            endpoint = shell.referee;
        }
        path.extend(std::iter::repeat_n(
            checked_trees::CheckedUnitStructuralPathSegment::Referent,
            crossings,
        ));
        if !is_view
            && validation::stored_structural_type_identity(program, endpoint, &[], &[])
                != validation::stored_structural_type_identity(program, referent, &[], &[])
        {
            return None;
        }
        let access_storage = if is_access_view {
            let named_symbol = |reference| {
                let reference = program
                    .type_reference_table
                    .peeled_type_reference(reference);
                match program.type_reference_table.type_reference(reference) {
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                    _ => None,
                }
            };
            let view = named_symbol(referent)?;
            let mut plans = program
                .access_view_plans
                .iter()
                .filter(|plan| plan.data_symbol == view);
            let (Some(plan), None) = (plans.next(), plans.next()) else {
                return None;
            };
            Some(
                formation
                    .interpretation
                    .access_storage(program, storage_contract, plan)?,
            )
        } else {
            None
        };

        let source = match place.root {
            facts::PlaceRoot::Expression(expression) => {
                // An alias can name existing storage without publishing a value.
                // Retain its checked root type with the projection. Root reference
                // shells keep their separate stored-descriptor namespace.
                let logical_root_type_identity = expression_root_type
                    .filter(|reference| validation::reference_shell(program, *reference).is_none())
                    .map(|reference| {
                        validation::stored_structural_type_identity(program, reference, &[], &[])
                    })
                    .filter(|identity| identity.is_valid());
                SourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    logical_root_type_identity,
                }
            }
            facts::PlaceRoot::Symbol(root) => {
                if let Some(parameter) = program.state_parameter_by_symbol(state, root) {
                    if parameter.is_const || parameter.relevance.is_erased() {
                        return None;
                    }
                    if program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                    {
                        if parameter.is_mutable || access != StructuralAccess::SharedBorrow {
                            return None;
                        }
                        let position = program
                            .state_parameters(state)
                            .iter()
                            .filter(|parameter| {
                                !parameter.is_const
                                    && !parameter.relevance.is_erased()
                                    && program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_some()
                            })
                            .position(|parameter| parameter.symbol == root)?;
                        SourcePlan::ScalarParameter {
                            parameter_index: u32::try_from(position).ok()?,
                        }
                    } else {
                        let position = program
                            .state_parameters(state)
                            .iter()
                            .filter(|parameter| {
                                !parameter.is_const
                                    && !parameter.relevance.is_erased()
                                    && program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_none()
                            })
                            .position(|parameter| parameter.symbol == root)?;
                        SourcePlan::Parameter {
                            parameter_index: u32::try_from(position).ok()?,
                        }
                    }
                } else {
                    let (declaration_index, owner) =
                        program.state_local_declaration(state, root)?;
                    if declaration_index >= statement_index || owner.relevance.is_erased() {
                        return None;
                    }
                    if validation::reference_shell(program, owner.type_reference).is_some()
                        && path.is_empty()
                        && validation::stored_structural_type_identity(
                            program,
                            owner.type_reference,
                            &[],
                            &[],
                        ) == validation::stored_structural_type_identity(
                            program,
                            endpoint,
                            &[],
                            &[],
                        )
                    {
                        SourcePlan::StructuralLocal { symbol: root }
                    } else if let Some(shell) =
                        validation::reference_shell(program, owner.type_reference)
                    {
                        let supplied = StructuralAccess::from(shell.access);
                        if !matches!(
                            (supplied, access),
                            (StructuralAccess::MutableBorrow, _)
                                | (
                                    StructuralAccess::SharedBorrow,
                                    StructuralAccess::SharedBorrow
                                )
                                | (
                                    StructuralAccess::WriteOnlyBorrow,
                                    StructuralAccess::WriteOnlyBorrow
                                )
                        ) {
                            return None;
                        }
                        SourcePlan::StructuralLocalBorrow {
                            symbol: root,
                            root_type_identity: if place.segments.is_empty() {
                                validation::stored_structural_type_identity(
                                    program,
                                    endpoint,
                                    &[],
                                    &[],
                                )
                            } else {
                                validation::stored_structural_type_identity(
                                    program,
                                    shell.referee,
                                    &[],
                                    &[],
                                )
                            },
                            root_reference_hops: if place.segments.is_empty() {
                                u32::try_from(crossings).ok()?
                            } else {
                                0
                            },
                        }
                    } else {
                        if owner.is_mutable
                            && program
                                .primitive_type_reference(owner.type_reference)
                                .is_some()
                        {
                            SourcePlan::PrimitiveLocal { symbol: root }
                        } else {
                            SourcePlan::StructuralLocal { symbol: root }
                        }
                    }
                }
            }
            _ => return None,
        };
        let storage = if let Some(storage) = access_storage {
            storage
        } else if is_view {
            validation::reference_view_storage(program, endpoint, referent)?
        } else {
            CheckedReferenceStorage::Declared
        };
        Some(ReferenceCapture {
            source: CheckedUnitStructuralArgumentPlan {
                source,
                path,
                projection_evaluations: Default::default(),
                type_identity: validation::stored_structural_type_identity(
                    program,
                    endpoint,
                    &[],
                    &[],
                ),
                access,
            },
            storage,
            backing_type: endpoint,
        })
    }

    pub(crate) fn access_can_supply(source: StructuralAccess, result: StructuralAccess) -> bool {
        source == result
            || matches!(
                source,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            )
    }

    /*
        Record every standing requirement on the exact supplying endpoint. This
        runs when publishing a formation, not during repeated geometry queries.
        Unavailable inventory remains a refusal for the consuming lowerer.
    */
    pub(crate) fn qualification_contract(
        program: &dyn validation::CheckingInput,
        result_type: TypeReferenceHandle,
        backing_type: TypeReferenceHandle,
        storage: CheckedReferenceStorage,
    ) -> checked_trees::CheckedReferenceSourceContract {
        use checked_trees::CheckedReferenceSourceContract;
        if !matches!(
            storage,
            CheckedReferenceStorage::Declared | CheckedReferenceStorage::View { .. }
        ) || !validation::stores_reference_shape(program, result_type, &[])
        {
            return CheckedReferenceSourceContract::SeparateStorage;
        }
        let inventory = || {
            let contract =
                validation::structural_result_qualification_contract(program, backing_type)?;
            let mut source_requirements = contract
                .qualifications
                .iter()
                .map(|domain| checked_trees::CheckedSourceQualification {
                    path: Vec::new(),
                    semantic_domain: *domain,
                })
                .collect::<Vec<_>>();
            for row in &contract.projected_qualifications {
                source_requirements.push(checked_trees::CheckedSourceQualification {
                    path: validation::canonical_qualification_path(
                        program,
                        backing_type,
                        &row.path,
                    )?,
                    semantic_domain: row.domain,
                });
            }
            Ok::<_, &'static str>(CheckedReferenceSourceContract::Standing {
                contract,
                source_requirements,
            })
        };
        inventory()
            .unwrap_or_else(|reason| CheckedReferenceSourceContract::Unavailable(reason.to_owned()))
    }

    /*
        State arrivals may materialize a pointer after their operand frame completes.
        Retain its supplying type now; lowering must not reopen the source expression.
    */
    pub(crate) fn qualification_contract_at_site(
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        result_type: TypeReferenceHandle,
        endpoint_identity: typed_trees::type_identity::TypeIdentityId,
    ) -> (
        TypeReferenceHandle,
        checked_trees::CheckedReferenceSourceContract,
    ) {
        let source = validation::reference_formation_source(program, expression)
            .map_or(expression, |formation| formation.source);
        let Some(mut backing_type) =
            validation::expression_type_reference_in_state(program, state, statement_index, source)
        else {
            return (
                TypeReferenceHandle::invalid(),
                checked_trees::CheckedReferenceSourceContract::Unavailable(
                    "state loan lost its supplying type".to_owned(),
                ),
            );
        };
        let mut remaining = program.type_reference_table.type_reference_count();
        while validation::stored_structural_type_identity(program, backing_type, &[], &[])
            != endpoint_identity
        {
            let next = remaining.checked_sub(1).and_then(|left| {
                validation::reference_shell(program, backing_type)
                    .map(|shell| (left, shell.referee))
            });
            let Some((left, referee)) = next else {
                return (
                    backing_type,
                    checked_trees::CheckedReferenceSourceContract::Unavailable(
                        "state loan supplying type differs from its checked endpoint".to_owned(),
                    ),
                );
            };
            remaining = left;
            backing_type = referee;
        }
        (
            backing_type,
            qualification_contract(
                program,
                result_type,
                backing_type,
                CheckedReferenceStorage::Declared,
            ),
        )
    }
}
mod snapshots {
    /*
        Shared lookup of current assignment values. Storage invalidation owns their
        lifetime; neither a local initializer nor a nonliteral expression is replayed.
    */
    use validation::CanonicalPlace;

    use crate::flow::{canonical_place_from_semantic_place, canonical_place_from_symbol};
    use facts::{FactContext, FactPayload, FactPlan, ScalarValue};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    /*
        Assignment inputs shared by queries over one immutable context selection.
        The borrowed plan prevents this batch from crossing a fact mutation or reset.
        Keep every selected context, duplicate and payload in publication order;
        agreement and scalar refusal still belong to the ordinary snapshot readers.
    */
    pub(crate) struct AssignmentObservationBatch<'input> {
        program: &'input dyn ::validation::CheckingInput,
        semantic: &'input FactPlan,
        observations: Vec<AssignmentObservation>,
    }

    /*
        Share assignment inputs within one unchanged context selection. Each
        operand prefix or storage epoch supplies its own reader rather than
        joining rosters. Values and proof decisions are checked on every query.
    */
    pub(crate) struct RootAssignmentObservations<'input> {
        program: &'input dyn ::validation::CheckingInput,
        semantic: &'input FactPlan,
        contexts: &'input [facts::FactContextHandle],
        roots: std::collections::HashMap<facts::PlaceRoot, RootAssignmentInputBatch<'input>>,
    }

    impl<'input> RootAssignmentObservations<'input> {
        pub(crate) fn new(
            program: &'input dyn ::validation::CheckingInput,
            semantic: &'input FactPlan,
            contexts: &'input [facts::FactContextHandle],
        ) -> Self {
            Self {
                program,
                semantic,
                contexts,
                roots: std::collections::HashMap::new(),
            }
        }

        fn for_root(&mut self, root: facts::PlaceRoot) -> &mut RootAssignmentInputBatch<'input> {
            self.roots
                .entry(root)
                .or_insert_with(|| RootAssignmentInputBatch {
                    program: self.program,
                    semantic: self.semantic,
                    root,
                    contexts: self.contexts.iter(),
                    current: None,
                    observations: Vec::new(),
                    #[cfg(test)]
                    work: AssignmentInputWork::default(),
                })
        }

        pub(crate) fn scalar_value(&mut self, place: &CanonicalPlace) -> Option<ScalarValue> {
            if let Some(payload) = literal_assignment_payload(self.program, place) {
                return scalar_value_from_payloads(self.program, self.semantic, [payload]);
            }
            let program = self.program;
            let semantic = self.semantic;
            scalar_value_from_payloads(program, semantic, self.for_root(place.root).payloads(place))
        }

        pub(crate) fn integer_bounds(
            &mut self,
            place: &CanonicalPlace,
        ) -> Option<facts::IntegerRange> {
            if let Some(payload) = literal_assignment_payload(self.program, place) {
                return integer_bounds_from_payloads(self.program, self.semantic, [payload]);
            }
            let program = self.program;
            let semantic = self.semantic;
            integer_bounds_from_payloads(
                program,
                semantic,
                self.for_root(place.root).payloads(place),
            )
        }
    }

    /*
        Prepare only the assignment rows a query reaches. Later fields replay that
        prefix and continue reading when necessary. A refusal never fills the tail
        or becomes an answer for another field; scalar agreement stays fresh.
    */
    struct RootAssignmentInputBatch<'input> {
        program: &'input dyn ::validation::CheckingInput,
        semantic: &'input FactPlan,
        root: facts::PlaceRoot,
        contexts: std::slice::Iter<'input, facts::FactContextHandle>,
        current: Option<(&'input FactContext, facts::RootAssignmentFacts<'input>)>,
        observations: Vec<RootAssignmentInput<'input>>,
        #[cfg(test)]
        work: AssignmentInputWork,
    }

    #[cfg(test)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    struct AssignmentInputWork {
        contexts: usize,
        canonicalizations: usize,
        snapshots: usize,
    }

    struct RootAssignmentInput<'input> {
        place: CanonicalPlace,
        context: &'input FactContext,
        fact: &'input facts::Fact,
        completed_snapshot: std::cell::OnceCell<bool>,
    }

    impl<'input> RootAssignmentInputBatch<'input> {
        fn next_observation(&mut self) -> Option<RootAssignmentInput<'input>> {
            loop {
                if let Some((context, assignments)) = &mut self.current {
                    for fact in assignments {
                        #[cfg(test)]
                        {
                            self.work.canonicalizations += 1;
                        }
                        if let Some(place) = assignment_place(self.program, self.semantic, fact) {
                            return Some(RootAssignmentInput {
                                place,
                                context,
                                fact,
                                completed_snapshot: std::cell::OnceCell::new(),
                            });
                        }
                    }
                }
                let context = self.semantic.contexts.get(*self.contexts.next()?);
                #[cfg(test)]
                {
                    self.work.contexts += 1;
                }
                self.current = Some((
                    context,
                    self.semantic.assignments_with_root(context, self.root),
                ));
            }
        }

        fn payloads<'query>(
            &'query mut self,
            subject: &'query CanonicalPlace,
        ) -> impl Iterator<Item = FactPayload> + 'query {
            let mut position = 0;
            std::iter::from_fn(move || {
                loop {
                    if position == self.observations.len() {
                        let observation = self.next_observation()?;
                        self.observations.push(observation);
                    }
                    let observation = &self.observations[position];
                    position += 1;
                    if !crate::flow::canonical_places_agree(
                        self.program,
                        &observation.place,
                        subject,
                    ) {
                        continue;
                    }
                    // This fixed input association is needed only for a matching
                    // row. An unrelated field must not trigger its snapshot scan.
                    if *observation.completed_snapshot.get_or_init(|| {
                        #[cfg(test)]
                        {
                            self.work.snapshots += 1;
                        }
                        has_completed_call_snapshot(
                            self.program,
                            self.semantic,
                            observation.context,
                            observation.fact,
                        )
                    }) {
                        continue;
                    }
                    return Some(observation.fact.payload);
                }
            })
        }
    }

    impl<'input> AssignmentObservationBatch<'input> {
        pub(crate) fn prepare<'context>(
            program: &'input dyn ::validation::CheckingInput,
            semantic: &'input FactPlan,
            contexts: impl IntoIterator<Item = &'context FactContext>,
        ) -> Self {
            let observations = contexts
                .into_iter()
                .flat_map(|context| prepare_assignment_observations(program, semantic, context))
                .collect();
            Self {
                program,
                semantic,
                observations,
            }
        }

        /*
            Prepare the assignments one source root can supply. The context already
            retains this roster, including its original order within each root.
            Other roots cannot agree with any queried member of that source, so do
            not reconstruct them or scan the context's non-assignment facts.
        */
        pub(crate) fn prepare_for_root<'context>(
            program: &'input dyn ::validation::CheckingInput,
            semantic: &'input FactPlan,
            contexts: impl IntoIterator<Item = &'context FactContext>,
            root: facts::PlaceRoot,
        ) -> Self {
            let observations = contexts
                .into_iter()
                .flat_map(|context| {
                    semantic
                        .assignments_with_root(context, root)
                        .filter_map(move |fact| {
                            assignment_observation(program, semantic, context, fact)
                        })
                })
                .collect();
            Self {
                program,
                semantic,
                observations,
            }
        }

        fn payloads<'query>(
            &'query self,
            subject: &'query CanonicalPlace,
        ) -> impl Iterator<Item = FactPayload> + 'query {
            let literal = literal_assignment_payload(self.program, subject);
            let observations = if literal.is_some() {
                &[][..]
            } else {
                self.observations.as_slice()
            };
            literal.into_iter().chain(
                observations
                    .iter()
                    .filter(move |observation| {
                        crate::flow::canonical_places_agree(
                            self.program,
                            &observation.place,
                            subject,
                        )
                    })
                    .map(|observation| observation.payload),
            )
        }

        pub(crate) fn scalar_value(&self, subject: &CanonicalPlace) -> Option<ScalarValue> {
            scalar_value_from_payloads(self.program, self.semantic, self.payloads(subject))
        }
    }

    /*
        Read scalar bindings and structural fields from the same live storage facts.
        Bindings use dense scalar positions; structural paths use authored parameters.
    */
    pub(crate) struct PlaceScalarValues<'a, Resolve> {
        pub program: &'a dyn ::validation::CheckingInput,
        pub parameters: &'a [typed_trees::signature::StateParameter],
        pub symbols: &'a [symbols::SymbolHandle],
        pub value_at_place: Resolve,
    }

    impl<Resolve: FnMut(&CanonicalPlace, ExpressionHandle) -> Option<ScalarValue>>
        super::ScalarValueSource for PlaceScalarValues<'_, Resolve>
    {
        fn binding(&mut self, position: usize) -> Option<ScalarValue> {
            self.storage(*self.symbols.get(position)?)
        }

        fn storage(&mut self, symbol: symbols::SymbolHandle) -> Option<ScalarValue> {
            (self.value_at_place)(
                &canonical_place_from_symbol(symbol)?,
                ExpressionHandle::invalid(),
            )
        }

        fn structural_field(
            &mut self,
            observation: typed_trees::expression::ExpressionHandle,
            parameter_position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Option<ScalarValue> {
            if path.is_empty() {
                return None;
            }
            let (symbol, segments, _, _) = super::resolve_structural_parameter_path(
                self.program,
                self.parameters,
                parameter_position,
                path,
            )?;
            let mut place = canonical_place_from_symbol(symbol)?;
            place.segments = segments;
            (self.value_at_place)(&place, observation)
        }
    }

    pub(crate) fn scalar_value_at_place<'a>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: impl IntoIterator<Item = &'a FactContext>,
        subject: &CanonicalPlace,
    ) -> Option<ScalarValue> {
        if subject.segments.is_empty()
            && let facts::PlaceRoot::Expression(expression) = subject.root
            && let Some(value) = scalar_literal(program, expression)
        {
            return Some(value);
        }
        scalar_value_from_payloads(
            program,
            semantic,
            payloads_at_place(program, semantic, contexts, subject),
        )
    }

    pub(crate) fn scalar_value_from_payloads(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        payloads: impl IntoIterator<Item = FactPayload>,
    ) -> Option<ScalarValue> {
        let mut retained = None;
        for payload in payloads {
            let incoming = match payload {
                FactPayload::AssignedScalarValue { value } => {
                    match semantic.scalar_values.get(value) {
                        ScalarValue::Unknown => None,
                        value => Some(value.clone()),
                    }
                }
                FactPayload::AssignedValue { value } => scalar_literal(program, value),
                FactPayload::AssignedIntegerBounds { bounds }
                    if semantic.integer_ranges.is_valid(bounds) =>
                {
                    let bounds = semantic.integer_ranges.get(bounds);
                    (bounds.minimum == bounds.maximum)
                        .then(|| ScalarValue::Integer(bounds.minimum.clone()))
                }
                _ => None,
            }?;
            if retained
                .as_ref()
                .is_some_and(|retained| retained != &incoming)
            {
                return None;
            }
            retained = Some(incoming);
        }
        retained
    }

    pub(crate) fn literal_at_place<'a>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: impl IntoIterator<Item = &'a FactContext>,
        subject: &CanonicalPlace,
    ) -> Option<ExpressionHandle> {
        let mut retained = None;
        for payload in payloads_at_place(program, semantic, contexts, subject) {
            let value = match payload {
                FactPayload::AssignedValue { value } if is_assignment_literal(program, value) => {
                    value
                }
                _ => return None,
            };
            if retained.is_some_and(|previous| previous != value) {
                return None;
            }
            retained = Some(value);
        }
        retained
    }

    pub(crate) fn integer_bounds_at_place<'a>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: impl IntoIterator<Item = &'a FactContext>,
        subject: &CanonicalPlace,
    ) -> Option<facts::IntegerRange> {
        integer_bounds_from_payloads(
            program,
            semantic,
            payloads_at_place(program, semantic, contexts, subject),
        )
    }

    pub(crate) fn integer_bounds_from_payloads(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        payloads: impl IntoIterator<Item = FactPayload>,
    ) -> Option<facts::IntegerRange> {
        let mut retained: Option<facts::IntegerRange> = None;
        for payload in payloads {
            let incoming = match payload {
                FactPayload::AssignedIntegerBounds { bounds }
                    if semantic.integer_ranges.is_valid(bounds) =>
                {
                    semantic.integer_ranges.get(bounds).clone()
                }
                FactPayload::AssignedScalarValue { value } => {
                    let ScalarValue::Integer(value) = semantic.scalar_values.get(value) else {
                        return None;
                    };
                    facts::IntegerRange {
                        minimum: value.clone(),
                        maximum: value.clone(),
                    }
                }
                FactPayload::AssignedValue { value }
                    if program.expression_table.expression_is_valid(value) =>
                {
                    let literal = program.expression_table.integer_node(value)?;
                    let value = literal.value_bignum()?;
                    facts::IntegerRange {
                        minimum: value.clone(),
                        maximum: value,
                    }
                }
                _ => return None,
            };
            if incoming.minimum > incoming.maximum {
                return None;
            }
            retained = Some(match retained {
                Some(previous) => facts::IntegerRange {
                    minimum: previous.minimum.min(incoming.minimum),
                    maximum: previous.maximum.max(incoming.maximum),
                },
                None => incoming,
            });
        }
        retained
    }

    /*
        The exact IEEE equality recorded on `subject`.

        The producer stores one `FloatPredicate` equality, including the sign bit
        of zero. This read returns that bound's spelling. Two recorded equalities
        must be the same IEEE value; a bit difference that IEEE still equates,
        such as the two signed zeros, is one record. An inequality or a NaN
        exclusion is not an exact value.
    */
    pub(crate) fn ieee_equal_spelling_at_place<'a>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: impl IntoIterator<Item = &'a FactContext>,
        subject: &CanonicalPlace,
    ) -> Option<String> {
        let mut retained: Option<String> = None;
        for context in contexts {
            for fact in semantic.context_view(context).facts() {
                let facts::FactPayload::FloatPredicate { predicate } = fact.payload else {
                    continue;
                };
                let Some(spelling) = predicate.equal_bound_spelling() else {
                    continue;
                };
                let place = fact.place.as_place()?;
                let candidate = canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                )?;
                if !crate::flow::canonical_places_agree(program, &candidate, subject) {
                    continue;
                }
                if let Some(existing) = &retained {
                    let same = numerics::literals::FloatLiteral::parse(existing)
                        .zip(numerics::literals::FloatLiteral::parse(&spelling))
                        .is_some_and(|(left, right)| left.value_f64() == right.value_f64());
                    if !same {
                        return None;
                    }
                } else {
                    retained = Some(spelling);
                }
            }
        }
        retained
    }

    /*
        Read assignments only as far as the scalar reader needs them. Unknown or
        conflicting values already settle the query, so later contexts need not
        rebuild their places. Keep the original context and assignment order.
    */
    fn payloads_at_place<'a>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: impl IntoIterator<Item = &'a FactContext>,
        subject: &CanonicalPlace,
    ) -> impl Iterator<Item = FactPayload> {
        let literal = literal_assignment_payload(program, subject);
        let contexts = literal
            .is_none()
            .then(|| contexts.into_iter())
            .into_iter()
            .flatten();
        literal.into_iter().chain(contexts.flat_map(move |context| {
            semantic
                .assignments_with_root(context, subject.root)
                .filter_map(move |fact| {
                    let place = semantic.places.get(fact.place.as_place()?);
                    let candidate = canonical_place_from_semantic_place(program, semantic, place)?;
                    if !crate::flow::canonical_places_agree(program, &candidate, subject) {
                        return None;
                    }
                    if has_completed_call_snapshot(program, semantic, context, fact) {
                        return None;
                    }
                    Some(fact.payload)
                })
        }))
    }

    fn has_completed_call_snapshot(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: &FactContext,
        fact: &facts::Fact,
    ) -> bool {
        let FactPayload::AssignedValue { value } = fact.payload else {
            return false;
        };
        let Some(place) = fact.place.as_place() else {
            return false;
        };
        matches!(
            program.expression_table.expression(value),
            ExpressionNode::Call(_)
        ) && semantic
            .assignments_with_root(context, semantic.places.get(place).root)
            .any(|snapshot| {
                snapshot.place == fact.place
                    && snapshot.point == fact.point
                    && snapshot.origin == fact.origin
                    && match snapshot.payload {
                        FactPayload::AssignedScalarValue { value } => {
                            !matches!(semantic.scalar_values.get(value), ScalarValue::Unknown)
                        }
                        FactPayload::AssignedIntegerBounds { bounds } => {
                            semantic.integer_ranges.is_valid(bounds)
                        }
                        _ => false,
                    }
            })
    }

    /*
        Literal payloads are immutable authored values, including the exact landed
        float bits. Keeping their handles does not re-evaluate a source expression.
        Copies and state-edge joins use this same admission as direct assignments.
    */
    pub(crate) fn is_assignment_literal(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        program.expression_table.expression_is_valid(expression)
            && (typed_trees::byte_predicates::literal_bytes(program, expression).is_some()
                || matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Integer(_)
                        | ExpressionNode::Float(_)
                        | ExpressionNode::Boolean(_)
                        | ExpressionNode::String(_)
                ))
    }

    pub(crate) fn literal_assignment_payload(
        program: &dyn ::validation::CheckingInput,
        subject: &CanonicalPlace,
    ) -> Option<FactPayload> {
        if let Some(expression) = subject.root.as_expression()
            && subject.segments.is_empty()
            && is_assignment_literal(program, expression)
        {
            Some(FactPayload::AssignedValue { value: expression })
        } else {
            None
        }
    }

    /*
        Canonical assignment inputs from one immutable published context. The flow
        builder may retain these only until its semantic sweep is discarded.
    */
    #[derive(Debug)]
    pub(crate) struct AssignmentObservation {
        pub place: CanonicalPlace,
        pub payload: FactPayload,
    }

    pub(crate) fn prepare_assignment_observations(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: &FactContext,
    ) -> Vec<AssignmentObservation> {
        semantic
            .context_view(context)
            .facts()
            .filter_map(|fact| assignment_observation(program, semantic, context, fact))
            .collect()
    }

    fn assignment_observation(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: &FactContext,
        fact: &facts::Fact,
    ) -> Option<AssignmentObservation> {
        let place = assignment_place(program, semantic, fact)?;
        if has_completed_call_snapshot(program, semantic, context, fact) {
            return None;
        }
        Some(AssignmentObservation {
            place,
            payload: fact.payload,
        })
    }

    fn assignment_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        fact: &facts::Fact,
    ) -> Option<CanonicalPlace> {
        if !matches!(
            fact.payload,
            FactPayload::AssignedValue { .. }
                | FactPayload::AssignedScalarValue { .. }
                | FactPayload::AssignedIntegerBounds { .. }
        ) {
            return None;
        }
        let place = fact.place.as_place()?;
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
    }

    /*
        The immutable owned storage whose retained scalar observation remains valid
        throughout a statement. An immutable reference binding freezes its descriptor,
        not its referent, so every projection prefix must remain in owned storage.
        Range and borrow consumers share this fence before using entry observations:
        operand effects cannot then retarget the observed value within the statement.
    */
    pub(crate) fn immutable_owned_place_root(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        statement: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<symbols::SymbolHandle> {
        let symbol = place.root.as_symbol()?;
        crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .take(statement)
            .filter_map(|statement| statement.as_local_data())
            .find(|local| local.symbol == symbol && !local.is_mutable)?;
        let mut prefix = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        };
        for segment in place.segments.iter().map(Some).chain(std::iter::once(None)) {
            let reference = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement,
                &prefix,
            )?;
            let reference = validation::peel_declared_constraints(program, reference);
            if matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            ) {
                return None;
            }
            if let Some(segment) = segment {
                if !matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                ) {
                    return None;
                }
                prefix.segments.push(*segment);
            }
        }
        Some(symbol)
    }

    fn scalar_literal(
        program: &dyn validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ScalarValue> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
            ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{payloads_at_place, scalar_value_from_payloads};
        use facts::{Fact, FactPayload, FactPlace, FactPlan, PlaceRoot, ScalarValue};
        use std::cell::Cell;
        use typed_trees::{TypedTrees, expression::ExpressionNode};
        use validation::CanonicalPlace;

        #[test]
        fn scalar_payload_stream_preserves_results_and_stops_after_refusal() {
            let mut program = TypedTrees::default();
            let literal = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let symbol = symbols::SymbolHandle::from_arena_index(1);
            let subject = CanonicalPlace {
                root: PlaceRoot::Symbol(symbol),
                segments: Vec::new(),
            };
            for (first, second, expected, expected_reads) in [
                (ScalarValue::Unknown, ScalarValue::Boolean(true), None, 1),
                (
                    ScalarValue::Boolean(true),
                    ScalarValue::Boolean(false),
                    None,
                    2,
                ),
                (
                    ScalarValue::Boolean(true),
                    ScalarValue::Boolean(true),
                    Some(ScalarValue::Boolean(true)),
                    64,
                ),
            ] {
                let mut semantic = FactPlan::default();
                let place = semantic.append_symbol_place(symbol);
                let contexts = (0..64)
                    .map(|position| {
                        let value = semantic.scalar_values.append(if position == 0 {
                            first.clone()
                        } else {
                            second.clone()
                        });
                        let fact = semantic.append_fact(Fact {
                            place: FactPlace::Place(place),
                            payload: FactPayload::AssignedScalarValue { value },
                            ..Default::default()
                        });
                        let mut refs = arena::HandleSpan::empty();
                        semantic.append_ref(&mut refs, fact);
                        semantic.append_context(Default::default(), refs)
                    })
                    .collect::<Vec<_>>();
                let eager_reads = Cell::new(0);
                let eager = payloads_at_place(
                    &program,
                    &semantic,
                    contexts.iter().map(|context| {
                        eager_reads.set(eager_reads.get() + 1);
                        semantic.contexts.get(*context)
                    }),
                    &subject,
                )
                .collect::<Vec<_>>();
                let direct = scalar_value_from_payloads(&program, &semantic, eager);
                let stream_reads = Cell::new(0);
                let streamed = scalar_value_from_payloads(
                    &program,
                    &semantic,
                    payloads_at_place(
                        &program,
                        &semantic,
                        contexts.iter().map(|context| {
                            stream_reads.set(stream_reads.get() + 1);
                            semantic.contexts.get(*context)
                        }),
                        &subject,
                    ),
                );
                assert_eq!(direct, expected);
                assert_eq!(streamed, direct);
                assert_eq!(eager_reads.get(), 64);
                assert_eq!(stream_reads.get(), expected_reads);

                let literal_subject = CanonicalPlace {
                    root: PlaceRoot::Expression(literal),
                    segments: Vec::new(),
                };
                let literal_reads = Cell::new(0);
                let literal_result = scalar_value_from_payloads(
                    &program,
                    &semantic,
                    payloads_at_place(
                        &program,
                        &semantic,
                        contexts.iter().map(|context| {
                            literal_reads.set(literal_reads.get() + 1);
                            semantic.contexts.get(*context)
                        }),
                        &literal_subject,
                    ),
                );
                assert_eq!(literal_result, Some(ScalarValue::Boolean(true)));
                assert_eq!(literal_reads.get(), 0);
                eprintln!(
                    "scalar payloads: eager contexts={}, streamed contexts={}, literal contexts={}",
                    eager_reads.get(),
                    stream_reads.get(),
                    literal_reads.get(),
                );
            }
        }
        fn context_with_facts(
            semantic: &mut FactPlan,
            facts: impl IntoIterator<Item = Fact>,
        ) -> facts::FactContextHandle {
            let mut refs = arena::HandleSpan::empty();
            for fact in facts {
                let fact = semantic.append_fact(fact);
                semantic.append_ref(&mut refs, fact);
            }
            semantic.append_context(Default::default(), refs)
        }

        fn stored_scalar(
            semantic: &mut FactPlan,
            place: facts::PlaceHandle,
            value: ScalarValue,
        ) -> Fact {
            let value = semantic.scalar_values.append(value);
            Fact {
                place: FactPlace::Place(place),
                payload: FactPayload::AssignedScalarValue { value },
                ..Default::default()
            }
        }

        #[test]
        fn root_assignment_inputs_preserve_prefix_extension_scope_and_snapshots() {
            use super::RootAssignmentObservations;
            use facts::{FactOrigin, IntegerRange, Place, PlaceSegment, ProgramPoint};
            use numerics::bignum::BigInt;
            use typed_trees::expression::TableCallExpression;

            let mut program = TypedTrees::default();
            let literal = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let root = PlaceRoot::Symbol(symbols::SymbolHandle::from_parts(1, 11));
            let first = CanonicalPlace {
                root,
                segments: vec![PlaceSegment::FixedIndex { index: 0 }],
            };
            let last = CanonicalPlace {
                root,
                segments: vec![PlaceSegment::FixedIndex { index: 1 }],
            };
            for (initial, following, expected, prefix, visited) in [
                (ScalarValue::Unknown, ScalarValue::Boolean(true), None, 1, 1),
                (
                    ScalarValue::Boolean(true),
                    ScalarValue::Boolean(false),
                    None,
                    2,
                    2,
                ),
                (
                    ScalarValue::Boolean(true),
                    ScalarValue::Boolean(true),
                    Some(ScalarValue::Boolean(true)),
                    65,
                    64,
                ),
            ] {
                let mut semantic = FactPlan::default();
                let first_segments = semantic.place_segments.insert_many(first.segments.clone());
                let first_place = semantic.append_place(Place {
                    root,
                    segments: first_segments,
                });
                let last_segments = semantic.place_segments.insert_many(last.segments.clone());
                let last_place = semantic.append_place(Place {
                    root,
                    segments: last_segments,
                });
                let mut contexts = Vec::new();
                for position in 0..64 {
                    let fact = stored_scalar(
                        &mut semantic,
                        first_place,
                        if position == 0 {
                            initial.clone()
                        } else {
                            following.clone()
                        },
                    );
                    let mut facts = vec![fact];
                    if position == 63 {
                        facts.push(stored_scalar(
                            &mut semantic,
                            last_place,
                            ScalarValue::Boolean(true),
                        ));
                    }
                    contexts.push(context_with_facts(&mut semantic, facts));
                }
                let mut inputs = RootAssignmentObservations::new(&program, &semantic, &contexts);
                let direct_visits = Cell::new(0);
                for _ in 0..3 {
                    assert_eq!(
                        super::scalar_value_at_place(
                            &program,
                            &semantic,
                            contexts.iter().map(|handle| {
                                direct_visits.set(direct_visits.get() + 1);
                                semantic.contexts.get(*handle)
                            }),
                            &first,
                        ),
                        expected,
                    );
                    assert_eq!(inputs.scalar_value(&first), expected);
                    assert_eq!(inputs.roots[&root].observations.len(), prefix);
                    assert_eq!(inputs.roots[&root].work.contexts, visited);
                    assert_eq!(inputs.roots[&root].work.canonicalizations, prefix);
                    assert_eq!(inputs.roots[&root].work.snapshots, visited);
                }
                assert_eq!(direct_visits.get(), visited * 3);
                let first_work = inputs.roots[&root].work;
                // A refused first field cannot exhaust the root for another field.
                assert_eq!(inputs.scalar_value(&last), Some(ScalarValue::Boolean(true)));
                assert_eq!(inputs.roots[&root].observations.len(), 65);
                assert_eq!(inputs.roots[&root].work.contexts, 64);
                assert_eq!(inputs.roots[&root].work.canonicalizations, 65);
                assert_eq!(inputs.roots[&root].work.snapshots, visited + 1);
                let completed_work = inputs.roots[&root].work;
                for _ in 0..3 {
                    assert_eq!(inputs.scalar_value(&first), expected);
                    assert_eq!(inputs.scalar_value(&last), Some(ScalarValue::Boolean(true)));
                    assert_eq!(inputs.roots[&root].observations.len(), 65);
                    assert_eq!(inputs.roots[&root].work, completed_work);
                }
                let direct = super::scalar_value_at_place(
                    &program,
                    &semantic,
                    contexts.iter().map(|handle| semantic.contexts.get(*handle)),
                    &first,
                );
                assert_eq!(direct, expected);
                let retained_segments: usize = inputs.roots[&root]
                    .observations
                    .iter()
                    .map(|row| row.place.segments.len())
                    .sum();
                assert_eq!(retained_segments, 65);
                assert!(inputs.roots[&root].observations.capacity() <= 65 * 2);
                let literal_place = CanonicalPlace {
                    root: PlaceRoot::Expression(literal),
                    segments: Vec::new(),
                };
                assert_eq!(
                    inputs.scalar_value(&literal_place),
                    Some(ScalarValue::Boolean(true))
                );
                assert!(!inputs.roots.contains_key(&literal_place.root));
                eprintln!(
                    "root inputs: direct contexts={} for 3 queries, prepared={first_work:?}, extended={completed_work:?}, after replays={:?}, retained rows=65, retained segments={retained_segments}, literal cursors=0",
                    direct_visits.get(),
                    inputs.roots[&root].work,
                );
            }

            // A scalar refusal on a range is not a refusal to read its complete hull.
            let mut semantic = FactPlan::default();
            let place = semantic.append_symbol_place(root.as_symbol().unwrap());
            let subject = CanonicalPlace {
                root,
                segments: Vec::new(),
            };
            let mut contexts = Vec::new();
            for (minimum, maximum) in [(2, 5), (-4, 13)] {
                let bounds = semantic.integer_ranges.append(IntegerRange {
                    minimum: BigInt::from_i64(minimum),
                    maximum: BigInt::from_i64(maximum),
                });
                contexts.push(context_with_facts(
                    &mut semantic,
                    [Fact {
                        place: FactPlace::Place(place),
                        payload: FactPayload::AssignedIntegerBounds { bounds },
                        ..Default::default()
                    }],
                ));
            }
            let mut inputs = RootAssignmentObservations::new(&program, &semantic, &contexts);
            assert_eq!(inputs.scalar_value(&subject), None);
            assert_eq!(inputs.roots[&root].observations.len(), 1);
            let hull = inputs.integer_bounds(&subject).unwrap();
            assert_eq!(hull.minimum, BigInt::from_i64(-4));
            assert_eq!(hull.maximum, BigInt::from_i64(13));
            assert_eq!(inputs.roots[&root].observations.len(), 2);
            assert_eq!(inputs.scalar_value(&subject), None);
            assert_eq!(inputs.roots[&root].observations.len(), 2);

            drop(inputs);

            // Equal slots in different generations and different rosters stay separate.
            let mut semantic = FactPlan::default();
            let place = semantic.append_symbol_place(root.as_symbol().unwrap());
            let fact = stored_scalar(&mut semantic, place, ScalarValue::Boolean(false));
            let before = context_with_facts(&mut semantic, [fact]);
            semantic.contexts.begin_page(29);
            let other_root = PlaceRoot::Symbol(symbols::SymbolHandle::from_parts(1, 12));
            let other_place = semantic.append_symbol_place(other_root.as_symbol().unwrap());
            let true_fact = stored_scalar(&mut semantic, place, ScalarValue::Boolean(true));
            let other_fact = stored_scalar(&mut semantic, other_place, ScalarValue::Boolean(false));
            let after = context_with_facts(&mut semantic, [true_fact, other_fact]);
            assert_eq!(before.arena_index(), after.arena_index());
            assert_ne!(before.generation(), after.generation());
            let mut root_facts = semantic.assignments_with_root(semantic.contexts.get(after), root);
            assert_eq!(root_facts.next(), Some(&true_fact));
            assert_eq!(root_facts.next(), None);
            assert_eq!(root_facts.next(), None);
            let before_roster = [before];
            let after_roster = [after, after];
            let mut old_inputs =
                RootAssignmentObservations::new(&program, &semantic, &before_roster);
            let mut new_inputs =
                RootAssignmentObservations::new(&program, &semantic, &after_roster);
            assert_eq!(
                old_inputs.scalar_value(&subject),
                Some(ScalarValue::Boolean(false))
            );
            assert_eq!(
                new_inputs.scalar_value(&subject),
                Some(ScalarValue::Boolean(true))
            );
            assert_eq!(new_inputs.roots[&root].observations.len(), 2);
            assert_eq!(
                new_inputs.scalar_value(&CanonicalPlace {
                    root: other_root,
                    segments: Vec::new()
                }),
                Some(ScalarValue::Boolean(false))
            );
            assert_eq!(new_inputs.roots[&other_root].observations.len(), 2);
            assert_eq!(
                old_inputs.scalar_value(&subject),
                Some(ScalarValue::Boolean(false))
            );

            drop(old_inputs);
            drop(new_inputs);

            // Completed call observations suppress only the same context/place/point/origin.
            let call = program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: Default::default(),
                    authored_machine_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    requirement_application: Default::default(),
                    receiver: Default::default(),
                    target_symbol: symbols::SymbolHandle::from_arena_index(2),
                    static_machine_parameter: Default::default(),
                    target: typed_trees::name::Identifier::generated("snapshot"),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }));

            let mut semantic = FactPlan::default();
            let place = semantic.append_symbol_place(root.as_symbol().unwrap());
            let segments = semantic.place_segments.insert_many(last.segments.clone());
            let other_place = semantic.append_place(Place { root, segments });
            let source = Fact {
                place: FactPlace::Place(other_place),
                payload: FactPayload::AssignedValue { value: call },
                origin: FactOrigin::CallEnsures,
                ..Default::default()
            };
            let mut completed =
                stored_scalar(&mut semantic, other_place, ScalarValue::Boolean(true));
            completed.origin = source.origin;
            let ordinary = stored_scalar(&mut semantic, place, ScalarValue::Boolean(true));
            let context = context_with_facts(&mut semantic, [source, completed, ordinary]);
            let contexts = [context];
            let mut inputs = RootAssignmentObservations::new(&program, &semantic, &contexts);
            assert_eq!(
                inputs.scalar_value(&subject),
                Some(ScalarValue::Boolean(true))
            );
            assert_eq!(inputs.roots[&root].work.canonicalizations, 3);
            assert_eq!(inputs.roots[&root].work.snapshots, 1);
            assert!(
                inputs.roots[&root].observations[0]
                    .completed_snapshot
                    .get()
                    .is_none()
            );
            assert!(
                inputs.roots[&root].observations[1]
                    .completed_snapshot
                    .get()
                    .is_none()
            );
            assert_eq!(inputs.scalar_value(&last), Some(ScalarValue::Boolean(true)));
            assert_eq!(inputs.roots[&root].work.snapshots, 3);
            let completed_work = inputs.roots[&root].work;
            assert_eq!(inputs.scalar_value(&last), Some(ScalarValue::Boolean(true)));
            assert_eq!(inputs.roots[&root].work, completed_work);
            drop(inputs);

            for (same_context, same_point, same_origin, expected) in [
                (true, true, true, Some(ScalarValue::Boolean(true))),
                (false, true, true, None),
                (true, false, true, None),
                (true, true, false, None),
            ] {
                let mut semantic = FactPlan::default();
                let place = semantic.append_symbol_place(root.as_symbol().unwrap());
                let source = Fact {
                    place: FactPlace::Place(place),
                    payload: FactPayload::AssignedValue { value: call },
                    origin: FactOrigin::CallEnsures,
                    ..Default::default()
                };
                let mut completed = stored_scalar(&mut semantic, place, ScalarValue::Boolean(true));
                completed.origin = if same_origin {
                    source.origin
                } else {
                    FactOrigin::StatementTransfer
                };
                completed.point = if same_point {
                    source.point
                } else {
                    ProgramPoint::Definition {
                        symbol: root.as_symbol().unwrap(),
                    }
                };
                let contexts = if same_context {
                    vec![context_with_facts(&mut semantic, [source, completed])]
                } else {
                    vec![
                        context_with_facts(&mut semantic, [source]),
                        context_with_facts(&mut semantic, [completed]),
                    ]
                };
                let direct = super::scalar_value_at_place(
                    &program,
                    &semantic,
                    contexts.iter().map(|handle| semantic.contexts.get(*handle)),
                    &subject,
                );
                let mut inputs = RootAssignmentObservations::new(&program, &semantic, &contexts);
                for _ in 0..2 {
                    assert_eq!(inputs.scalar_value(&subject), direct);
                    assert_eq!(direct, expected);
                }
            }
        }
    }
}
pub(crate) use evaluation::boolean as evaluate_checked_boolean;
pub(crate) use evaluation::evaluate as evaluate_checked_scalar;
pub(crate) use evaluation::{
    BoundScalarValues, ScalarValueSource, admitted_integer, integer_magnitude, integer_type,
    representation_view_value,
};
pub(crate) use place_intervals::LivePlaceIntervals;
pub(crate) use snapshots::{
    AssignmentObservation, AssignmentObservationBatch, PlaceScalarValues,
    RootAssignmentObservations, ieee_equal_spelling_at_place, immutable_owned_place_root,
    integer_bounds_at_place, is_assignment_literal, literal_assignment_payload, literal_at_place,
    prepare_assignment_observations, scalar_value_at_place, scalar_value_from_payloads,
};
#[path = "values_scalar.rs"]
pub(crate) mod scalar;
pub(crate) use scalar::subslice_endpoints;

/*
    Owned mutable parameters use current-state scalar storage, never reference
    rebinding or the immutable incoming-parameter expression namespace.
    IEEE carriers have the same invocation-local storage obligation as integers:
    their selected operations read the current bits, including signed zero and
    NaN, rather than the entry snapshot. Receiving refinements remain the
    declaration-owned storage planner's separate admission check.
*/
pub(crate) fn mutable_scalar_parameter_type(
    program: &dyn ::validation::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<typed_trees::types::PrimitiveType> {
    use typed_trees::types::PrimitiveType;
    if (!parameter.is_mutable) || parameter.is_const || !parameter.symbol.is_valid() {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    (matches!(
        primitive,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
    ) || primitive.is_fixed_integer())
    .then_some(primitive)
}

pub(crate) use scalar::{
    applied_projection_segments, assignment_target_primitive_type,
    build_checked_scalar_expression_plans, evaluate_closed_boolean_expression, exclusive_reference,
    is_scalar_return_call, landed_for_primitive, lower_callable_parameter_range_requirements,
    lower_callable_scalar_contract_predicate, lower_current_state_predicate, lower_entry_predicate,
    lower_entry_scalar, lower_machine_entry_boolean_expression,
    lower_machine_entry_crash_contract_expression, lower_machine_entry_scalar_argument,
    lower_machine_parameter_boolean_expression, lower_operator_crash_contract_expression,
    lower_parameter_boolean_expression, lower_parameter_scalar_expression,
    lower_scalar_contract_predicate, lower_scalar_parameter_refinement_predicates,
    lower_signature_crash_contract_expression, lower_state_domain_predicate,
    lower_state_scalar_contract_predicate, lower_state_scalar_expression,
    lower_unit_scalar_argument, lower_unit_scalar_argument_for_places,
    nested_structural_call_return_type, occupies_scalar_position, operator_is_builtin,
    parameter_position, resolve_structural_parameter_path, retain_nested_call_arguments,
    scalar_expression_type, structural_data, structural_parameter_place,
    unit_scalar_binding_symbols, zeroed_scalar_local_initializer,
};

pub(crate) fn build_value_facts(
    program: &dyn ::validation::CheckingInput,
    proof_plan: &ProofPlan<'_>,
) -> CheckedValueFacts {
    let assignment_ranges = proof::checker::AssignmentRangeContext::new(proof_plan);
    let mut facts = CheckedValueFacts::default();
    for (reference, _) in program.type_reference_table.type_reference_rows() {
        let Some(primitive_type) = program.primitive_type_reference(reference) else {
            continue;
        };
        if let Some(predicates) = validation::scalar_type_refinement_predicates(
            program,
            reference,
            checked_trees::CheckedScalarExpression::Parameter {
                position: 0,
                scalar_type: primitive_type.into(),
            },
        ) {
            facts.retain_scalar_type_refinement(reference, predicates);
        }
    }
    validation::visit_executable_value_occurrences(
        program,
        &typed_trees::references::DeclarationSelection::all(),
        &mut |expression, origin, expected_primitive| {
            let type_reference =
                validation::expression_type_reference_for_origin(program, expression, origin)
                    .unwrap_or_else(typed_trees::types::TypeReferenceHandle::invalid);
            let primitive_type = program
                .primitive_type_reference(type_reference)
                .or(expected_primitive);
            let integer_range = match (
                origin,
                program.expression_table.expression(expression),
                expected_primitive,
            ) {
                (
                    CheckedValueOrigin::StateStatement {
                        role: CheckedValueStatementRole::CallArgument,
                        ..
                    },
                    typed_trees::expression::ExpressionNode::Integer(literal),
                    Some(_),
                ) => literal.value_bignum().map(|value| CheckedIntegerRange {
                    minimum: value.clone(),
                    maximum: value,
                }),
                (
                    CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        role: CheckedValueStatementRole::AssignmentValue,
                    },
                    _,
                    _,
                ) => proof::checker::proved_assignment_integer_range_with_context(
                    proof_plan,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    &assignment_ranges,
                )
                .map(|range| CheckedIntegerRange {
                    minimum: range.minimum,
                    maximum: range.maximum,
                }),
                _ => None,
            };
            facts.values.insert(CheckedValueFact {
                expression,
                origin,
                type_reference,
                primitive_type,
                integer_range,
            });
        },
    );
    facts
}
