/*
    Borrow facts: the writable roots, loans, borrowing calls and argument
    accesses of every state, recorded as `checked_trees::BorrowFacts` for the
    later fact builders and the borrow checks in `checks::borrows`.

    `build_borrow_facts` is the entry; `facts::build_check_facts` calls it
    first. It sizes the arenas (`roots`), then for every state of every
    machine `state::append_state_borrow_facts` records, in order: the state's
    writable roots (`roots`); for each statement, the loans it forms
    (`loans`), each tracked as a `tracker::StateLoanTracker`, and its
    borrowing calls (`calls`, which records their argument accesses through
    `accesses`); then each loan's last use (`last_uses`); and finally the
    state's `StateBorrowFact` row.

    `view_link` resolves which input a returned view borrows; `loans` uses it
    to link a call result's loan to its argument, and `checks::borrows` uses
    it for the returned-view checks. `last_uses` and `loans` also export
    queries used elsewhere in the crate.

    Stored input descriptors seed opaque referent identities, not loans of the
    parameter's descriptor slots. Extraction and ordinary transfers preserve
    those identities in actual statement loans. Formation reads current carrier
    contents; replacement retires that slot's old occurrence after its RHS,
    while the complete historical ledger still owns lifetime analysis.
    Normal result leaves are recorded separately at formation, using that same
    current roster. Escape and simultaneous-hold checks consume those rows;
    they never rebuild a return's contents from historical local activations.
*/

use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use validation::{CallFrameResolver, machine_state_count};

use checked_trees::BorrowFacts;
pub(crate) mod accesses {
    /*
        The places a call's arguments access. `collect_argument_accesses`
        appends one `BorrowArgumentAccessFact` per accessed place and returns the
        call's span of them: a `&`, `&mut` or `&write` argument records a read,
        mutable or write-only access to its borrowed place, and any other argument
        records a read access for each place it reads (`read`).
        Forwarded reference bindings retain the selected formal's loan access;
        a reference name is not a copied read of its referent.
        Retained rows also distinguish copied observations and nested-call loans
        that finish during operand evaluation from loans held through invocation.
        `borrow::calls` calls it while building borrow facts, and `checks::borrows::calls::receiver`
        calls it into scratch arenas for the operand that follows an exclusive
        operation's receiver. A transition arm's target roster is the same
        collection under its own schedule, which also ends the reads of payloads
        its dispatch already copied (`validation::DispatchExtraction`).
        Each row retains the operand that produced it. The statement read judgment
        leaves a call's staged operands to that call's own judgment without a
        separate expression-recording mode or a parallel provenance walk.

        `borrow_access_place` (`place`) resolves an expression to the place it
        accesses, with `contextual` resolving names and members in the state and
        machine; the borrow checks use it too. `collection` holds the arenas one
        call appends to, and `records` writes one access row.
    */
    use crate::published::places::place_at;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
    use symbols::SymbolHandle;
    mod collection {
        use super::records;
        use crate::borrow::accesses::BorrowAccessPlace;
        use crate::borrow::accesses::borrow_access_place;
        use checked_trees::expression::ExpressionHandle;
        use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
        use records::append_argument_access;
        use symbols::SymbolHandle;
        use validation::DispatchExtraction;

        pub(super) struct BorrowAccessCollection<'a> {
            pub(super) program: &'a dyn ::validation::CheckingInput,
            access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
            argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
            accesses: &'a mut arena::HandleSpan<BorrowArgumentAccessFact>,
            pub(super) state_symbol: SymbolHandle,
            pub(super) statement_index: usize,
            pub(super) machine_symbol: SymbolHandle,
            dispatch_extraction: DispatchExtraction,
        }

        impl<'a> BorrowAccessCollection<'a> {
            pub(super) fn new(
                program: &'a dyn ::validation::CheckingInput,
                access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
                argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
                accesses: &'a mut arena::HandleSpan<BorrowArgumentAccessFact>,
                state_symbol: SymbolHandle,
                statement_index: usize,
                machine_symbol: SymbolHandle,
            ) -> Self {
                Self {
                    program,
                    access_segments,
                    argument_accesses,
                    accesses,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    dispatch_extraction: DispatchExtraction::default(),
                }
            }

            /*
                Collect a transition arm's own target roster, whose payload bindings
                were copied out of the dispatch's saved subject before the call.
            */
            pub(super) fn after_dispatch_extraction(mut self) -> Self {
                if let Some(machine) = self.program.machine_by_symbol(self.machine_symbol)
                    && let Some(state) = self.program.state_in_machine(machine, self.state_symbol)
                {
                    self.dispatch_extraction = DispatchExtraction::of_transition_arm(
                        self.program,
                        machine,
                        state,
                        self.statement_index,
                    );
                }
                self
            }

            pub(super) fn with_completed_dispatch_extraction(
                mut self,
                extraction: DispatchExtraction,
            ) -> Self {
                self.dispatch_extraction = extraction;
                self
            }

            pub(super) fn borrow_access_place(
                &self,
                expression: ExpressionHandle,
            ) -> Option<BorrowAccessPlace> {
                borrow_access_place(
                    self.program,
                    self.state_symbol,
                    self.statement_index,
                    expression,
                    self.machine_symbol,
                )
            }

            pub(super) fn value_type_reference(
                &self,
                expression: ExpressionHandle,
            ) -> Option<typed_trees::types::TypeReferenceHandle> {
                let (machine, state) = validation::machine_state_by_symbols(
                    self.program,
                    self.machine_symbol,
                    self.state_symbol,
                )?;
                validation::expression_result_type_reference(
                    self.program,
                    machine,
                    state,
                    expression,
                )
            }

            pub(super) fn append_argument_access(
                &mut self,
                access_place: BorrowAccessPlace,
                kind: BorrowAccessKind,
                expression: ExpressionHandle,
            ) {
                self.append_access(access_place, kind, None, expression);
            }

            pub(super) fn append_read_access(
                &mut self,
                access_place: BorrowAccessPlace,
                expression: ExpressionHandle,
            ) {
                let end_before_access = self.is_copyable_value(expression).then(|| {
                    if self.reads_extracted_copy(expression) {
                        0
                    } else {
                        self.access_count() + 1
                    }
                });
                self.append_access(
                    access_place,
                    BorrowAccessKind::Read,
                    end_before_access,
                    expression,
                );
            }

            pub(super) fn access_count(&self) -> usize {
                self.accesses.count() as usize
            }

            pub(super) fn reads_extracted_copy(&self, expression: ExpressionHandle) -> bool {
                self.dispatch_extraction
                    .reads_extracted_copy(self.program, expression, |payload| {
                        self.is_copyable_value(payload)
                    })
            }

            pub(super) fn retain_receiver_accesses(&mut self, first_access: usize) {
                for access in self
                    .argument_accesses
                    .span_mut_or_empty(*self.accesses)
                    .iter_mut()
                    .skip(first_access)
                {
                    access.end_before_access = None;
                    access.released_at = Default::default();
                }
            }

            pub(super) fn finish_completed_value(
                &mut self,
                first_access: usize,
                expression: ExpressionHandle,
                reference: typed_trees::types::TypeReferenceHandle,
            ) {
                if !reference.is_valid()
                    || !validation::has_contents(
                        self.program,
                        reference,
                        &[],
                        validation::ContentsRequirement::BorrowFree,
                    )
                {
                    return;
                }
                let end_before_access = self.access_count();
                for access in self
                    .argument_accesses
                    .span_mut_or_empty(*self.accesses)
                    .iter_mut()
                    .skip(first_access)
                {
                    if access.end_before_access.is_none() {
                        access.end_before_access = Some(end_before_access);
                        access.released_at = expression;
                    }
                }
            }

            /*
                The completed result and its producing inputs are one staged actual.
                Keep input holds live: an unselected input may still be owed by the
                result's declared lifetime. Only comparing the result with its own
                evaluation would count the same access twice; sibling actuals must
                still respect every retained input.
            */
            pub(super) fn bind_completed_argument(&mut self, first_access: usize) {
                if let Some(result) = self
                    .argument_accesses
                    .span_mut_or_empty(*self.accesses)
                    .last_mut()
                {
                    result.evaluation_start = Some(first_access);
                }
            }

            pub(super) fn is_copyable_value(&self, expression: ExpressionHandle) -> bool {
                self.value_type_reference(expression)
                    .is_some_and(|reference| super::is_copyable_value_type(self.program, reference))
            }

            fn append_access(
                &mut self,
                access_place: BorrowAccessPlace,
                kind: BorrowAccessKind,
                end_before_access: Option<usize>,
                expression: ExpressionHandle,
            ) {
                append_argument_access(
                    self.access_segments,
                    self.argument_accesses,
                    self.accesses,
                    access_place,
                    kind,
                    end_before_access,
                    expression,
                );
            }
        }
    }
    mod completed_dispatch {
        /*
            Complete a transition target's extraction lifetimes from actual observations.
            The early borrow producer cannot yet know the reaching stored operand graph.
            After flow settles, re-run its same ordered collector with authenticated
            observation/source correspondences. Only identical access rosters can gain
            the existing copied-payload end before access zero; paths, kinds and loans
            are unchanged. Nested operand invocations retain their own ordinary ends.
        */
        use arena::Arena;
        use checked_trees::CheckFacts;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use validation::CallSite;
        use validation::CheckingInput;
        use validation::DispatchExtraction;
        use validation::call_site_argument_expressions;
        use validation::find_call_site_in_state;
        use validation::transition_call_target;

        pub(crate) fn retain_completed_dispatch_extractions(
            program: &dyn CheckingInput,
            facts: &mut CheckFacts,
        ) {
            let calls = facts
                .borrow
                .states
                .iter()
                .flat_map(|(_, state)| {
                    facts
                        .borrow
                        .calls
                        .span_or_empty(state.calls)
                        .iter()
                        .cloned()
                        .map(move |call| (state.machine_symbol, state.state_symbol, call))
                })
                .collect::<Vec<_>>();
            for (machine, state, call) in calls {
                let Some(machine) = program.machine_by_symbol(machine) else {
                    continue;
                };
                let Some(state) = program.state_in_machine(machine, state) else {
                    continue;
                };
                let Some((site, extraction)) = transition_invocation(
                    program,
                    crate::flow::CapturedSubjectFacts::from(&*facts),
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let arguments = call_site_argument_expressions(program, &site);
                let mut segments = Arena::new();
                let mut accesses = Arena::new();
                let expected = super::collect_argument_accesses(
                    program,
                    &mut segments,
                    &mut accesses,
                    arguments,
                    state.symbol,
                    call.statement_index,
                    machine.symbol,
                    call.target_symbol,
                    super::ArgumentAccessSchedule::TransitionTarget(extraction),
                );
                let Some(expected) = accesses.span(expected) else {
                    continue;
                };
                let Some(actual) = facts.borrow.argument_accesses.span(call.accesses) else {
                    continue;
                };
                if actual.len() != expected.len()
                    || !actual.iter().zip(expected).all(|(actual, expected)| {
                        actual.expression == expected.expression
                            && actual.root == expected.root
                            && actual.kind == expected.kind
                            && facts.borrow.access_segments.span(actual.segments)
                                == segments.span(expected.segments)
                    })
                {
                    continue;
                }
                for (actual, expected) in facts
                    .borrow
                    .argument_accesses
                    .span_mut_or_empty(call.accesses)
                    .iter_mut()
                    .zip(expected)
                {
                    if actual.end_before_access.is_some() && expected.end_before_access == Some(0) {
                        actual.end_before_access = Some(0);
                        actual.released_at = expected.released_at;
                    }
                }
            }
        }

        /*
            Only the exact whole arm invocation inherits the dispatch's completed
            payload copies. A nested call, even to the same declaration, does not.
            The source collector authenticates exact selected case projections itself.
            Completed flow correspondence only enriches that relation for projections
            naming a different captured source; absence cannot manufacture such an alias.
            Both publication and independent access replay use this same authority.
        */
        pub(super) fn transition_invocation<'program>(
            program: &'program dyn CheckingInput,
            facts: crate::flow::CapturedSubjectFacts<'_>,
            machine: &'program Machine,
            state: &'program State,
            statement_index: usize,
            call_ordinal: usize,
        ) -> Option<(CallSite<'program>, Option<DispatchExtraction>)> {
            let site =
                find_call_site_in_state(program, machine, state, statement_index, call_ordinal)?;
            let target =
                transition_call_target(program, machine, state, statement_index, call_ordinal)?;
            if !site.completes_transition(program, target) {
                return None;
            }
            let extraction = crate::flow::completed_dispatch_extraction(
                program,
                facts,
                machine,
                state,
                statement_index,
            );
            Some((site, extraction))
        }
    }
    mod place {
        use crate::published::places::place_at;
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};
        use symbols::SymbolHandle;

        pub(crate) use checked_trees::CapturedPlace as BorrowAccessPlace;

        /*
            The declaration's outer reference grants its referent namespace directly.
            Stored entry leaves belong to that referent, even when a nested reference
            gives one the same empty structural owner path. This type/role query grants
            no contents fact, inner descriptor permission or lifetime extension.
        */
        pub(crate) fn is_borrowed_parameter_root(
            program: &dyn validation::CheckingInput,
            root: facts::PlaceRoot,
        ) -> bool {
            let Some(symbol) = root.as_symbol() else {
                return false;
            };
            if !symbol.is_valid()
                || program.symbols.get(symbol).kind != symbols::SymbolKind::Parameter
            {
                return false;
            }
            let Some(state) = program.state_by_symbol(program.symbols.get(symbol).parent) else {
                return false;
            };
            program
                .state_parameter_by_symbol(state, symbol)
                .is_some_and(|parameter| {
                    !parameter.is_const
                        && !parameter.relevance.is_erased()
                        && validation::reference_shell(program, parameter.type_reference).is_some()
                })
        }

        /*
            Paired coordinates produced by the one reference-formation judgment. The
            representation address is distinct from its full retained byte footprint.
            Both enter the same alias rebase; no consumer reconstructs admission.
        */
        pub(crate) struct BorrowAccessGeometry {
            pub(crate) place: BorrowAccessPlace,
            pub(crate) representation_address: Option<BorrowAccessPlace>,
        }

        impl BorrowAccessGeometry {
            pub(crate) fn declared(place: BorrowAccessPlace) -> Self {
                Self {
                    place,
                    representation_address: None,
                }
            }

            /*
                Primitive image formation can supply only read access, independently
                of the receiving reference annotation. Both direct bindings and carried
                constructor leaves use this same attenuation; transport preserves the
                resulting loan kind rather than upgrading it from the destination.
            */
            pub(crate) fn retained_access(
                &self,
                program: &dyn validation::CheckingInput,
                requested: &checked_trees::BorrowAccessKind,
            ) -> checked_trees::BorrowAccessKind {
                if is_image_storage_root(program, self.place.root) {
                    checked_trees::BorrowAccessKind::Read
                } else {
                    requested.clone()
                }
            }
        }

        /*
            An exact primitive image root outlives every activation and contains only
            immutable bytes, with no interior descriptor or owned cleanup dependency.
            This classifies the already captured root, never a local's initializer or
            another equal-valued literal. Access still needs its actual loan occurrence.
        */
        pub(crate) fn is_image_storage_root(
            program: &dyn validation::CheckingInput,
            root: facts::PlaceRoot,
        ) -> bool {
            let facts::PlaceRoot::Expression(expression) = root else {
                return false;
            };
            program.expression_table.expression_is_valid(expression)
                && matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::String(_)
                )
        }

        pub(crate) fn borrow_access_place(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            expression: ExpressionHandle,
            machine_symbol: SymbolHandle,
        ) -> Option<BorrowAccessPlace> {
            borrow_access_geometry(
                program,
                state_symbol,
                statement_index,
                expression,
                machine_symbol,
            )
            .map(|geometry| geometry.place)
        }

        pub(crate) fn borrow_access_geometry(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            expression: ExpressionHandle,
            machine_symbol: SymbolHandle,
        ) -> Option<BorrowAccessGeometry> {
            if is_image_storage_root(program, facts::PlaceRoot::Expression(expression)) {
                return Some(BorrowAccessGeometry::declared(BorrowAccessPlace {
                    incoming_reference: Default::default(),
                    incoming_owner_path: Default::default(),
                    root: facts::PlaceRoot::Expression(expression),
                    segments: Vec::new(),
                }));
            }
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
                        let mut address = place.clone();
                        address.segments.push(facts::PlaceSegment::FixedIndex {
                            index: footprint.start(),
                        });
                        place.segments.push(facts::PlaceSegment::FixedRange {
                            start: footprint.start(),
                            end: footprint.end(),
                        });
                        return Some(BorrowAccessGeometry {
                            place,
                            representation_address: Some(address),
                        });
                    }
                    if let Some(source) = validation::validate_runtime_indexed_recast_source(
                        program,
                        machine,
                        state,
                        statement_index,
                        expression,
                        cast.target_type,
                        access,
                    ) {
                        return borrow_access_place(
                            program,
                            state_symbol,
                            statement_index,
                            source,
                            machine_symbol,
                        )
                        .map(BorrowAccessGeometry::declared);
                    }
                    let indexed = program.expression_table.indexed_node(source)?;
                    let place = borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        indexed.collection,
                        machine_symbol,
                    )?;
                    let address = borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        source,
                        machine_symbol,
                    )?;
                    return Some(BorrowAccessGeometry {
                        place,
                        representation_address: Some(address),
                    });
                }
            }
            let place = place_at(program, state_symbol, statement_index, expression)?;
            Some(BorrowAccessGeometry::declared(BorrowAccessPlace {
                incoming_reference: Default::default(),
                incoming_owner_path: Default::default(),
                root: place.root,
                segments: place.segments,
            }))
        }
    }
    mod read {
        use super::super::children::recurse_shared_children;
        use crate::borrow::accesses::BorrowAccessCollection;
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};

        /*
            Evaluate a loan's place without inventing a copied read of its referent.
            Selectors and computed receivers still perform their ordinary accesses
            before the loan is retained for invocation.
        */
        pub(super) fn collect_place_evaluation_reads(
            collection: &mut BorrowAccessCollection<'_>,
            expression: ExpressionHandle,
        ) {
            let program = collection.program;
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(_) => {}
                ExpressionNode::Member(member) => {
                    collect_place_evaluation_reads(collection, member.receiver);
                }
                ExpressionNode::Indexed(indexed) => {
                    collect_place_evaluation_reads(collection, indexed.collection);
                    collect_read_accesses(collection, indexed.index);
                }
                ExpressionNode::Borrow(borrow) => {
                    collect_place_evaluation_reads(collection, borrow.target);
                }
                ExpressionNode::Cast(cast) => {
                    collect_place_evaluation_reads(collection, cast.value);
                }
                _ => {
                    if !recurse_shared_children(program, expression, &mut |child| {
                        collect_read_accesses(collection, child)
                    }) {
                        collect_read_accesses(collection, expression);
                    }
                }
            }
        }

        pub(super) fn collect_read_accesses(
            collection: &mut BorrowAccessCollection<'_>,
            expression: ExpressionHandle,
        ) {
            let metadata_receiver = validation::machine_state_by_symbols(
                collection.program,
                collection.machine_symbol,
                collection.state_symbol,
            )
            .and_then(|(machine, state)| {
                validation::collection_length_receiver(
                    collection.program,
                    machine,
                    Some(state),
                    expression,
                )
            });
            if let Some(receiver) = metadata_receiver {
                collect_place_evaluation_reads(collection, receiver);
                if let Some(place) = collection.borrow_access_place(receiver) {
                    collection.append_read_access(place, expression);
                }
                return;
            }
            if recurse_shared_children(collection.program, expression, &mut |child| {
                collect_read_accesses(collection, child);
            }) {
                return;
            }
            match collection.program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    let first_access = collection.access_count();
                    let no_runtime_receiver = collection
                        .program
                        .call_has_no_runtime_receiver_for_target(call);
                    if call.receiver.is_valid() && !no_runtime_receiver {
                        collect_read_accesses(collection, call.receiver);
                        collection.retain_receiver_accesses(first_access);
                    }

                    super::collect_target_argument_accesses(
                        collection,
                        collection
                            .program
                            .expression_table
                            .expression_handles(call.arguments),
                        collection.state_symbol,
                        collection.statement_index,
                        validation::call_target_symbol(collection.program, call),
                    );
                    let reference = collection
                        .value_type_reference(expression)
                        .unwrap_or_default();
                    collection.finish_completed_value(first_access, expression, reference);
                }
                ExpressionNode::Indexed(indexed) => {
                    if !append_read_access(collection, expression) {
                        collect_read_accesses(collection, indexed.collection);
                    }

                    collect_read_accesses(collection, indexed.index);
                }
                ExpressionNode::Member(member) => {
                    if !append_read_access(collection, expression)
                        && !(member.case_variant.is_some()
                            && collection.reads_extracted_copy(expression))
                    {
                        collect_read_accesses(collection, member.receiver);
                    }
                }
                ExpressionNode::Name(_) => {
                    append_read_access(collection, expression);
                }
                ExpressionNode::Borrow(_) => {
                    super::collect_single_expression_argument_access(collection, expression)
                }
                _ => {}
            }
        }

        fn append_read_access(
            collection: &mut BorrowAccessCollection<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(access_place) = collection.borrow_access_place(expression) else {
                return false;
            };

            collection.append_read_access(access_place, expression);
            true
        }
    }
    mod records {
        use crate::borrow::accesses::BorrowAccessPlace;
        use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};

        pub(super) fn append_argument_access(
            access_segments: &mut arena::Arena<facts::PlaceSegment>,
            argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
            accesses: &mut arena::HandleSpan<BorrowArgumentAccessFact>,
            access_place: BorrowAccessPlace,
            kind: BorrowAccessKind,
            end_before_access: Option<usize>,
            expression: checked_trees::expression::ExpressionHandle,
        ) {
            argument_accesses.append_to_span(
                accesses,
                BorrowArgumentAccessFact {
                    expression,
                    evaluation_start: None,
                    root: access_place.root,
                    segments: access_segments.insert_many(access_place.segments),
                    kind,
                    end_before_access,
                    released_at: if end_before_access.is_some() {
                        expression
                    } else {
                        Default::default()
                    },
                },
            );
        }
    }
    mod replay {
        /*
            Rejoin an expression invocation's ordered access roster to its source.

            Whole transition invocations complete extraction lifetimes from flow's
            observed operand graph; nested invocations keep ordinary argument-order
            ends. Access rows are observations, not formal positions;
            callers select operands through the exact authored call site returned here.
        */
        use validation::{CallSite, call_site_argument_expressions, find_call_site};

        use checked_trees::FlowCallFact;
        use symbols::SymbolHandle;

        pub(crate) fn rejoin_expression_call_accesses<'program>(
            program: &'program dyn ::validation::CheckingInput,
            facts: crate::flow::CapturedSubjectFacts<'_>,
            machine: SymbolHandle,
            state: SymbolHandle,
            source: &FlowCallFact,
        ) -> Option<CallSite<'program>> {
            let borrow = facts.borrow;
            let state_row = borrow.unique_state_borrow(machine, state)?;
            let mut calls = borrow.calls.span(state_row.calls)?.iter().filter(|call| {
                call.statement_index == source.statement_index
                    && call.call_ordinal == source.call_ordinal
            });
            let call = calls.next()?;
            if calls.next().is_some()
                || call.target_symbol != source.target_symbol
                || call.accesses != source.accesses
                || call.has_receiver != source.has_receiver
                || call.receiver_symbol != source.receiver_symbol
            {
                return None;
            }
            let site = find_call_site(
                program,
                machine,
                state,
                source.statement_index,
                source.call_ordinal,
            )?;
            let CallSite::Expression {
                expression,
                call: named_call,
            } = &site
            else {
                return None;
            };
            if *expression != source.authored_expression
                || validation::call_target_entry_symbol(
                    program,
                    validation::call_target_symbol(program, named_call),
                ) != source.target_symbol
            {
                return None;
            }
            let arguments = call_site_argument_expressions(program, &site);
            let mut expected_segments = arena::Arena::new();
            let mut expected_accesses = arena::Arena::new();
            let (owner, state_row) = validation::machine_state_by_symbols(program, machine, state)?;
            let invocation = super::completed_dispatch::transition_invocation(
                program,
                facts,
                owner,
                state_row,
                source.statement_index,
                source.call_ordinal,
            );
            let schedule = match invocation {
                Some((_, extraction)) => {
                    super::ArgumentAccessSchedule::TransitionTarget(extraction)
                }
                None => super::ArgumentAccessSchedule::Invocation,
            };
            let expected = super::collect_argument_accesses(
                program,
                &mut expected_segments,
                &mut expected_accesses,
                arguments,
                state,
                source.statement_index,
                machine,
                source.target_symbol,
                schedule,
            );
            let expected = expected_accesses.span(expected)?;
            let actual = borrow.argument_accesses.span(call.accesses)?;
            if expected.len() != actual.len() {
                return None;
            }
            for (expected, actual) in expected.iter().zip(actual) {
                if expected.expression != actual.expression
                    || expected.root != actual.root
                    || expected.kind != actual.kind
                    || expected.end_before_access != actual.end_before_access
                    || expected.released_at != actual.released_at
                    || expected_segments.span(expected.segments)?
                        != borrow.access_segments.span(actual.segments)?
                {
                    return None;
                }
            }
            Some(site)
        }
    }
    pub(crate) use completed_dispatch::retain_completed_dispatch_extractions;
    pub(crate) use replay::rejoin_expression_call_accesses;

    use collection::BorrowAccessCollection;
    pub(crate) use place::{
        BorrowAccessGeometry, BorrowAccessPlace, borrow_access_geometry, borrow_access_place,
        is_borrowed_parameter_root, is_image_storage_root,
    };
    use read::collect_read_accesses;

    /*
        The roster one access collection reads, and how the sequence that reaches
        it already evaluated the arguments. A call's own arguments are read while
        the invocation evaluates them. A transition arm's target roster follows a
        dispatch that copied its plain-copy payload bindings out of the saved
        subject before the target evaluates any argument, and names the extraction
        when selection completed that copy first; nested calls keep `Invocation`.
    */
    pub(crate) enum ArgumentAccessSchedule {
        Invocation,
        TransitionTarget(Option<validation::DispatchExtraction>),
    }

    pub(crate) fn collect_argument_accesses(
        program: &dyn ::validation::CheckingInput,
        access_segments: &mut arena::Arena<facts::PlaceSegment>,
        argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
        arguments: &[ExpressionHandle],
        state_symbol: SymbolHandle,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        schedule: ArgumentAccessSchedule,
    ) -> arena::HandleSpan<BorrowArgumentAccessFact> {
        let mut accesses = arena::HandleSpan::empty();

        {
            let mut collection = BorrowAccessCollection::new(
                program,
                access_segments,
                argument_accesses,
                &mut accesses,
                state_symbol,
                statement_index,
                machine_symbol,
            );
            if let ArgumentAccessSchedule::TransitionTarget(completed) = schedule {
                collection = collection.after_dispatch_extraction();
                if let Some(extraction) = completed {
                    collection = collection.with_completed_dispatch_extraction(extraction);
                }
            }

            collect_target_argument_accesses(
                &mut collection,
                arguments,
                state_symbol,
                statement_index,
                target_symbol,
            );
        }

        accesses
    }

    /*
        The ordinary typed access schedule distinguishes value observations from
        source-place loans. This query reuses that schedule when a selected arm
        must complete its payload copies before the first target operand.
    */
    pub(crate) fn dispatch_target_value_reads(
        program: &dyn ::validation::CheckingInput,
        arguments: &[ExpressionHandle],
        state: SymbolHandle,
        statement: usize,
        machine: SymbolHandle,
        target: SymbolHandle,
        extraction: validation::DispatchExtraction,
    ) -> Vec<ExpressionHandle> {
        let mut segments = arena::Arena::new();
        let mut rows = arena::Arena::new();
        let accesses = collect_argument_accesses(
            program,
            &mut segments,
            &mut rows,
            arguments,
            state,
            statement,
            machine,
            target,
            ArgumentAccessSchedule::TransitionTarget(Some(extraction)),
        );
        rows.span_or_empty(accesses)
            .iter()
            .filter(|access| {
                if access.kind != BorrowAccessKind::Read
                    || matches!(
                        program.expression_table.expression(access.expression),
                        ExpressionNode::Borrow(_)
                    )
                {
                    return false;
                }
                let completed_value_read =
                    access.end_before_access.is_some() && access.released_at == access.expression;
                let descriptor_read = validation::expression_type_reference_in_state(
                    program,
                    state,
                    statement,
                    access.expression,
                )
                .is_some_and(|reference| validation::reference_shell(program, reference).is_some());
                completed_value_read || descriptor_read
            })
            .map(|access| access.expression)
            .collect()
    }

    /* Nested and outer invocations retain the same formal-directed loan access. */
    fn collect_target_argument_accesses(
        collection: &mut BorrowAccessCollection<'_>,
        arguments: &[ExpressionHandle],
        state_symbol: SymbolHandle,
        statement_index: usize,
        target_symbol: SymbolHandle,
    ) {
        let program = collection.program;
        let parameters = validation::call_target_parameters(program, target_symbol);
        let instruction_parameters = parameters
            .is_none()
            .then(|| {
                validation::compiler_operations::global_descriptor_table_operand_types(
                    program,
                    program.machine_by_symbol(collection.machine_symbol)?,
                    program.state_by_symbol(state_symbol)?,
                    target_symbol,
                    arguments,
                )
            })
            .flatten();
        for (position, argument) in arguments.iter().enumerate() {
            /*
                An erased parameter demands no executable value
                (wiki/spec/proofs/mathematical_bindings.md), so its argument
                forms no runtime read or loan. The receiving state likewise
                installs no incoming-reference tracker for an erased formal;
                recording an access here would demand a retained backing that
                no runtime carrier can hold.
            */
            if parameters
                .and_then(|rows| rows.get(position))
                .is_some_and(|parameter| parameter.relevance.is_erased())
            {
                continue;
            }
            let receiving_type = parameters
                .and_then(|rows| rows.get(position))
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    instruction_parameters
                        .as_ref()
                        .and_then(|rows| rows.get(position))
                        .copied()
                })
                .unwrap_or_default();
            collect_formal_argument_accesses(
                collection,
                *argument,
                receiving_type,
                state_symbol,
                statement_index,
            );
        }
    }

    /*
        Evaluate an argument under its receiving type, then finish inputs that its
        completed value no longer borrows before collecting the next argument.
    */
    fn collect_formal_argument_accesses(
        collection: &mut BorrowAccessCollection<'_>,
        argument: ExpressionHandle,
        formal: typed_trees::types::TypeReferenceHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) {
        let first_access = collection.access_count();
        collect_formal_argument_evaluation(
            collection,
            argument,
            formal,
            state_symbol,
            statement_index,
        );
        collection.finish_completed_value(first_access, argument, formal);
    }

    /*
        Record the reads and loans needed to evaluate one argument. Its completed
        receiving value then determines which direct input holds can end.
    */
    fn collect_formal_argument_evaluation(
        collection: &mut BorrowAccessCollection<'_>,
        argument: ExpressionHandle,
        formal: typed_trees::types::TypeReferenceHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) {
        let program = collection.program;
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(argument) {
            collect_read_accesses(collection, dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    collect_read_accesses(collection, pattern);
                }
                collect_formal_argument_accesses(
                    collection,
                    arm.value,
                    formal,
                    state_symbol,
                    statement_index,
                );
            }
            return;
        }
        let forwarded_access = (|| {
            if !formal.is_valid()
                || matches!(
                    program.expression_table.expression(argument),
                    ExpressionNode::Borrow(_)
                )
            {
                return None;
            }
            let formal_reference = formal;
            let formal = validation::reference_shell(program, formal_reference)?;
            let reference = collection.value_type_reference(argument)?;
            if validation::reference_shell(program, reference).is_none()
                && !(validation::slice_view_shell(program, formal_reference, &[]).is_some()
                    && crate::published::semantic::calls::view_argument_names_existing_place(
                        program, argument,
                    ))
            {
                return None;
            }
            Some(BorrowAccessKind::from(formal.access))
        })();
        if let Some(kind) = forwarded_access
            && let Some(place) = collection.borrow_access_place(argument)
        {
            let first_access = collection.access_count();
            read::collect_place_evaluation_reads(collection, argument);
            collection.append_argument_access(place, kind, argument);
            collection.bind_completed_argument(first_access);
            return;
        }
        let implicit_loan = (|| {
            if !formal.is_valid()
                || matches!(
                    program.expression_table.expression(argument),
                    ExpressionNode::Borrow(_)
                )
            {
                return None;
            }
            let (referent, _) = crate::published::semantic::calls::shared_argument_referent(
                program, argument, formal,
            )?;
            place_at(program, state_symbol, statement_index, referent)?
                .root
                .as_symbol()?;
            Some(BorrowAccessKind::Read)
        })();
        if let Some(kind) = implicit_loan
            && let Some(place) = collection.borrow_access_place(argument)
        {
            read::collect_place_evaluation_reads(collection, argument);
            collection.append_argument_access(place, kind, argument);
            return;
        }
        collect_single_expression_argument_access(collection, argument);
    }

    /*
        One argument expression's own access: an authored borrow records its place
        under the borrow kind, anything else records a read for each place it reads.
    */
    fn collect_single_expression_argument_access(
        collection: &mut BorrowAccessCollection<'_>,
        expression: ExpressionHandle,
    ) {
        match collection.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner_expression) => {
                read::collect_place_evaluation_reads(collection, inner_expression.target);
                if let Some(access_place) = collection.borrow_access_place(expression) {
                    collection.append_argument_access(
                        access_place,
                        BorrowAccessKind::from(inner_expression.access),
                        expression,
                    );
                }
            }
            _ => collect_read_accesses(collection, expression),
        }
    }

    /*
        A completed value read can copy descriptor bytes without reading their
        referee. This is independent of whether the value owns inline storage.
    */
    pub(crate) fn is_copyable_value_type(
        program: &dyn validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        validation::reference_result_custody::is_copyable_stored_value(program, reference)
    }

    /*
        A detached owned copy releases its source home, independently of its
        contained loan frontier. The staged-value checker retains those leaves.
        A reference descriptor value is copyable but does not own its referee.
    */
    pub(crate) fn is_owned_copy_type(
        program: &dyn validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        is_copyable_value_type(program, reference)
            && validation::reference_shell(program, reference).is_none()
            && program.type_multiplicity(reference)
                == language_semantics::Multiplicity::Unrestricted
    }
}
pub(crate) mod calls {
    use crate::borrow::accesses::ArgumentAccessSchedule;
    use validation::{statement_call_can_dispatch_to_machine, statement_call_receiver_path};

    use checked_trees::statement::{StatementNode, TransitionGuardNode};
    use checked_trees::{BorrowArgumentAccessFact, BorrowCallFact};
    mod collection {
        use crate::borrow::accesses::{ArgumentAccessSchedule, collect_argument_accesses};
        use checked_trees::expression::ExpressionHandle;
        use checked_trees::{BorrowArgumentAccessFact, BorrowCallFact};
        use symbols::SymbolHandle;

        pub(super) struct BorrowCallCollection<'a> {
            pub(super) program: &'a dyn ::validation::CheckingInput,
            pub(super) machine: &'a typed_trees::machine::Machine,
            pub(super) state: &'a typed_trees::state::State,
            pub(super) statement_index: usize,
            pub(super) call_ordinal: &'a mut usize,
            pub(super) expressions: Vec<ExpressionHandle>,
            access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
            argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
            calls: &'a mut arena::Arena<BorrowCallFact>,
            state_calls: &'a mut arena::HandleSpan<BorrowCallFact>,
        }

        impl<'a> BorrowCallCollection<'a> {
            pub(super) fn new(
                program: &'a dyn ::validation::CheckingInput,
                machine: &'a typed_trees::machine::Machine,
                state: &'a typed_trees::state::State,
                statement_index: usize,
                call_ordinal: &'a mut usize,
                access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
                argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
                calls: &'a mut arena::Arena<BorrowCallFact>,
                state_calls: &'a mut arena::HandleSpan<BorrowCallFact>,
            ) -> Self {
                Self {
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    expressions: Vec::new(),
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                }
            }

            pub(super) fn collect_argument_accesses(
                &mut self,
                arguments: &[ExpressionHandle],
                target_symbol: SymbolHandle,
                schedule: ArgumentAccessSchedule,
            ) -> arena::HandleSpan<BorrowArgumentAccessFact> {
                collect_argument_accesses(
                    self.program,
                    self.access_segments,
                    self.argument_accesses,
                    arguments,
                    self.state.symbol,
                    self.statement_index,
                    self.machine.symbol,
                    target_symbol,
                    schedule,
                )
            }

            pub(super) fn append_borrow_call(
                &mut self,
                authored_expression: ExpressionHandle,
                receiver_symbol: SymbolHandle,
                target_symbol: SymbolHandle,
                has_receiver: bool,
                accesses: arena::HandleSpan<BorrowArgumentAccessFact>,
            ) {
                self.calls.append_to_span(
                    self.state_calls,
                    BorrowCallFact {
                        statement_index: self.statement_index,
                        call_ordinal: *self.call_ordinal,
                        authored_expression,
                        input_reference_loans: None,
                        reference_slot_effects: Default::default(),
                        completed_reference_loans: Default::default(),
                        receiver_symbol,
                        target_symbol,
                        has_receiver,
                        accesses,
                    },
                );
                *self.call_ordinal += 1;
            }
        }
    }
    mod expression {
        use crate::borrow::accesses::ArgumentAccessSchedule;
        use crate::borrow::calls::BorrowCallCollection;
        use validation::{
            call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
        };

        use checked_trees::expression::{ExpressionHandle, ExpressionNode};

        pub(super) fn collect_expression_borrow_calls(
            collection: &mut BorrowCallCollection<'_>,
            expression: ExpressionHandle,
        ) {
            if collection
                .program
                .static_type_context_is_unreachable(expression)
            {
                return;
            }
            if collection.expressions.contains(&expression) {
                return;
            }
            collection.expressions.push(expression);
            let program = collection.program;
            if super::super::children::recurse_shared_children(program, expression, &mut |child| {
                collect_expression_borrow_calls(collection, child);
            }) {
                return;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    let (receiver_symbol, receiver_path) =
                        call_receiver_parts(collection.program, call.receiver);
                    let selected_operator = typed_trees::operator::resolve_named_expression_call(
                        collection.program,
                        call,
                    );
                    let selected_target = validation::call_target_symbol(program, call);
                    let resolved_target = resolve_state_call_target(
                        collection.program,
                        collection.machine,
                        collection.state,
                        receiver_symbol,
                        selected_target,
                        receiver_path.as_deref(),
                        &call.target,
                    );
                    let is_machine_call = resolved_target.is_valid()
                        || receiver_can_dispatch_to_machine(
                            collection.program,
                            collection.machine,
                            collection.state,
                            receiver_symbol,
                            receiver_path.as_deref(),
                        );

                    if is_machine_call {
                        let accesses = collection.collect_argument_accesses(
                            collection
                                .program
                                .expression_table
                                .expression_handles(call.arguments),
                            if resolved_target.is_valid() {
                                resolved_target
                            } else {
                                call.target_symbol
                            },
                            ArgumentAccessSchedule::Invocation,
                        );
                        collection.append_borrow_call(
                            expression,
                            receiver_symbol,
                            if resolved_target.is_valid() {
                                resolved_target
                            } else {
                                call.target_symbol
                            },
                            selected_operator.map_or_else(
                                || {
                                    call.receiver.is_valid()
                                        && collection.program.symbols.get(receiver_symbol).kind
                                            != symbols::SymbolKind::ConformanceParameter
                                },
                                |operator| {
                                    call.receiver.is_valid()
                                        && collection.program.operator_parameters(operator).len()
                                            == collection
                                                .program
                                                .expression_table
                                                .expression_handles(call.arguments)
                                                .len()
                                                + 1
                                },
                            ),
                            accesses,
                        );
                    }

                    if call.receiver.is_valid() {
                        collect_expression_borrow_calls(collection, call.receiver);
                    }
                    for argument in collection
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                    {
                        collect_expression_borrow_calls(collection, *argument);
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    collect_expression_borrow_calls(collection, indexed.collection);
                    collect_expression_borrow_calls(collection, indexed.index);
                }
                ExpressionNode::Member(member) => {
                    collect_expression_borrow_calls(collection, member.receiver)
                }
                ExpressionNode::Borrow(inner_expression) => {
                    collect_expression_borrow_calls(collection, inner_expression.target)
                }
                _ => {}
            }
        }
    }
    mod reference_custody {
        /*
            A call publishes its returned references before the next operand uses them.
            A promised opaque result owns the completion's permission and retains every
            input lifetime. Stored replacements keep their exact earlier suppliers.
            The checking evaluator supplies the ordered entry/call/join graph; named
            bindings retain its completed values rather than creating another grant.
        */
        use super::super::state::{BorrowFactArenas, append_carrier_loans};
        use super::super::tracker::StateLoanTracker;
        use checked_trees::{BorrowCallFact, BorrowLoanFact};

        /*
            Retain unjoined invocation generations alongside the statement's joined
            successor roster. Flow installs only the exact call occurrence that ran;
            these historical rows never replace another arm's current source inventory.
        */
        pub(in crate::borrow) fn append_reference_custody(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            frames: Option<&validation::CallFrameResolver<'_>>,
            arenas: &mut BorrowFactArenas<'_>,
            calls: arena::HandleSpan<BorrowCallFact>,
            loans: &mut arena::HandleSpan<BorrowLoanFact>,
            history: &mut Vec<StateLoanTracker>,
            statement_entry: &[StateLoanTracker],
        ) -> Result<Option<Vec<StateLoanTracker>>, diagnostics::Diagnostic> {
            let Some(schedule) = frames.and_then(|frames| {
                frames.stored_reference_call_contexts(machine, state, statement_index)
            }) else {
                return Ok(None);
            };
            let schedule_len = schedule.len();
            let mut contexts: Vec<Option<Vec<StateLoanTracker>>> = Vec::new();
            let mut published_calls = Vec::new();
            for (position, context) in schedule.into_iter().enumerate() {
                use validation::StoredReferenceCallContext;
                let current = match context {
                    StoredReferenceCallContext::StatementCompletion { entry } => {
                        if position + 1 != schedule_len {
                            return Err(diagnostics::Diagnostic::error(
                                "normal reference-slot completion precedes another context",
                            ));
                        }
                        return contexts.get_mut(entry).map(Option::take).ok_or_else(|| {
                            diagnostics::Diagnostic::error(
                                "normal reference-slot completion names an unavailable predecessor",
                            )
                        });
                    }
                    StoredReferenceCallContext::Entry if contexts.is_empty() => {
                        Some(statement_entry.to_vec())
                    }
                    StoredReferenceCallContext::Entry => {
                        return Err(diagnostics::Diagnostic::error(
                            "normal reference-slot context repeats its entry",
                        ));
                    }
                    StoredReferenceCallContext::Join { alternatives } => {
                        if alternatives.is_empty() {
                            contexts.push(None);
                            continue;
                        }
                        let mut joined = Some(Vec::new());
                        for alternative in alternatives {
                            let Some(roster) = contexts.get(alternative) else {
                                return Err(diagnostics::Diagnostic::error(
                                    "normal reference-slot join names an unavailable predecessor context",
                                ));
                            };
                            match (&mut joined, roster) {
                                (Some(joined), Some(roster)) => {
                                    for tracker in roster {
                                        if !joined.contains(tracker) {
                                            joined.push(tracker.clone());
                                        }
                                    }
                                }
                                _ => joined = None,
                            }
                        }
                        joined
                    }
                    StoredReferenceCallContext::Invocation {
                        entry,
                        expression,
                        effects,
                    } => {
                        let Some(entry) = contexts.get(entry) else {
                            return Err(diagnostics::Diagnostic::error(
                                "normal reference-slot invocation names an unavailable entry context",
                            ));
                        };
                        let (Some(incoming), Some(effects)) = (entry, effects) else {
                            contexts.push(None);
                            continue;
                        };
                        let mut matching = calls.handles().filter(|handle| {
                            let call = arenas.calls.get(*handle);
                            call.statement_index == statement_index
                                && call.authored_expression == expression
                        });
                        let Some(call) = matching.next().filter(|_| matching.next().is_none())
                        else {
                            return Err(diagnostics::Diagnostic::error(format!(
                                "machine `{}` state `{}` statement {statement_index}: normal reference-slot completion for {expression:?} has no unique checked invocation",
                                machine.name, state.name,
                            )));
                        };
                        if published_calls.contains(&call) {
                            return Err(diagnostics::Diagnostic::error(
                                "normal reference-slot invocation has more than one completion context",
                            ));
                        }
                        published_calls.push(call);
                        arenas.calls.get_mut(call).input_reference_loans = Some(
                            arenas
                                .loan_suppliers
                                .insert_many(incoming.iter().map(|loan| loan.handle)),
                        );
                        let mut destinations = arena::HandleSpan::empty();
                        for effect in &effects {
                            let segments = arenas
                                .access_segments
                                .insert_many(effect.destination.local_segments.iter().copied());
                            arenas.reference_slot_effects.append_to_span(
                                &mut destinations,
                                checked_trees::BorrowCallReferenceSlotEffect {
                                    root: facts::PlaceRoot::Symbol(effect.destination.local_symbol),
                                    segments,
                                    owner_path: arenas.owner_segments.insert_many(
                                        crate::borrow::owner_path_from_place_segments(
                                            program,
                                            &effect.destination.local_segments,
                                        ),
                                    ),
                                    may_select: effect.may_select,
                                },
                            );
                        }
                        arenas.calls.get_mut(call).reference_slot_effects = destinations;
                        let mut current = incoming
                            .iter()
                            .filter(|tracker| {
                                !effects.iter().any(|effect| {
                                    !effect.may_select
                                        && tracker.storage_owner
                                            == facts::PlaceRoot::Symbol(
                                                effect.destination.local_symbol,
                                            )
                                        && crate::borrow::place_path_covers_owner(
                                            program,
                                            &effect.destination.local_segments,
                                            &tracker.owner_path,
                                        )
                                })
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        let mut produced = crate::borrow::loans::stored_reference_effect_loans(
                            program, effects, incoming,
                        );
                        produced.extend(
                            crate::borrow::loans::complete_invocation_results(
                                program,
                                machine,
                                state,
                                statement_index,
                                expression,
                                statement_entry,
                                incoming,
                                frames,
                            )
                            .map_err(|reason| {
                                let target = program.expression_table.call_node(expression)
                                    .map_or("<unknown>", |call| call.target.as_str());
                                diagnostics::Diagnostic::error(format!(
                                    "normal call `{target}` ({expression:?}) in machine `{}` state `{}` statement {statement_index}: {reason}",
                                    machine.name, state.name,
                                ))
                            })?,
                        );
                        for loan in &mut produced {
                            loan.formation_expression = expression;
                        }
                        let published = append_carrier_loans(
                            arenas,
                            loans,
                            &mut current,
                            history,
                            produced,
                            statement_index,
                            call,
                        );
                        retain_result_input_lifetimes(arenas, published, expression);
                        arenas.calls.get_mut(call).completed_reference_loans = published;
                        Some(current)
                    }
                };
                contexts.push(current);
            }
            Err(diagnostics::Diagnostic::error(
                "normal reference-slot schedule has no statement completion",
            ))
        }

        /*
            A definite transferred slot replaces exactly the descriptor occurrences
            stored beneath that destination. May-select destinations keep unselected
            occurrences. Backing address equality supplies no reaccess authority.
        */
        pub(crate) fn replaces_reference_loan(
            program: &dyn ::validation::CheckingInput,
            borrow: &checked_trees::BorrowFacts,
            call: &BorrowCallFact,
            loan: arena::Handle<checked_trees::BorrowLoanFact>,
        ) -> bool {
            let previous = borrow.loans.get(loan);
            borrow
                .reference_slot_effects
                .span_or_empty(call.reference_slot_effects)
                .iter()
                .any(|effect| {
                    !effect.may_select
                        && effect.root == previous.storage_owner
                        && crate::borrow::place_path_covers_owner(
                            program,
                            borrow.access_segments.span_or_empty(effect.segments),
                            borrow.loan_owner_path(previous),
                        )
                })
        }

        /*
            A returned pointer records its complete input debts when the call completes.
            Carrier publication already groups equal storage and field paths, so one
            pass can attach each list without a later search through the state's loans.
        */
        fn retain_result_input_lifetimes(
            arenas: &mut BorrowFactArenas<'_>,
            published: arena::HandleSpan<BorrowLoanFact>,
            expression: typed_trees::expression::ExpressionHandle,
        ) {
            let mut retained = Vec::new();
            let mut first = 0u32;
            for group in arenas
                .loans
                .span_or_empty(published)
                .chunk_by(|left, right| {
                    left.owner_symbol == right.owner_symbol
                        && left.storage_owner == right.storage_owner
                        && arenas.owner_segments.span(left.owner_path)
                            == arenas.owner_segments.span(right.owner_path)
                })
            {
                let start = first;
                first += group.len() as u32;
                let permissions = group
                    .iter()
                    .enumerate()
                    .filter_map(|(offset, loan)| {
                        (loan.storage_owner == facts::PlaceRoot::Expression(expression)
                            && loan.lineage == checked_trees::BorrowLoanLineage::DirectRoot
                            && loan.formation == checked_trees::BorrowLoanFormation::ReturnedDescriptor
                            && arenas.incoming_references.is_valid(loan.incoming_reference)
                            && matches!(arenas.incoming_references.get(loan.incoming_reference).origin,
                            checked_trees::IncomingReferenceOrigin::Invocation { expression: source, .. }
                                if source == expression))
                        .then_some(published.handle_at(start + offset as u32))
                    })
                    .collect::<Vec<_>>();
                if permissions.is_empty() {
                    continue;
                }
                let debts = arenas
                    .loan_suppliers
                    .insert_many(group.iter().enumerate().filter_map(|(offset, loan)| {
                        (loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency)
                            .then_some(published.handle_at(start + offset as u32))
                    }));
                retained.extend(
                    permissions
                        .into_iter()
                        .map(|permission| (permission, debts)),
                );
            }
            for (permission, debts) in retained {
                arenas.loans.get_mut(permission).retained_input_lifetimes = Some(debts);
            }
        }
    }
    mod transitions {
        use crate::borrow::accesses::ArgumentAccessSchedule;
        use crate::borrow::calls::BorrowCallCollection;
        use crate::borrow::calls::expression;
        use checked_trees::statement::TransitionTargetNode;

        pub(super) fn collect_transition_target_borrow_calls(
            collection: &mut BorrowCallCollection<'_>,
            target: typed_trees::statement::TransitionTargetHandle,
        ) {
            match collection.program.statement_table.transition_target(target) {
                TransitionTargetNode::Named {
                    arguments, path, ..
                } => {
                    let target_symbol =
                        validation::call_target_state(collection.program, path.symbol)
                            .map_or(path.symbol, |state| state.symbol);
                    let accesses = collection.collect_argument_accesses(
                        collection
                            .program
                            .statement_table
                            .expression_handles(*arguments),
                        target_symbol,
                        ArgumentAccessSchedule::TransitionTarget(None),
                    );
                    collection.append_borrow_call(
                        Default::default(),
                        path.head_symbol,
                        target_symbol,
                        path.members.count() > 1,
                        accesses,
                    );

                    for argument in collection
                        .program
                        .statement_table
                        .expression_handles(*arguments)
                    {
                        expression::collect_expression_borrow_calls(collection, *argument);
                    }
                }
                TransitionTargetNode::Value(expression) => {
                    expression::collect_expression_borrow_calls(collection, *expression)
                }
                TransitionTargetNode::Terminal => {}
            }
        }
    }
    pub(in crate::borrow) use reference_custody::append_reference_custody;
    pub(crate) use reference_custody::replaces_reference_loan;

    use collection::BorrowCallCollection;
    use transitions::collect_transition_target_borrow_calls;

    pub(crate) fn collect_statement_borrow_calls(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        statement: &StatementNode,
        call_ordinal: &mut usize,
        access_segments: &mut arena::Arena<facts::PlaceSegment>,
        argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
        calls: &mut arena::Arena<BorrowCallFact>,
        state_calls: &mut arena::HandleSpan<BorrowCallFact>,
    ) {
        let mut collection = BorrowCallCollection::new(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        );

        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                expression::collect_expression_borrow_calls(&mut collection, assignment.target);
                expression::collect_expression_borrow_calls(&mut collection, assignment.value)
            }
            StatementNode::Call(call) => {
                let resolved_target = validation::resolve_state_call_target(
                    program,
                    machine,
                    state,
                    call.receiver_symbol,
                    call.target_symbol,
                    validation::statement_call_receiver_members(program, call),
                    &call.target,
                );
                if resolved_target.is_valid()
                    || statement_call_can_dispatch_to_machine(program, machine, state, call)
                {
                    let receiver_path =
                        statement_call_receiver_path(program, state, statement_index, call);
                    let accesses = collection.collect_argument_accesses(
                        program.statement_table.expression_handles(call.arguments),
                        if resolved_target.is_valid() {
                            resolved_target
                        } else {
                            call.target_symbol
                        },
                        ArgumentAccessSchedule::Invocation,
                    );
                    collection.append_borrow_call(
                        Default::default(),
                        call.receiver_symbol,
                        if resolved_target.is_valid() {
                            resolved_target
                        } else {
                            call.target_symbol
                        },
                        receiver_path.is_some()
                            && program.symbols.get(call.receiver_symbol).kind
                                != symbols::SymbolKind::ConformanceParameter,
                        accesses,
                    );
                }

                for argument in program.statement_table.expression_handles(call.arguments) {
                    expression::collect_expression_borrow_calls(&mut collection, *argument);
                }
            }
            StatementNode::Expression(expression) => {
                expression::collect_expression_borrow_calls(&mut collection, *expression)
            }
            StatementNode::LocalData(local_data) => {
                if local_data.initial_value.is_valid() {
                    expression::collect_expression_borrow_calls(
                        &mut collection,
                        local_data.initial_value,
                    );
                }
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(expression) = transition.guard {
                    expression::collect_expression_borrow_calls(&mut collection, expression);
                }

                collect_transition_target_borrow_calls(&mut collection, transition.target);

                if transition.continuation.is_valid() {
                    collect_transition_target_borrow_calls(
                        &mut collection,
                        transition.continuation,
                    );
                }
            }
        }
    }
}
mod children {
    /*
        Traversal arms identical across the borrow walks. A child-bearing node
        recurses through `visit`; any other node returns `false` so the caller
        classifies it under its own rules.
    */
    use crate::flow::expressions;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};

    /*
        Recurse `visit` over the child expressions the borrow walks share:
        match subjects/patterns/values, atomics, array/range/unary operands,
        binary operands, cast values, and struct-literal fields. Returns
        `false` for nodes the walks classify differently.
    */
    pub(super) fn recurse_shared_children(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        visit: &mut dyn FnMut(ExpressionHandle),
    ) -> bool {
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                visit(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        visit(pattern);
                    }
                    visit(arm.value);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                for child in validation::atomic_execution_children(program, atomic) {
                    visit(child);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    visit(*value);
                }
            }
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    visit(range.start);
                }
                if range.end.is_valid() {
                    visit(range.end);
                }
            }
            ExpressionNode::Binary(binary) => {
                visit(binary.left);
                let skipped = matches!(
                    (binary.operator, program.static_boolean_value(binary.left)),
                    (typed_trees::expression::BinaryOperator::And, Some(false))
                        | (typed_trees::expression::BinaryOperator::Or, Some(true))
                );
                if !skipped {
                    visit(binary.right);
                }
            }
            ExpressionNode::Cast(cast) => visit(cast.value),
            ExpressionNode::Unary(unary) => visit(unary.operand),
            ExpressionNode::StructLiteral(struct_literal) => {
                for field in expressions(program).struct_fields(struct_literal.fields) {
                    visit(field.value);
                }
            }
            _ => return false,
        }
        true
    }
}
mod last_uses {
    use checked_trees::statement::StatementNode;
    use checked_trees::{BorrowArgumentAccessFact, BorrowCallFact};
    use symbols::SymbolHandle;

    use super::tracker::StateLoanTracker;
    pub(crate) use usage::owner_path_overlaps_place_segments;
    use usage::{statement_uses_local_name, statement_uses_owner_path, statement_uses_symbol};

    mod usage {
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};
        use checked_trees::statement::StatementNode;
        use symbols::SymbolHandle;

        mod expressions {
            use crate::flow::expressions;
            use checked_trees::expression::ExpressionHandle;
            use typed_trees::expression::ExpressionNode;
            use typed_trees::{SubtreeEdges, SubtreeVisit};

            pub(super) fn expression_uses_local_name(
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
                local_name: &str,
            ) -> bool {
                program.subtree_any(
                    expression,
                    SubtreeEdges::SAVED_SOURCES,
                    |_, node| match node {
                        ExpressionNode::Name(path)
                            if expressions(program)
                                .name_path_members(path.members)
                                .first()
                                .is_some_and(|member| member.as_str() == local_name) =>
                        {
                            SubtreeVisit::Found
                        }
                        _ => SubtreeVisit::Descend,
                    },
                )
            }
        }
        mod transitions {
            use super::expressions::expression_uses_local_name;

            pub(super) fn transition_guard_uses_local_name(
                program: &dyn ::validation::CheckingInput,
                guard: typed_trees::statement::TransitionGuardNode,
                local_name: &str,
            ) -> bool {
                match guard {
                    typed_trees::statement::TransitionGuardNode::Always => false,
                    typed_trees::statement::TransitionGuardNode::When(expression) => {
                        expression_uses_local_name(program, expression, local_name)
                    }
                }
            }

            pub(super) fn transition_target_uses_local_name(
                program: &dyn ::validation::CheckingInput,
                target: &typed_trees::statement::TransitionTargetNode,
                local_name: &str,
            ) -> bool {
                match target {
                    typed_trees::statement::TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        program
                            .statement_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|member| member.as_str() == local_name)
                            || program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .any(|argument| {
                                    expression_uses_local_name(program, *argument, local_name)
                                })
                    }
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        expression_uses_local_name(program, *expression, local_name)
                    }
                    typed_trees::statement::TransitionTargetNode::Terminal => false,
                }
            }
        }

        use expressions::expression_uses_local_name;
        use transitions::{transition_guard_uses_local_name, transition_target_uses_local_name};
        pub(super) use validation::statement_uses_symbol;

        use crate::borrow::owner_segment_matches_place_segment;
        use crate::borrow::tracker::BorrowOwnerSegment;
        use crate::published::places::place_at;

        pub(super) fn statement_uses_local_name(
            program: &dyn ::validation::CheckingInput,
            statement: &StatementNode,
            local_name: &str,
        ) -> bool {
            match statement {
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
                StatementNode::Assignment(assignment) => {
                    expression_uses_local_name(program, assignment.target, local_name)
                        || expression_uses_local_name(program, assignment.value, local_name)
                }
                StatementNode::Call(call) => {
                    program
                        .statement_table
                        .name_path_members(call.receiver)
                        .first()
                        .is_some_and(|member| member.as_str() == local_name)
                        || program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .any(|argument| {
                                expression_uses_local_name(program, *argument, local_name)
                            })
                }
                StatementNode::Expression(expression) => {
                    expression_uses_local_name(program, *expression, local_name)
                }
                StatementNode::LocalData(local_data) => {
                    expression_uses_local_name(program, local_data.initial_value, local_name)
                }
                StatementNode::Transition(transition) => {
                    transition_guard_uses_local_name(program, transition.guard, local_name)
                        || transition_target_uses_local_name(
                            program,
                            program.statement_table.transition_target(transition.target),
                            local_name,
                        )
                        || transition_target_uses_local_name(
                            program,
                            program
                                .statement_table
                                .transition_target(transition.continuation),
                            local_name,
                        )
                }
            }
        }

        pub(super) fn statement_uses_owner_path(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            statement: &StatementNode,
            owner_symbol: SymbolHandle,
            owner_name: &str,
            owner_path: &[BorrowOwnerSegment],
        ) -> bool {
            let uses = |expression| {
                expression_uses_owner_path(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                    owner_symbol,
                    owner_path,
                )
            };
            match statement {
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
                StatementNode::Assignment(assignment) => {
                    uses(assignment.target) || uses(assignment.value)
                }
                StatementNode::Call(call) => {
                    call.receiver_symbol == owner_symbol
                        || validation::statement_call_receiver_members(program, call)
                            .and_then(|members| members.first())
                            .is_some_and(|member| member.as_str() == owner_name)
                        || program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .any(|argument| uses(*argument))
                }
                StatementNode::Expression(expression) => uses(*expression),
                StatementNode::LocalData(local_data) => uses(local_data.initial_value),
                StatementNode::Transition(transition) => {
                    matches!(transition.guard, typed_trees::statement::TransitionGuardNode::When(expression) if uses(expression))
                        || transition_target_uses_owner_path(
                            program,
                            state_symbol,
                            statement_index,
                            program.statement_table.transition_target(transition.target),
                            owner_symbol,
                            owner_path,
                        )
                        || (transition.continuation.is_valid()
                            && transition_target_uses_owner_path(
                                program,
                                state_symbol,
                                statement_index,
                                program
                                    .statement_table
                                    .transition_target(transition.continuation),
                                owner_symbol,
                                owner_path,
                            ))
                }
            }
        }

        pub(crate) fn owner_path_overlaps_place_segments(
            program: &dyn ::validation::CheckingInput,
            owner_path: &[BorrowOwnerSegment],
            place_segments: &[facts::PlaceSegment],
        ) -> bool {
            owner_path.iter().zip(place_segments).all(|(owner, place)| {
                owner_segment_matches_place_segment(program, owner, place).unwrap_or({
                    matches!(
                        (owner, place),
                        (
                            BorrowOwnerSegment::DynamicIndex,
                            facts::PlaceSegment::FixedIndex { .. }
                                | facts::PlaceSegment::Index { .. }
                        )
                    )
                })
            })
        }

        fn expression_uses_owner_path(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            expression: ExpressionHandle,
            owner_symbol: SymbolHandle,
            owner_path: &[BorrowOwnerSegment],
        ) -> bool {
            let edges = typed_trees::SubtreeEdges {
                pattern_dependencies: true,
                ..typed_trees::SubtreeEdges::SAVED_SOURCES
            };
            program.subtree_any(expression, edges, |handle, node| {
                if place_at(program, state_symbol, statement_index, handle).is_some_and(|place| {
                    matches!(place.root, facts::PlaceRoot::Symbol(root) if root == owner_symbol)
                        && owner_path_overlaps_place_segments(program, owner_path, &place.segments)
                }) {
                    typed_trees::SubtreeVisit::Found
                } else if matches!(node, ExpressionNode::Member(_)) {
                    typed_trees::SubtreeVisit::Prune
                } else {
                    typed_trees::SubtreeVisit::Descend
                }
            })
        }

        fn transition_target_uses_owner_path(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            target: &typed_trees::statement::TransitionTargetNode,
            owner_symbol: SymbolHandle,
            owner_path: &[BorrowOwnerSegment],
        ) -> bool {
            let uses = |expression| {
                expression_uses_owner_path(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                    owner_symbol,
                    owner_path,
                )
            };
            match target {
                typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| uses(*argument)),
                typed_trees::statement::TransitionTargetNode::Value(expression) => {
                    uses(*expression)
                }
                typed_trees::statement::TransitionTargetNode::Terminal => false,
            }
        }
    }

    /*
        Whether a lexical place rooted at `symbol` remains live after one source
        statement. This is the statement-bound first consumer of canonical
        liveness outside borrow expiry; finer intra-statement ordering can extend
        the same query without changing carry semantics.
    */
    pub(crate) fn place_is_used_after_statement(
        program: &dyn ::validation::CheckingInput,
        statements: arena::HandleSpan<StatementNode>,
        statement_index: usize,
        symbol: SymbolHandle,
        local_name: &str,
    ) -> bool {
        program
            .statement_table
            .admitted_execution_statements(statements)
            .skip_while(|(index, _)| *index != statement_index)
            .skip(1)
            .any(|(_, statement)| {
                usage::statement_uses_symbol(program, statement, symbol)
                    || usage::statement_uses_local_name(program, statement, local_name)
            })
    }

    pub(super) fn update_state_loan_last_uses(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statements: arena::HandleSpan<StatementNode>,
        borrow_calls: &[BorrowCallFact],
        access_segments: &arena::Arena<facts::PlaceSegment>,
        argument_accesses: &arena::Arena<BorrowArgumentAccessFact>,
        loan_trackers: &[StateLoanTracker],
        loans: &mut arena::Arena<checked_trees::BorrowLoanFact>,
        loan_suppliers: &arena::Arena<arena::Handle<checked_trees::BorrowLoanFact>>,
    ) {
        if loan_trackers.is_empty() {
            return;
        }

        let order = program.statement_table.admitted_execution_order(statements);

        for borrow_call in borrow_calls {
            for access in argument_accesses.span_or_empty(borrow_call.accesses) {
                for tracker in loan_trackers {
                    if !loans.is_valid(tracker.handle) {
                        continue;
                    }
                    if tracker.storage_owner == access.root
                        && owner_path_overlaps_place_segments(
                            program,
                            &tracker.owner_path,
                            access_segments.span_or_empty(access.segments),
                        )
                    {
                        extend_last_use(
                            &order,
                            &mut loans.get_mut(tracker.handle).last_use_statement_index,
                            borrow_call.statement_index,
                        );
                    }
                }
            }
        }

        for (statement_index, statement) in program
            .statement_table
            .admitted_execution_statements(statements)
        {
            for tracker in loan_trackers {
                if !loans.is_valid(tracker.handle) {
                    continue;
                }
                if tracker.storage_owner != facts::PlaceRoot::Symbol(tracker.owner_symbol) {
                    continue;
                }
                let used = if tracker.owner_path.is_empty() {
                    statement_uses_local_name(program, statement, tracker.owner_name.as_str())
                        || statement_uses_symbol(program, statement, tracker.owner_symbol)
                } else {
                    statement_uses_owner_path(
                        program,
                        state_symbol,
                        statement_index,
                        statement,
                        tracker.owner_symbol,
                        tracker.owner_name.as_str(),
                        &tracker.owner_path,
                    )
                };
                if used {
                    extend_last_use(
                        &order,
                        &mut loans.get_mut(tracker.handle).last_use_statement_index,
                        statement_index,
                    );
                }
            }
        }

        if let Some(state) = program.state_by_symbol(state_symbol) {
            for tracker in loan_trackers {
                if !loans.is_valid(tracker.handle) {
                    continue;
                }
                if !tracker.owner_path.is_empty()
                    && program
                        .state_parameter_by_symbol(state, tracker.owner_symbol)
                        .is_some()
                {
                    loans.get_mut(tracker.handle).last_use_statement_index =
                        order.indices().last().copied().unwrap_or_default();
                }
            }
        }

        loop {
            let mut changed = false;
            for alias in loan_trackers {
                if !loans.is_valid(alias.handle) {
                    continue;
                }
                let alias = loans.get(alias.handle);
                let last_use = alias.last_use_statement_index;
                let suppliers = loan_suppliers.span_or_empty(alias.supplying_loans);
                for supplier in suppliers {
                    if loans.is_valid(*supplier) {
                        changed |= extend_last_use(
                            &order,
                            &mut loans.get_mut(*supplier).last_use_statement_index,
                            last_use,
                        );
                    }
                }
            }
            for carrier in loan_trackers {
                if !loans.is_valid(carrier.handle) {
                    continue;
                }
                let temporary = matches!(carrier.storage_owner, facts::PlaceRoot::Expression(_));
                let carrier_statement = loans.get(carrier.handle).statement_index;
                for alias in loan_trackers {
                    if !loans.is_valid(alias.handle) {
                        continue;
                    }
                    let alias_statement = loans.get(alias.handle).statement_index;
                    if !(order.precedes(carrier_statement, alias_statement)
                        || (temporary && carrier_statement == alias_statement))
                        || alias.place.root != carrier.storage_owner
                        || !owner_path_overlaps_place_segments(
                            program,
                            &carrier.owner_path,
                            &alias.place.segments,
                        )
                    {
                        continue;
                    }
                    let alias_last_use = loans.get(alias.handle).last_use_statement_index;
                    let carried = loans.get_mut(carrier.handle);
                    changed |= extend_last_use(
                        &order,
                        &mut carried.last_use_statement_index,
                        alias_last_use,
                    );
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn extend_last_use(
        order: &typed_trees::statement::StatementExecutionOrder,
        previous: &mut usize,
        next: usize,
    ) -> bool {
        let Some(next_position) = order.position(next) else {
            return false;
        };
        if order
            .position(*previous)
            .is_some_and(|position| position >= next_position)
        {
            return false;
        }
        *previous = next;
        true
    }
}
pub(crate) mod lifetime_contract {
    /*
        Rejoin a retained input edge to the original applied call declaration.
        An older stored capture supplies provenance, not a stronger loan contract.
    */
    use checked_trees::{FlowControlFacts, FlowOperandRetainedSupplier, ViewReturnInputSource};
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn source_type(
        program: &dyn validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        control: &FlowControlFacts,
        supplier: &FlowOperandRetainedSupplier,
    ) -> Option<TypeReferenceHandle> {
        let contract = supplier.contract?;
        if !control.operands.is_valid(contract.input)
            || !control.operands.is_valid(supplier.operand)
        {
            return None;
        }
        let input = control.operands.get(contract.input);
        let machine = program.machine_holding_state(contract.state_symbol)?;
        let borrowed = borrow.call_at(
            machine.symbol,
            contract.state_symbol,
            contract.statement_index,
            contract.call_ordinal,
        )?;
        let borrowed = borrow.calls.get(borrowed);
        if borrowed.authored_expression != contract.invocation {
            return None;
        }
        let site = validation::find_call_site(
            program,
            machine.symbol,
            contract.state_symbol,
            contract.statement_index,
            contract.call_ordinal,
        )?;
        let call = contract
            .invocation
            .is_valid()
            .then(|| program.expression_table.call_node(contract.invocation))
            .flatten();
        let receiver = usize::from(call.is_some_and(|call| call.receiver.is_valid()));
        let arguments = if contract.output_parameter.is_none() {
            program.expression_table.expression_handles(call?.arguments)
        } else {
            validation::call_site_argument_expressions(program, &site)
        };
        if arguments.get(contract.parameter_index).copied() != Some(input.expression)
            || input.state_symbol != contract.state_symbol
            || input.statement_index != contract.statement_index
        {
            return None;
        }
        let state = control.unique_state(machine.symbol, contract.state_symbol)?;
        let mut actual_calls = state.calls.handles().filter_map(|handle| {
            let recorded = control.calls.get(handle);
            (recorded.statement_index == contract.statement_index
                && recorded.call_ordinal == contract.call_ordinal
                && recorded.authored_expression == contract.invocation
                && recorded.target_symbol == borrowed.target_symbol)
                .then_some(recorded)
        });
        let actual = actual_calls.next()?;
        if actual_calls.next().is_some() {
            return None;
        }
        if actual.operands.count() as usize != arguments.len() + receiver
            || actual
                .operands
                .handles()
                .nth(contract.parameter_index + receiver)
                != Some(contract.input)
        {
            return None;
        }
        let source_path = control.operand_retained_paths.span(contract.source_path)?;
        let owner_path = control.operand_retained_paths.span(contract.owner_path)?;
        let projection = control.operand_retained_paths.span(supplier.projection)?;
        if supplier.operand != contract.input || !same_path(program, projection, source_path) {
            return None;
        }
        match contract.source {
            ViewReturnInputSource::Stored => {
                validate_stored_sources(program, control, &contract, actual.invocation_storage)?;
            }
            ViewReturnInputSource::Referent if !contract.stored_sources.is_empty() => return None,
            ViewReturnInputSource::Referent => {}
        }
        let identity_type = program.normalized_type_identity(contract.source_type);
        if !identity_type.is_valid() {
            return None;
        }
        let (source, source_type, path) = if let Some(position) = contract.output_parameter {
            let parameters = site.parameters(program, machine.symbol)?;
            let parameter = parameters.get(position)?;
            let identity = facts::InvocationOutputIdentity {
                state_symbol: contract.state_symbol,
                statement_index: contract.statement_index,
                call_ordinal: contract.call_ordinal,
                parameter_symbol: parameter.symbol,
            };
            let completion = completed_output_retention(
                borrow,
                machine.symbol,
                contract.state_symbol,
                contract.statement_index,
                borrowed,
                identity,
                parameter.type_reference,
                &crate::borrow::owner_path_from_place_segments(program, owner_path),
            );
            if let Err(reason) = completion {
                if ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT output-lifetime-contract-refused output={identity:?} reason={reason}"
                    );
                }
                return None;
            }
            let relation =
                validation::borrow_contracts::resolve_provider_outputs(program, parameters).ok()?;
            if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete
            {
                return None;
            }
            let output = relation.outputs.iter().find(|output| {
                output.destination
                    == validation::borrow_contracts::ProviderOutputDestination::Parameter(position)
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(&output.owner_path),
                        owner_path,
                    )
            })?;
            let edge = output.dependencies.iter().find(|edge| {
                edge.parameter_index == contract.parameter_index
                    && edge.source == contract.source
                    && program.normalized_type_identity(edge.source_type) == identity_type
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(&edge.source_path),
                        source_path,
                    )
            })?;
            (edge.source, edge.source_type, source_path)
        } else {
            let edges = crate::borrow::call_reference_lifetime_sources(
                program,
                contract.state_symbol,
                call?,
            )?;
            let edge = edges.iter().find(|edge| {
                edge.parameter_index == contract.parameter_index
                    && edge.source == contract.source
                    && program.normalized_type_identity(edge.source_type) == identity_type
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(&edge.source_path),
                        source_path,
                    )
                    && same_path(
                        program,
                        &crate::borrow::owner_place_segments(&edge.owner_path),
                        owner_path,
                    )
            })?;
            (edge.source, edge.source_type, source_path)
        };
        let carrier = match source {
            ViewReturnInputSource::Referent => source_type,
            ViewReturnInputSource::Stored => {
                validation::reference_result_custody::input_carrier_type(program, source_type)?
            }
        };
        validation::project_type_reference_from_segments(program, carrier, path)
    }

    /*
        Rejoin every frozen member to the same immutable invocation-entry cut.
        This checks which value was selected; its borrowing permission is checked
        by the source consumer, independently of the declared lifetime edge.
    */
    fn validate_stored_sources(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        contract: &checked_trees::FlowOperandLifetimeContract,
        snapshot: arena::Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<()> {
        use checked_trees::FlowOperandStoredSource;
        let input = control.operands.get(contract.input);
        let carrier = validation::reference_result_custody::input_carrier_type(
            program,
            input.type_reference,
        )?;
        let path = validation::corresponding_place_projection(
            program,
            carrier,
            control.operand_retained_paths.span(contract.source_path)?,
        )?;
        let selected = validation::project_type_reference_from_segments(program, carrier, &path)?;
        let leaves = if validation::reference_shell(program, selected).is_some() {
            vec![Vec::new()]
        } else {
            validation::borrow_contracts::carried_reference_paths(program, selected)?
                .into_iter()
                .map(|(path, _)| crate::borrow::owner_place_segments(&path))
                .collect()
        };
        let mut recorded = control
            .operand_stored_sources
            .span(contract.stored_sources)?
            .iter();
        if leaves.is_empty() {
            return None;
        }
        for leaf in leaves {
            let mut full_path = path.clone();
            full_path.extend_from_slice(&leaf);
            let sources = validation::completed_storage::stored_reference_sources_at_path(
                program,
                control,
                contract.input,
                &full_path,
                snapshot,
            );
            let mut expected = Vec::new();
            flatten_stored_sources(sources, &mut expected)?;
            if expected.is_empty() {
                return None;
            }
            for expected in expected {
                let recorded = recorded.next()?;
                let (FlowOperandStoredSource::Value { path, .. }
                | FlowOperandStoredSource::Incoming { path, .. }) = recorded;
                if !same_path(program, control.operand_retained_paths.span(*path)?, &leaf) {
                    return None;
                }
                match (*recorded, expected) {
                    (
                        FlowOperandStoredSource::Value {
                            operand,
                            reference,
                            projection,
                            slot,
                            ..
                        },
                        validation::completed_storage::StoredOperand::Value {
                            capture,
                            reference: expected_reference,
                            projection: expected_projection,
                            slot: expected_slot,
                        },
                    ) => {
                        if operand != capture
                            || reference != expected_reference
                            || !same_path(
                                program,
                                control.operand_retained_paths.span(projection)?,
                                &expected_projection,
                            )
                        {
                            return None;
                        }
                        match (slot, expected_slot) {
                            (Some(slot), Some(expected))
                                if stored_slot_matches(program, control, slot, &expected) => {}
                            (None, None) => {}
                            _ => return None,
                        }
                    }
                    (
                        FlowOperandStoredSource::Incoming { slot, .. },
                        validation::completed_storage::StoredOperand::Incoming(expected),
                    ) if stored_slot_matches(program, control, slot, &expected) => {}
                    _ => return None,
                }
            }
        }
        recorded.next().is_none().then_some(())
    }

    fn flatten_stored_sources(
        source: validation::completed_storage::StoredOperand,
        output: &mut Vec<validation::completed_storage::StoredOperand>,
    ) -> Option<()> {
        use validation::completed_storage::StoredOperand;
        match source {
            StoredOperand::Unavailable => return None,
            StoredOperand::Alternatives(sources) => {
                if sources.is_empty() {
                    return None;
                }
                for source in sources {
                    flatten_stored_sources(source, output)?;
                }
            }
            source => output.push(source),
        }
        Some(())
    }

    fn stored_slot_matches(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        slot: checked_trees::FlowOperandReferent,
        expected: &validation::CanonicalPlace,
    ) -> bool {
        slot.root == expected.root
            && control
                .operand_referent_segments
                .span(slot.segments)
                .is_some_and(|path| same_path(program, path, &expected.segments))
    }

    pub(crate) enum RetainedLifetimeInput<'a> {
        Value {
            operand: arena::Handle<checked_trees::FlowOperandFact>,
            path: &'a [facts::PlaceSegment],
            bound: TypeReferenceHandle,
        },
        Incoming {
            bound: TypeReferenceHandle,
        },
    }

    /*
        A saved field can hold either of several references. Keep those possible
        occupants together: returning access requires the same duty on every arm,
        while lifetime retention conservatively holds every possible input.
        Separate groups are simultaneous declared leaves, not alternative occupants.
    */
    pub(crate) fn retained_inputs<'a>(
        program: &dyn validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        control: &'a FlowControlFacts,
        supplier: &FlowOperandRetainedSupplier,
    ) -> Option<Vec<Vec<RetainedLifetimeInput<'a>>>> {
        let bound = source_type(program, borrow, control, supplier)?;
        let contract = supplier.contract?;
        if contract.source == ViewReturnInputSource::Referent {
            return Some(vec![vec![RetainedLifetimeInput::Value {
                operand: supplier.operand,
                path: control.operand_retained_paths.span(supplier.projection)?,
                bound,
            }]]);
        }
        let mut inputs: Vec<(Vec<facts::PlaceSegment>, Vec<RetainedLifetimeInput<'a>>)> =
            Vec::new();
        for member in control
            .operand_stored_sources
            .span(contract.stored_sources)?
        {
            use checked_trees::FlowOperandStoredSource;
            let (FlowOperandStoredSource::Value { path, .. }
            | FlowOperandStoredSource::Incoming { path, .. }) = member;
            let path = validation::corresponding_place_projection(
                program,
                bound,
                control.operand_retained_paths.span(*path)?,
            )?;
            let bound = validation::project_type_reference_from_segments(program, bound, &path)?;
            let input = match *member {
                FlowOperandStoredSource::Value {
                    operand,
                    projection,
                    ..
                } => RetainedLifetimeInput::Value {
                    operand,
                    path: control.operand_retained_paths.span(projection)?,
                    bound,
                },
                FlowOperandStoredSource::Incoming { .. } => {
                    RetainedLifetimeInput::Incoming { bound }
                }
            };
            if let Some((_, group)) = inputs.iter_mut().find(|(candidate, _)| *candidate == path) {
                group.push(input);
            } else {
                inputs.push((path, vec![input]));
            }
        }
        (!inputs.is_empty()).then(|| inputs.into_iter().map(|(_, group)| group).collect())
    }

    fn same_path(
        program: &dyn validation::CheckingInput,
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                validation::storage_path_relations::canonical_place_segments_equal(
                    program, *left, *right,
                )
            })
    }

    /*
        Check the whole declared input list before using any one captured edge.
        Each returned access is the declaration's ceiling, which can be weaker
        than the reference type stored inside a shared container.
    */
    pub(crate) fn output_input_accesses(
        program: &dyn validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        control: &FlowControlFacts,
        identity: facts::InvocationOutputIdentity,
        owner_path: &[checked_trees::BorrowLoanOwnerSegment],
        sources: arena::HandleSpan<FlowOperandRetainedSupplier>,
    ) -> Result<Vec<language_semantics::ReferenceAccess>, &'static str> {
        let machine = program
            .machine_holding_state(identity.state_symbol)
            .ok_or("output lifetime inputs have no owning machine")?;
        let site = validation::find_call_site(
            program,
            machine.symbol,
            identity.state_symbol,
            identity.statement_index,
            identity.call_ordinal,
        )
        .ok_or("output lifetime inputs have no original call")?;
        let parameters = site
            .parameters(program, machine.symbol)
            .ok_or("output lifetime inputs have no applied parameter list")?;
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == identity.parameter_symbol)
            .ok_or("output lifetime inputs name another output parameter")?;
        let relation = validation::borrow_contracts::resolve_provider_outputs(program, parameters)
            .map_err(|_| "output lifetime input relation is unavailable")?;
        if relation.readiness != validation::borrow_contracts::BorrowRelationReadiness::Complete {
            return Err("output lifetime input relation is not complete");
        }
        let mut outputs = relation.outputs.iter().filter(|output| {
            output.destination
                == validation::borrow_contracts::ProviderOutputDestination::Parameter(position)
                && same_path(
                    program,
                    &crate::borrow::owner_place_segments(&output.owner_path),
                    &crate::borrow::owner_place_segments(owner_path),
                )
        });
        let output = outputs
            .next()
            .ok_or("output lifetime inputs have no declared output leaf")?;
        if outputs.next().is_some() {
            return Err("output lifetime inputs have ambiguous declared leaves");
        }
        let recorded = control
            .operand_retained_suppliers
            .span(sources)
            .ok_or("output lifetime inputs have an unavailable capture list")?;
        if recorded.len() != output.dependencies.len() {
            return Err("output lifetime inputs do not cover the whole declared frontier");
        }
        let mut used = vec![false; output.dependencies.len()];
        let mut accesses = Vec::with_capacity(recorded.len());
        for supplier in recorded {
            let contract = supplier
                .contract
                .ok_or("output lifetime input has no applied edge")?;
            if contract.output_parameter != Some(position)
                || contract.state_symbol != identity.state_symbol
                || contract.statement_index != identity.statement_index
                || contract.call_ordinal != identity.call_ordinal
                || !same_path(
                    program,
                    control
                        .operand_retained_paths
                        .span(contract.owner_path)
                        .ok_or("output lifetime input has no output path")?,
                    &crate::borrow::owner_place_segments(owner_path),
                )
                || source_type(program, borrow, control, supplier).is_none()
            {
                return Err("output lifetime input does not match its exact captured declaration");
            }
            let source_path = control
                .operand_retained_paths
                .span(contract.source_path)
                .ok_or("output lifetime input has no source path")?;
            let matches = output
                .dependencies
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.parameter_index == contract.parameter_index
                        && edge.source == contract.source
                        && program.normalized_type_identity(edge.source_type)
                            == program.normalized_type_identity(contract.source_type)
                        && same_path(
                            program,
                            &crate::borrow::owner_place_segments(&edge.source_path),
                            source_path,
                        )
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = matches.as_slice() else {
                return Err("output lifetime input has no unique declared edge");
            };
            if std::mem::replace(&mut used[*index], true) {
                return Err("output lifetime input repeats one edge instead of another");
            }
            accesses.push(output.dependencies[*index].access);
        }
        Ok(accesses)
    }

    /*
        Authenticate a completed output namespace and every retained input debt.
        This is source correspondence; use sites must separately establish the
        exact performed FlowCall exit or a retained descendant of that occurrence.
    */
    pub(crate) fn completed_output_retention(
        borrow: &checked_trees::BorrowFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: usize,
        call: &checked_trees::BorrowCallFact,
        identity: facts::InvocationOutputIdentity,
        carrier_type: typed_trees::types::TypeReferenceHandle,
        output_path: &[checked_trees::BorrowLoanOwnerSegment],
    ) -> Result<Vec<facts::PlaceRoot>, &'static str> {
        let performed = borrow
            .call_at(machine, state, statement_index, call.call_ordinal)
            .filter(|handle| borrow.calls.get(*handle) == call)
            .ok_or("output has no exact performed invocation")?;
        if identity.state_symbol != state
            || identity.statement_index != statement_index
            || identity.call_ordinal != call.call_ordinal
        {
            return Err("output identity names another invocation");
        }
        let mut selected = Vec::new();
        for handle in call.completed_reference_loans.handles() {
            if !borrow.loans.is_valid(handle) {
                return Err("output completion names an unavailable loan");
            }
            let loan = borrow.loans.get(handle);
            if loan.established_by_call != performed {
                return Err("output loan belongs to another invocation");
            }
            if !loan.incoming_reference.is_valid() {
                continue;
            }
            if !borrow.incoming_references.is_valid(loan.incoming_reference) {
                return Err("output loan has an unavailable identity");
            }
            let incoming = borrow.incoming_references.get(loan.incoming_reference);
            if incoming.origin == (checked_trees::IncomingReferenceOrigin::Output { identity })
                && borrow.owner_segments.span_or_empty(incoming.owner_path) == output_path
            {
                if borrow.supplied_output_call(handle) != Some(performed) {
                    return Err("output permission has no normal-call source correspondence");
                }
                if incoming.machine_symbol != machine || incoming.state_symbol != state {
                    return Err("output permission belongs to another state");
                }
                if incoming.carrier_type != carrier_type {
                    return Err("output permission does not match the captured carrier type");
                }
                if loan.root != facts::PlaceRoot::InvocationOutput(identity)
                    || !loan.segments.is_empty()
                {
                    return Err("output permission does not match the exact opaque backing");
                }
                if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                    return Err("output lifetime debt cannot supply descriptor permission");
                }
                selected.push(loan);
            }
        }
        if selected.is_empty() {
            return Err("completed invocation has no supplied descriptor for this leaf");
        }
        let mut roots = Vec::new();
        for output in selected {
            for handle in call.completed_reference_loans.handles() {
                let loan = borrow.loans.get(handle);
                if loan.storage_owner != output.storage_owner
                    || borrow.loan_owner_path(loan) != borrow.loan_owner_path(output)
                    || loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                {
                    continue;
                }
                let known = match loan.root {
                    facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
                    facts::PlaceRoot::Expression(expression) => expression.is_valid(),
                    facts::PlaceRoot::InvocationOutput(identity) => {
                        identity.is_valid() && loan.incoming_reference.is_valid()
                    }
                    facts::PlaceRoot::InvocationReferent { .. } => {
                        loan.incoming_reference.is_valid()
                    }
                    _ => false,
                };
                if !known {
                    return Err("an output lifetime debt has no known backing identity");
                }
                crate::shared::collections::push_unique(&mut roots, loan.root);
            }
        }
        Ok(roots)
    }
}
mod loans {
    /*
        Loans carried by completed values, saved bindings and replacement stores.
        A normal opaque call publishes its result through `complete_invocation_results`;
        bindings retain that same result before forming any new reference. Each loan
        records its owner (the local, and the field path inside it that holds the
        reference), the place it borrows, its access kind and its lineage.

        `statement_borrow_loans` is the entry; `state::append_state_borrow_facts`
        calls it for each statement in order and records every returned
        `StatementBorrowLoan` as a loan fact. Loans come from:

        - a `let` of reference type (`reference_local_borrow_loans`);
        - a `let` of borrow-carrying data (`borrow_carrying_data_loans`), one loan
          for each reference its initializer supplies (`aggregate`);
        - an assignment to an earlier local, or to a field of one, whose target is
          a reference or borrow-carrying data, handled as the equivalent `let`
          with the field path prefixed to each loan's owner path.

        An already evaluated carrier retains all its reference leaves and input
        lifetimes. New call results use `returned_carriers`; a selection retains
        every possible arm's source. Other formations obtain their place from a borrow,
        recast, place expression or view-returning call
        (`helper_call_borrow_loan_place`, which resolves the call's returned-view
        source through `view_link`). A place rooted at a local that already holds
        a loan in this state is rebased onto that loan's place, once per such
        loan. A copied reference carries its existing borrowing occurrence into
        later view formation. Each selected supplier retains its nearest real
        borrowing parent and the exact saved descriptor used to reach it. Copies
        add no borrow event; unknown source relationships remain unretained.

        `owner_paths` matches owner field paths against place segments, and
        `types` answers whether a type is a reference and with which access.
        Other modules use `borrow_initializer_expressions` (persistent storage
        check),
        `call_declares_direct_view_source` (resource lineage replay), and
        `types::reference_borrow_access_kind` (borrow check details).
        The state builder also captures normal returned leaves through
        `returned_value_loans` while its current descriptor roster is available.
        Those leaves are facts, separate from local activation/lifecycle events.
    */
    use crate::borrow::view_link::is_borrow_carrying_data;
    use crate::flow::{expressions, state_statements};
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::name::Identifier;
    use checked_trees::statement::StatementNode;
    use symbols::SymbolHandle;

    use super::accesses::{self, borrow_access_place};
    use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
    use aggregate::borrowed_initializers;
    use owner_paths::owner_path_matches;
    pub(crate) use owner_paths::{
        owner_path_from_place_segments, owner_place_segments, owner_segment_matches_place_segment,
        store_destination_places,
    };
    pub(crate) use owner_paths::{owner_path_selects_leaf, place_path_covers_owner};

    mod aggregate {
        use crate::borrow::view_link::substituted_returns_borrow;
        use crate::flow::expressions;
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};
        use symbols::SymbolHandle;

        use super::super::tracker::BorrowOwnerSegment;

        #[derive(Clone)]
        pub(in crate::borrow) struct BorrowedInitializer {
            pub(super) owner_path: Vec<BorrowOwnerSegment>,
            pub(super) expression: ExpressionHandle,
            pub(super) kind: BorrowedInitializerKind,
        }

        #[derive(Clone, Copy)]
        pub(in crate::borrow) enum BorrowedInitializerKind {
            Reference {
                access: language_semantics::ReferenceAccess,
                type_reference: typed_trees::types::TypeReferenceHandle,
            },
            Aggregate {
                type_reference: typed_trees::types::TypeReferenceHandle,
            },
        }

        /*
            Resolve every structurally carried reference leaf or direct aggregate-value
            source, together with its projection within the aggregate owner. Aggregate
            sources are expanded later from call-return or transferred-local loan facts.
        */
        pub(super) fn borrowed_initializers(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
            expression: ExpressionHandle,
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            owner_path: &[BorrowOwnerSegment],
        ) -> Vec<BorrowedInitializer> {
            use typed_trees::types::TypeReferenceNode;

            if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
            {
                return expressions(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .flat_map(|arm| {
                        borrowed_initializers(
                            program,
                            type_reference,
                            arm.value,
                            substitutions,
                            owner_path,
                        )
                    })
                    .collect();
            }

            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { access, .. } => vec![BorrowedInitializer {
                    owner_path: owner_path.to_vec(),
                    expression,
                    kind: BorrowedInitializerKind::Reference {
                        access: *access,
                        type_reference,
                    },
                }],
                TypeReferenceNode::Constrained { base_type, .. } => borrowed_initializers(
                    program,
                    *base_type,
                    expression,
                    substitutions,
                    owner_path,
                ),
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    if let Some(initializer) = aggregate_value_initializer(
                        program,
                        type_reference,
                        expression,
                        substitutions,
                        owner_path,
                    ) {
                        return vec![initializer];
                    }
                    let Some(values) = program.expression_table.array_literal_node(expression)
                    else {
                        return Vec::new();
                    };
                    expressions(program)
                        .expression_handles(*values)
                        .iter()
                        .enumerate()
                        .flat_map(|(index, value)| {
                            let mut element_path = owner_path.to_vec();
                            element_path.push(BorrowOwnerSegment::FixedIndex(index));
                            borrowed_initializers(
                                program,
                                *element_type,
                                *value,
                                substitutions,
                                &element_path,
                            )
                        })
                        .collect()
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } => {
                    if let Some(initializer) = aggregate_value_initializer(
                        program,
                        type_reference,
                        expression,
                        substitutions,
                        owner_path,
                    ) {
                        return vec![initializer];
                    }
                    let Some(definition) = program.data_definition_by_symbol(*base_symbol) else {
                        return Vec::new();
                    };
                    if program
                        .data_application_definition(type_reference)
                        .is_some()
                    {
                        return borrowed_data_literal_initializers(
                            program,
                            type_reference,
                            expression,
                            &[],
                            owner_path,
                        );
                    }
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    let mut nested_substitutions = substitutions.to_vec();
                    nested_substitutions.extend(
                        program
                            .data_type_parameters(definition)
                            .iter()
                            .zip(arguments.iter())
                            .map(|(parameter, argument)| (parameter.symbol, *argument)),
                    );
                    borrowed_data_literal_initializers(
                        program,
                        type_reference,
                        expression,
                        &nested_substitutions,
                        owner_path,
                    )
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, concrete)) = substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    {
                        return borrowed_initializers(
                            program,
                            *concrete,
                            expression,
                            substitutions,
                            owner_path,
                        );
                    }
                    if let Some(initializer) = aggregate_value_initializer(
                        program,
                        type_reference,
                        expression,
                        substitutions,
                        owner_path,
                    ) {
                        return vec![initializer];
                    }
                    borrowed_data_literal_initializers(
                        program,
                        type_reference,
                        expression,
                        substitutions,
                        owner_path,
                    )
                }
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => Vec::new(),
            }
        }

        fn aggregate_value_initializer(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
            expression: ExpressionHandle,
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            owner_path: &[BorrowOwnerSegment],
        ) -> Option<BorrowedInitializer> {
            let is_direct_aggregate_value = match program.expression_table.expression(expression) {
                ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_) => true,
                ExpressionNode::Cast(cast) => !cast.form.is_recast(),
                _ => false,
            };
            if !substituted_returns_borrow(program, type_reference, substitutions, &[0])
                || !is_direct_aggregate_value
            {
                return None;
            }
            Some(BorrowedInitializer {
                owner_path: owner_path.to_vec(),
                expression,
                kind: BorrowedInitializerKind::Aggregate { type_reference },
            })
        }

        fn borrowed_data_literal_initializers(
            program: &dyn ::validation::CheckingInput,
            receiving: typed_trees::types::TypeReferenceHandle,
            expression: ExpressionHandle,
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            owner_path: &[BorrowOwnerSegment],
        ) -> Vec<BorrowedInitializer> {
            let Some(literal) = program.expression_table.struct_literal_node(expression) else {
                return Vec::new();
            };
            if program
                .data_constructor_definition(receiving, literal.type_symbol)
                .is_none()
            {
                return Vec::new();
            }
            let selected_case = literal.case_symbol.and_then(|selected| {
                program.data_constructor_case(receiving, literal.type_symbol, selected)
            });
            if literal.case_symbol.is_some() != selected_case.is_some()
                || literal.case_name.is_some() != selected_case.is_some()
            {
                return Vec::new();
            }

            let mut fields = Vec::new();
            let mut initializers = Vec::new();
            for literal_field in program.expression_table.struct_fields(literal.fields) {
                let Some(field) = program.data_constructor_field(
                    receiving,
                    literal.type_symbol,
                    literal_field.field_symbol,
                ) else {
                    return Vec::new();
                };
                if fields.contains(&field.symbol) {
                    return Vec::new();
                }
                fields.push(field.symbol);
                let mut field_path = owner_path.to_vec();
                if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
                    if selected_case.map(|case| case.symbol) != Some(variant) {
                        return Vec::new();
                    }
                    field_path.push(BorrowOwnerSegment::Case(variant));
                }
                field_path.push(BorrowOwnerSegment::Field(field.symbol));
                initializers.extend(borrowed_initializers(
                    program,
                    field.type_reference,
                    literal_field.value,
                    substitutions,
                    &field_path,
                ));
            }
            initializers
        }
    }
    mod owner_paths {
        use super::super::tracker::BorrowOwnerSegment;
        use crate::flow::expressions;

        /* Retain unknown selections as unknown coordinates, never as absent loans. */
        pub(crate) fn owner_place_segments(
            path: &[BorrowOwnerSegment],
        ) -> Vec<facts::PlaceSegment> {
            path.iter()
                .map(|segment| match segment {
                    BorrowOwnerSegment::Field(symbol) => {
                        facts::PlaceSegment::Field { symbol: *symbol }
                    }
                    BorrowOwnerSegment::Case(variant) => {
                        facts::PlaceSegment::Case { variant: *variant }
                    }
                    BorrowOwnerSegment::FixedIndex(index) => {
                        facts::PlaceSegment::FixedIndex { index: *index }
                    }
                    BorrowOwnerSegment::DynamicIndex => facts::PlaceSegment::Index {
                        expression: typed_trees::expression::ExpressionHandle::invalid(),
                    },
                })
                .collect()
        }

        /*
            Resolve the storage containing a replaced reference slot. A carried loan
            is an address crossing only where the declaration itself is a reference;
            the old loan held by the slot being replaced is deliberately not crossed.
            A referent write also crosses the selected descriptor itself; rebinding
            that descriptor crosses only its ancestors.
        */
        pub(crate) fn store_destination_places<'loan>(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            target: checked_trees::CapturedPlace,
            writes_referent: bool,
            loans: impl Iterator<
                Item = (
                    facts::PlaceRoot,
                    &'loan [BorrowOwnerSegment],
                    arena::Handle<checked_trees::IncomingReferenceIdentity>,
                    &'loan [facts::PlaceSegment],
                    facts::PlaceRoot,
                    &'loan [facts::PlaceSegment],
                ),
            >,
        ) -> Vec<checked_trees::CapturedPlace> {
            let owner_type = validation::canonical_place_type_reference(
                program,
                state.symbol,
                state.statement_nodes.count() as usize,
                &validation::CanonicalPlace {
                    root: target.root,
                    segments: Vec::new(),
                },
            );
            let mut destinations = Vec::new();
            for (owner, owner_path, incoming_reference, incoming_owner_path, source, source_path) in
                loans
            {
                if owner != target.root
                    || owner_path.len() > target.segments.len()
                    || (owner_path.len() == target.segments.len() && !writes_referent)
                    || !owner_path_matches(program, owner_path, &target.segments)
                    || !owner_type
                        .and_then(|reference| {
                            validation::project_type_reference_from_segments(
                                program,
                                reference,
                                &target.segments[..owner_path.len()],
                            )
                        })
                        .is_some_and(|reference| {
                            super::types::is_reference_type(program, reference)
                        })
                {
                    continue;
                }
                let mut segments = source_path.to_vec();
                segments.extend_from_slice(&target.segments[owner_path.len()..]);
                let destination = checked_trees::CapturedPlace {
                    incoming_reference,
                    incoming_owner_path: if incoming_owner_path.is_empty()
                        && owner_path.contains(&BorrowOwnerSegment::DynamicIndex)
                    {
                        target.segments[..owner_path.len()].to_vec()
                    } else {
                        incoming_owner_path.to_vec()
                    },
                    root: source,
                    segments,
                };
                crate::shared::collections::push_unique(&mut destinations, destination);
            }
            if destinations.is_empty() {
                destinations.push(target);
            }
            destinations
        }

        pub(crate) fn owner_path_from_place_segments(
            program: &dyn ::validation::CheckingInput,
            segments: &[facts::PlaceSegment],
        ) -> Vec<BorrowOwnerSegment> {
            segments
                .iter()
                .map(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => BorrowOwnerSegment::Field(*symbol),
                    facts::PlaceSegment::Case { variant } => BorrowOwnerSegment::Case(*variant),
                    facts::PlaceSegment::FixedIndex { index } => {
                        BorrowOwnerSegment::FixedIndex(*index)
                    }
                    facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                        BorrowOwnerSegment::DynamicIndex
                    }
                    facts::PlaceSegment::Index { expression } => expressions(program)
                        .constant_integer_value(*expression)
                        .and_then(|value| usize::try_from(value).ok())
                        .map(BorrowOwnerSegment::FixedIndex)
                        .unwrap_or(BorrowOwnerSegment::DynamicIndex),
                })
                .collect()
        }

        /*
            Whether one owner-path segment selects the same storage as one place
            segment. `DynamicIndex` owners match no fixed shape here — each caller
            decides which index spellings it accepts.
        */
        pub(crate) fn owner_segment_matches_place_segment(
            program: &dyn ::validation::CheckingInput,
            owner: &BorrowOwnerSegment,
            place: &facts::PlaceSegment,
        ) -> Option<bool> {
            match (owner, place) {
                (
                    BorrowOwnerSegment::Field(owner_symbol),
                    facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => Some(
                    !place_symbol.is_valid() || owner_symbol == place_symbol || {
                        program
                            .any_field_owner(*owner_symbol)
                            .zip(program.any_field_owner(*place_symbol))
                            .is_some_and(|(owner, selected)| {
                                owner.1.path_identity() == selected.1.path_identity()
                            })
                    },
                ),
                (
                    BorrowOwnerSegment::Case(owner_variant),
                    facts::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => Some(owner_variant == place_variant),
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::FixedIndex { index: place_index },
                ) => Some(owner_index == place_index),
                (
                    BorrowOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::Index { expression },
                ) => Some(
                    expressions(program)
                        .constant_integer_value(*expression)
                        .and_then(|value| usize::try_from(value).ok())
                        .is_none_or(|place_index| *owner_index == place_index),
                ),
                _ => None,
            }
        }

        pub(super) fn owner_path_matches(
            program: &dyn ::validation::CheckingInput,
            owner_path: &[BorrowOwnerSegment],
            place_segments: &[facts::PlaceSegment],
        ) -> bool {
            owner_path.len() <= place_segments.len()
                && owner_path.iter().zip(place_segments).all(|(owner, place)| {
                    owner_segment_matches_place_segment(program, owner, place).unwrap_or({
                        matches!(
                            (owner, place),
                            (
                                BorrowOwnerSegment::DynamicIndex,
                                facts::PlaceSegment::FixedIndex { .. }
                                    | facts::PlaceSegment::FixedRange { .. }
                                    | facts::PlaceSegment::OpaqueRange
                                    | facts::PlaceSegment::Index { .. }
                            )
                        )
                    })
                })
        }

        /* A selected projection retains every carried loan it may include. */
        pub(super) fn place_path_matches_owner_prefix(
            program: &dyn ::validation::CheckingInput,
            place_segments: &[facts::PlaceSegment],
            owner_path: &[BorrowOwnerSegment],
        ) -> bool {
            place_segments.len() <= owner_path.len()
                && owner_path_matches(program, &owner_path[..place_segments.len()], place_segments)
        }

        /*
            Retirement requires definite coverage, unlike access conflict detection.
            One unknown element overlaps many homes but overwrites only one of them;
            conversely, an exact element cannot retire a loan whose home may be another
            element. Replacing an enclosing collection still covers all its children.
        */
        pub(crate) fn place_path_covers_owner(
            program: &dyn ::validation::CheckingInput,
            place_segments: &[facts::PlaceSegment],
            owner_path: &[BorrowOwnerSegment],
        ) -> bool {
            place_segments.len() <= owner_path.len()
                && owner_path.iter().zip(place_segments).all(|(owner, place)| {
                    match (owner, place) {
                        (BorrowOwnerSegment::DynamicIndex, _) => false,
                        (
                            BorrowOwnerSegment::FixedIndex(owner_index),
                            facts::PlaceSegment::FixedRange { start, end },
                        ) => *start <= *owner_index && *owner_index < *end,
                        (_, facts::PlaceSegment::Index { expression }) => {
                            let BorrowOwnerSegment::FixedIndex(owner_index) = owner else {
                                return false;
                            };
                            expressions(program)
                                .constant_integer_value(*expression)
                                .and_then(|value| usize::try_from(value).ok())
                                .is_some_and(|place_index| *owner_index == place_index)
                        }
                        (_, facts::PlaceSegment::Field { symbol }) if !symbol.is_valid() => false,
                        _ => owner_segment_matches_place_segment(program, owner, place)
                            .unwrap_or(false),
                    }
                })
        }

        /*
            Select one exact descriptor leaf from its declared owner path, including
            a collection's abstract leaf family. This is a shape join only: callers
            still owe current-storage eligibility, bounds, formation and permission.
        */
        pub(crate) fn owner_path_selects_leaf(
            program: &dyn ::validation::CheckingInput,
            owner: &[BorrowOwnerSegment],
            selected: &[facts::PlaceSegment],
        ) -> bool {
            owner.len() == selected.len()
                && owner
                    .iter()
                    .zip(selected)
                    .all(|(owner, selected)| match (owner, selected) {
                        (
                            BorrowOwnerSegment::DynamicIndex,
                            facts::PlaceSegment::FixedIndex { .. },
                        ) => true,
                        (
                            BorrowOwnerSegment::DynamicIndex,
                            facts::PlaceSegment::Index { expression },
                        ) => expression.is_valid(),
                        (_, facts::PlaceSegment::Field { symbol }) if !symbol.is_valid() => false,
                        _ => {
                            owner_segment_matches_place_segment(program, owner, selected)
                                == Some(true)
                        }
                    })
        }
    }
    mod returned_carriers {
        /*
            Returned carrier loans retain the declaration's complete lifetime frontier.
            A body relation may refine its backing geometry, but cannot invent a lifetime
            source. Source selectors and result selectors are distinct.
            Resolve the complete producer relation before selecting result leaves, then
            translate those leaves into the destination's checked storage paths. Runtime
            carrier equality erases regions and cannot equate prepared member symbols.
            Lifetime-only carriers transport the supplier's complete existing loan debt.
            They contain no reference view whose declared access could attenuate that
            debt: treating an opaque Pending as a shared view would release exclusive
            suspension while the provider still retains its mutable original pointer.
        */
        use super::aggregate_loans::borrow_carrying_data_loans;
        use super::aggregate_loans::result_source_loans;
        use super::call_views::CallViewReturnContract;
        use super::is_reference_type;
        use super::owner_paths::owner_path_from_place_segments;
        use super::owner_paths::owner_place_segments as place_segments;
        use super::owner_paths::owner_segment_matches_place_segment;
        use crate::borrow::BorrowOwnerSegment;
        use crate::borrow::loans::StatementBorrowLoan;
        use crate::borrow::loans::borrowed_initializer_loans;
        use crate::borrow::loans::borrowed_initializers;
        use crate::borrow::loans::call_view_return_contract;
        use crate::borrow::loans::owner_path_matches;
        use crate::borrow::loans::reference_borrow_access_kind;
        use crate::borrow::structural_owner_path;
        use crate::borrow::tracker::StateLoanTracker;
        use crate::borrow::view_link::ViewReturnFieldSource;
        use crate::borrow::view_link::ViewReturnSource;
        use crate::borrow::view_link::resolve_view_return_source;
        use crate::flow::expressions;
        use checked_trees::BorrowAccessKind;
        use checked_trees::BorrowLoanLineage;
        use checked_trees::BorrowReturnedLoanFact;
        use checked_trees::CapturedPlace;
        use checked_trees::CheckFacts;
        use checked_trees::StateBorrowFact;
        use checked_trees::statement::TableLocalData;
        use facts::PlaceRoot;
        use facts::PlaceSegment;
        use language_semantics::MachineSupplyMode;
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceHandle;
        use validation::CallFrameResolver;
        use validation::ReferenceResultLeafOrigin;
        use validation::ReferenceResultTransport;
        use validation::borrow_contracts::ViewReturnInputSource;
        use validation::call_target_entry_symbol;
        use validation::effective_member_symbol;
        use validation::expression_type_reference_in_state;
        use validation::find_state;
        use validation::project_type_reference_from_segments;
        use validation::reference_result_custody::declared_carrier_references;
        use validation::reference_result_custody::input_carrier_type;
        use validation::reference_result_custody::loan_owner_path;
        use validation::reference_result_custody::origin_place_segments;
        use validation::reference_shell;

        use checked_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use symbols::SymbolHandle;

        /*
            Descriptor candidates and lifetime debts share one completed relation.
            Address consumers use only descriptor candidates; unused declared inputs
            remain independently retained debts in the loan producer below.
        */
        pub(crate) fn call_reference_sources(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            call: &TableCallExpression,
        ) -> Option<Vec<(ViewReturnFieldSource, Vec<PlaceSegment>)>> {
            let sources = completed_call_reference_sources(program, state_symbol, call)?;
            if sources
                .iter()
                .any(|source| matches!(source, CompletedReturnSource::UnresolvedDescriptor { .. }))
            {
                return None;
            }
            Some(
                sources
                    .into_iter()
                    .filter_map(CompletedReturnSource::descriptor_candidate)
                    .collect(),
            )
        }

        /*
            Complete returned descriptor alternatives, keeping immutable image storage
            distinct from input suppliers, lifetime debts and unavailable spatial rows.
        */
        pub(crate) fn completed_call_reference_sources(
            program: &dyn validation::CheckingInput,
            state_symbol: SymbolHandle,
            call: &TableCallExpression,
        ) -> Option<Vec<CompletedReturnSource>> {
            let fields = call_reference_lifetime_sources(program, state_symbol, call)?;
            Some(completed_sources(
                program,
                call.target_symbol,
                fields,
                program.expression_table.expression_handles(call.arguments),
            ))
        }

        /*
            The complete declared lifetime relation, without optional body geometry.
            Captured values compose this relation with their actual input lifetimes;
            no signature fallback establishes an exact returned address.
        */
        pub(crate) fn call_reference_lifetime_sources(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            call: &TableCallExpression,
        ) -> Option<Vec<ViewReturnFieldSource>> {
            let contract = call_view_return_contract(program, state_symbol, call)?;
            match contract.source(program) {
                validation::borrow_contracts::ViewReturnSource::NotApplicable => Some(Vec::new()),
                _ => contract.reference_sources(program),
            }
        }

        /*
            A source selected by the body can identify a descriptor, while another
            input may still owe its declared lifetime without identifying that address.
            If body reconstruction is unavailable, descriptor candidates retain the
            conservative lifetime/interference frontier. Actual address selection or a
            further reference load still needs its own exact captured geometry; the
            frontier alone cannot identify that address or grant a stored descriptor.
        */
        pub(crate) enum CompletedReturnSource {
            /*
                Positive body-derived image formation, never inferred from an empty
                input roster. It supplies no caller storage address or loan ancestry.
            */
            ImmutableImage {
                owner_path: Vec<BorrowOwnerSegment>,
                kind: BorrowAccessKind,
            },
            DescriptorCandidate {
                field: ViewReturnFieldSource,
                projection: Vec<PlaceSegment>,
            },
            LifetimeDependency {
                field: ViewReturnFieldSource,
            },
            /*
                A recognized opaque output is not an input-relative address. Keep
                this unavailable spatial alternative beside all declared lifetime debts;
                another return route must not mask it with a convenient input source.
            */
            UnresolvedDescriptor {
                owner_path: Vec<BorrowOwnerSegment>,
                kind: BorrowAccessKind,
            },
        }

        impl CompletedReturnSource {
            fn descriptor_candidate(self) -> Option<(ViewReturnFieldSource, Vec<PlaceSegment>)> {
                match self {
                    Self::DescriptorCandidate { field, projection } => Some((field, projection)),
                    Self::ImmutableImage { .. }
                    | Self::LifetimeDependency { .. }
                    | Self::UnresolvedDescriptor { .. } => None,
                }
            }

            /*
                Capture every declared input obligation. A retained lifetime cannot
                become a spatial root merely because it shares the result's owner path.
            */
            pub(super) fn capture_loans(
                self,
                local: &TableLocalData,
                owner_prefix: &[BorrowOwnerSegment],
                capture: impl FnOnce(
                    ViewReturnFieldSource,
                    Vec<PlaceSegment>,
                ) -> Result<Vec<StatementBorrowLoan>, &'static str>,
            ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
                match self {
                    Self::ImmutableImage { .. } => Ok(Vec::new()),
                    Self::UnresolvedDescriptor {
                        mut owner_path,
                        kind,
                    } => {
                        let mut path = owner_prefix.to_vec();
                        path.append(&mut owner_path);
                        Ok(vec![StatementBorrowLoan {
                            formation_owner_path: None,
                            incoming_origin: None,
                            formation_expression: local.initial_value,
                            representation_address: None,
                            owner_symbol: local.symbol,
                            storage_owner: PlaceRoot::Symbol(local.symbol),
                            owner_name: local.name.clone(),
                            owner_path: path,
                            retained_operand: None,
                            place: CapturedPlace::default(),
                            source_owner_symbol: SymbolHandle::invalid(),
                            supplying_loans: Vec::new(),
                            lineage: BorrowLoanLineage::UnretainedDerived,
                            kind,
                            formation: super::BorrowFormation::ReturnedDescriptor,
                        }])
                    }
                    Self::DescriptorCandidate { field, projection } => capture(field, projection),
                    Self::LifetimeDependency { field } => {
                        let mut loans = capture(field, Vec::new())?;
                        for loan in &mut loans {
                            loan.lineage = BorrowLoanLineage::LifetimeDependency;
                        }
                        Ok(loans)
                    }
                }
            }
        }

        pub(super) fn completed_sources(
            program: &dyn ::validation::CheckingInput,
            target: SymbolHandle,
            fields: Vec<ViewReturnFieldSource>,
            arguments: &[ExpressionHandle],
        ) -> Vec<CompletedReturnSource> {
            let (dependencies, fields): (Vec<_>, Vec<_>) =
                fields.into_iter().partition(|field| field.lifetime_only);
            let mut sources =
                match completed_field_sources(program, target, &fields, None, arguments) {
                    Some(Ok(sources)) => sources,
                    Some(Err(_)) => Vec::new(),
                    None => fields
                        .into_iter()
                        .map(|field| CompletedReturnSource::DescriptorCandidate {
                            field,
                            projection: Vec::new(),
                        })
                        .collect(),
                };
            sources.extend(
                dependencies
                    .into_iter()
                    .map(|field| CompletedReturnSource::LifetimeDependency { field }),
            );
            sources
        }

        /*
            Compare actual normal-return captures with the declaration's lifetime
            frontier. Constructor projection, copies and replacement already selected
            their current descriptors during formation; replaying their syntax here
            could silently treat an unrecognized source as permission to return it.
        */
        pub(crate) fn returned_body_source_mismatch(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: &State,
            call_frames: Option<&CallFrameResolver<'_>>,
        ) -> Option<usize> {
            let fields = match resolve_view_return_source(program, state) {
                ViewReturnSource::Fields { fields } => fields,
                ViewReturnSource::Parameter { parameter_index } => vec![ViewReturnFieldSource {
                    lifetime_only: false,
                    source: validation::borrow_contracts::ViewReturnInputSource::Referent,
                    owner_path: Vec::new(),
                    source_path: Vec::new(),
                    source_type: program
                        .state_parameters(state)
                        .get(parameter_index)?
                        .type_reference,
                    source_reference_hops: 0,
                    parameter_index,
                    kind: reference_borrow_access_kind(program, state.return_type)?,
                }],
                ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => return None,
            };
            if let Some(sources) =
                call_frames.and_then(|frames| frames.reference_result_leaf_origins(state.symbol))
                && let Some(Err(parameter)) =
                    judge_completed_field_sources(program, state, &fields, sources)
            {
                return Some(parameter);
            }
            let (_, borrowing) = facts.borrow.first_state_borrow_by_symbol(state.symbol)?;
            for returned in facts.borrow.returned_loans.span(borrowing.returned_loans)? {
                let (parameter_symbol, source_path) = if returned.incoming_reference.is_valid() {
                    let incoming = facts
                        .borrow
                        .incoming_references
                        .get(returned.incoming_reference);
                    (
                        incoming.parameter_symbol(),
                        facts.borrow.owner_segments.span(incoming.owner_path)?,
                    )
                } else {
                    let Some(parameter) =
                        program.state_parameters(state).iter().find(|parameter| {
                            returned.root == PlaceRoot::Symbol(parameter.symbol)
                                && reference_shell(program, parameter.type_reference).is_some()
                        })
                    else {
                        continue;
                    };
                    (parameter.symbol, &[][..])
                };
                let parameter_index = program
                    .state_parameters(state)
                    .iter()
                    .position(|parameter| parameter.symbol == parameter_symbol)?;
                if !fields
                    .iter()
                    .any(|field| field.parameter_index == parameter_index)
                {
                    return Some(parameter_index);
                }
                let owner_path = structural_owner_path(
                    program,
                    facts.borrow.owner_segments.span(returned.owner_path)?,
                )?;
                let source = if returned.incoming_reference.is_valid() {
                    ViewReturnInputSource::Stored
                } else {
                    ViewReturnInputSource::Referent
                };
                if !fields.iter().any(|field| {
                    field.parameter_index == parameter_index
                        && ((field.lifetime_only
                            && returned.lineage == BorrowLoanLineage::LifetimeDependency)
                            || field.kind == returned.kind)
                        && structural_owner_path(program, &field.owner_path).as_ref()
                            == Some(&owner_path)
                        && ((field.source == source
                            && source_path_covers(program, &field.source_path, source_path))
                            || incoming_reborrow_matches_input_lifetime(
                                program,
                                facts,
                                borrowing,
                                field,
                                returned,
                                source_path,
                            ))
                }) {
                    return Some(parameter_index);
                }
            }
            None
        }

        /*
            A borrow of a carrier bounds reborrows of its stored permissions, without
            making their referents part of the carrier's inline storage. The exact
            entry permission and typed leaf establish this lifetime correspondence;
            the returned capture keeps its incoming namespace, suffix and lineage.
            In particular, this neither turns Stored into Referent nor invents an
            immediate parent from a matching path. Formation still owns that judgment.
        */
        fn incoming_reborrow_matches_input_lifetime(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &StateBorrowFact,
            field: &ViewReturnFieldSource,
            returned: &BorrowReturnedLoanFact,
            source_path: &[BorrowOwnerSegment],
        ) -> bool {
            if field.lifetime_only
                || field.source != ViewReturnInputSource::Referent
                || !field.source_path.is_empty()
                || !facts
                    .borrow
                    .state_owns_incoming_reference(state, returned.incoming_reference)
            {
                return false;
            }
            let incoming = facts
                .borrow
                .incoming_references
                .get(returned.incoming_reference);
            let Some(input) = reference_shell(program, field.source_type) else {
                return false;
            };
            if incoming.carrier_type != input.referee {
                return false;
            }
            let Some(leaves) = declared_carrier_references(program, input.referee) else {
                return false;
            };
            leaves.iter().any(|leaf| {
                let Some(segments) =
                    origin_place_segments(program, input.referee, leaf.path.iter().cloned())
                else {
                    return false;
                };
                let owner_path = owner_path_from_place_segments(program, &segments);
                source_path_covers(program, &owner_path, source_path)
            })
        }

        /*
            Refine declaration-level lifetime candidates with the checked body's
            complete returned-leaf relation. Descriptor selection in an owned input
            stays separate from a projection beneath a borrowed input. A declaration
            without a reconstructible body retains its conservative lifetime footprint;
            executable custody still requires an exact returned-origin relation. `None`
            means unavailable reconstruction, `Err` names a contradictory formal input,
            and `Ok` preserves selected descriptor candidates and every remaining
            declared lifetime obligation with its distinct dependency role.
        */
        fn completed_field_sources(
            program: &dyn ::validation::CheckingInput,
            target: SymbolHandle,
            fields: &[ViewReturnFieldSource],
            call_frames: Option<&CallFrameResolver<'_>>,
            arguments: &[ExpressionHandle],
        ) -> Option<Result<Vec<CompletedReturnSource>, usize>> {
            let entered_target = call_target_entry_symbol(program, target);
            let (machine, state) = find_state(program, entered_target)?;
            if machine.supply_mode != MachineSupplyMode::CheckedBody || !machine.body_is_present {
                return None;
            }
            let prepared_frames;
            let call_frames = match call_frames {
                Some(call_frames) => call_frames,
                None => {
                    prepared_frames = CallFrameResolver::new(program)?;
                    &prepared_frames
                }
            };
            let transports =
                call_frames.reference_result_transports_for_arguments(state.symbol, arguments)?;
            let mut sources = Vec::new();
            for transport in transports {
                match transport {
                    ReferenceResultTransport::Reference(leaf) => sources.push(leaf),
                    ReferenceResultTransport::Value {
                        result_path,
                        source,
                        type_reference,
                    } => {
                        for leaf in declared_carrier_references(program, type_reference)? {
                            let suffix = origin_place_segments(program, type_reference, leaf.path)?;
                            let mut destination = result_path.clone();
                            destination.extend_from_slice(&suffix);
                            let mut source = source.clone();
                            source.segments.extend(suffix);
                            sources.push(ReferenceResultLeafOrigin {
                                result_path: destination,
                                source,
                            });
                        }
                    }
                }
            }
            judge_completed_field_sources(program, state, fields, sources)
        }

        fn judge_completed_field_sources(
            program: &dyn validation::CheckingInput,
            state: &State,
            fields: &[ViewReturnFieldSource],
            sources: Vec<ReferenceResultLeafOrigin>,
        ) -> Option<Result<Vec<CompletedReturnSource>, usize>> {
            let parameters = program.state_parameters(state);
            let declared_paths = fields
                .iter()
                .map(|field| structural_owner_path(program, &field.owner_path))
                .collect::<Option<Vec<_>>>()?;
            let mut output = Vec::with_capacity(sources.len());
            let mut covered = vec![false; fields.len()];
            for source in sources {
                let owner_path = structural_owner_path(
                    program,
                    &owner_path_from_place_segments(program, &source.result_path),
                )?;
                let result_leaf = project_type_reference_from_segments(
                    program,
                    state.return_type,
                    &source.result_path,
                )?;
                let kind = reference_borrow_access_kind(program, result_leaf)?;
                if source.source.is_image_storage {
                    output.push(CompletedReturnSource::ImmutableImage {
                        owner_path: owner_path_from_place_segments(program, &source.result_path),
                        kind,
                    });
                    continue;
                }
                if !source.source.recasts.is_empty() || source.source.representation.is_some() {
                    return None;
                }
                if source.source.invocation_output.is_some() {
                    output.push(CompletedReturnSource::UnresolvedDescriptor {
                        owner_path: owner_path_from_place_segments(program, &source.result_path),
                        kind,
                    });
                    continue;
                }
                let (parameter_index, parameter) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == source.source.root)?;
                let projection = if is_reference_type(program, parameter.type_reference) {
                    source
                        .source
                        .segments
                        .iter()
                        .map(|segment| {
                            segment.erase_runtime_selector(|expression| {
                                program.expression_table.range_node(expression).is_some()
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let mut matched = false;
                for (index, field) in fields.iter().enumerate().filter(|(index, field)| {
                    !field.lifetime_only
                        && declared_paths[*index] == owner_path
                        && (field.source == ViewReturnInputSource::Referent
                            || stored_source_covers(
                                program,
                                parameter.type_reference,
                                &field.source_path,
                                &source.source.segments,
                            ))
                        && field.parameter_index == parameter_index
                        && field.kind == kind
                }) {
                    matched = true;
                    covered[index] = true;
                    output.push(CompletedReturnSource::DescriptorCandidate {
                        field: field.clone(),
                        projection: projection.clone(),
                    });
                }
                if !matched {
                    return Some(Err(parameter_index));
                }
            }
            for (index, field) in fields.iter().enumerate() {
                if !covered[index] {
                    output.push(CompletedReturnSource::LifetimeDependency {
                        field: field.clone(),
                    });
                }
            }
            Some(Ok(output))
        }

        /*
            Declaration element families conservatively cover possible input sources.
            This comparison grants no equality, disjointness or restoration of runtime
            places; those retain their evaluated selector identities in captured loans.
        */
        fn source_path_covers(
            program: &dyn validation::CheckingInput,
            declared: &[BorrowOwnerSegment],
            captured: &[BorrowOwnerSegment],
        ) -> bool {
            declared.len() == captured.len()
                && owner_path_matches(program, declared, &place_segments(captured))
        }

        /*
            A stored descriptor may supply a projection of its backing. The source
            path identifies that descriptor, not the complete returned address. Only
            its first crossing is supplied here; another stored reference needs its
            own captured descriptor identity rather than authority from a prefix.
        */
        fn stored_source_covers(
            program: &dyn validation::CheckingInput,
            parameter: TypeReferenceHandle,
            declared: &[BorrowOwnerSegment],
            selected: &[PlaceSegment],
        ) -> bool {
            if !owner_path_matches(program, declared, selected) {
                return false;
            }
            let Some(carrier) = input_carrier_type(program, parameter) else {
                return false;
            };
            let Some(descriptor) =
                project_type_reference_from_segments(program, carrier, &selected[..declared.len()])
            else {
                return false;
            };
            if reference_shell(program, descriptor).is_none() {
                return false;
            }
            (declared.len() + 1..selected.len()).all(|prefix| {
                project_type_reference_from_segments(program, carrier, &selected[..prefix])
                    .is_some_and(|reference| reference_shell(program, reference).is_none())
            })
        }

        macro_rules! available {
            ($query:expr) => {
                match $query {
                    Some(value) => value,
                    None => return Ok(None),
                }
            };
        }

        /*
            Follow finite call operands without collapsing carried source leaves into
            the single-place query used for bare reference inputs.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn result_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &TableLocalData,
            expression: ExpressionHandle,
            owner_path: &[BorrowOwnerSegment],
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Option<Vec<StatementBorrowLoan>>, &'static str> {
            let mut loans = available!(relative_result_loans(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                local_data,
                expression,
                loan_trackers,
                sources,
            )?);
            let prefix = available!(crate::borrow::carrier_path(
                program,
                local_data.type_reference,
                owner_path
            ));
            for loan in &mut loans {
                let mut path = prefix.clone();
                path.extend(available!(structural_owner_path(program, &loan.owner_path)));
                loan.owner_path =
                    available!(loan_owner_path(program, local_data.type_reference, &path));
            }
            Ok(Some(loans))
        }

        pub(super) fn relative_result_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &TableLocalData,
            mut expression: ExpressionHandle,
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Option<Vec<StatementBorrowLoan>>, &'static str> {
            let mut access_limits = Vec::new();
            loop {
                let call = match program.expression_table.expression(expression) {
                    ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                        let type_reference = available!(expression_type_reference_in_state(
                            program,
                            state_symbol,
                            statement_index,
                            expression,
                        ));
                        let carrier = TableLocalData {
                            type_reference,
                            initial_value: expression,
                            ..local_data.clone()
                        };
                        let state = available!(program.state_by_symbol(state_symbol));
                        return Ok(Some(limit_access(
                            borrow_carrying_data_loans(
                                program,
                                state,
                                statement_index,
                                machine_symbol,
                                &carrier,
                                loan_trackers,
                                sources,
                            )?,
                            &access_limits,
                        )));
                    }
                    ExpressionNode::Call(call) => call,
                    ExpressionNode::Indexed(indexed) => {
                        let loans = available!(relative_result_loans(
                            program,
                            state_symbol,
                            statement_index,
                            machine_symbol,
                            local_data,
                            indexed.collection,
                            loan_trackers,
                            sources,
                        )?);
                        let index = expressions(program)
                            .constant_integer_value(indexed.index)
                            .and_then(|value| usize::try_from(value).ok());
                        let loans = loans
                            .into_iter()
                            .filter_map(|mut loan| {
                                let suffix = match loan.owner_path.as_slice() {
                                    [] => &[][..],
                                    [BorrowOwnerSegment::FixedIndex(candidate), suffix @ ..]
                                        if index.is_none_or(|index| index == *candidate) =>
                                    {
                                        suffix
                                    }
                                    [BorrowOwnerSegment::DynamicIndex, suffix @ ..] => suffix,
                                    _ => return None,
                                };
                                loan.owner_path = suffix.to_vec();
                                Some(loan)
                            })
                            .collect();
                        return Ok(Some(limit_access(loans, &access_limits)));
                    }
                    ExpressionNode::Member(member) => {
                        let loans = available!(relative_result_loans(
                            program,
                            state_symbol,
                            statement_index,
                            machine_symbol,
                            local_data,
                            member.receiver,
                            loan_trackers,
                            sources,
                        )?);
                        let field = effective_member_symbol(program, member.receiver, member);
                        let selects_field = |candidate| {
                            owner_segment_matches_place_segment(
                                program,
                                &BorrowOwnerSegment::Field(candidate),
                                &PlaceSegment::Field { symbol: field },
                            ) == Some(true)
                        };
                        let loans = loans
                            .into_iter()
                            .filter_map(|mut loan| {
                                let suffix = match loan.owner_path.as_slice() {
                                    [] => &[][..],
                                    [BorrowOwnerSegment::Field(candidate), suffix @ ..]
                                        if selects_field(*candidate) =>
                                    {
                                        suffix
                                    }
                                    [
                                        BorrowOwnerSegment::Case(variant),
                                        BorrowOwnerSegment::Field(candidate),
                                        suffix @ ..,
                                    ] if selects_field(*candidate)
                                        && member.case_variant.as_ref().is_none_or(
                                            |selected| {
                                                program.symbols.name(*variant) == selected.as_str()
                                            },
                                        ) =>
                                    {
                                        suffix
                                    }
                                    _ => return None,
                                };
                                loan.owner_path = suffix.to_vec();
                                Some(loan)
                            })
                            .collect();
                        return Ok(Some(limit_access(loans, &access_limits)));
                    }
                    ExpressionNode::Borrow(borrow) => {
                        let target_type = available!(expression_type_reference_in_state(
                            program,
                            state_symbol,
                            statement_index,
                            borrow.target,
                        ));
                        if reference_shell(program, target_type).is_none() {
                            return Ok(None);
                        }
                        access_limits.push(BorrowAccessKind::from(borrow.access));
                        expression = borrow.target;
                        continue;
                    }
                    _ => return Ok(None),
                };
                let contract = available!(call_view_return_contract(program, state_symbol, call));
                let completed = TableLocalData {
                    initial_value: expression,
                    type_reference: contract.return_type_reference(program),
                    ..local_data.clone()
                };
                if let Some(loans) =
                    super::invocation_results::retain(program, &completed, loan_trackers)?
                {
                    return Ok(Some(limit_access(loans, &access_limits)));
                }
                if matches!(contract.source(program), ViewReturnSource::NotApplicable) {
                    return Ok(super::invocation_results::form(
                        program,
                        state_symbol,
                        machine_symbol,
                        local_data,
                        expression,
                        validation::call_target_symbol(program, call),
                        contract.return_type_reference(program),
                    )
                    .map(|loans| limit_access(loans, &access_limits)));
                }
                let mut fields = available!(contract.reference_sources(program));
                let target = validation::call_target_symbol(program, call);
                let opaque_result =
                    !validation::call_target_has_checked_body_contract(program, target);
                if opaque_result {
                    for field in &mut fields {
                        field.lifetime_only = true;
                    }
                }
                let mut loans = result_source_loans(
                    program,
                    state_symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    call.target_symbol,
                    program.expression_table.expression_handles(call.arguments),
                    fields,
                    &contract.substitutions,
                    &[],
                    sources,
                )?;
                for loan in &mut loans {
                    loan.formation_expression = expression;
                    loan.formation_owner_path = Some(loan.owner_path.clone());
                    if let Some(retained) = &mut loan.retained_operand
                        && !retained.formation_expression.is_valid()
                    {
                        retained.formation_expression = expression;
                    }
                    let projected = loan.formation == super::BorrowFormation::ReturnedProjection;
                    if !projected {
                        loan.formation = super::BorrowFormation::ReturnedDescriptor;
                    }
                    if !projected
                        && loan.place.root.is_valid()
                        && loan.lineage != BorrowLoanLineage::LifetimeDependency
                    {
                        loan.lineage = if loan.source_owner_symbol.is_valid() {
                            BorrowLoanLineage::UnretainedDerived
                        } else {
                            BorrowLoanLineage::DirectRoot
                        };
                    }
                }
                if opaque_result {
                    loans.extend(
                        super::invocation_results::form(
                            program,
                            state_symbol,
                            machine_symbol,
                            local_data,
                            expression,
                            target,
                            contract.return_type_reference(program),
                        )
                        .ok_or("completed result reference permissions are unavailable")?,
                    );
                }
                return Ok(Some(limit_access(loans, &access_limits)));
            }
        }

        fn limit_access(
            mut loans: Vec<StatementBorrowLoan>,
            limits: &[BorrowAccessKind],
        ) -> Vec<StatementBorrowLoan> {
            for access in limits.iter().rev() {
                attenuate(&mut loans, access);
            }
            loans
        }

        pub(super) fn attenuate(loans: &mut [StatementBorrowLoan], access: &BorrowAccessKind) {
            for loan in loans {
                if loan.kind == BorrowAccessKind::Mutable
                    && loan.lineage != BorrowLoanLineage::LifetimeDependency
                {
                    loan.kind = access.clone();
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn argument_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &TableLocalData,
            argument: ExpressionHandle,
            field: &ViewReturnFieldSource,
            result_owner_path: &[BorrowOwnerSegment],
            loan_trackers: &[StateLoanTracker],
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            carried_arguments: &mut Vec<(usize, Vec<StatementBorrowLoan>)>,
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let source_segments = place_segments(&field.source_path);
            let cached_index = if let Some(position) = carried_arguments
                .iter()
                .position(|(parameter, _)| *parameter == field.parameter_index)
            {
                position
            } else {
                let argument_owner = TableLocalData {
                    type_reference: CallViewReturnContract::carrier_reference(
                        program,
                        field.source_type,
                        substitutions,
                    ),
                    initial_value: argument,
                    ..local_data.clone()
                };
                let mut loans = Vec::new();
                for initializer in
                    borrowed_initializers(program, field.source_type, argument, substitutions, &[])
                {
                    loans.extend(borrowed_initializer_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        &argument_owner,
                        initializer,
                        loan_trackers,
                        sources,
                    )?);
                }
                let position = carried_arguments.len();
                carried_arguments.push((field.parameter_index, loans));
                position
            };
            let carried = carried_arguments[cached_index]
                .1
                .iter()
                .filter(|loan| {
                    loan.owner_path.len() <= source_segments.len()
                        && loan.owner_path.iter().zip(&source_segments).all(
                            |(owner, source)| match (owner, source) {
                                (
                                    BorrowOwnerSegment::Field(owner),
                                    PlaceSegment::Field { symbol },
                                ) => program
                                    .any_field_owner(*owner)
                                    .zip(program.any_field_owner(*symbol))
                                    .is_some_and(|(owner, source)| {
                                        owner.1.path_identity() == source.1.path_identity()
                                    }),
                                (
                                    BorrowOwnerSegment::Case(owner),
                                    PlaceSegment::Case { variant },
                                ) => program
                                    .variant_owner(*owner)
                                    .zip(program.variant_owner(*variant))
                                    .is_some_and(|(owner, source)| {
                                        owner.1.path_identity() == source.1.path_identity()
                                    }),
                                _ => owner_path_matches(
                                    program,
                                    std::slice::from_ref(owner),
                                    std::slice::from_ref(source),
                                ),
                            },
                        )
                })
                .cloned()
                .collect::<Vec<_>>();

            Ok(carried
                .into_iter()
                .map(|mut loan| {
                    loan.owner_path = result_owner_path.to_vec();
                    if !field.lifetime_only
                        && loan.lineage != BorrowLoanLineage::LifetimeDependency
                        && matches!(
                            field.kind,
                            BorrowAccessKind::Read | BorrowAccessKind::WriteOnly
                        )
                    {
                        loan.kind = field.kind.clone();
                    }
                    loan.lineage = loan.lineage.derived();
                    loan
                })
                .collect())
        }
    }
    pub(crate) use returned_carriers::{
        CompletedReturnSource, call_reference_lifetime_sources, call_reference_sources,
        completed_call_reference_sources, returned_body_source_mismatch,
    };
    mod replacement {
        /*
            Capture a store's descriptor subtree before evaluating its value. Both
            current-loan retirement and final receiving-loan formation use this same
            destination; RHS effects cannot redirect it or survive its replacement.
        */
        use super::super::tracker::{BorrowOwnerSegment, StateLoanTracker};

        pub(in crate::borrow) struct CarrierReplacement {
            destination: checked_trees::CapturedPlace,
            preserves_root_reference: bool,
        }

        impl CarrierReplacement {
            pub(in crate::borrow) fn replaces(
                &self,
                program: &dyn validation::CheckingInput,
                owner: facts::PlaceRoot,
                path: &[BorrowOwnerSegment],
            ) -> bool {
                owner == self.destination.root
                    && !(self.preserves_root_reference
                        && path.len() == self.destination.segments.len())
                    && super::place_path_covers_owner(program, &self.destination.segments, path)
            }
        }

        pub(in crate::borrow) fn capture_replacement(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            statement: &typed_trees::statement::StatementNode,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
            current: &[StateLoanTracker],
        ) -> Option<CarrierReplacement> {
            let assignment = statement.as_assignment()?;
            let writes_referent = if validation::declared_place_type_raw(
                program,
                machine,
                Some(state),
                assignment.target,
            )
            .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
            {
                !call_frames?.assignment_replaces_reference_binding(machine, statement)?
            } else {
                false
            };
            let target = super::borrow_access_place(
                program,
                state.symbol,
                statement_index,
                assignment.target,
                machine.symbol,
            )?;
            let destinations = super::store_destination_places(
                program,
                state,
                target.clone(),
                writes_referent,
                current.iter().map(|loan| {
                    (
                        loan.storage_owner,
                        loan.owner_path.as_slice(),
                        loan.place.incoming_reference,
                        loan.place.incoming_owner_path.as_slice(),
                        loan.place.root,
                        loan.place.segments.as_slice(),
                    )
                }),
            );
            let [destination] = destinations.as_slice() else {
                return None;
            };
            Some(CarrierReplacement {
                destination: destination.clone(),
                preserves_root_reference: writes_referent && destination == &target,
            })
        }
    }
    mod stored_calls {
        /*
            A call can install a reference in caller-owned storage independently of its
            result value. Its body-derived slot relation supplies the same exact
            owner/backing pairs as an assignment; type shape alone supplies no loan.
            The persistent roster follows lexical fallthrough. Departing transition
            operands retain their own normal-call custody and outgoing-edge records.
        */
        use super::{StatementBorrowLoan, owner_path_from_place_segments};

        /*
            Observe the call-completed slots at one operand's completion. This scratch roster
            retains incoming descriptor identities; it publishes no local loan node.
        */
        pub(super) fn roster_at_expression(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
            frames: &validation::CallFrameResolver<'_>,
            incoming: &[super::StateLoanTracker],
        ) -> Result<Vec<super::StateLoanTracker>, validation::StoredReferenceRelationRefusal>
        {
            let effects = match frames.stored_reference_effects_at_expression(
                machine,
                state,
                statement_index,
                expression,
            ) {
                Ok(effects) => effects,
                Err(validation::StoredReferenceRelationRefusal::TemplateDependent)
                    if independent_owned_formation(
                        program,
                        machine,
                        state,
                        statement_index,
                        expression,
                    ) =>
                {
                    return Ok(Vec::new());
                }
                Err(reason) => return Err(reason),
            };
            let produced = from_effects(program, effects.clone(), incoming);
            let mut current = incoming
                .iter()
                .filter(|loan| {
                    !effects.iter().any(|effect| {
                        !effect.may_select
                            && loan.storage_owner
                                == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                            && super::place_path_covers_owner(
                                program,
                                &effect.destination.local_segments,
                                &loan.owner_path,
                            )
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            current.extend(produced.into_iter().map(|loan| super::StateLoanTracker {
                representation_address: loan.representation_address,
                handle: Default::default(),
                owner_symbol: loan.owner_symbol,
                storage_owner: loan.storage_owner,
                owner_name: loan.owner_name,
                kind: loan.kind,
                lineage: loan.lineage,
                formation: loan.formation,
                formation_expression: loan.formation_expression,
                formation_owner_path: loan.formation_owner_path,
                permission_sources: Vec::new(),
                owner_path: loan.owner_path,
                place: loan.place,
                retained_operand: loan.retained_operand,
            }));
            Ok(current)
        }

        /*
            A declared owned address can be borrowed even while unrelated descriptor
            transfers remain pending. Use the ordinary geometry and type projection;
            a loaded pointer, incoming referent or unavailable type still needs its
            current source roster. Case, index and access admission remain with their
            ordinary formation owners; this establishes neither contents nor permission.
        */
        fn independent_owned_formation(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            let Some(reference) =
                validation::expression_result_type_reference(program, machine, state, expression)
            else {
                return false;
            };
            if validation::reference_shell(program, reference).is_none()
                && validation::slice_view_shell(program, reference, &[]).is_none()
            {
                return false;
            }
            let Some(geometry) = super::call_views::argument_borrow_loan_geometry(
                program,
                state.symbol,
                statement_index,
                machine.symbol,
                expression,
            ) else {
                return false;
            };
            let owned = |place: &super::accesses::BorrowAccessPlace| {
                if place.incoming_reference.is_valid() || !place.incoming_owner_path.is_empty() {
                    return false;
                }
                if super::accesses::is_image_storage_root(program, place.root) {
                    return true;
                }
                if !matches!(
                    place.root,
                    facts::PlaceRoot::Symbol(_) | facts::PlaceRoot::Expression(_)
                ) {
                    return false;
                }
                let owner_path = super::owner_path_from_place_segments(program, &place.segments);
                validation::canonical_owned_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: super::owner_place_segments(&owner_path),
                    },
                )
                .is_some()
            };
            owned(&geometry.place) && geometry.representation_address.as_ref().is_none_or(owned)
        }

        pub(super) fn loans(
            program: &dyn ::validation::CheckingInput,
            machine_symbol: symbols::SymbolHandle,
            statement: &checked_trees::statement::StatementNode,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
            incoming: &[super::StateLoanTracker],
            completed: Option<&[super::StateLoanTracker]>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let Some(machine) = program.machine(machine_symbol) else {
                return Ok(Vec::new());
            };
            let Some(frames) = call_frames else {
                return Ok(Vec::new());
            };
            let effects = match frames.stored_reference_fallthrough_effects(machine, statement) {
                Ok(effects) => effects,
                Err(validation::StoredReferenceRelationRefusal::TemplateDependent) => {
                    return Ok(Vec::new());
                }
                Err(validation::StoredReferenceRelationRefusal::Unavailable(reason)) => {
                    return Err(reason);
                }
            };
            if effects.is_empty() {
                return Ok(Vec::new());
            }
            let completed =
                completed.ok_or("stored call has no complete statement-successor roster")?;
            let mut output = Vec::new();
            for source in completed {
                let selected = effects.iter().any(|effect| {
                    let destination = &effect.destination.local_segments;
                    // A fixed store installs exactly the loans its destination covers.
                    // A store at a runtime-selected element (`slots[position]`) installs
                    // a loan whose owner path holds `DynamicIndex` there. Coverage, which
                    // decides retirement, never holds for that segment, so the new loan
                    // is selected by the destination's shape: `DynamicIndex` stands for
                    // the selected element, whose index expression belongs to the callee
                    // frame. Loans the statement already carried in stay out either way.
                    let installed = if effect.may_select {
                        destination.len() <= source.owner_path.len()
                            && source.owner_path.iter().zip(destination).all(
                                |(owner, place)| match (owner, place) {
                                    (
                                        super::BorrowOwnerSegment::DynamicIndex,
                                        facts::PlaceSegment::Index { .. }
                                        | facts::PlaceSegment::FixedIndex { .. },
                                    ) => true,
                                    _ => {
                                        super::owner_segment_matches_place_segment(
                                            program, owner, place,
                                        ) == Some(true)
                                    }
                                },
                            )
                    } else {
                        super::place_path_covers_owner(program, destination, &source.owner_path)
                    };
                    source.storage_owner
                        == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                        && installed
                        && (!effect.may_select
                            || !incoming.iter().any(|entry| entry.handle == source.handle))
                });
                if !selected {
                    continue;
                }
                output.push(StatementBorrowLoan {
                    formation_owner_path: None,
                    incoming_origin: None,
                    formation_expression: Default::default(),
                    representation_address: source.representation_address.clone(),
                    owner_symbol: source.owner_symbol,
                    storage_owner: source.storage_owner,
                    owner_name: source.owner_name.clone(),
                    owner_path: source.owner_path.clone(),
                    retained_operand: source.retained_operand,
                    place: source.place.clone(),
                    source_owner_symbol: source.owner_symbol,
                    supplying_loans: std::iter::once(source.handle)
                        .filter(|handle| handle.is_valid())
                        .collect(),
                    lineage: source.lineage.derived(),
                    kind: source.kind.clone(),
                    formation: super::BorrowFormation::ReturnedDescriptor,
                });
            }
            Ok(output)
        }

        pub(in crate::borrow) fn from_effects(
            program: &dyn ::validation::CheckingInput,
            effects: Vec<validation::StoredReferenceEffect>,
            loan_trackers: &[super::StateLoanTracker],
        ) -> Vec<StatementBorrowLoan> {
            let mut loans = Vec::new();
            for effect in effects {
                let owner = effect.destination.local_symbol;
                let owner_path =
                    owner_path_from_place_segments(program, &effect.destination.local_segments);
                if effect.retains_incoming && !effect.may_select {
                    for source in loan_trackers.iter().filter(|source| {
                        source.storage_owner == facts::PlaceRoot::Symbol(owner)
                            && super::place_path_covers_owner(
                                program,
                                &effect.destination.local_segments,
                                &source.owner_path,
                            )
                    }) {
                        loans.push(StatementBorrowLoan {
                            formation_owner_path: None,
                            incoming_origin: None,
                            formation_expression: Default::default(),
                            representation_address: None,
                            owner_symbol: owner,
                            storage_owner: facts::PlaceRoot::Symbol(owner),
                            owner_name: program.symbols.name(owner).into(),
                            owner_path: source.owner_path.clone(),
                            retained_operand: source.retained_operand,
                            place: source.place.clone(),
                            source_owner_symbol: source.owner_symbol,
                            supplying_loans: std::iter::once(source.handle)
                                .filter(|handle| handle.is_valid())
                                .collect(),
                            lineage: source.lineage.derived(),
                            kind: source.kind.clone(),
                            formation: super::BorrowFormation::ReturnedDescriptor,
                        });
                    }
                }
                for source in &effect.sources {
                    let Some(source_access) = source.lifetime_access.or(effect.reference_access())
                    else {
                        continue;
                    };
                    let backing = &source.backing;
                    if let Some(identity) = backing.invocation_output {
                        let access = source_access;
                        let supplied = (identity.is_valid()
                            && !backing.is_image_storage
                            && backing.recasts.is_empty()
                            && backing.representation.is_none())
                        .then(|| validation::find_state(program, identity.state_symbol))
                        .flatten()
                        .and_then(|(machine, state)| {
                            let parameter =
                                validation::invocation_output_parameter(program, identity)?;
                            let reference = validation::project_type_reference_from_segments(
                                program,
                                parameter.type_reference,
                                &backing.segments,
                            )?;
                            let shell = validation::reference_shell(program, reference)?;
                            (shell.access == access).then_some((
                                machine,
                                state,
                                parameter.type_reference,
                            ))
                        });
                        if let Some((machine, state, carrier_type)) = supplied {
                            loans.push(StatementBorrowLoan {
                                formation_owner_path: None,
                                incoming_origin: Some(super::PendingIncomingReference {
                                    identity: checked_trees::IncomingReferenceIdentity {
                                        machine_symbol: machine.symbol,
                                        state_symbol: state.symbol,
                                        origin: checked_trees::IncomingReferenceOrigin::Output {
                                            identity,
                                        },
                                        carrier_type,
                                        owner_path: Default::default(),
                                    },
                                    owner_path: owner_path_from_place_segments(
                                        program,
                                        &backing.segments,
                                    ),
                                }),
                                formation_expression: match validation::find_call_site_in_state(
                                    program,
                                    machine,
                                    state,
                                    identity.statement_index,
                                    identity.call_ordinal,
                                ) {
                                    Some(validation::CallSite::Expression {
                                        expression, ..
                                    }) => expression,
                                    _ => Default::default(),
                                },
                                representation_address: None,
                                owner_symbol: owner,
                                storage_owner: facts::PlaceRoot::Symbol(owner),
                                owner_name: program.symbols.name(owner).into(),
                                owner_path: owner_path.clone(),
                                retained_operand: None,
                                place: checked_trees::CapturedPlace {
                                    incoming_reference: Default::default(),
                                    incoming_owner_path: Default::default(),
                                    root: facts::PlaceRoot::InvocationOutput(identity),
                                    segments: Vec::new(),
                                },
                                source_owner_symbol: Default::default(),
                                supplying_loans: Vec::new(),
                                lineage: if source.lifetime_access.is_some() {
                                    checked_trees::BorrowLoanLineage::LifetimeDependency
                                } else {
                                    checked_trees::BorrowLoanLineage::SuppliedOutput
                                },
                                kind: checked_trees::BorrowAccessKind::from(access),
                                formation: super::BorrowFormation::ReturnedDescriptor,
                            });
                            continue;
                        }
                    }
                    let captured = super::call_views::rebase_borrow_places_through_local_loans(
                        program,
                        checked_trees::CapturedPlace {
                            incoming_reference: Default::default(),
                            incoming_owner_path: Default::default(),
                            root: facts::PlaceRoot::Symbol(source.carrier_symbol),
                            segments: source.carrier_segments.clone(),
                        },
                        loan_trackers,
                    );
                    let relevant = |candidate: &&super::RebasedBorrowPlace| {
                        source.lifetime_access.is_some() || candidate.lifetime_access.is_none()
                    };
                    let complete_carrier = captured.iter().find(relevant).is_some()
                        && captured.iter().filter(relevant).all(|candidate| {
                            effect.sources.iter().any(|alternative| {
                                alternative.carrier_symbol == source.carrier_symbol
                                    && alternative.carrier_segments == source.carrier_segments
                                    && alternative.lifetime_access.is_some()
                                        == source.lifetime_access.is_some()
                                    && transferred_place(
                                        &alternative.backing,
                                        alternative.carrier_backing.as_ref(),
                                        alternative.lifetime_access.is_some(),
                                        candidate,
                                    )
                                    .is_some()
                            })
                        });
                    let matching: Vec<_> = captured
                        .iter()
                        .filter_map(|candidate| {
                            complete_carrier
                                .then(|| {
                                    transferred_place(
                                        backing,
                                        source.carrier_backing.as_ref(),
                                        source.lifetime_access.is_some(),
                                        candidate,
                                    )
                                })
                                .flatten()
                                .map(|place| Some((candidate, place)))
                        })
                        .collect();
                    let matching = if matching.is_empty() {
                        vec![None]
                    } else {
                        matching
                    };
                    for supplying in matching {
                        let supplier = supplying.as_ref().map(|(source, _)| *source);
                        loans.push(StatementBorrowLoan {
                            formation_owner_path: None,
                            incoming_origin: None,
                            formation_expression: Default::default(),
                            representation_address: None,
                            owner_symbol: owner,
                            storage_owner: facts::PlaceRoot::Symbol(owner),
                            owner_name: program.symbols.name(owner).into(),
                            owner_path: owner_path.clone(),
                            retained_operand: supplier.and_then(|source| source.retained_operand),
                            place: supplying.as_ref().map_or_else(
                                || checked_trees::CapturedPlace {
                                    incoming_reference: Default::default(),
                                    incoming_owner_path: Default::default(),
                                    root: facts::PlaceRoot::Symbol(backing.root),
                                    segments: backing.segments.clone(),
                                },
                                |(_, place)| place.clone(),
                            ),
                            source_owner_symbol: supplier
                                .map_or(Default::default(), |source| source.source_owner_symbol),
                            supplying_loans: supplier
                                .map(|source| source.parent_loan)
                                .filter(|loan| loan.is_valid())
                                .into_iter()
                                .collect(),
                            lineage: if source.lifetime_access.is_some()
                                || supplier.is_some_and(|source| source.lifetime_access.is_some())
                            {
                                checked_trees::BorrowLoanLineage::LifetimeDependency
                            } else {
                                checked_trees::BorrowLoanLineage::UnretainedDerived
                            },
                            kind: supplier
                                .and_then(|source| source.lifetime_access.clone())
                                .unwrap_or_else(|| {
                                    checked_trees::BorrowAccessKind::from(source_access)
                                }),
                            formation: super::BorrowFormation::ReturnedDescriptor,
                        });
                    }
                }
            }
            loans
        }

        /*
            Rejoin the exact observed descriptor to its produced projection. The
            completed checking query has already crossed every real pointer load;
            `carrier_backing` names that final load's unprojected referent. Geometry
            alone, a common root, or another loan cannot establish this association.
        */
        fn transferred_place(
            backing: &validation::LocalWriteSource,
            carrier_backing: Option<&validation::LocalWriteSource>,
            lifetime_only: bool,
            candidate: &super::RebasedBorrowPlace,
        ) -> Option<checked_trees::CapturedPlace> {
            if candidate.lifetime_access.is_some() && !lifetime_only {
                return None;
            }
            if (candidate.place.root == facts::PlaceRoot::Symbol(backing.root)
                && candidate.place.segments == backing.segments)
                || (candidate.place.incoming_reference.is_valid()
                    && candidate.source_place.root == facts::PlaceRoot::Symbol(backing.root)
                    && candidate.source_place.segments == backing.segments)
            {
                return Some(candidate.place.clone());
            }
            let base = carrier_backing?;
            if !base.root.is_valid()
                || base.invocation_output.is_some()
                || backing.invocation_output.is_some()
                || base.is_image_storage
                || backing.is_image_storage
                || !base.recasts.is_empty()
                || !backing.recasts.is_empty()
                || base.representation.is_some()
                || backing.representation.is_some()
                || backing.root != base.root
                || !backing.segments.starts_with(&base.segments)
            {
                return None;
            }
            let projection = &backing.segments[base.segments.len()..];
            if candidate.place.incoming_reference.is_valid() {
                let home = &candidate.source_place;
                if home.root != facts::PlaceRoot::Symbol(base.root)
                    || candidate.place.root != home.root
                    || !base.segments.starts_with(&home.segments)
                    || base.segments[home.segments.len()..] != candidate.place.segments
                {
                    return None;
                }
                let mut place = candidate.place.clone();
                place.segments.extend_from_slice(projection);
                Some(place)
            } else {
                (candidate.place.root == facts::PlaceRoot::Symbol(base.root)
                    && candidate.place.segments == base.segments)
                    .then(|| checked_trees::CapturedPlace {
                        incoming_reference: Default::default(),
                        incoming_owner_path: Default::default(),
                        root: facts::PlaceRoot::Symbol(backing.root),
                        segments: backing.segments.clone(),
                    })
            }
        }
    }
    pub(in crate::borrow) use replacement::{CarrierReplacement, capture_replacement};
    pub(in crate::borrow) use stored_calls::from_effects as stored_reference_effect_loans;
    pub(crate) mod types {
        pub(super) fn is_reference_type(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            match program.type_reference_table.type_reference(type_reference) {
                typed_trees::types::TypeReferenceNode::Reference { .. } => true,
                typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    is_reference_type(program, *base_type)
                }
                typed_trees::types::TypeReferenceNode::ConstExpression(_)
                | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
                | typed_trees::types::TypeReferenceNode::Generic { .. }
                | typed_trees::types::TypeReferenceNode::Named { .. }
                | typed_trees::types::TypeReferenceNode::Slice { .. }
                | typed_trees::types::TypeReferenceNode::Unit => false,
            }
        }

        pub(crate) fn reference_borrow_access_kind(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<checked_trees::BorrowAccessKind> {
            match program.type_reference_table.type_reference(type_reference) {
                typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                    Some(checked_trees::BorrowAccessKind::from(*access))
                }
                typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    reference_borrow_access_kind(program, *base_type)
                }
                typed_trees::types::TypeReferenceNode::ConstExpression(_)
                | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
                | typed_trees::types::TypeReferenceNode::Generic { .. }
                | typed_trees::types::TypeReferenceNode::Named { .. }
                | typed_trees::types::TypeReferenceNode::Slice { .. }
                | typed_trees::types::TypeReferenceNode::Unit => None,
            }
        }
    }

    use types::{is_reference_type, reference_borrow_access_kind};

    mod descriptor_loads {
        /*
            Prepare resources for pointer values outside the owned descriptor inventory.

            Preparation grants nothing. Flow must bind the row to an actual completed
            descriptor read, retaining its source access and lifetime profile before
            activating it. A stored pointer is independent of the receiver's address:
            its namespace cannot be obtained by appending a field to that address.
        */
        use super::super::tracker::StateLoanTracker;
        use super::{PendingIncomingReference, StatementBorrowLoan};
        use checked_trees::{
            BorrowLoanLineage, CapturedPlace, IncomingReferenceIdentity, IncomingReferenceOrigin,
        };
        use facts::PlaceRoot;
        use typed_trees::{
            expression::ExpressionNode, machine::Machine, state::State, statement::StatementNode,
        };

        pub(in crate::borrow) fn prepare(
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            statement: &StatementNode,
            current: &[StateLoanTracker],
            pending: &mut Vec<StatementBorrowLoan>,
        ) {
            validation::visit_statement_value_occurrences(
                program,
                machine.symbol,
                state.symbol,
                statement_index,
                statement,
                &mut |expression, _, _| {
                    if !matches!(
                        program.expression_table.expression(expression),
                        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                    ) || pending
                        .iter()
                        .any(|loan| loan.storage_owner == PlaceRoot::Expression(expression))
                    {
                        return;
                    }
                    let Some(carrier_type) = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        expression,
                    ) else {
                        return;
                    };
                    let Some(kind) =
                        super::types::reference_borrow_access_kind(program, carrier_type)
                    else {
                        return;
                    };
                    let Some(source) = crate::published::places::place_at(
                        program,
                        state.symbol,
                        statement_index,
                        expression,
                    ) else {
                        return;
                    };
                    if current.iter().any(|loan| {
                        loan.storage_owner == source.root
                            && loan.owner_path.len() == source.segments.len()
                            && super::owner_path_matches(
                                program,
                                &loan.owner_path,
                                &source.segments,
                            )
                    }) {
                        return;
                    }
                    let rebased = super::call_views::rebase_borrow_places_through_local_loans(
                        program,
                        CapturedPlace {
                            root: source.root,
                            segments: source.segments.clone(),
                            ..Default::default()
                        },
                        current,
                    );
                    let cover = |source: &super::RebasedBorrowPlace| {
                        current.iter().find(|supplier| {
                            supplier.storage_owner == source.source_place.root
                                && super::owner_path_matches(
                                    program,
                                    &supplier.owner_path,
                                    &source.source_place.segments,
                                )
                                && (supplier.handle == source.parent_loan
                                    || !supplier.handle.is_valid())
                        })
                    };
                    if !rebased.is_empty()
                        && rebased.iter().all(|source| cover(source).is_some())
                        && rebased.iter().all(|source| {
                            cover(source).is_some_and(|loan| {
                                loan.owner_path.len() == source.source_place.segments.len()
                            })
                        })
                    {
                        return;
                    }
                    pending.push(StatementBorrowLoan {
                        incoming_origin: Some(PendingIncomingReference {
                            identity: IncomingReferenceIdentity {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                origin: IncomingReferenceOrigin::DescriptorLoad { expression },
                                carrier_type,
                                owner_path: Default::default(),
                            },
                            owner_path: Vec::new(),
                        }),
                        formation_expression: expression,
                        formation_owner_path: Some(Vec::new()),
                        owner_symbol: state.symbol,
                        storage_owner: PlaceRoot::Expression(expression),
                        owner_name: state.name.clone(),
                        owner_path: Vec::new(),
                        place: CapturedPlace {
                            root: PlaceRoot::Expression(expression),
                            ..Default::default()
                        },
                        representation_address: None,
                        retained_operand: None,
                        source_owner_symbol: Default::default(),
                        supplying_loans: Vec::new(),
                        lineage: BorrowLoanLineage::DirectRoot,
                        kind,
                        formation: super::BorrowFormation::Ordinary,
                    });
                },
            );
        }
    }
    pub(super) use descriptor_loads::prepare as prepare_descriptor_loads;

    mod invocation_results {
        /*
            A completed call supplies permission to use each returned reference.
            Its input lifetimes are retained separately; these rows do not claim that
            the pointer selects an input's storage. Checked bodies still owe return
            checking, and opaque providers still owe admission of their safe contract.
            Bare references and reference fields share the same invocation formation.
        */
        use super::{PendingIncomingReference, StatementBorrowLoan};
        use checked_trees::expression::ExpressionHandle;
        use checked_trees::statement::TableLocalData;
        use checked_trees::{
            BorrowLoanLineage, CapturedPlace, IncomingReferenceIdentity, IncomingReferenceOrigin,
        };
        use facts::PlaceRoot;
        use symbols::SymbolHandle;
        use typed_trees::types::TypeReferenceHandle;
        use validation::reference_result_custody::loan_place_segments;

        pub(super) fn form(
            program: &dyn validation::CheckingInput,
            state_symbol: SymbolHandle,
            machine_symbol: SymbolHandle,
            local: &TableLocalData,
            expression: ExpressionHandle,
            target: SymbolHandle,
            result_type: TypeReferenceHandle,
        ) -> Option<Vec<StatementBorrowLoan>> {
            validation::borrow_contracts::carried_reference_paths(program, result_type)?
                .into_iter()
                .map(|(owner_path, access)| {
                    let path =
                        validation::borrow_contracts::structural_owner_path(program, &owner_path)?;
                    let segments = loan_place_segments(program, result_type, &path)?;
                    let carrier_type = validation::project_type_reference_from_segments(
                        program,
                        result_type,
                        &segments,
                    )?;
                    Some(StatementBorrowLoan {
                        formation_owner_path: Some(owner_path.clone()),
                        incoming_origin: Some(PendingIncomingReference {
                            identity: IncomingReferenceIdentity {
                                machine_symbol,
                                state_symbol,
                                origin: IncomingReferenceOrigin::Invocation { expression, target },
                                carrier_type,
                                owner_path: Default::default(),
                            },
                            owner_path: owner_path.clone(),
                        }),
                        formation_expression: expression,
                        representation_address: None,
                        owner_symbol: local.symbol,
                        storage_owner: PlaceRoot::Symbol(local.symbol),
                        owner_name: local.name.clone(),
                        owner_path,
                        place: CapturedPlace {
                            incoming_reference: Default::default(),
                            incoming_owner_path: Default::default(),
                            root: PlaceRoot::Expression(expression),
                            segments: Vec::new(),
                        },
                        retained_operand: None,
                        source_owner_symbol: SymbolHandle::invalid(),
                        supplying_loans: Vec::new(),
                        lineage: BorrowLoanLineage::DirectRoot,
                        kind: access,
                        formation: super::BorrowFormation::ReturnedDescriptor,
                    })
                })
                .collect()
        }

        /*
            Publish one opaque call's returned value at its actual normal completion.
            Binding and direct forwarding retain every reference from that result.
        */
        #[allow(clippy::too_many_arguments)]
        pub(in crate::borrow) fn complete(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: ExpressionHandle,
            statement_entry: &[super::super::tracker::StateLoanTracker],
            completed_values: &[super::super::tracker::StateLoanTracker],
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return Ok(Vec::new());
            };
            if validation::call_returns_unit(program, machine, expression)
                || validation::call_target_has_checked_body_contract(
                    program,
                    validation::call_target_symbol(program, call),
                )
            {
                return Ok(Vec::new());
            }
            let result =
                validation::expression_result_type_reference(program, machine, state, expression)
                    .ok_or("completed call result has no checked value type")?;
            if !super::types::is_reference_type(program, result)
                && !crate::borrow::view_link::is_borrow_carrying_data(program, result)
            {
                return Ok(Vec::new());
            }
            if validation::proof_only_classification(program)
                .proof_only_mention(program.typed_program(), result)
                .is_some()
            {
                return Ok(Vec::new());
            }
            let captured_types =
                validation::borrow_contracts::captured_type_parameters(program, state.symbol);
            if matches!(
                validation::borrow_contracts::declaration_lifetime_frontier(
                    program,
                    result,
                    &captured_types
                ),
                validation::borrow_contracts::DeclarationLifetimeFrontier::TemplateDependent {
                    has_borrows: false
                }
            ) {
                return Ok(Vec::new());
            }
            let value = checked_trees::statement::TableLocalData {
                role: language_core::LocalBindingRole::Value,
                symbol: state.symbol,
                name: state.name.clone(),
                type_reference: result,
                initial_value: expression,
                is_mutable: false,
                type_is_inferred: false,
                is_operand_capture: false,
                relevance: language_core::BindingRelevance::Relevant,
                reflected_query_origin: None,
            };
            let mut incoming = statement_entry
                .iter()
                .filter(|source| !matches!(source.storage_owner, PlaceRoot::Expression(_)))
                .cloned()
                .collect::<Vec<_>>();
            incoming.extend(
                completed_values
                    .iter()
                    .filter(|source| matches!(source.storage_owner, PlaceRoot::Expression(_)))
                    .cloned(),
            );
            let sources = super::BorrowFormationSources {
                statement_entry: &incoming,
                call_frames: frames,
            };
            let mut loans = super::returned_carriers::relative_result_loans(
                program,
                state.symbol,
                statement_index,
                machine.symbol,
                &value,
                expression,
                &incoming,
                &sources,
            )?
            .ok_or("completed call result has no complete reference set")?;
            for loan in &mut loans {
                loan.storage_owner = PlaceRoot::Expression(expression);
                loan.formation_expression = expression;
            }
            Ok(loans)
        }

        /*
            Saving an evaluated value retains every reference and required input lifetime.
            It adds no permission, and input lifetimes keep their original access.
        */
        pub(super) fn retain(
            program: &dyn validation::CheckingInput,
            local: &TableLocalData,
            current: &[super::super::tracker::StateLoanTracker],
        ) -> Result<Option<Vec<StatementBorrowLoan>>, &'static str> {
            if !super::types::is_reference_type(program, local.type_reference)
                && !crate::borrow::view_link::is_borrow_carrying_data(program, local.type_reference)
            {
                return Ok(None);
            }
            let cohort = current
                .iter()
                .filter(|source| source.storage_owner == PlaceRoot::Expression(local.initial_value))
                .collect::<Vec<_>>();
            if !cohort
                .iter()
                .any(|source| source.lineage != BorrowLoanLineage::LifetimeDependency)
            {
                return Ok(None);
            }
            cohort
                .into_iter()
                .map(|source| {
                    if !source.handle.is_valid() {
                        return Err("saved descriptor has no established permission occurrence");
                    }
                    let owner_path = if source.formation_expression == local.initial_value {
                        source
                            .formation_owner_path
                            .as_ref()
                            .unwrap_or(&source.owner_path)
                    } else {
                        &source.owner_path
                    };
                    let kind = if source.lineage == BorrowLoanLineage::LifetimeDependency {
                        source.kind.clone()
                    } else {
                        let path = super::owner_place_segments(owner_path);
                        let reference = validation::project_type_reference_from_segments(
                            program,
                            local.type_reference,
                            &path,
                        )
                        .ok_or("saved descriptor has no declared receiving leaf")?;
                        let kind = super::types::reference_borrow_access_kind(program, reference)
                            .ok_or("saved descriptor's receiving leaf is not a reference")?;
                        source
                            .kind
                            .direct_reborrow_effect(&kind)
                            .ok_or("saved descriptor exceeds its completed source access")?;
                        kind
                    };
                    Ok(StatementBorrowLoan {
                        incoming_origin: None,
                        formation_expression: local.initial_value,
                        formation_owner_path: Some(owner_path.clone()),
                        owner_symbol: local.symbol,
                        storage_owner: PlaceRoot::Symbol(local.symbol),
                        owner_name: local.name.clone(),
                        owner_path: owner_path.clone(),
                        place: source.place.clone(),
                        representation_address: source.representation_address.clone(),
                        retained_operand: source.retained_operand,
                        source_owner_symbol: source.owner_symbol,
                        supplying_loans: vec![source.handle],
                        lineage: if source.lineage == BorrowLoanLineage::LifetimeDependency {
                            BorrowLoanLineage::LifetimeDependency
                        } else {
                            BorrowLoanLineage::UnretainedDerived
                        },
                        kind,
                        formation: super::BorrowFormation::Ordinary,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        }
    }
    pub(in crate::borrow) use invocation_results::complete as complete_invocation_results;
    mod reference_borrows {
        /*
            reference borrows slice of loans.
        */
        use super::super::accesses::{self, borrow_access_place};
        use super::super::tracker::StateLoanTracker;
        use super::StatementBorrowLoan;
        use super::call_views::*;
        use super::types::{is_reference_type, reference_borrow_access_kind};
        use crate::borrow::view_link::is_borrow_carrying_data;
        use crate::flow::expressions;
        use checked_trees::expression::ExpressionHandle;
        use symbols::SymbolHandle;

        /*
            Whether a non-reference-typed `let` binds a view anyway: an immutable
            element-spelling subslice binding (`let v: u8 = arr[a..b]`) binds the
            view its initializer's own result type names, lending the range
            read-only exactly as its `&[u8]` spelling does.
        */
        pub(super) fn element_view_local_binding(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
        ) -> bool {
            if local_data.is_mutable {
                return false;
            }
            let Some(machine) = program.machine(machine_symbol) else {
                return false;
            };
            validation::expression_result_type_reference(
                program,
                machine,
                state,
                local_data.initial_value,
            )
            .and_then(|result| validation::slice_view_shell(program, result, &[]))
            .is_some()
        }

        pub(super) fn reference_local_borrow_loans(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let Some(local_access) =
                reference_borrow_access_kind(program, local_data.type_reference).or_else(|| {
                    element_view_local_binding(program, state, machine_symbol, local_data)
                        .then_some(checked_trees::BorrowAccessKind::Read)
                })
            else {
                return Ok(Vec::new());
            };
            if let Some(mut loans) = super::returned_carriers::result_loans(
                program,
                state.symbol,
                statement_index,
                machine_symbol,
                local_data,
                local_data.initial_value,
                &[],
                loan_trackers,
                sources,
            )? {
                super::returned_carriers::attenuate(&mut loans, &local_access);
                return Ok(loans);
            }
            if let Some(mut loans) = super::aggregate_loans::stored_reference_loans(
                program,
                state.symbol,
                statement_index,
                machine_symbol,
                local_data,
                local_data.initial_value,
                &[],
                loan_trackers,
            ) {
                super::returned_carriers::attenuate(&mut loans, &local_access);
                return Ok(loans);
            }
            if let checked_trees::expression::ExpressionNode::Match(dispatch) =
                expressions(program).expression(local_data.initial_value)
            {
                let mut loans = Vec::new();
                for arm in expressions(program).match_arms(dispatch.arms) {
                    let selected_value = checked_trees::statement::TableLocalData {
                        initial_value: arm.value,
                        ..local_data.clone()
                    };
                    loans.extend(reference_local_borrow_loans(
                        program,
                        state,
                        statement_index,
                        machine_symbol,
                        &selected_value,
                        loan_trackers,
                        sources,
                    )?);
                }
                return Ok(loans);
            }
            let formed_geometry =
                validation::reference_formation_source(program, local_data.initial_value).and_then(
                    |_| {
                        accesses::borrow_access_geometry(
                            program,
                            state.symbol,
                            statement_index,
                            local_data.initial_value,
                            machine_symbol,
                        )
                    },
                );
            let inferred_place = match expressions(program).expression(local_data.initial_value) {
                checked_trees::expression::ExpressionNode::String(_) => borrow_access_place(
                    program,
                    state.symbol,
                    statement_index,
                    local_data.initial_value,
                    machine_symbol,
                ),
                checked_trees::expression::ExpressionNode::Call(call) => {
                    helper_call_borrow_loan_place(
                        program,
                        state.symbol,
                        statement_index,
                        machine_symbol,
                        call,
                    )
                }
                checked_trees::expression::ExpressionNode::Indexed(_)
                | checked_trees::expression::ExpressionNode::Member(_)
                | checked_trees::expression::ExpressionNode::Name(_) => {
                    let windowed = matches!(
                        expressions(program).expression(local_data.initial_value),
                        checked_trees::expression::ExpressionNode::Indexed(indexed)
                            if matches!(
                                program.expression_table.expression(indexed.index),
                                checked_trees::expression::ExpressionNode::Range(_)
                            )
                    );
                    let value_carries_borrow = windowed
                        || validation::expression_type_reference_in_state(
                            program,
                            state.symbol,
                            statement_index,
                            local_data.initial_value,
                        )
                        .is_some_and(|value_type| {
                            is_reference_type(program, value_type)
                                || is_borrow_carrying_data(program, value_type)
                        });
                    if value_carries_borrow {
                        borrow_access_place(
                            program,
                            state.symbol,
                            statement_index,
                            local_data.initial_value,
                            machine_symbol,
                        )
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let Some(geometry) = formed_geometry
                .or_else(|| inferred_place.map(accesses::BorrowAccessGeometry::declared))
            else {
                return Ok(Vec::new());
            };

            let is_call_result = !is_access_formation(program, local_data.initial_value)
                && matches!(
                    expressions(program).expression(local_data.initial_value),
                    checked_trees::expression::ExpressionNode::Call(_)
                );
            let retained_access = geometry.retained_access(program, &local_access);
            let rebased = rebase_reference_formation_geometry(
                program,
                state.symbol,
                statement_index,
                local_data.initial_value,
                local_data.type_reference,
                geometry,
                loan_trackers,
            );
            Ok(rebased
                .iter()
                .map(|source| StatementBorrowLoan {
                    formation_owner_path: Some(Vec::new()),
                    incoming_origin: None,
                    formation_expression: local_data.initial_value,
                    representation_address: source.representation_address.clone(),
                    owner_symbol: local_data.symbol,
                    storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                    owner_name: local_data.name.clone(),
                    owner_path: Vec::new(),
                    place: source.place.clone(),
                    retained_operand: source.retained_operand,
                    source_owner_symbol: source.source_owner_symbol,
                    supplying_loans: std::iter::once(source.source_loan)
                        .filter(|loan| {
                            loan.is_valid()
                                && (source.lifetime_access.is_some() || *loan != source.parent_loan)
                        })
                        .collect(),
                    lineage: retained_reference_lineage(
                        program,
                        local_data.initial_value,
                        source,
                        &rebased,
                    ),
                    kind: source
                        .lifetime_access
                        .clone()
                        .unwrap_or_else(|| retained_access.clone()),
                    formation: if is_call_result {
                        super::BorrowFormation::ReturnedDescriptor
                    } else {
                        super::BorrowFormation::Ordinary
                    },
                })
                .collect())
        }

        /*
            The sealed placed factory forms a borrow from its captured source; it does
            not return a descriptor whose parent was lost at an ordinary call boundary.
        */
        pub(super) fn is_access_formation(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            validation::reference_formation_source(program, expression)
                .is_some_and(|formation| formation.interpretation.is_access_view())
        }
    }
    pub(in crate::borrow) use aggregate_loans::borrowed_initializer_loans;
    use aggregate_loans::*;
    pub(crate) use call_views::argument_borrow_loan_place;
    pub(in crate::borrow) use call_views::call_view_return_contract;
    use reference_borrows::*;

    mod aggregate_loans {
        /*
            Capture and transport the loans contained in ordinary values.

            A call's returned leaf identifies exact storage for execution, while its
            lifetime retains the supplied input region. Capture that prefix before
            appending the callee's leaf projection. Keeping both judgments on one
            captured path preserves selector and incoming-reference identity; widening
            to the caller's root would incorrectly freeze unrelated sibling fields.
            Carried values transport this obligation with the descriptor, even after
            the original holder retires. Ordinary borrows have no call-input prefix.
        */
        use super::super::accesses::{self, borrow_access_place};
        use super::super::tracker::{BorrowOwnerSegment, StateLoanTracker};
        use super::StatementBorrowLoan;
        use super::aggregate::{
            BorrowedInitializer, BorrowedInitializerKind, borrowed_initializers,
        };
        use super::call_views::*;
        use super::owner_paths::{owner_path_matches, place_path_matches_owner_prefix};
        use super::types::is_reference_type;
        use crate::borrow::view_link::{ViewReturnFieldSource, is_mutably_borrow_carrying_data};
        use checked_trees::expression::ExpressionHandle;
        use symbols::SymbolHandle;
        use validation::borrow_contracts::ViewReturnInputSource;

        /*
            The loan created by constructing a borrow-carrying `data` value in a `let`:
            the value borrows the source its reference field is initialized from.
            Struct/case literals and fixed arrays are followed recursively so wrapping
            one or several views in another checked aggregate cannot erase their loans.
            A nested direct helper call or moved/projected aggregate is then expanded
            under the enclosing field/index prefix with its original loan polarity.
        */
        pub(super) fn borrow_carrying_data_loans(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let initializers = borrowed_initializers(
                program,
                local_data.type_reference,
                local_data.initial_value,
                &[],
                &[],
            );

            let mut loans = Vec::new();
            for initializer in initializers {
                loans.extend(borrowed_initializer_loans(
                    program,
                    state.symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    initializer,
                    loan_trackers,
                    sources,
                )?);
            }
            Ok(loans)
        }

        #[allow(clippy::too_many_arguments)]
        pub(in crate::borrow) fn borrowed_initializer_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            initializer: BorrowedInitializer,
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            Ok(match initializer.kind {
                BorrowedInitializerKind::Reference {
                    access,
                    type_reference,
                } => {
                    let kind = checked_trees::BorrowAccessKind::from(access);
                    if let Some(mut loans) = super::returned_carriers::result_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        initializer.expression,
                        &initializer.owner_path,
                        loan_trackers,
                        sources,
                    )? {
                        super::returned_carriers::attenuate(&mut loans, &kind);
                        return Ok(loans);
                    }
                    if let Some(mut loans) = stored_reference_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        initializer.expression,
                        &initializer.owner_path,
                        loan_trackers,
                    ) {
                        super::returned_carriers::attenuate(&mut loans, &kind);
                        return Ok(loans);
                    }
                    let Some(geometry) = argument_borrow_loan_geometry(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        initializer.expression,
                    ) else {
                        return Ok(Vec::new());
                    };
                    let retained_access = geometry.retained_access(program, &kind);
                    let rebased = rebase_reference_formation_geometry(
                        program,
                        state_symbol,
                        statement_index,
                        initializer.expression,
                        type_reference,
                        geometry,
                        loan_trackers,
                    );
                    rebased
                        .iter()
                        .map(|source| StatementBorrowLoan {
                            formation_owner_path: Some(Vec::new()),
                            incoming_origin: None,
                            formation_expression: initializer.expression,
                            representation_address: source.representation_address.clone(),
                            owner_symbol: local_data.symbol,
                            storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                            owner_name: local_data.name.clone(),
                            owner_path: initializer.owner_path.clone(),
                            place: source.place.clone(),
                            retained_operand: source.retained_operand,
                            source_owner_symbol: source.source_owner_symbol,
                            supplying_loans: std::iter::once(source.source_loan)
                                .filter(|loan| {
                                    loan.is_valid()
                                        && (source.lifetime_access.is_some()
                                            || *loan != source.parent_loan)
                                })
                                .collect(),
                            lineage: retained_reference_lineage(
                                program,
                                initializer.expression,
                                source,
                                &rebased,
                            ),
                            kind: source
                                .lifetime_access
                                .clone()
                                .unwrap_or_else(|| retained_access.clone()),
                            formation: super::BorrowFormation::Ordinary,
                        })
                        .collect()
                }
                BorrowedInitializerKind::Aggregate { type_reference } => {
                    aggregate_expression_borrow_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        type_reference,
                        initializer.expression,
                        &initializer.owner_path,
                        loan_trackers,
                        sources,
                    )?
                }
            })
        }

        /*
            Transporting a stored descriptor keeps its exact historical suppliers.
            A borrow, recast or returned view instead establishes its own relation;
            equal backing alone cannot identify descriptor transport.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn stored_reference_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            expression: ExpressionHandle,
            owner_path: &[BorrowOwnerSegment],
            loan_trackers: &[StateLoanTracker],
        ) -> Option<Vec<StatementBorrowLoan>> {
            if validation::reference_formation_source(program, expression).is_some() {
                return None;
            }
            let reference = validation::expression_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                expression,
            )?;
            validation::reference_shell(program, reference)?;
            let source = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                expression,
                machine_symbol,
            )?;
            if !source.incoming_reference.is_valid()
                && source.segments.is_empty()
                && accesses::is_borrowed_parameter_root(program, source.root)
            {
                return None;
            }
            let sources = rebase_borrow_places_through_local_loans(program, source, loan_trackers);
            let mut loans = Vec::new();
            for source in sources {
                let mut selected = loan_trackers.iter().filter(|loan| {
                    loan.handle == source.parent_loan
                        && loan.storage_owner == source.source_place.root
                        && loan.owner_path.len() == source.source_place.segments.len()
                        && owner_path_matches(
                            program,
                            &loan.owner_path,
                            &source.source_place.segments,
                        )
                });
                let first = selected.next()?;
                loans.extend(transferred_aggregate_loans(
                    program,
                    local_data,
                    expression,
                    &source.source_place,
                    owner_path,
                    std::iter::once(first).chain(selected),
                ));
            }
            (!loans.is_empty()).then_some(loans)
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn aggregate_expression_borrow_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            type_reference: typed_trees::types::TypeReferenceHandle,
            expression: ExpressionHandle,
            owner_path_prefix: &[BorrowOwnerSegment],
            loan_trackers: &[StateLoanTracker],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            Ok(match program.expression_table.expression(expression) {
                checked_trees::expression::ExpressionNode::Call(call) => {
                    if let Some(field_loans) = super::returned_carriers::result_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        expression,
                        owner_path_prefix,
                        loan_trackers,
                        sources,
                    )? {
                        return Ok(field_loans);
                    }

                    let Some(place) = helper_call_borrow_loan_place(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        call,
                    ) else {
                        return Ok(Vec::new());
                    };
                    let rebased =
                        rebase_borrow_places_through_local_loans(program, place, loan_trackers);
                    rebased
                        .iter()
                        .map(|source| StatementBorrowLoan {
                            formation_owner_path: Some(Vec::new()),
                            incoming_origin: None,
                            formation_expression: expression,
                            representation_address: None,
                            owner_symbol: local_data.symbol,
                            storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                            owner_name: local_data.name.clone(),
                            owner_path: owner_path_prefix.to_vec(),
                            place: source.place.clone(),
                            retained_operand: source.retained_operand,
                            source_owner_symbol: source.source_owner_symbol,
                            supplying_loans: std::iter::once(source.source_loan)
                                .filter(|loan| {
                                    loan.is_valid()
                                        && (source.lifetime_access.is_some()
                                            || *loan != source.parent_loan)
                                })
                                .collect(),
                            lineage: retained_reference_lineage(
                                program, expression, source, &rebased,
                            ),
                            kind: source.lifetime_access.clone().unwrap_or_else(|| {
                                if is_mutably_borrow_carrying_data(program, type_reference) {
                                    checked_trees::BorrowAccessKind::Mutable
                                } else {
                                    checked_trees::BorrowAccessKind::Read
                                }
                            }),
                            formation: super::BorrowFormation::Ordinary,
                        })
                        .collect()
                }
                checked_trees::expression::ExpressionNode::Name(_)
                | checked_trees::expression::ExpressionNode::Member(_)
                | checked_trees::expression::ExpressionNode::Indexed(_) => {
                    if let Some(loans) = super::returned_carriers::result_loans(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        local_data,
                        expression,
                        owner_path_prefix,
                        loan_trackers,
                        sources,
                    )? {
                        return Ok(loans);
                    }
                    let Some(source) = borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        expression,
                        machine_symbol,
                    ) else {
                        return Ok(Vec::new());
                    };
                    rebase_borrow_places_through_local_loans(program, source, loan_trackers)
                        .iter()
                        .flat_map(|source| {
                            let storage = if source.lifetime_access.is_some() {
                                &source.source_place
                            } else {
                                &source.place
                            };
                            transferred_aggregate_loans(
                                program,
                                local_data,
                                expression,
                                storage,
                                owner_path_prefix,
                                loan_trackers.iter(),
                            )
                        })
                        .collect()
                }
                checked_trees::expression::ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                    let nested = borrowed_initializers(
                        program,
                        cast.target_type,
                        cast.value,
                        &[],
                        owner_path_prefix,
                    );
                    let mut loans = Vec::new();
                    for initializer in nested {
                        loans.extend(borrowed_initializer_loans(
                            program,
                            state_symbol,
                            statement_index,
                            machine_symbol,
                            local_data,
                            initializer,
                            loan_trackers,
                            sources,
                        )?);
                    }
                    loans
                }
                _ => Vec::new(),
            })
        }

        /*
            Expression calls and named normal completions transport the same declared
            result-to-input relation. The caller supplies its applied contract; this
            operation composes actual captured leaves without inventing another call.
        */
        pub(super) fn result_source_loans(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            local_data: &checked_trees::statement::TableLocalData,
            target: SymbolHandle,
            arguments: &[ExpressionHandle],
            fields: Vec<ViewReturnFieldSource>,
            substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
            owner_path_prefix: &[BorrowOwnerSegment],
            sources: &super::BorrowFormationSources<'_, '_>,
        ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
            let mut carried_arguments = Vec::new();

            let completed =
                super::returned_carriers::completed_sources(program, target, fields, arguments);
            let argument_rosters = arguments
                .iter()
                .map(|argument| {
                    sources.at_argument(
                        program,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        *argument,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut loans = Vec::new();
            for source in completed {
                loans.extend(source.capture_loans(
                    local_data,
                    owner_path_prefix,
                    |field, projection| {
                        let mut owner_path = owner_path_prefix.to_vec();
                        owner_path.extend(field.owner_path.iter().copied());
                        let Some(argument) = arguments.get(field.parameter_index).copied() else {
                            return Ok(Vec::new());
                        };
                        let loan_trackers = argument_rosters[field.parameter_index].as_slice();
                        let source_type = CallViewReturnContract::carrier_reference(
                            program,
                            field.source_type,
                            substitutions,
                        );
                        if !is_reference_type(program, source_type) {
                            return super::returned_carriers::argument_loans(
                                program,
                                state_symbol,
                                statement_index,
                                machine_symbol,
                                local_data,
                                argument,
                                &field,
                                &owner_path,
                                loan_trackers,
                                substitutions,
                                &mut carried_arguments,
                                sources,
                            );
                        }
                        if field.source == ViewReturnInputSource::Referent
                            && let Some(mut loans) =
                                super::returned_carriers::relative_result_loans(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    machine_symbol,
                                    local_data,
                                    argument,
                                    loan_trackers,
                                    sources,
                                )?
                        {
                            if !field.lifetime_only {
                                super::returned_carriers::attenuate(&mut loans, &field.kind);
                            }
                            for loan in &mut loans {
                                loan.owner_path = owner_path.clone();
                                if field.lifetime_only {
                                    loan.lineage =
                                        checked_trees::BorrowLoanLineage::LifetimeDependency;
                                }
                                loan.retained_operand.get_or_insert(
                                    checked_trees::BorrowRetainedOperand {
                                        formation_expression: Default::default(),
                                        depth: loan.place.segments.len(),
                                    },
                                );
                                if !projection.is_empty() {
                                    loan.representation_address = None;
                                }
                                loan.place.segments.extend_from_slice(&projection);
                            }
                            return Ok(loans);
                        }
                        let Some(mut place) = argument_borrow_loan_place(
                            program,
                            state_symbol,
                            statement_index,
                            machine_symbol,
                            argument,
                        ) else {
                            return Ok(Vec::new());
                        };
                        if field.source == ViewReturnInputSource::Stored {
                            if projection.is_empty() {
                                place
                                    .segments
                                    .extend(super::owner_paths::owner_place_segments(
                                        &field.source_path,
                                    ));
                            } else {
                                place.segments.extend_from_slice(&projection);
                            }
                        }
                        let projected = field.source == ViewReturnInputSource::Referent
                            && !field.lifetime_only
                            && !projection.is_empty();
                        let rebased =
                            rebase_borrow_places_through_local_loans(program, place, loan_trackers);
                        let rebased = if projected {
                            retain_formation_parents(rebased, loan_trackers)
                        } else {
                            rebased
                        };
                        Ok(rebased
                            .iter()
                            .map(|source| {
                                let mut place = source.place.clone();
                                if field.source == ViewReturnInputSource::Referent {
                                    place.segments.extend_from_slice(&projection);
                                }
                                StatementBorrowLoan {
                                    formation_owner_path: None,
                                    incoming_origin: None,
                                    formation_expression: argument,
                                    representation_address: None,
                                    owner_symbol: local_data.symbol,
                                    storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                                    owner_name: local_data.name.clone(),
                                    owner_path: owner_path.clone(),
                                    place,
                                    retained_operand: Some(source.retained_operand.unwrap_or(
                                        checked_trees::BorrowRetainedOperand {
                                            formation_expression: Default::default(),
                                            depth: source.place.segments.len(),
                                        },
                                    )),
                                    source_owner_symbol: source.source_owner_symbol,
                                    supplying_loans: if projected && source.source_loan.is_valid() {
                                        vec![source.source_loan]
                                    } else if source.parent_loan.is_valid()
                                        && (field.source == ViewReturnInputSource::Stored
                                            || projection.is_empty())
                                    {
                                        vec![source.parent_loan]
                                    } else {
                                        Vec::new()
                                    },
                                    lineage: if field.lifetime_only {
                                        checked_trees::BorrowLoanLineage::LifetimeDependency
                                    } else if projected && source.parent_lineage_is_retained {
                                        checked_trees::BorrowLoanLineage::Reborrow {
                                            parent_loan: source.parent_loan,
                                        }
                                    } else {
                                        retained_reference_lineage(
                                            program, argument, source, &rebased,
                                        )
                                    },
                                    kind: if field.lifetime_only {
                                        source
                                            .lifetime_access
                                            .clone()
                                            .unwrap_or_else(|| field.kind.clone())
                                    } else {
                                        field.kind.clone()
                                    },
                                    formation: if projected {
                                        super::BorrowFormation::ReturnedProjection
                                    } else {
                                        super::BorrowFormation::Ordinary
                                    },
                                }
                            })
                            .collect::<Vec<_>>())
                    },
                )?);
            }
            Ok(loans)
        }

        fn transferred_aggregate_loans<'loan>(
            program: &dyn ::validation::CheckingInput,
            local_data: &checked_trees::statement::TableLocalData,
            expression: ExpressionHandle,
            source: &accesses::BorrowAccessPlace,
            owner_path_prefix: &[BorrowOwnerSegment],
            loan_trackers: impl DoubleEndedIterator<Item = &'loan StateLoanTracker>,
        ) -> Vec<StatementBorrowLoan> {
            loan_trackers
                .rev()
                .filter_map(|loan| {
                    if loan.storage_owner != source.root {
                        return None;
                    }

                    let owner_path = if place_path_matches_owner_prefix(
                        program,
                        &source.segments,
                        &loan.owner_path,
                    ) {
                        loan.owner_path[source.segments.len()..].to_vec()
                    } else if owner_path_matches(program, &loan.owner_path, &source.segments) {
                        Vec::new()
                    } else {
                        return None;
                    };

                    let formation_owner_path = Some(owner_path.clone());
                    let mut prefixed_owner_path = owner_path_prefix.to_vec();
                    prefixed_owner_path.extend(owner_path);
                    Some(StatementBorrowLoan {
                        formation_owner_path,
                        incoming_origin: None,
                        formation_expression: expression,
                        representation_address: None,
                        owner_symbol: local_data.symbol,
                        storage_owner: facts::PlaceRoot::Symbol(local_data.symbol),
                        owner_name: local_data.name.clone(),
                        owner_path: prefixed_owner_path,
                        place: loan.place.clone(),
                        retained_operand: loan.retained_operand,
                        source_owner_symbol: loan.owner_symbol,
                        supplying_loans: std::iter::once(loan.handle)
                            .filter(|handle| handle.is_valid())
                            .collect(),
                        lineage: loan.lineage.derived(),
                        kind: loan.kind.clone(),
                        formation: super::BorrowFormation::Ordinary,
                    })
                })
                .collect()
        }
    }
    mod call_views {
        /*
            Rebase completed descriptor loans through caller storage at formation.
            Declared referee storage stops pointer traversal; interpreted geometry and
            independent lifetime debts retain their supplying occurrence.
        */
        use super::super::accesses;
        use super::super::tracker::StateLoanTracker;
        use super::RebasedBorrowPlace;
        use super::owner_paths::owner_path_matches;
        use crate::borrow::view_link::{
            ViewReturnFieldSource, ViewReturnSource, resolve_signature_view_return_source,
        };

        use arena::Handle;
        use checked_trees::expression::ExpressionHandle;
        use symbols::SymbolHandle;
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn helper_call_borrow_loan_place(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            call: &checked_trees::expression::TableCallExpression,
        ) -> Option<accesses::BorrowAccessPlace> {
            match call_view_return_contract(program, state_symbol, call)?.source(program) {
                ViewReturnSource::NotApplicable
                | ViewReturnSource::Ambiguous(_)
                | ViewReturnSource::Fields { .. } => None,
                ViewReturnSource::Parameter { parameter_index } => {
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    let argument = arguments.get(parameter_index).copied()?;
                    argument_borrow_loan_place(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        argument,
                    )
                }
            }
        }

        pub(super) fn call_view_return_source(
            program: &dyn ::validation::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> ViewReturnSource {
            if let Some(relation) = validation::borrow_contracts::installed_context_lifetime_source(
                program,
                target_symbol,
            ) {
                return relation;
            }
            let Some((parameters, return_type, _)) = call_view_signature(program, target_symbol)
            else {
                return ViewReturnSource::NotApplicable;
            };
            resolve_signature_view_return_source(program, parameters, return_type)
        }

        /*
            One call's declaration and applied telescope travel together. Returned
            fields, direct-reference ancestry and nested transfers must consume this
            same relation: looking up the raw signature again loses an evidence
            binder's application and can silently erase its aggregate loans.
        */
        pub(in crate::borrow) struct CallViewReturnContract<'program> {
            parameters: &'program [StateParameter],
            return_type: TypeReferenceHandle,
            pub(super) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            substitution_scope_starts: Vec<usize>,
            requirement_signature: Option<&'program typed_trees::signature::StateSignature>,
            argument_owner: SymbolHandle,
            target: SymbolHandle,
        }

        impl CallViewReturnContract<'_> {
            /*
                Every returned reference leaf has one declaration relation, including a
                bare reference's empty result path. Body precision refines these sources
                separately; choosing a different formation path for bare references
                would discard the returned projection beneath their declared input.
            */
            pub(in crate::borrow) fn reference_sources(
                &self,
                program: &dyn ::validation::CheckingInput,
            ) -> Option<Vec<ViewReturnFieldSource>> {
                match self.source(program) {
                    ViewReturnSource::Fields { fields } => Some(fields),
                    ViewReturnSource::Parameter { parameter_index } => {
                        Some(vec![ViewReturnFieldSource {
                            lifetime_only: false,
                            source: validation::borrow_contracts::ViewReturnInputSource::Referent,
                            owner_path: Vec::new(),
                            source_path: Vec::new(),
                            source_type: self.parameter_type_reference(program, parameter_index),
                            source_reference_hops: 0,
                            parameter_index,
                            kind: super::types::reference_borrow_access_kind(
                                program,
                                self.return_type_reference(program),
                            )?,
                        }])
                    }
                    ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => None,
                }
            }

            pub(in crate::borrow) fn source(
                &self,
                program: &dyn ::validation::CheckingInput,
            ) -> ViewReturnSource {
                if let Some(relation) =
                    validation::borrow_contracts::installed_context_lifetime_source(
                        program,
                        self.target,
                    )
                {
                    return relation;
                }
                if let Some(signature) = self.requirement_signature {
                    return validation::borrow_contracts::resolve_applied_signature_view_return_source(
                        program,
                        signature,
                        &self.substitutions,
                        &self.substitution_scope_starts,
                        self.argument_owner,
                    );
                }
                if self.substitutions.is_empty() {
                    resolve_signature_view_return_source(program, self.parameters, self.return_type)
                } else {
                    crate::borrow::view_link::resolve_substituted_view_return_source(
                        program,
                        self.parameters,
                        self.return_type,
                        &self.substitutions,
                        &self.substitution_scope_starts,
                    )
                }
            }

            pub(in crate::borrow) fn return_type_reference(
                &self,
                program: &dyn ::validation::CheckingInput,
            ) -> TypeReferenceHandle {
                Self::carrier_reference(program, self.return_type, &self.substitutions)
            }

            pub(in crate::borrow) fn parameter_type_reference(
                &self,
                program: &dyn ::validation::CheckingInput,
                parameter_index: usize,
            ) -> TypeReferenceHandle {
                self.parameters.get(parameter_index).map_or_else(
                    TypeReferenceHandle::invalid,
                    |parameter| {
                        Self::carrier_reference(
                            program,
                            parameter.type_reference,
                            &self.substitutions,
                        )
                    },
                )
            }

            /*
                Only the borrow carrier is projected here; qualifications remain on
                the original declaration. Composite types stay borrowed nodes and the
                structural frontier walks them under the complete substitution roster.
            */
            pub(in crate::borrow) fn carrier_reference(
                program: &dyn ::validation::CheckingInput,
                mut reference: TypeReferenceHandle,
                substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            ) -> TypeReferenceHandle {
                let mut visited = Vec::new();
                loop {
                    if visited.contains(&reference) {
                        return TypeReferenceHandle::invalid();
                    }
                    visited.push(reference);
                    match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                        TypeReferenceNode::Named { symbol, .. } => {
                            let Some((_, argument)) = substitutions
                                .iter()
                                .rev()
                                .find(|(parameter, _)| parameter == symbol)
                            else {
                                return reference;
                            };
                            reference = *argument;
                        }
                        _ => return reference,
                    }
                }
            }
        }

        pub(in crate::borrow) fn call_view_return_contract<'program>(
            program: &'program dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            call: &checked_trees::expression::TableCallExpression,
        ) -> Option<CallViewReturnContract<'program>> {
            let (caller, state) = validation::find_state(program, state_symbol)?;
            let (
                parameters,
                return_type,
                substitutions,
                substitution_scope_starts,
                requirement_signature,
            ) = if !call.static_machine_parameter.is_valid()
                && let Ok(Some(requirement)) = validation::named_conformance_target_requirement(
                    program,
                    caller,
                    call.target_symbol,
                ) {
                (
                    program.state_signature_parameters(requirement.signature),
                    requirement.signature.return_type,
                    requirement.application.substitutions,
                    requirement.application.substitution_scope_starts,
                    Some(requirement.signature),
                )
            } else {
                let (parameters, return_type, signature) =
                    if call.static_machine_parameter.is_valid() {
                        let signature = program
                            .retained_static_machine_contract(call.static_machine_parameter)?
                            .signature();
                        (
                            program.state_signature_parameters(signature),
                            signature.return_type,
                            Some(signature),
                        )
                    } else {
                        call_view_signature(program, call.target_symbol)?
                    };
                let substitutions = signature
                    .and_then(|signature| {
                        validation::closed_static_call_type_bindings(
                            program,
                            caller,
                            state,
                            signature,
                            &call.machine_arguments,
                            program.expression_table.expression_handles(call.arguments),
                        )
                    })
                    .unwrap_or_default();
                (parameters, return_type, substitutions, vec![0], signature)
            };
            Some(CallViewReturnContract {
                parameters,
                return_type,
                substitutions,
                substitution_scope_starts,
                requirement_signature,
                argument_owner: caller.symbol,
                target: call.target_symbol,
            })
        }

        /*
            True when the call target's own declaration names one exact borrow source
            for its returned view: a single direct reference parameter.
            This is the promotion gate for call-result loan provenance --
            carrier-field sources, ambiguous signatures, and view-free calls all stay
            deliberately unretained.
        */
        pub(crate) fn call_declares_direct_view_source(
            program: &dyn ::validation::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> bool {
            matches!(
                call_view_return_source(program, target_symbol),
                ViewReturnSource::Parameter { .. }
            )
        }

        fn call_view_signature(
            program: &dyn ::validation::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> Option<(
            &[typed_trees::signature::StateParameter],
            typed_trees::types::TypeReferenceHandle,
            Option<&typed_trees::signature::StateSignature>,
        )> {
            if let Some(slot) = program.dynamic_slot_by_symbol(target_symbol) {
                return Some((&slot.parameters, slot.return_type, None));
            }
            if let Some(target_state) = validation::call_target_state(program, target_symbol) {
                return Some((
                    program.state_parameters(target_state),
                    target_state.return_type,
                    None,
                ));
            }
            if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
                return Some((
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    Some(signature),
                ));
            }
            for trait_definition in program.traits() {
                if let Some(signature) =
                    program.trait_machine_signature_by_symbol(trait_definition, target_symbol)
                {
                    return Some((
                        program.state_signature_parameters(signature),
                        signature.return_type,
                        Some(signature),
                    ));
                }
            }
            None
        }

        /*
            The loan place for a call argument that the called machine's returned view
            borrows (elision rule 1). A view-producing call follows its declared input
            relation. Explicit address formation instead lends the captured storage,
            including a call result's own temporary home.
        */
        pub(crate) fn argument_borrow_loan_place(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            argument: ExpressionHandle,
        ) -> Option<accesses::BorrowAccessPlace> {
            argument_borrow_loan_geometry(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            )
            .map(|geometry| geometry.place)
        }

        pub(super) fn argument_borrow_loan_geometry(
            program: &dyn ::validation::CheckingInput,
            state_symbol: SymbolHandle,
            statement_index: usize,
            machine_symbol: SymbolHandle,
            argument: ExpressionHandle,
        ) -> Option<accesses::BorrowAccessGeometry> {
            match program.expression_table.expression(argument) {
                checked_trees::expression::ExpressionNode::Call(inner_call) => {
                    helper_call_borrow_loan_place(
                        program,
                        state_symbol,
                        statement_index,
                        machine_symbol,
                        inner_call,
                    )
                    .map(accesses::BorrowAccessGeometry::declared)
                }
                _ => accesses::borrow_access_geometry(
                    program,
                    state_symbol,
                    statement_index,
                    argument,
                    machine_symbol,
                ),
            }
        }

        pub(super) fn rebase_borrow_places_through_local_loans(
            program: &dyn ::validation::CheckingInput,
            place: accesses::BorrowAccessPlace,
            loan_trackers: &[StateLoanTracker],
        ) -> Vec<RebasedBorrowPlace> {
            rebase_borrow_geometry_through_local_loans(
                program,
                accesses::BorrowAccessGeometry::declared(place),
                loan_trackers,
            )
        }

        pub(super) fn rebase_borrow_geometry_through_local_loans(
            program: &dyn ::validation::CheckingInput,
            geometry: accesses::BorrowAccessGeometry,
            loan_trackers: &[StateLoanTracker],
        ) -> Vec<RebasedBorrowPlace> {
            rebase_borrow_places_following(program, geometry, loan_trackers, None, &mut Vec::new())
        }

        /*
            Address formation stops at the authored referee's stored identity. A
            borrow of descriptor bytes must not become a reborrow of its pointee.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn rebase_reference_formation_geometry(
            program: &dyn validation::CheckingInput,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
            geometry: accesses::BorrowAccessGeometry,
            loan_trackers: &[StateLoanTracker],
        ) -> Vec<RebasedBorrowPlace> {
            let declared = validation::reference_formation_source(program, expression).is_some_and(
                |formation| {
                    formation.interpretation
                        == validation::ReferenceFormationInterpretation::Declared
                },
            );
            let expected = validation::reference_shell(program, reference).map(|shell| {
                validation::stored_structural_type_identity(program, shell.referee, &[], &[])
            });
            let Some(expected) = expected.filter(|identity| declared && identity.is_valid()) else {
                let sources =
                    rebase_borrow_geometry_through_local_loans(program, geometry, loan_trackers);
                return retained_formation_sources(program, expression, sources, loan_trackers);
            };
            let stop = |place: &accesses::BorrowAccessPlace| {
                let Some(mut reference) = validation::canonical_place_type_reference(
                    program,
                    state,
                    statement,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: Vec::new(),
                    },
                ) else {
                    return false;
                };
                if let Some(symbol) = place.root.as_symbol()
                    && program.state_by_symbol(state).is_some_and(|state| {
                        program.state_parameter_by_symbol(state, symbol).is_some()
                    })
                    && let Some(shell) = validation::reference_shell(program, reference)
                {
                    reference = shell.referee;
                }
                validation::owned_projection_type_reference(program, reference, &place.segments)
                    .is_some_and(|stored| {
                        validation::stored_structural_type_identity(program, stored, &[], &[])
                            == expected
                    })
            };
            let sources = rebase_borrow_places_following(
                program,
                geometry,
                loan_trackers,
                Some(&stop),
                &mut Vec::new(),
            );
            retained_formation_sources(program, expression, sources, loan_trackers)
        }

        /*
            A saved descriptor forwards its established permission, including every
            selected alternative. Stop at the nearest real borrow, so a shared child
            of mutable storage never regains its ancestor's write authority.
        */
        fn retained_formation_sources(
            program: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
            sources: Vec<RebasedBorrowPlace>,
            loan_trackers: &[StateLoanTracker],
        ) -> Vec<RebasedBorrowPlace> {
            if validation::reference_formation_source(program, expression).is_none() {
                return sources;
            }
            retain_formation_parents(sources, loan_trackers)
        }

        pub(super) fn retain_formation_parents(
            sources: Vec<RebasedBorrowPlace>,
            loan_trackers: &[StateLoanTracker],
        ) -> Vec<RebasedBorrowPlace> {
            sources
                .into_iter()
                .flat_map(|source| {
                    let Some(descriptor) = loan_trackers
                        .iter()
                        .find(|loan| loan.handle == source.source_loan)
                        .filter(|loan| !loan.permission_sources.is_empty())
                    else {
                        return vec![source];
                    };
                    descriptor
                        .permission_sources
                        .iter()
                        .map(|parent| RebasedBorrowPlace {
                            parent_loan: parent.loan,
                            source_owner_symbol: parent.owner_symbol,
                            parent_lineage_is_retained: true,
                            ..source.clone()
                        })
                        .collect()
                })
                .collect()
        }

        /*
            A shared-reference parameter cannot be written through, so a statement
            loan it carries at its own root was installed by rebinding the parameter
            (`selected = &local`), not by a store into the caller's referent. Reading
            the parameter then reads that new descriptor; forwarding the formal would
            let a returned view of an invocation-local slot pass as the caller's
            referent. Entry leaves keep their incoming identity and are not rebinds.
        */
        fn shared_parameter_rebound(
            program: &dyn ::validation::CheckingInput,
            root: facts::PlaceRoot,
            loan_trackers: &[StateLoanTracker],
        ) -> bool {
            let Some(symbol) = root.as_symbol() else {
                return false;
            };
            let shared = program
                .state_by_symbol(program.symbols.get(symbol).parent)
                .and_then(|state| program.state_parameter_by_symbol(state, symbol))
                .and_then(|parameter| {
                    validation::reference_shell(program, parameter.type_reference)
                })
                .is_some_and(|shell| shell.access == language_semantics::ReferenceAccess::Shared);
            shared
                && loan_trackers.iter().any(|loan| {
                    loan.storage_owner == root
                        && loan.owner_path.is_empty()
                        && !loan.place.incoming_reference.is_valid()
                })
        }

        fn rebase_borrow_places_following(
            program: &dyn ::validation::CheckingInput,
            geometry: accesses::BorrowAccessGeometry,
            loan_trackers: &[StateLoanTracker],
            storage_boundary: Option<&dyn Fn(&accesses::BorrowAccessPlace) -> bool>,
            active: &mut Vec<accesses::BorrowAccessPlace>,
        ) -> Vec<RebasedBorrowPlace> {
            let accesses::BorrowAccessGeometry {
                place,
                representation_address,
            } = geometry;
            let parameter_referent = !place.incoming_reference.is_valid()
                && place.segments.is_empty()
                && accesses::is_borrowed_parameter_root(program, place.root)
                && !(active.is_empty()
                    && shared_parameter_rebound(program, place.root, loan_trackers));
            let source_loans: Vec<&StateLoanTracker> = loan_trackers
                .iter()
                .rev()
                .filter(|loan| {
                    loan.storage_owner == place.root
                        && owner_path_matches(program, &loan.owner_path, &place.segments)
                })
                .collect();
            if parameter_referent
                || source_loans.is_empty()
                || place.incoming_reference.is_valid()
                || storage_boundary.is_some_and(|stop| stop(&place))
                || active.contains(&place)
            {
                return vec![RebasedBorrowPlace {
                    source_place: place.clone(),
                    representation_address: if active.contains(&place) {
                        None
                    } else {
                        representation_address
                    },
                    retained_operand: None,
                    place,
                    source_owner_symbol: SymbolHandle::invalid(),
                    parent_loan: Handle::invalid(),
                    parent_lineage_is_retained: false,
                    source_loan: Handle::invalid(),
                    lifetime_access: None,
                }];
            }

            active.push(place.clone());
            let rebased = source_loans
                .into_iter()
                .map(|source_loan| {
                    let lifetime_only =
                        source_loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency;
                    let remainder = &place.segments[source_loan.owner_path.len()..];
                    let mut rebased_segments = Vec::with_capacity(
                        source_loan
                            .place
                            .segments
                            .len()
                            .saturating_add(remainder.len()),
                    );
                    rebased_segments.extend(source_loan.place.segments.iter().copied());
                    if !lifetime_only {
                        rebased_segments.extend(remainder.iter().copied());
                    }

                    let address = if lifetime_only {
                        None
                    } else if remainder.is_empty() && representation_address.is_none() {
                        source_loan.representation_address.clone()
                    } else if source_loan.representation_address.is_some() {
                        None
                    } else {
                        representation_address.as_ref().and_then(|address| {
                            if address.root != place.root
                                || address.incoming_reference != place.incoming_reference
                                || address.incoming_owner_path != place.incoming_owner_path
                                || !owner_path_matches(
                                    program,
                                    &source_loan.owner_path,
                                    &address.segments,
                                )
                            {
                                return None;
                            }
                            let mut segments = source_loan.place.segments.clone();
                            segments.extend_from_slice(
                                &address.segments[source_loan.owner_path.len()..],
                            );
                            crate::flow::compose_captured_view_segments(&mut segments)?;
                            Some(accesses::BorrowAccessPlace {
                                incoming_reference: source_loan.place.incoming_reference,
                                incoming_owner_path: if source_loan
                                    .place
                                    .incoming_owner_path
                                    .is_empty()
                                    && source_loan.owner_path.contains(
                                        &super::super::tracker::BorrowOwnerSegment::DynamicIndex,
                                    ) {
                                    place.segments[..source_loan.owner_path.len()].to_vec()
                                } else {
                                    source_loan.place.incoming_owner_path.clone()
                                },
                                root: source_loan.place.root,
                                segments,
                            })
                        })
                    };
                    RebasedBorrowPlace {
                        source_place: place.clone(),
                        representation_address: address,
                        lifetime_access: lifetime_only.then(|| source_loan.kind.clone()),
                        retained_operand: source_loan.retained_operand,
                        place: accesses::BorrowAccessPlace {
                            incoming_reference: source_loan.place.incoming_reference,
                            incoming_owner_path: if source_loan.place.incoming_owner_path.is_empty()
                                && source_loan.owner_path.contains(
                                    &super::super::tracker::BorrowOwnerSegment::DynamicIndex,
                                ) {
                                place.segments[..source_loan.owner_path.len()].to_vec()
                            } else {
                                source_loan.place.incoming_owner_path.clone()
                            },
                            root: source_loan.place.root,
                            segments: rebased_segments,
                        },
                        source_owner_symbol: source_loan.owner_symbol,
                        parent_loan: source_loan.handle,
                        source_loan: source_loan.handle,
                        parent_lineage_is_retained: false,
                    }
                })
                .flat_map(|source| {
                    if source.lifetime_access.is_some() {
                        return vec![source];
                    }
                    let nested = rebase_borrow_places_following(
                        program,
                        accesses::BorrowAccessGeometry {
                            place: source.place.clone(),
                            representation_address: source.representation_address.clone(),
                        },
                        loan_trackers,
                        storage_boundary,
                        active,
                    );
                    nested
                        .into_iter()
                        .map(move |nested| {
                            if nested.source_owner_symbol.is_valid() {
                                nested
                            } else {
                                RebasedBorrowPlace {
                                    place: nested.place,
                                    representation_address: nested.representation_address,
                                    ..source.clone()
                                }
                            }
                        })
                        .collect()
                })
                .collect();
            active.pop();
            rebased
        }

        pub(super) fn retained_reference_lineage(
            program: &dyn ::validation::CheckingInput,
            formation: ExpressionHandle,
            source: &RebasedBorrowPlace,
            _rebased: &[RebasedBorrowPlace],
        ) -> checked_trees::BorrowLoanLineage {
            if source.lifetime_access.is_some() {
                return checked_trees::BorrowLoanLineage::LifetimeDependency;
            }
            let is_reference_formation =
                validation::reference_formation_source(program, formation).is_some();
            let declared_source = match program.expression_table.expression(formation) {
                checked_trees::expression::ExpressionNode::Call(call) => {
                    call_declares_direct_view_source(program, call.target_symbol)
                }
                _ => false,
            };
            let force_unretained = matches!(
                program.expression_table.expression(formation),
                checked_trees::expression::ExpressionNode::Cast(_)
                    | checked_trees::expression::ExpressionNode::Call(_)
            ) && !declared_source
                && !is_reference_formation;
            if force_unretained {
                return checked_trees::BorrowLoanLineage::UnretainedDerived;
            }
            if !source.source_owner_symbol.is_valid() {
                return checked_trees::BorrowLoanLineage::DirectRoot;
            }
            if is_reference_formation
                && source.parent_loan.is_valid()
                && source.parent_lineage_is_retained
            {
                checked_trees::BorrowLoanLineage::Reborrow {
                    parent_loan: source.parent_loan,
                }
            } else {
                checked_trees::BorrowLoanLineage::UnretainedDerived
            }
        }
    }

    /*
        Construction scratch for one supplied descriptor namespace. Its path is
        allocated by the receiving resource owner, after alternatives are complete.
    */
    #[derive(Clone, PartialEq, Eq)]
    pub(super) struct PendingIncomingReference {
        pub(super) identity: checked_trees::IncomingReferenceIdentity,
        pub(super) owner_path: Vec<checked_trees::BorrowLoanOwnerSegment>,
    }

    pub(super) use checked_trees::BorrowLoanFormation as BorrowFormation;

    #[derive(Clone)]
    pub(super) struct StatementBorrowLoan {
        pub(super) incoming_origin: Option<PendingIncomingReference>,
        pub(super) formation_expression: ExpressionHandle,
        pub(super) formation_owner_path: Option<Vec<BorrowOwnerSegment>>,
        pub(super) owner_symbol: SymbolHandle,
        pub(super) storage_owner: facts::PlaceRoot,
        pub(super) owner_name: Identifier,
        pub(super) owner_path: Vec<BorrowOwnerSegment>,
        pub(super) place: accesses::BorrowAccessPlace,
        pub(super) representation_address: Option<accesses::BorrowAccessPlace>,
        pub(super) retained_operand: Option<checked_trees::BorrowRetainedOperand>,
        pub(super) source_owner_symbol: SymbolHandle,
        pub(super) supplying_loans: Vec<Handle<checked_trees::BorrowLoanFact>>,
        pub(super) lineage: checked_trees::BorrowLoanLineage,
        pub(super) kind: checked_trees::BorrowAccessKind,
        pub(super) formation: BorrowFormation,
    }

    #[derive(Clone)]
    struct RebasedBorrowPlace {
        /*
            The descriptor location before following its captured pointee. A
            containing pointer is not the supplier of a stored child descriptor.
        */
        source_place: accesses::BorrowAccessPlace,
        place: accesses::BorrowAccessPlace,
        representation_address: Option<accesses::BorrowAccessPlace>,
        retained_operand: Option<checked_trees::BorrowRetainedOperand>,
        source_owner_symbol: SymbolHandle,
        parent_loan: Handle<checked_trees::BorrowLoanFact>,
        parent_lineage_is_retained: bool,
        /* Exact descriptor occurrence read before transparent copies were followed. */
        source_loan: Handle<checked_trees::BorrowLoanFact>,
        /*
            Nonspatial debt carries the actual supplier's access. A nominal lifetime
            slot neither attenuates an exclusive loan nor grants a reference view.
        */
        lifetime_access: Option<checked_trees::BorrowAccessKind>,
    }

    /*
        Every call input observes the ordered prefix ending at that input. Keep
        the statement-entry roster available: a later sibling can replace a slot
        after an earlier pointer was already evaluated.
    */
    #[derive(Clone, Copy)]
    pub(super) struct BorrowFormationSources<'source, 'program> {
        statement_entry: &'source [StateLoanTracker],
        call_frames: Option<&'source validation::CallFrameResolver<'program>>,
    }

    impl BorrowFormationSources<'_, '_> {
        fn at_argument(
            &self,
            program: &dyn validation::CheckingInput,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            argument: ExpressionHandle,
        ) -> Result<Vec<StateLoanTracker>, &'static str> {
            let frames = self
                .call_frames
                .ok_or("call input loan formation has no prepared call-frame context")?;
            let machine = program
                .machine(machine)
                .ok_or("call input loan formation has no owning machine")?;
            let state = program
                .state_by_symbol(state)
                .ok_or("call input loan formation has no owning state")?;
            stored_calls::roster_at_expression(
                program,
                machine,
                state,
                statement,
                argument,
                frames,
                self.statement_entry,
            )
            .map_err(|reason| match reason {
                validation::StoredReferenceRelationRefusal::TemplateDependent => {
                    "call input reference slots depend on an unresolved template at this evaluation"
                }
                validation::StoredReferenceRelationRefusal::Unavailable(_) => {
                    "call input has no complete reference-slot relation at its own evaluation"
                }
            })
        }
    }

    /*
        A reference keeps its computed backing's stored descriptors live, but those
        descriptors are not fields of the reference variable. Capture them once
        under the backing's storage identity. Aliases retain that identity and the
        ordinary lifetime closure extends its contents without replaying formation.
    */
    pub(super) fn retain_backing_contents(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        pending: &mut Vec<StatementBorrowLoan>,
        current: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<(), &'static str> {
        let sources = BorrowFormationSources {
            statement_entry: current,
            call_frames,
        };
        let (owner, name) = statement
            .as_local_data()
            .map_or((state.symbol, &state.name), |local| {
                (local.symbol, &local.name)
            });
        let mut backings = Vec::new();
        validation::visit_statement_value_occurrences(
            program,
            machine_symbol,
            state.symbol,
            statement_index,
            statement,
            &mut |expression, _, _| {
                let Some(borrow) = program.expression_table.borrow_expression(expression) else {
                    return;
                };
                let Some(place) = place_at(program, state.symbol, statement_index, borrow.target)
                else {
                    return;
                };
                if let facts::PlaceRoot::Expression(expression) = place.root
                    && !backings.contains(&expression)
                    && !current.iter().any(|loan| loan.storage_owner == place.root)
                {
                    backings.push(expression);
                }
            },
        );
        for expression in backings {
            let Some(type_reference) = validation::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) else {
                continue;
            };
            if validation::reference_shell(program, type_reference).is_some()
                || !is_borrow_carrying_data(program, type_reference)
            {
                continue;
            }
            let backing = checked_trees::statement::TableLocalData {
                role: language_core::LocalBindingRole::Value,
                symbol: owner,
                name: name.clone(),
                type_reference,
                initial_value: expression,
                is_mutable: false,
                type_is_inferred: false,
                is_operand_capture: false,
                relevance: language_core::BindingRelevance::Relevant,
                reflected_query_origin: None,
            };
            let contents = borrow_carrying_data_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                &backing,
                current,
                &sources,
            )?;
            pending.extend(contents.into_iter().map(|mut loan| {
                loan.storage_owner = facts::PlaceRoot::Expression(expression);
                loan
            }));
        }
        Ok(())
    }

    /*
        The initializer expressions that supply the references structurally carried
        by `type_reference`. Persistent-storage checking uses this same traversal so
        its static-source exemption cannot disagree with local aggregate loan
        attribution about which nested fields actually borrow.
    */
    pub(crate) fn borrow_initializer_expressions(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        expression: ExpressionHandle,
    ) -> Vec<ExpressionHandle> {
        borrowed_initializers(program, type_reference, expression, &[], &[])
            .into_iter()
            .map(|initializer| initializer.expression)
            .collect()
    }

    /*
        Form normal result leaves through the ordinary binding operation while the
        state's current incoming and established descriptors are available. The
        result owner is only a path namespace; it creates no local activation.
    */
    pub(super) fn returned_value_loans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        loan_trackers: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<
        Vec<(
            checked_trees::statement::TransitionTargetHandle,
            StatementBorrowLoan,
        )>,
        &'static str,
    > {
        use checked_trees::statement::{TransitionExit, TransitionTargetNode};
        let sources = BorrowFormationSources {
            statement_entry: loan_trackers,
            call_frames,
        };

        if !is_reference_type(program, state.return_type)
            && !is_borrow_carrying_data(program, state.return_type)
        {
            return Ok(Vec::new());
        }
        let returned_value = checked_trees::statement::TableLocalData {
            role: language_core::LocalBindingRole::Value,
            symbol: state.symbol,
            name: state.name.clone(),
            type_reference: state.return_type,
            initial_value: ExpressionHandle::invalid(),
            is_mutable: false,
            type_is_inferred: false,
            is_operand_capture: false,
            relevance: language_core::BindingRelevance::Relevant,
            reflected_query_origin: None,
        };
        let expression_loans = |expression| {
            let mut value = returned_value.clone();
            value.initial_value = expression;
            binding_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                &value,
                loan_trackers,
                call_frames,
            )
        };
        let transition = match statement {
            StatementNode::Expression(expression)
                if statement_index + 1
                    == state_statements(program, state.statement_nodes).len() =>
            {
                return Ok(expression_loans(*expression)?
                    .into_iter()
                    .map(|loan| (Default::default(), loan))
                    .collect());
            }
            StatementNode::Transition(transition)
                if transition.exit == TransitionExit::Ordinary =>
            {
                transition
            }
            _ => return Ok(Vec::new()),
        };
        let mut returned = Vec::new();
        for target_handle in transition.targets() {
            let loans = match program.statement_table.transition_target(target_handle) {
                TransitionTargetNode::Value(expression) => expression_loans(*expression)?,
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    use crate::borrow::view_link::{
                        ViewReturnFieldSource, ViewReturnSource, resolve_view_return_source,
                    };

                    let Some((_, target)) = validation::find_state(program, path.symbol)
                        .or_else(|| validation::find_machine_head(program, path.symbol))
                    else {
                        continue;
                    };
                    let fields = match resolve_view_return_source(program, target) {
                        ViewReturnSource::Fields { fields } => fields,
                        ViewReturnSource::Parameter { parameter_index } => {
                            let Some(kind) =
                                reference_borrow_access_kind(program, target.return_type)
                            else {
                                continue;
                            };
                            let Some(parameter) =
                                program.state_parameters(target).get(parameter_index)
                            else {
                                continue;
                            };
                            vec![ViewReturnFieldSource {
                                lifetime_only: false,
                                source:
                                    validation::borrow_contracts::ViewReturnInputSource::Referent,
                                owner_path: Vec::new(),
                                source_path: Vec::new(),
                                source_type: parameter.type_reference,
                                source_reference_hops: 0,
                                parameter_index,
                                kind,
                            }]
                        }
                        ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => {
                            continue;
                        }
                    };
                    aggregate_loans::result_source_loans(
                        program,
                        state.symbol,
                        statement_index,
                        machine_symbol,
                        &returned_value,
                        target.symbol,
                        program.statement_table.expression_handles(*arguments),
                        fields,
                        &[],
                        &[],
                        &sources,
                    )?
                }
                TransitionTargetNode::Terminal => Vec::new(),
            };
            returned.extend(loans.into_iter().map(|loan| (target_handle, loan)));
        }
        Ok(returned)
    }

    pub(super) fn statement_borrow_loans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        loan_trackers: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        replacement: Option<&CarrierReplacement>,
        completed_references: Option<&[StateLoanTracker]>,
    ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
        let mut loans = stored_calls::loans(
            program,
            machine_symbol,
            statement,
            call_frames,
            loan_trackers,
            completed_references,
        )?;
        if let Some(replacement) = replacement {
            loans.retain(|loan| {
                !replacement.replaces(program, loan.storage_owner, &loan.owner_path)
            });
        }
        loans.extend(value_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            statement,
            loan_trackers,
            call_frames,
        )?);
        Ok(loans)
    }

    /*
        Local publication, stored replacement and normal return share one value
        formation. Each source leaf observes the same completed-call prefix; its
        consumer's destination and lifetime remain separate judgments.
    */
    fn binding_borrow_loans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        local_data: &checked_trees::statement::TableLocalData,
        loan_trackers: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
        if let Some(loans) = invocation_results::retain(program, local_data, loan_trackers)? {
            return Ok(loans);
        }
        let sources = BorrowFormationSources {
            statement_entry: loan_trackers,
            call_frames,
        };
        if let checked_trees::expression::ExpressionNode::Match(dispatch) =
            expressions(program).expression(local_data.initial_value)
        {
            let mut loans = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if program.static_type_context_is_unreachable(arm.value) {
                    continue;
                }
                let mut selected = local_data.clone();
                selected.type_reference =
                    program.statically_refined_type_reference(arm.value, selected.type_reference);
                selected.initial_value = arm.value;
                loans.extend(binding_borrow_loans(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    &selected,
                    loan_trackers,
                    call_frames,
                )?);
            }
            return Ok(loans);
        }
        if !is_reference_type(program, local_data.type_reference)
            && is_borrow_carrying_data(program, local_data.type_reference)
        {
            let frames = call_frames.ok_or("value formation has no prepared call-frame context")?;
            let machine = program
                .machine(machine_symbol)
                .ok_or("value formation has no owning machine")?;
            let mut loans = Vec::new();
            for initializer in borrowed_initializers(
                program,
                local_data.type_reference,
                local_data.initial_value,
                &[],
                &[],
            ) {
                let current = match stored_calls::roster_at_expression(
                    program,
                    machine,
                    state,
                    statement_index,
                    initializer.expression,
                    frames,
                    loan_trackers,
                ) {
                    Ok(current) => current,
                    Err(validation::StoredReferenceRelationRefusal::TemplateDependent) => continue,
                    Err(validation::StoredReferenceRelationRefusal::Unavailable(_)) => {
                        return Err("operand has no complete completed call-slot relation");
                    }
                };
                loans.extend(borrowed_initializer_loans(
                    program,
                    state.symbol,
                    statement_index,
                    machine_symbol,
                    local_data,
                    initializer,
                    &current,
                    &sources,
                )?);
            }
            return Ok(loans);
        }
        if !is_reference_type(program, local_data.type_reference)
            && !element_view_local_binding(program, state, machine_symbol, local_data)
        {
            return Ok(Vec::new());
        }
        let frames = call_frames.ok_or("value formation has no prepared call-frame context")?;
        let machine = program
            .machine(machine_symbol)
            .ok_or("value formation has no owning machine")?;
        let current = match stored_calls::roster_at_expression(
            program,
            machine,
            state,
            statement_index,
            local_data.initial_value,
            frames,
            loan_trackers,
        ) {
            Ok(current) => current,
            Err(validation::StoredReferenceRelationRefusal::TemplateDependent) => {
                return Ok(Vec::new());
            }
            Err(validation::StoredReferenceRelationRefusal::Unavailable(_)) => {
                return Err("operand has no complete completed call-slot relation");
            }
        };
        reference_local_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            local_data,
            &current,
            &sources,
        )
    }

    fn value_borrow_loans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        loan_trackers: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
        match statement {
            StatementNode::Assignment(assignment) => assignment_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                assignment,
                loan_trackers,
                call_frames,
            ),
            StatementNode::LocalData(local_data) => binding_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                local_data,
                loan_trackers,
                call_frames,
            ),
            _ => Ok(Vec::new()),
        }
    }

    /*
        A store installs the replacement's loans in its actual carrier. A projected
        store through a local reference belongs to that reference's backing storage,
        so ending the temporary reference cannot end loans installed in the parent.
        These rows retain conflicts within the source state; they do not establish
        outlives authority for transporting local storage into a successor.
    */
    fn assignment_borrow_loans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        machine_symbol: SymbolHandle,
        assignment: &checked_trees::statement::TableAssignment,
        loan_trackers: &[StateLoanTracker],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<Vec<StatementBorrowLoan>, &'static str> {
        let Some(target) = borrow_access_place(
            program,
            state.symbol,
            statement_index,
            assignment.target,
            machine_symbol,
        ) else {
            return Ok(Vec::new());
        };
        let target_symbol = target.root.as_symbol().unwrap_or(state.symbol);
        let owner_name: Identifier = program.symbols.name(target_symbol).into();
        let Some(target_type) = program.machine(machine_symbol).and_then(|machine| {
            validation::assignment_stored_value_type(program, machine, state, assignment)
        }) else {
            return Ok(Vec::new());
        };
        if !is_reference_type(program, target_type)
            && !is_borrow_carrying_data(program, target_type)
        {
            return Ok(Vec::new());
        }
        let synthetic_local = checked_trees::statement::TableLocalData {
            role: language_core::LocalBindingRole::Value,
            symbol: target_symbol,
            name: owner_name,
            type_reference: target_type,
            initial_value: assignment.value,
            is_mutable: true,
            type_is_inferred: false,
            is_operand_capture: false,
            relevance: language_core::BindingRelevance::Relevant,
            reflected_query_origin: None,
        };

        let loans = binding_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
            call_frames,
        )?;
        let writes_referent = validation::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        )
        .is_some_and(|reference| is_reference_type(program, reference))
            && !is_reference_type(program, target_type);
        let destinations = store_destination_places(
            program,
            state,
            target,
            writes_referent,
            loan_trackers.iter().map(|loan| {
                (
                    loan.storage_owner,
                    loan.owner_path.as_slice(),
                    loan.place.incoming_reference,
                    loan.place.incoming_owner_path.as_slice(),
                    loan.place.root,
                    loan.place.segments.as_slice(),
                )
            }),
        );
        let mut installed = Vec::new();
        for destination in &destinations {
            let destination_symbol = destination.root.as_symbol().unwrap_or(target_symbol);
            for mut loan in loans.iter().cloned() {
                let projection = owner_paths::owner_place_segments(&loan.owner_path);
                let segments =
                    facts::project_captured_view_segments(&destination.segments, &projection)
                        .ok_or(
                            "stored reference occurrence lies outside its captured destination",
                        )?;
                loan.owner_symbol = destination_symbol;
                loan.storage_owner = destination.root;
                loan.owner_name = program.symbols.name(destination_symbol).into();
                loan.owner_path = owner_path_from_place_segments(program, &segments);
                installed.push(loan);
            }
        }
        Ok(installed)
    }

    /*
        Reading an input descriptor creates a permission observation, not an owned
        storage home. A copied or constructed carrier has a different storage owner
        and keeps its normal cleanup even when it retains this incoming identity.
    */
    pub(crate) fn is_descriptor_load_observation(
        borrow: &checked_trees::BorrowFacts,
        loan: &checked_trees::BorrowLoanFact,
    ) -> bool {
        borrow.incoming_references.is_valid(loan.incoming_reference)
            && matches!(borrow.incoming_references.get(loan.incoming_reference).origin,
                checked_trees::IncomingReferenceOrigin::DescriptorLoad { expression }
                    if loan.storage_owner == facts::PlaceRoot::Expression(expression)
                        && loan.formation_expression == expression)
    }
}
mod machine_rows {
    /*
        One machine's borrow rows, built on its own arenas and appended to the
        program's in roster order.

        A state's borrow facts read only rows that state wrote: its loan trackers
        start empty, and every arena lookup goes through a handle it minted. So
        machines can be built on separate workers, each from fresh arenas. Appending
        them in roster order reproduces the serial arenas exactly once every handle
        is moved by the number of rows the earlier machines left in its arena. The
        destructuring below names every field, so a new handle-bearing field fails
        to compile here instead of silently keeping a machine-local index.
    */
    use arena::{Arena, Handle, HandleSpan};
    use checked_trees::{
        BorrowArgumentAccessFact, BorrowCallFact, BorrowCallReferenceSlotEffect, BorrowLoanFact,
        BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowReturnedLoanFact, BorrowWritableRootFact,
        IncomingReferenceIdentity, StateBorrowFact,
    };
    use facts::PlaceSegment;

    use super::state::BorrowFactArenas;

    /* Every arena `append_state_borrow_facts` writes, owned by one machine. */
    #[derive(Default)]
    pub(super) struct MachineBorrowRows {
        incoming_references: Arena<IncomingReferenceIdentity>,
        writable_roots: Arena<BorrowWritableRootFact>,
        access_segments: Arena<PlaceSegment>,
        owner_segments: Arena<BorrowLoanOwnerSegment>,
        argument_accesses: Arena<BorrowArgumentAccessFact>,
        calls: Arena<BorrowCallFact>,
        reference_slot_effects: Arena<BorrowCallReferenceSlotEffect>,
        loans: Arena<BorrowLoanFact>,
        loan_suppliers: Arena<Handle<BorrowLoanFact>>,
        returned_loans: Arena<BorrowReturnedLoanFact>,
        states: Arena<StateBorrowFact>,
    }

    impl MachineBorrowRows {
        pub(super) fn arenas(&mut self) -> BorrowFactArenas<'_> {
            BorrowFactArenas {
                incoming_references: &mut self.incoming_references,
                writable_roots: &mut self.writable_roots,
                access_segments: &mut self.access_segments,
                owner_segments: &mut self.owner_segments,
                argument_accesses: &mut self.argument_accesses,
                calls: &mut self.calls,
                reference_slot_effects: &mut self.reference_slot_effects,
                loans: &mut self.loans,
                loan_suppliers: &mut self.loan_suppliers,
                returned_loans: &mut self.returned_loans,
                states: &mut self.states,
            }
        }

        /* Append these rows after everything already in `program`. */
        pub(super) fn append_to(self, program: &mut BorrowFactArenas<'_>) {
            let shift = Shift {
                incoming_references: rows(program.incoming_references),
                writable_roots: rows(program.writable_roots),
                access_segments: rows(program.access_segments),
                owner_segments: rows(program.owner_segments),
                argument_accesses: rows(program.argument_accesses),
                calls: rows(program.calls),
                reference_slot_effects: rows(program.reference_slot_effects),
                loans: rows(program.loans),
                loan_suppliers: rows(program.loan_suppliers),
                returned_loans: rows(program.returned_loans),
            };
            append(
                program.incoming_references,
                self.incoming_references,
                |row| shift.incoming_reference(row),
            );
            append(program.writable_roots, self.writable_roots, |row| row);
            append(program.access_segments, self.access_segments, |row| row);
            append(program.owner_segments, self.owner_segments, |row| row);
            append(program.argument_accesses, self.argument_accesses, |row| {
                shift.argument_access(row)
            });
            append(program.calls, self.calls, |row| shift.call(row));
            append(
                program.reference_slot_effects,
                self.reference_slot_effects,
                |row| shift.reference_slot_effect(row),
            );
            append(program.loans, self.loans, |row| shift.loan(row));
            append(program.loan_suppliers, self.loan_suppliers, |row| {
                shift.handle(row, shift.loans)
            });
            append(program.returned_loans, self.returned_loans, |row| {
                shift.returned_loan(row)
            });
            append(program.states, self.states, |row| shift.state(row));
        }
    }

    fn rows<T: Default>(arena: &Arena<T>) -> u32 {
        u32::try_from(arena.len()).expect("borrow arena exceeds handle range")
    }

    fn append<T: Default>(
        program: &mut Arena<T>,
        machine: Arena<T>,
        mut shift: impl FnMut(T) -> T,
    ) {
        for row in machine.into_items() {
            program.append(shift(row));
        }
    }

    /* Rows already in each program arena before this machine's. */
    struct Shift {
        incoming_references: u32,
        writable_roots: u32,
        access_segments: u32,
        owner_segments: u32,
        argument_accesses: u32,
        calls: u32,
        reference_slot_effects: u32,
        loans: u32,
        loan_suppliers: u32,
        returned_loans: u32,
    }

    impl Shift {
        /* The invalid handle stays invalid: it means "no row", not row zero. */
        fn handle<T>(&self, handle: Handle<T>, rows: u32) -> Handle<T> {
            if !handle.is_valid() {
                return handle;
            }
            Handle::from_parts(
                handle
                    .arena_index()
                    .checked_add(rows)
                    .expect("borrow arena exceeds handle range"),
                handle.generation(),
            )
        }

        fn span<T>(&self, span: HandleSpan<T>, rows: u32) -> HandleSpan<T> {
            if !span.start().is_valid() {
                return span;
            }
            HandleSpan::from_parts(self.handle(span.start(), rows), span.count())
        }

        fn lineage(&self, lineage: BorrowLoanLineage) -> BorrowLoanLineage {
            match lineage {
                BorrowLoanLineage::Reborrow { parent_loan } => BorrowLoanLineage::Reborrow {
                    parent_loan: self.handle(parent_loan, self.loans),
                },
                other @ (BorrowLoanLineage::DirectRoot
                | BorrowLoanLineage::UnretainedDerived
                | BorrowLoanLineage::SuppliedOutput
                | BorrowLoanLineage::LifetimeDependency) => other,
            }
        }

        fn incoming_reference(&self, row: IncomingReferenceIdentity) -> IncomingReferenceIdentity {
            let IncomingReferenceIdentity {
                machine_symbol,
                state_symbol,
                origin,
                carrier_type,
                owner_path,
            } = row;
            IncomingReferenceIdentity {
                machine_symbol,
                state_symbol,
                origin,
                carrier_type,
                owner_path: self.span(owner_path, self.owner_segments),
            }
        }

        fn argument_access(&self, row: BorrowArgumentAccessFact) -> BorrowArgumentAccessFact {
            let BorrowArgumentAccessFact {
                expression,
                evaluation_start,
                root,
                segments,
                kind,
                end_before_access,
                released_at,
            } = row;
            BorrowArgumentAccessFact {
                expression,
                evaluation_start,
                root,
                segments: self.span(segments, self.access_segments),
                kind,
                end_before_access,
                released_at,
            }
        }

        fn call(&self, row: BorrowCallFact) -> BorrowCallFact {
            let BorrowCallFact {
                statement_index,
                call_ordinal,
                authored_expression,
                input_reference_loans,
                reference_slot_effects,
                completed_reference_loans,
                receiver_symbol,
                target_symbol,
                has_receiver,
                accesses,
            } = row;
            BorrowCallFact {
                statement_index,
                call_ordinal,
                authored_expression,
                input_reference_loans: input_reference_loans
                    .map(|span| self.span(span, self.loan_suppliers)),
                reference_slot_effects: self
                    .span(reference_slot_effects, self.reference_slot_effects),
                completed_reference_loans: self.span(completed_reference_loans, self.loans),
                receiver_symbol,
                target_symbol,
                has_receiver,
                accesses: self.span(accesses, self.argument_accesses),
            }
        }

        fn reference_slot_effect(
            &self,
            row: BorrowCallReferenceSlotEffect,
        ) -> BorrowCallReferenceSlotEffect {
            let BorrowCallReferenceSlotEffect {
                root,
                segments,
                owner_path,
                may_select,
            } = row;
            BorrowCallReferenceSlotEffect {
                root,
                segments: self.span(segments, self.access_segments),
                owner_path: self.span(owner_path, self.owner_segments),
                may_select,
            }
        }

        fn loan(&self, row: BorrowLoanFact) -> BorrowLoanFact {
            let BorrowLoanFact {
                formation,
                incoming_reference,
                incoming_owner_path,
                representation_address,
                statement_index,
                established_by_call,
                formation_expression,
                formation_owner_path,
                retained_operand,
                last_use_statement_index,
                owner_symbol,
                storage_owner,
                owner_path,
                source_owner_symbol,
                supplying_loans,
                retained_input_lifetimes,
                lineage,
                root,
                segments,
                kind,
            } = row;
            BorrowLoanFact {
                formation,
                incoming_reference: self.handle(incoming_reference, self.incoming_references),
                incoming_owner_path: self.span(incoming_owner_path, self.access_segments),
                representation_address: self.span(representation_address, self.access_segments),
                statement_index,
                established_by_call: self.handle(established_by_call, self.calls),
                formation_expression,
                formation_owner_path: formation_owner_path
                    .map(|span| self.span(span, self.owner_segments)),
                retained_operand,
                last_use_statement_index,
                owner_symbol,
                storage_owner,
                owner_path: self.span(owner_path, self.owner_segments),
                source_owner_symbol,
                supplying_loans: self.span(supplying_loans, self.loan_suppliers),
                retained_input_lifetimes: retained_input_lifetimes
                    .map(|span| self.span(span, self.loan_suppliers)),
                lineage: self.lineage(lineage),
                root,
                segments: self.span(segments, self.access_segments),
                kind,
            }
        }

        fn returned_loan(&self, row: BorrowReturnedLoanFact) -> BorrowReturnedLoanFact {
            let BorrowReturnedLoanFact {
                lineage,
                retained_operand,
                statement_index,
                transition_target,
                formation_expression,
                formation_owner_path,
                incoming_reference,
                incoming_owner_path,
                root,
                segments,
                owner_path,
                kind,
            } = row;
            BorrowReturnedLoanFact {
                lineage: self.lineage(lineage),
                retained_operand,
                statement_index,
                transition_target,
                formation_expression,
                formation_owner_path: formation_owner_path
                    .map(|span| self.span(span, self.owner_segments)),
                incoming_reference: self.handle(incoming_reference, self.incoming_references),
                incoming_owner_path: self.span(incoming_owner_path, self.access_segments),
                root,
                segments: self.span(segments, self.access_segments),
                owner_path: self.span(owner_path, self.owner_segments),
                kind,
            }
        }

        fn state(&self, row: StateBorrowFact) -> StateBorrowFact {
            let StateBorrowFact {
                machine_symbol,
                state_symbol,
                incoming_references,
                writable_roots,
                mutable_parameter_count,
                calls,
                loans,
                returned_loans,
            } = row;
            StateBorrowFact {
                machine_symbol,
                state_symbol,
                incoming_references: self.span(incoming_references, self.incoming_references),
                writable_roots: self.span(writable_roots, self.writable_roots),
                mutable_parameter_count,
                calls: self.span(calls, self.calls),
                loans: self.span(loans, self.loans),
                returned_loans: self.span(returned_loans, self.returned_loans),
            }
        }
    }
}
mod owner_geometry {
    /*
        Translate nominal loan owner paths to the checked structural namespace.
        Prepared occurrences can own different field and case symbols. Their exact
        constructor correspondence selects one slot and retains its type and region
        operands; following another declaration would lose the next nested field.
    */
    use checked_trees::{BorrowLoanOwnerSegment, CheckedUnitStructuralPathSegment};
    use typed_trees::data::DataMember;

    pub(crate) use validation::borrow_contracts::structural_owner_path;

    /*
        A named formal presents its referee as borrowed storage; a named local
        retains its whole stored carrier. Authenticate that declaration before
        translating a loan's owner path. Only the formal's outer pointer is absent
        from the checked parameter shape: nested references remain stored boundaries.
        Canonical sequence descriptors already have their own presentation and do
        not acquire an invented pointer hop. This chooses geometry, not permission.
    */
    pub(crate) fn named_carrier_path(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        owner: symbols::SymbolHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        owner_path: &[BorrowLoanOwnerSegment],
    ) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
        let reference = if let Some(parameter) = program.state_parameter_by_symbol(state, owner) {
            if program.state_local_declaration(state, owner).is_some()
                || program.normalized_type_identity(parameter.type_reference)
                    != program.normalized_type_identity(reference)
            {
                return None;
            }
            validation::reference_result_custody::stored_loan_parts(
                program,
                parameter.type_reference,
            )
            .map_or(reference, |(referee, _)| referee)
        } else {
            reference
        };
        carrier_path(program, reference, owner_path)
    }

    /*
        Translate an occurrence's symbolic owner path through its declarations.
        Common fields and active payload fields keep distinct nominal namespaces.
        A stored descriptor can belong to a borrowed containing record or array.
        Owner selectors name that declared storage, so unwrap its reference shells
        as `origin_owner_path` does in the reverse direction. This translates an
        already captured loan path; it neither forms a borrow nor grants access.
    */
    pub(crate) fn carrier_path(
        program: &dyn ::validation::CheckingInput,
        mut reference: typed_trees::types::TypeReferenceHandle,
        owner_path: &[BorrowLoanOwnerSegment],
    ) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
        let owner_path = validation::corresponding_loan_owner_path(program, reference, owner_path)?;
        let mut output = Vec::with_capacity(owner_path.len());
        let mut payload = None;
        for segment in &owner_path {
            reference = program
                .type_reference_table
                .peeled_type_reference(reference);
            match segment {
                BorrowLoanOwnerSegment::FixedIndex(element_index) => {
                    let typed_trees::types::TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } = program.type_reference_table.type_reference(reference)
                    else {
                        return None;
                    };
                    let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                        return None;
                    };
                    if element_index >= length || payload.is_some() {
                        return None;
                    }
                    output.push(CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(*element_index).ok()?,
                    ));
                    reference = *element_type;
                }
                BorrowLoanOwnerSegment::Case(symbol) => {
                    let (source, _) = program.variant_owner(*symbol)?;
                    let data = program.data_constructor_definition(reference, source.symbol)?;
                    if payload.is_some() {
                        return None;
                    }
                    let variant =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.symbol == *symbol => {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    output.push(CheckedUnitStructuralPathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            variant.identity,
                            variant.name.clone(),
                        ),
                    ));
                    payload = Some(variant);
                }
                BorrowLoanOwnerSegment::Field(symbol) => {
                    let field = if let Some(variant) = payload.take() {
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.symbol == *symbol)?
                    } else {
                        let (source, _) = program.any_field_owner(*symbol)?;
                        let data = program.data_constructor_definition(reference, source.symbol)?;
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Field(field) if field.symbol == *symbol => Some(field),
                                _ => None,
                            })?
                    };
                    output.push(CheckedUnitStructuralPathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ));
                    reference = field.type_reference;
                }
                BorrowLoanOwnerSegment::DynamicIndex => return None,
            }
        }
        payload.is_none().then_some(output)
    }
}
mod roots {
    use checked_trees::statement::StatementNode;
    use checked_trees::{BorrowRootKind, BorrowWritableRootFact};

    pub(super) fn append_state_writable_roots(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        writable_roots: &mut arena::Arena<BorrowWritableRootFact>,
        writable_roots_span: &mut arena::HandleSpan<BorrowWritableRootFact>,
    ) {
        for owned in program.machine_owned_data(machine) {
            writable_roots.append_to_span(
                writable_roots_span,
                BorrowWritableRootFact {
                    symbol: owned.symbol,
                    kind: BorrowRootKind::OwnedData,
                },
            );
        }

        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(local_data) = statement.as_local_data() else {
                continue;
            };
            writable_roots.append_to_span(
                writable_roots_span,
                BorrowWritableRootFact {
                    symbol: local_data.symbol,
                    kind: BorrowRootKind::LocalData,
                },
            );
        }

        for parameter in program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_mutable)
        {
            writable_roots.append_to_span(
                writable_roots_span,
                BorrowWritableRootFact {
                    symbol: parameter.symbol,
                    kind: BorrowRootKind::MutableParameter,
                },
            );
        }
    }

    pub(super) fn mutable_parameter_count(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> usize {
        program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_mutable)
            .count()
    }

    pub(super) fn estimated_borrow_root_capacity(
        program: &dyn ::validation::CheckingInput,
    ) -> usize {
        program
            .machines()
            .iter()
            .map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .map(|state| {
                        let local_data_count =
                            crate::flow::state_statements(program, state.statement_nodes)
                                .iter()
                                .filter(|statement| {
                                    matches!(statement, StatementNode::LocalData(_))
                                })
                                .count();

                        program.machine_owned_data(machine).len()
                            + local_data_count
                            + mutable_parameter_count(program, state)
                    })
                    .sum::<usize>()
            })
            .sum()
    }
}
mod state {
    use arena::Arena;
    use arena::Handle;
    use arena::HandleSpan;
    use checked_trees::{
        BorrowAccessKind, BorrowCallReferenceSlotEffect, BorrowLoanFact, BorrowLoanLineage,
        BorrowLoanOwnerSegment as OwnerSegment, BorrowReturnedLoanFact, CapturedPlace,
        IncomingReferenceIdentity as ReferenceIdentity, IncomingReferenceOrigin as ReferenceOrigin,
    };
    use checked_trees::{
        BorrowArgumentAccessFact, BorrowCallFact, BorrowWritableRootFact, StateBorrowFact,
    };
    use diagnostics::Diagnostic;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;
    use validation::CallFrameResolver;
    use validation::borrow_contracts::retained_lifetime_owner_paths;
    use validation::peel_declared_constraints;
    use validation::reference_result_custody::declared_carrier_references;
    use validation::reference_result_custody::input_carrier_type;
    use validation::reference_result_custody::origin_place_segments;
    use validation::reference_shell;

    use super::calls::collect_statement_borrow_calls;
    use super::last_uses::update_state_loan_last_uses;
    use super::loans::{StatementBorrowLoan, returned_value_loans, statement_borrow_loans};
    use super::roots::{append_state_writable_roots, mutable_parameter_count};
    use super::tracker::StateLoanTracker;

    pub(super) struct BorrowFactArenas<'arenas> {
        pub(super) incoming_references: &'arenas mut Arena<ReferenceIdentity>,
        pub(super) writable_roots: &'arenas mut Arena<BorrowWritableRootFact>,
        pub(super) access_segments: &'arenas mut Arena<PlaceSegment>,
        pub(super) owner_segments: &'arenas mut Arena<OwnerSegment>,
        pub(super) argument_accesses: &'arenas mut Arena<BorrowArgumentAccessFact>,
        pub(super) calls: &'arenas mut Arena<BorrowCallFact>,
        pub(super) reference_slot_effects: &'arenas mut Arena<BorrowCallReferenceSlotEffect>,
        pub(super) loans: &'arenas mut Arena<BorrowLoanFact>,
        pub(super) loan_suppliers: &'arenas mut Arena<Handle<BorrowLoanFact>>,
        pub(super) returned_loans: &'arenas mut Arena<BorrowReturnedLoanFact>,
        pub(super) states: &'arenas mut Arena<StateBorrowFact>,
    }

    pub(super) fn append_state_borrow_facts(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        arenas: &mut BorrowFactArenas<'_>,
        state_loan_trackers: &mut Vec<StateLoanTracker>,
        call_frames: Option<&CallFrameResolver<'_>>,
    ) -> Result<(), Diagnostic> {
        state_loan_trackers.clear();
        let incoming_references = append_incoming_reference_trackers(
            program,
            machine,
            state,
            arenas,
            state_loan_trackers,
        );
        let mut current_loans = state_loan_trackers.clone();
        let mut writable_roots_span = HandleSpan::empty();
        append_state_writable_roots(
            program,
            machine,
            state,
            arenas.writable_roots,
            &mut writable_roots_span,
        );

        let mut calls_span = HandleSpan::empty();
        let mut loans_span = HandleSpan::empty();
        let mut returned_loans_span = HandleSpan::empty();
        for (statement_index, statement) in program
            .statement_table
            .admitted_execution_statements(state.statement_nodes)
        {
            let statement_entry_loans = current_loans.clone();
            let mut backing_loans = Vec::new();
            super::loans::retain_backing_contents(
                program,
                state,
                statement_index,
                machine.symbol,
                statement,
                &mut backing_loans,
                &current_loans,
                call_frames,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "machine `{}` statement {statement_index}: {reason}",
                    machine.name
                ))
            })?;
            super::loans::prepare_descriptor_loads(
                program,
                machine,
                state,
                statement_index,
                statement,
                &current_loans,
                &mut backing_loans,
            );
            append_carrier_loans(
                arenas,
                &mut loans_span,
                &mut current_loans,
                state_loan_trackers,
                backing_loans,
                statement_index,
                Default::default(),
            );
            let replacement = super::loans::capture_replacement(
                program,
                machine,
                state,
                statement_index,
                statement,
                call_frames,
                &current_loans,
            );
            let mut call_ordinal = 0usize;
            collect_statement_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                statement,
                &mut call_ordinal,
                arenas.access_segments,
                arenas.argument_accesses,
                arenas.calls,
                &mut calls_span,
            );
            let completed_references = super::calls::append_reference_custody(
                program,
                machine,
                state,
                statement_index,
                call_frames,
                arenas,
                calls_span,
                &mut loans_span,
                state_loan_trackers,
                &statement_entry_loans,
            )?;
            if let Some(completed) = &completed_references {
                current_loans.extend(
                    completed
                        .iter()
                        .filter(|tracker| {
                            let row = arenas.loans.get(tracker.handle);
                            row.statement_index == statement_index
                                && matches!(row.storage_owner, PlaceRoot::Expression(expression)
                            if expression == row.formation_expression)
                        })
                        .cloned(),
                );
            }
            for (transition_target, mut returned) in returned_value_loans(
                program,
                state,
                statement_index,
                machine.symbol,
                statement,
                &current_loans,
                call_frames,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "machine `{}` state `{}` statement {}: {reason}",
                    machine.name, state.name, statement_index,
                ))
            })? {
                capture_incoming_origin(arenas, &mut returned);
                let segments = arenas.access_segments.insert_many(returned.place.segments);
                let formation_owner_path = returned
                    .formation_owner_path
                    .map(|path| arenas.owner_segments.insert_many(path));
                let owner_path = arenas.owner_segments.insert_many(returned.owner_path);
                arenas.returned_loans.append_to_span(
                    &mut returned_loans_span,
                    BorrowReturnedLoanFact {
                        lineage: returned.lineage,
                        retained_operand: returned.retained_operand,
                        statement_index,
                        transition_target,
                        formation_expression: returned.formation_expression,
                        formation_owner_path,
                        incoming_reference: returned.place.incoming_reference,
                        incoming_owner_path: arenas
                            .access_segments
                            .insert_many(returned.place.incoming_owner_path),
                        root: returned.place.root,
                        segments,
                        owner_path,
                        kind: returned.kind,
                    },
                );
            }
            let pending_loans = statement_borrow_loans(
                program,
                state,
                statement_index,
                machine.symbol,
                statement,
                &current_loans,
                call_frames,
                replacement.as_ref(),
                completed_references.as_deref(),
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "machine `{}` state `{}` statement {}: {reason}",
                    machine.name, state.name, statement_index,
                ))
            })?;
            retire_replaced_carrier_loans(
                program,
                machine,
                statement,
                call_frames,
                replacement.as_ref(),
                &mut current_loans,
            );
            append_carrier_loans(
                arenas,
                &mut loans_span,
                &mut current_loans,
                state_loan_trackers,
                pending_loans,
                statement_index,
                Default::default(),
            );
        }

        update_state_loan_last_uses(
            program,
            state.symbol,
            state.statement_nodes,
            arenas.calls.span_or_empty(calls_span),
            arenas.access_segments,
            arenas.argument_accesses,
            state_loan_trackers,
            arenas.loans,
            arenas.loan_suppliers,
        );

        arenas.states.append(StateBorrowFact {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            incoming_references,
            writable_roots: writable_roots_span,
            mutable_parameter_count: mutable_parameter_count(program, state),
            calls: calls_span,
            loans: loans_span,
            returned_loans: returned_loans_span,
        });
        Ok(())
    }

    /*
        Local, stored and returned carriers publish the same invocation permission.
        Leaving a pending origin on a returned row would turn its external referent
        into a temporary descriptor home during the independent escape check.
    */
    fn capture_incoming_origin(arenas: &mut BorrowFactArenas<'_>, loan: &mut StatementBorrowLoan) {
        if let Some(pending) = loan.incoming_origin.take() {
            let mut incoming = pending.identity;
            let existing = matches!(incoming.origin, ReferenceOrigin::Output { .. })
                .then(|| {
                    arenas
                        .incoming_references
                        .iter()
                        .find_map(|(handle, prior)| {
                            (prior.machine_symbol == incoming.machine_symbol
                                && prior.state_symbol == incoming.state_symbol
                                && prior.origin == incoming.origin
                                && prior.carrier_type == incoming.carrier_type
                                && arenas.owner_segments.span(prior.owner_path)
                                    == Some(pending.owner_path.as_slice()))
                            .then_some(handle)
                        })
                })
                .flatten();
            let identity = if let Some(identity) = existing {
                identity
            } else {
                incoming.owner_path = arenas.owner_segments.insert_many(pending.owner_path);
                arenas.incoming_references.append(incoming)
            };
            loan.place.incoming_reference = identity;
            if let Some(address) = &mut loan.representation_address {
                address.incoming_reference = identity;
            }
        }
    }

    pub(super) fn append_carrier_loans(
        arenas: &mut BorrowFactArenas<'_>,
        loans_span: &mut HandleSpan<BorrowLoanFact>,
        current: &mut Vec<StateLoanTracker>,
        history: &mut Vec<StateLoanTracker>,
        pending: Vec<StatementBorrowLoan>,
        statement_index: usize,
        established_by_call: Handle<BorrowCallFact>,
    ) -> HandleSpan<BorrowLoanFact> {
        let first = loans_span.count();
        for mut pending in grouped_carrier_loans(pending) {
            capture_incoming_origin(arenas, &mut pending);
            let segments = arenas
                .access_segments
                .insert_many(pending.place.segments.clone());
            if pending
                .representation_address
                .as_ref()
                .is_some_and(|address| {
                    address.root != pending.place.root
                        || address.incoming_reference != pending.place.incoming_reference
                        || address.incoming_owner_path != pending.place.incoming_owner_path
                })
            {
                pending.representation_address = None;
            }
            let representation_address =
                pending
                    .representation_address
                    .as_ref()
                    .map_or_else(Default::default, |address| {
                        arenas
                            .access_segments
                            .insert_many(address.segments.iter().copied())
                    });
            let owner_path = arenas
                .owner_segments
                .insert_many(pending.owner_path.iter().copied());
            let mut permission_sources = Vec::new();
            if pending.lineage == checked_trees::BorrowLoanLineage::UnretainedDerived
                && !pending.supplying_loans.is_empty()
            {
                let mut complete = true;
                for supplier in &pending.supplying_loans {
                    let Some(source) = history.iter().find(|source| source.handle == *supplier)
                    else {
                        complete = false;
                        break;
                    };
                    if source.owner_symbol != pending.source_owner_symbol
                        || source.kind.direct_reborrow_effect(&pending.kind).is_none()
                        || source.permission_sources.is_empty()
                    {
                        complete = false;
                        break;
                    }
                    for parent in &source.permission_sources {
                        crate::shared::collections::push_unique(
                            &mut permission_sources,
                            parent.clone(),
                        );
                    }
                }
                if !complete {
                    permission_sources.clear();
                }
            }
            let handle = arenas.loans.append_to_span(
                loans_span,
                BorrowLoanFact {
                    formation: pending.formation,
                    incoming_reference: pending.place.incoming_reference,
                    incoming_owner_path: arenas
                        .access_segments
                        .insert_many(pending.place.incoming_owner_path.clone()),
                    representation_address,
                    statement_index,
                    established_by_call,
                    formation_expression: pending.formation_expression,
                    formation_owner_path: pending
                        .formation_owner_path
                        .as_ref()
                        .map(|path| arenas.owner_segments.insert_many(path.iter().copied())),
                    retained_operand: pending.retained_operand,
                    last_use_statement_index: statement_index,
                    owner_symbol: pending.owner_symbol,
                    storage_owner: pending.storage_owner,
                    owner_path,
                    source_owner_symbol: pending.source_owner_symbol,
                    supplying_loans: arenas.loan_suppliers.insert_many(pending.supplying_loans),
                    retained_input_lifetimes: None,
                    lineage: pending.lineage.clone(),
                    root: pending.place.root,
                    segments,
                    kind: pending.kind.clone(),
                },
            );
            let completed_return = pending.lineage == checked_trees::BorrowLoanLineage::DirectRoot
                && arenas
                    .incoming_references
                    .is_valid(pending.place.incoming_reference)
                && matches!(arenas.incoming_references.get(pending.place.incoming_reference).origin,
                    ReferenceOrigin::Invocation { expression, .. }
                        if expression == pending.formation_expression
                            && pending.place.root == PlaceRoot::Expression(expression));
            if matches!(
                pending.lineage,
                checked_trees::BorrowLoanLineage::DirectRoot
                    | checked_trees::BorrowLoanLineage::Reborrow { .. }
            ) && (pending.formation != super::loans::BorrowFormation::ReturnedDescriptor
                || completed_return)
            {
                permission_sources.push(super::tracker::RetainedBorrowSource {
                    loan: handle,
                    owner_symbol: pending.owner_symbol,
                });
            }
            let tracker = StateLoanTracker {
                permission_sources,
                handle,
                owner_symbol: pending.owner_symbol,
                storage_owner: pending.storage_owner,
                owner_name: pending.owner_name,
                kind: pending.kind,
                lineage: pending.lineage,
                formation: pending.formation,
                formation_expression: pending.formation_expression,
                formation_owner_path: pending.formation_owner_path,
                owner_path: pending.owner_path,
                place: pending.place,
                representation_address: pending.representation_address,
                retained_operand: pending.retained_operand,
            };
            current.push(tracker.clone());
            history.push(tracker);
        }
        let count = loans_span.count() - first;
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(loans_span.handle_at(first), count)
        }
    }

    /*
        Arena layout groups one descriptor's alternative origins contiguously.
        This is allocation order, not execution order: formation expressions and
        statement boundaries still name each event. Repeated declared candidates
        form a set, so forwarding a group cannot multiply identical obligations.
    */
    fn grouped_carrier_loans(mut loans: Vec<StatementBorrowLoan>) -> Vec<StatementBorrowLoan> {
        loans.sort_by_cached_key(|loan| {
            (
                loan.owner_symbol.arena_index(),
                loan.owner_symbol.generation(),
                match loan.storage_owner {
                    PlaceRoot::Symbol(symbol) => (0u8, symbol.arena_index(), symbol.generation()),
                    PlaceRoot::Expression(expression) => {
                        (1u8, expression.arena_index(), expression.generation())
                    }
                    PlaceRoot::TypeReference(reference) => {
                        (2u8, reference.arena_index(), reference.generation())
                    }
                    PlaceRoot::Unknown => (3u8, 0, 0),
                    PlaceRoot::InvocationOutput(identity) => (
                        5u8,
                        identity.state_symbol.arena_index(),
                        identity.state_symbol.generation(),
                    ),
                    PlaceRoot::InvocationReferent { parameter, .. } => {
                        (4u8, parameter.arena_index(), parameter.generation())
                    }
                },
                loan.owner_path
                    .iter()
                    .map(|segment| match segment {
                        OwnerSegment::Field(symbol) => {
                            (0u8, symbol.arena_index() as u64, symbol.generation())
                        }
                        OwnerSegment::Case(symbol) => {
                            (1u8, symbol.arena_index() as u64, symbol.generation())
                        }
                        OwnerSegment::FixedIndex(element) => (2u8, *element as u64, 0),
                        OwnerSegment::DynamicIndex => (3u8, 0, 0),
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let mut grouped: Vec<StatementBorrowLoan> = Vec::with_capacity(loans.len());
        for loan in loans {
            let duplicate = grouped
                .iter_mut()
                .rev()
                .take_while(|previous| {
                    previous.owner_symbol == loan.owner_symbol
                        && previous.owner_path == loan.owner_path
                })
                .find(|previous| {
                    previous.incoming_origin == loan.incoming_origin
                        && previous.formation_expression == loan.formation_expression
                        && previous.formation_owner_path == loan.formation_owner_path
                        && previous.storage_owner == loan.storage_owner
                        && previous.source_owner_symbol == loan.source_owner_symbol
                        && previous.supplying_loans.is_empty() == loan.supplying_loans.is_empty()
                        && (previous.supplying_loans == loan.supplying_loans
                            || !loan.owner_path.contains(&OwnerSegment::DynamicIndex))
                        && previous.place == loan.place
                        && previous.representation_address == loan.representation_address
                        && previous.retained_operand == loan.retained_operand
                        && previous.kind == loan.kind
                        && previous.lineage == loan.lineage
                        && previous.formation == loan.formation
                });
            if let Some(previous) = duplicate {
                for supplier in loan.supplying_loans {
                    crate::shared::collections::push_unique(
                        &mut previous.supplying_loans,
                        supplier,
                    );
                }
            } else {
                grouped.push(loan);
            }
        }
        grouped
    }

    /*
        A write through a reference retains its descriptor. Replacing a carrier
        removes only the old leaves within that exact destination; earlier facts
        remain available to the flow lifetime ledger and already detached values.
    */
    fn retire_replaced_carrier_loans(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        statement: &StatementNode,
        call_frames: Option<&CallFrameResolver<'_>>,
        replacement: Option<&super::loans::CarrierReplacement>,
        current: &mut Vec<StateLoanTracker>,
    ) {
        if let Some(effects) = call_frames.and_then(|frames| {
            frames
                .stored_reference_fallthrough_effects(machine, statement)
                .ok()
        }) {
            current.retain(|loan| {
                !effects.iter().any(|effect| {
                    !effect.may_select
                        && loan.storage_owner == PlaceRoot::Symbol(effect.destination.local_symbol)
                        && super::loans::place_path_covers_owner(
                            program,
                            &effect.destination.local_segments,
                            &loan.owner_path,
                        )
                })
            });
        }
        if let Some(replacement) = replacement {
            current.retain(|loan| {
                !replacement.replaces(program, loan.storage_owner, &loan.owner_path)
            });
        }
    }

    /*
        Entry permissions precede statement loans: carrying a descriptor into this
        invocation did not borrow its containing slot. Only a later establishment
        gets a loan row and activation; transfers retain the entry's opaque identity.
    */
    fn append_incoming_reference_trackers(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        arenas: &mut BorrowFactArenas<'_>,
        trackers: &mut Vec<StateLoanTracker>,
    ) -> HandleSpan<ReferenceIdentity> {
        let mut incoming = HandleSpan::empty();
        for parameter in program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_const)
        {
            let carrier = input_carrier_type(program, parameter.type_reference);
            if !parameter.relevance.is_erased()
                && let Some(carrier) = carrier
                && let Some(paths) = retained_lifetime_owner_paths(program, carrier)
            {
                for owner_path in paths {
                    let path = arenas
                        .owner_segments
                        .insert_many(owner_path.iter().copied());
                    let incoming_reference = arenas.incoming_references.append(ReferenceIdentity {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        origin: ReferenceOrigin::Parameter {
                            symbol: parameter.symbol,
                        },
                        carrier_type: carrier,
                        owner_path: path,
                    });
                    trackers.push(StateLoanTracker {
                        handle: Handle::invalid(),
                        owner_symbol: parameter.symbol,
                        storage_owner: PlaceRoot::Symbol(parameter.symbol),
                        owner_name: parameter.name.clone(),
                        kind: BorrowAccessKind::Read,
                        lineage: BorrowLoanLineage::LifetimeDependency,
                        representation_address: None,
                        formation: super::loans::BorrowFormation::Ordinary,
                        formation_expression: Default::default(),
                        formation_owner_path: None,
                        permission_sources: Vec::new(),
                        owner_path,
                        retained_operand: None,
                        place: CapturedPlace {
                            incoming_reference,
                            incoming_owner_path: Vec::new(),
                            root: PlaceRoot::Symbol(parameter.symbol),
                            segments: Vec::new(),
                        },
                    });
                }
            }
            let parameter_type = peel_declared_constraints(program, parameter.type_reference);
            if reference_shell(program, parameter_type).is_some() {
                arenas.incoming_references.append_to_span(
                    &mut incoming,
                    ReferenceIdentity {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        origin: ReferenceOrigin::ParameterReferent {
                            symbol: parameter.symbol,
                        },
                        carrier_type: parameter_type,
                        owner_path: HandleSpan::empty(),
                    },
                );
            }
            if parameter.relevance.is_erased() {
                continue;
            }
            let Some(carrier) = carrier else {
                continue;
            };
            let Some(sources) = declared_carrier_references(program, carrier) else {
                continue;
            };
            for source in sources {
                let Some(segments) =
                    origin_place_segments(program, carrier, source.path.iter().cloned())
                else {
                    continue;
                };
                let owner_path = super::owner_path_from_place_segments(program, &segments);
                let Some(kind) = source.access.borrow_access_kind() else {
                    continue;
                };
                let path = arenas
                    .owner_segments
                    .insert_many(owner_path.iter().copied());
                let incoming_reference = arenas.incoming_references.append_to_span(
                    &mut incoming,
                    ReferenceIdentity {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        origin: ReferenceOrigin::Parameter {
                            symbol: parameter.symbol,
                        },
                        carrier_type: carrier,
                        owner_path: path,
                    },
                );
                trackers.push(StateLoanTracker {
                    representation_address: None,
                    handle: Handle::invalid(),
                    owner_symbol: parameter.symbol,
                    storage_owner: PlaceRoot::Symbol(parameter.symbol),
                    owner_name: parameter.name.clone(),
                    kind,
                    lineage: BorrowLoanLineage::UnretainedDerived,
                    formation: super::loans::BorrowFormation::Ordinary,
                    formation_expression: Default::default(),
                    formation_owner_path: None,
                    permission_sources: Vec::new(),
                    owner_path,
                    retained_operand: None,
                    place: CapturedPlace {
                        incoming_reference,
                        incoming_owner_path: Vec::new(),
                        root: PlaceRoot::Symbol(parameter.symbol),
                        segments: Vec::new(),
                    },
                });
            }
        }
        incoming
    }
}
pub(crate) mod supplied_outputs {
    /*
        A helper's completed output permits use of the pointer it installed.
        Keep that event on the saved value, independently of the variable that
        later holds it. Original input restrictions are constructed separately;
        they cannot provide the pointer's permission or address.
    */
    use arena::Handle;
    use checked_trees::{BorrowFacts, BorrowLoanFact, FlowCallFact, FlowFacts, FlowOperandFact};

    /*
        The namespace is established only by this exact call's normal exit.
        Later uses separately require a current holder or a captured descendant.
    */
    pub(crate) fn normal_call(
        borrow: &BorrowFacts,
        flow: &FlowFacts,
        loan: Handle<BorrowLoanFact>,
    ) -> Option<Handle<FlowCallFact>> {
        let source = borrow.supplied_output_call(loan)?;
        let invocation = borrow.calls.get(source);
        let incoming = borrow
            .incoming_references
            .get(borrow.loans.get(loan).incoming_reference);
        let state = flow
            .control
            .unique_state(incoming.machine_symbol, incoming.state_symbol)?;
        let mut calls = state.calls.handles().filter(|handle| {
            let actual = flow.control.calls.get(*handle);
            actual.statement_index == invocation.statement_index
                && actual.call_ordinal == invocation.call_ordinal
                && actual.target_symbol == invocation.target_symbol
                && actual.authored_expression == invocation.authored_expression
                && flow
                    .borrow_loan_constraints(actual.exit_constraints)
                    .any(|live| live == loan)
        });
        let actual = calls.next()?;
        calls.next().is_none().then_some(actual)
    }

    /*
        Read the supplied permission attached to this produced descriptor.
        This query precedes resource installation: it proves the completed grant,
        while the resource constructor must still prove every input lifetime and
        the complete holder/cohort end before the program is admitted.
    */
    pub(crate) fn captured_permission(
        program: &dyn validation::CheckingInput,
        borrow: &BorrowFacts,
        flow: &FlowFacts,
        capture: &FlowOperandFact,
    ) -> Result<Handle<BorrowLoanFact>, &'static str> {
        let checked_trees::FlowDescriptorPermission::SuppliedOutput { loan } =
            capture.descriptor_permission
        else {
            return Err("the descriptor has no supplied-output permission");
        };
        if !borrow.loans.is_valid(loan)
            || borrow.loans.get(loan).lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
        {
            return Err("the produced output has no original supplied loan");
        }
        let call = normal_call(borrow, flow, loan)
            .ok_or("the produced output has no exact normal completion")?;
        let invocation = flow.control.calls.get(call);
        if capture.statement_index != invocation.statement_index
            || capture.storage_snapshot != invocation.invocation_storage
        {
            return Err("the produced output belongs to another capture cut");
        }
        let row = borrow.loans.get(loan);
        let incoming = borrow.incoming_references.get(row.incoming_reference);
        let (_, owner) = borrow
            .first_state_borrow(incoming.machine_symbol, incoming.state_symbol)
            .ok_or("the produced output has no borrowing owner")?;
        if !borrow.state_owns_loan(owner, loan) {
            return Err("the produced output carries another state's permission");
        }
        let state = flow
            .control
            .unique_state(incoming.machine_symbol, incoming.state_symbol)
            .ok_or("the produced output has no unique owning state")?;
        if capture.state_symbol != state.state_symbol
            || !state.calls.handles().any(|member| member == call)
        {
            return Err("the produced output permission belongs to another state");
        }
        let path = super::owner_place_segments(
            borrow
                .owner_segments
                .span(incoming.owner_path)
                .ok_or("the produced output lost its original field path")?,
        );
        let supplied =
            validation::project_type_reference_from_segments(program, incoming.carrier_type, &path)
                .and_then(|reference| validation::reference_shell(program, reference))
                .ok_or("the produced output has no declared reference leaf")?;
        let presented = validation::reference_shell(program, capture.type_reference)
            .ok_or("the produced output capture is not a reference")?;
        if supplied.access != presented.access
            || checked_trees::BorrowAccessKind::from(presented.access) != row.kind
            || !validation::storage_origin_types_match(program, supplied.referee, presented.referee)
        {
            return Err("the produced output differs from its checked reference type or access");
        }
        Ok(loan)
    }
}
mod tracker {
    use arena::Handle;
    use checked_trees::name::Identifier;
    use symbols::SymbolHandle;

    use super::accesses::BorrowAccessPlace;

    pub(crate) use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

    /*
        Nearest borrowing occurrence whose permission a descriptor transports.
        A copy carries this association; it does not become another borrowing parent.
    */
    #[derive(Clone, PartialEq, Eq)]
    pub(super) struct RetainedBorrowSource {
        pub(super) loan: Handle<checked_trees::BorrowLoanFact>,
        pub(super) owner_symbol: SymbolHandle,
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(super) struct StateLoanTracker {
        pub(super) handle: Handle<checked_trees::BorrowLoanFact>,
        pub(super) owner_symbol: SymbolHandle,
        pub(super) storage_owner: facts::PlaceRoot,
        pub(super) owner_name: Identifier,
        pub(super) kind: checked_trees::BorrowAccessKind,
        pub(super) lineage: checked_trees::BorrowLoanLineage,
        /*
            A result root supplies no borrowing ancestry on its own. An exact
            descriptor transfer may still carry an earlier borrowing occurrence.
        */
        pub(super) formation: super::loans::BorrowFormation,
        /*
            Value-relative result path at formation. A descriptor loaded from a
            field keeps that field as its source home, while the produced value's
            own reference is at the empty path.
        */
        pub(super) formation_expression: checked_trees::expression::ExpressionHandle,
        pub(super) formation_owner_path: Option<Vec<BorrowOwnerSegment>>,
        pub(super) permission_sources: Vec<RetainedBorrowSource>,
        /*
            Projection within a borrow-carrying owner that holds this loan. Fixed
            array literal positions retain their ordinal; a dynamic later index
            still conservatively matches every ordinal.
        */
        pub(super) owner_path: Vec<BorrowOwnerSegment>,
        pub(super) place: BorrowAccessPlace,
        pub(super) representation_address: Option<BorrowAccessPlace>,
        pub(super) retained_operand: Option<checked_trees::BorrowRetainedOperand>,
    }
}
pub(crate) use validation::borrow_contracts as view_link;

pub(crate) use last_uses::{owner_path_overlaps_place_segments, place_is_used_after_statement};
pub(crate) use loans::argument_borrow_loan_place;
pub(crate) use loans::borrow_initializer_expressions;
pub(crate) use loans::call_reference_lifetime_sources;
pub(crate) use loans::is_descriptor_load_observation;
pub(crate) use loans::owner_path_from_place_segments;
pub(crate) use loans::owner_path_selects_leaf;
pub(crate) use loans::owner_place_segments;
pub(crate) use loans::owner_segment_matches_place_segment;
pub(crate) use loans::place_path_covers_owner;
pub(crate) use loans::returned_body_source_mismatch;
pub(crate) use loans::store_destination_places;
pub(crate) use loans::types::reference_borrow_access_kind;
pub(crate) use loans::{
    CompletedReturnSource, call_reference_sources, completed_call_reference_sources,
};
pub(crate) use owner_geometry::carrier_path;
pub(crate) use owner_geometry::named_carrier_path;
pub(crate) use owner_geometry::structural_owner_path;
pub(crate) use tracker::BorrowOwnerSegment;

use roots::estimated_borrow_root_capacity;
use state::{BorrowFactArenas, append_state_borrow_facts};

/*
    The caller prepares call frames once for this immutable program. `None`
    retains failed preparation; source reconstruction must not retry it per call.
*/
pub(crate) fn build_borrow_facts(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&CallFrameResolver<'_>>,
) -> Result<BorrowFacts, Vec<Diagnostic>> {
    let mut writable_roots = Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut incoming_references = Arena::new();
    let mut access_segments = Arena::with_capacity(program.expression_table.expression_count());
    let mut owner_segments = Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses = Arena::with_capacity(program.expression_table.expression_count());
    let mut calls = Arena::with_capacity(program.statement_table.statement_count());
    let mut reference_slot_effects = Arena::new();
    let mut loans = Arena::with_capacity(program.statement_table.statement_count());
    let mut loan_suppliers = Arena::new();
    let mut returned_loans = Arena::new();
    let mut states = Arena::with_capacity(machine_state_count(program));
    let machines = validation::checking_thread_pool().install(|| {
        program
            .machines()
            .par_iter()
            .map_init(Vec::new, |state_loan_trackers, machine| {
                let mut rows = machine_rows::MachineBorrowRows::default();
                for state in program.machine_states(machine) {
                    append_state_borrow_facts(
                        program,
                        machine,
                        state,
                        &mut rows.arenas(),
                        state_loan_trackers,
                        call_frames,
                    )?;
                }
                Ok(rows)
            })
            .collect::<Vec<Result<_, Diagnostic>>>()
    });

    {
        let mut arenas = BorrowFactArenas {
            incoming_references: &mut incoming_references,
            writable_roots: &mut writable_roots,
            access_segments: &mut access_segments,
            owner_segments: &mut owner_segments,
            argument_accesses: &mut argument_accesses,
            calls: &mut calls,
            reference_slot_effects: &mut reference_slot_effects,
            loans: &mut loans,
            loan_suppliers: &mut loan_suppliers,
            returned_loans: &mut returned_loans,
            states: &mut states,
        };
        for rows in machines {
            rows.map_err(|diagnostic| vec![diagnostic])?
                .append_to(&mut arenas);
        }
    }

    Ok(BorrowFacts::with_roots(
        incoming_references,
        writable_roots,
        access_segments,
        owner_segments,
        argument_accesses,
        calls,
        reference_slot_effects,
        loans,
        loan_suppliers,
        returned_loans,
        states,
    ))
}
