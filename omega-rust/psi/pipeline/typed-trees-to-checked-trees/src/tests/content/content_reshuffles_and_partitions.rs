use super::checked;
use language_semantics::content::{
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
};

#[test]
fn checked_facts_infer_exact_content_reshuffles_through_transparent_paths() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Wrapped { region: Region in Owned; }

        data Main {}
        machine Main::forward(region: Region in Owned) -> Region in Owned {
            region
        }
        machine Main::pack(region: Region in Owned) -> Wrapped {
            Wrapped { region: region }
        }
        machine Main::unpack(wrapped: Wrapped) -> Region in Owned {
            wrapped.region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let reshuffles = &checked.facts.qualifications.content.identity_reshuffles;
    assert_eq!(reshuffles.len(), 3, "identity reshuffles: {reshuffles:#?}");

    let row_for = |state_name: &str| {
        let state_symbol = checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == state_name)
            .expect("named state")
            .symbol;
        reshuffles
            .iter()
            .find(|row| row.state_symbol == state_symbol)
            .expect("inferred reshuffle row")
    };

    let forward = row_for("forward");
    assert_ne!(
        forward.claim_identity,
        language_semantics::PermissionClaimIdentity::Unknown
    );
    assert!(matches!(
        forward.plan.equation.left(),
        ContentConservationTerm::Projection { subject, .. }
            if subject.version == ContentPlaceVersion::Entry
                && subject.segments.is_empty()
    ));
    assert!(matches!(
        forward.plan.equation.right(),
        ContentConservationTerm::Projection { subject, .. }
            if subject.version == ContentPlaceVersion::Current
                && subject.segments.is_empty()
    ));

    let pack = row_for("pack");
    let pack_paths = [pack.plan.equation.left(), pack.plan.equation.right()]
        .into_iter()
        .map(|term| match term {
            ContentConservationTerm::Projection { subject, .. } => subject,
            ContentConservationTerm::Separate(_) => panic!("reshuffles never infer separation"),
        })
        .collect::<Vec<_>>();
    assert!(pack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Entry && subject.segments.is_empty()
    }));
    assert!(pack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Current
            && matches!(subject.segments.as_slice(), [ContentPlaceSegment::Field(field)] if field.name == "region")
    }));

    let unpack = row_for("unpack");
    let unpack_paths = [unpack.plan.equation.left(), unpack.plan.equation.right()]
        .into_iter()
        .map(|term| match term {
            ContentConservationTerm::Projection { subject, .. } => subject,
            ContentConservationTerm::Separate(_) => panic!("reshuffles never infer separation"),
        })
        .collect::<Vec<_>>();
    assert!(unpack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Entry
            && matches!(subject.segments.as_slice(), [ContentPlaceSegment::Field(field)] if field.name == "region")
    }));
    assert!(unpack_paths.iter().any(|subject| {
        subject.version == ContentPlaceVersion::Current && subject.segments.is_empty()
    }));
}

#[test]
fn checked_facts_compose_authored_partitions_through_a_direct_wrapper() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }
        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }

        boundary trait Splitter {
            machine partition(
                left: Region in Owned,
                right: Region in Owned
            ) -> Pair
            ensures
                separate(
                    Owned::content(old(&left)),
                    Owned::content(old(&right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main<'s> { splitter: &'s mut Splitter; }
        machine Main::forward(&mut self, pair: Pair) -> Pair
        reaches Splitter requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            self.splitter.partition(pair.left, pair.right)
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let state_symbol = |name: &str| {
        checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let partition = checked
        .traits()
        .iter()
        .flat_map(|trait_definition| checked.trait_machine_signatures(trait_definition))
        .find(|signature| signature.name.as_str() == "partition")
        .expect("partition requirement")
        .symbol;
    let forward = state_symbol("forward");

    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .conservation_plans
            .len(),
        1,
        "only the primitive authors a source theorem"
    );
    let compositions = &checked.facts.qualifications.content.partition_compositions;
    assert_eq!(
        compositions.len(),
        1,
        "the authored theorem should instantiate through the direct wrapper: {compositions:#?}\nauthored: {:#?}\npermissions: {:#?}",
        checked.facts.qualifications.content.conservation_plans,
        checked.facts.flow.ownership.permissions,
    );
    let forward_row = compositions
        .iter()
        .find(|row| row.state_symbol == forward)
        .expect("direct wrapper composition");
    assert_eq!(forward_row.source_callable, partition);
    assert_eq!(
        forward_row.source_report_fingerprint,
        forward_row.source_plan.report_fingerprint
    );
    assert_eq!(forward_row.call_ordinal, 0);
    assert_eq!(forward_row.input_claim_identities.len(), 2);
    assert_eq!(forward_row.input_claim_bindings.len(), 2);
    assert!(forward_row.input_claim_bindings.iter().all(|binding| {
        binding.entry_place.version == ContentPlaceVersion::Entry
            && matches!(binding.entry_place.root, ContentPlaceRoot::Parameter { .. })
            && forward_row
                .input_claim_identities
                .contains(&binding.claim_identity)
    }));
    assert!(forward_row.result_rewrites.is_empty());
    assert_eq!(
        forward_row.substitutions.len(),
        4,
        "both source inputs and both result fields have exact substitutions"
    );
    assert!(
        forward_row
            .input_claim_identities
            .iter()
            .all(|identity| { *identity != language_semantics::PermissionClaimIdentity::Unknown })
    );

    for row in compositions {
        assert!(matches!(
            row.plan.equation.left(),
            ContentConservationTerm::Separate(children) if children.len() == 2
        ));
        assert!(matches!(
            row.plan.equation.right(),
            ContentConservationTerm::Separate(children) if children.len() == 2
        ));
    }
}

#[test]
fn checked_facts_compose_partitions_through_exact_staged_result_rewrites() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }
        data Envelope { pair: Pair; }
        data Double { first: Pair; second: Pair; }

        boundary trait Splitter {
            machine partition(
                left: Region in Owned,
                right: Region in Owned
            ) -> Pair
            ensures
                separate(
                    Owned::content(old(&left)),
                    Owned::content(old(&right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        boundary trait PairSplitter {
            machine partition(pair: Pair) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair.left)),
                    Owned::content(old(&pair.right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main<'s> {
            splitter: &'s mut Splitter;
            pair_splitter: &'s mut PairSplitter;
        }
        machine Main::repack(&mut self, pair: Pair) -> Pair
        reaches Splitter requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            result
        }
        machine Main::envelope(&mut self, pair: Pair) -> Envelope
        reaches Splitter requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            Envelope { pair: result }
        }
        machine Main::two_hop(&mut self, pair: Pair) -> Pair
        reaches Splitter requires
            pair.left in Region::Owned;
            pair.right in Region::Owned
        {
            let result: Pair = self.splitter.partition(pair.left, pair.right);
            let forwarded: Pair = result;
            forwarded
        }
        machine Main::aggregate_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        reaches PairSplitter {
            self.pair_splitter.partition(Pair { left: left, right: right })
        }
        machine Main::two_calls(&mut self, first: Pair, second: Pair) -> Double
        reaches Splitter requires
            first.left in Region::Owned;
            first.right in Region::Owned;
            second.left in Region::Owned;
            second.right in Region::Owned
        {
            let left: Pair = self.splitter.partition(first.left, first.right);
            let right: Pair = self.splitter.partition(second.left, second.right);
            Double { first: left, second: right }
        }
        machine Main::forward_double(&mut self, first: Pair, second: Pair) -> Double
        requires
            first.left in Region::Owned;
            first.right in Region::Owned;
            second.left in Region::Owned;
            second.right in Region::Owned
        {
            self.two_calls(first, second)
        }
        machine Main::main(&mut self) {}
    "#;

    let checked_program = checked(source);
    let compositions = &checked_program
        .facts
        .qualifications
        .content
        .partition_compositions;
    assert_eq!(compositions.len(), 8, "compositions: {compositions:#?}");
    let state_symbol = |name: &str| {
        checked_program
            .machines()
            .iter()
            .flat_map(|machine| checked_program.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let composition = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("repack"))
        .expect("staged-local composition");
    assert_eq!(composition.statement_index, 0);
    assert_eq!(composition.call_ordinal, 0);
    assert_eq!(composition.input_claim_identities.len(), 2);
    assert_eq!(composition.result_rewrites.len(), 2);
    assert_eq!(composition.substitutions.len(), 4);
    assert!(
        composition
            .result_rewrites
            .iter()
            .all(|rewrite| rewrite.claim_identity
                != language_semantics::PermissionClaimIdentity::Unknown)
    );
    assert!(matches!(
        composition.plan.equation.left(),
        ContentConservationTerm::Separate(children) if children.len() == 2
    ));
    assert!(matches!(
        composition.plan.equation.right(),
        ContentConservationTerm::Separate(children) if children.len() == 2
    ));

    let envelope = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("envelope"))
        .expect("nested aggregate composition");
    assert_eq!(envelope.result_rewrites.len(), 2);
    let output_substitutions = envelope
        .substitutions
        .iter()
        .filter(|substitution| substitution.source.root == ContentPlaceRoot::Result)
        .collect::<Vec<_>>();
    assert_eq!(output_substitutions.len(), 2);
    assert!(output_substitutions.iter().all(|substitution| {
        matches!(
            substitution.target.segments.as_slice(),
            [ContentPlaceSegment::Field(outer), ContentPlaceSegment::Field(_)]
                if outer.name == "pair"
        )
    }));
    let two_hop = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("two_hop"))
        .expect("multi-hop local composition");
    assert_eq!(two_hop.result_rewrites.len(), 2);
    assert!(two_hop.result_rewrites.iter().all(|rewrite| {
        rewrite.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            && rewrite.source == rewrite.target
    }));
    let aggregate_argument = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("aggregate_argument"))
        .expect("record aggregate argument composition");
    assert_eq!(aggregate_argument.input_claim_identities.len(), 2);
    assert!(aggregate_argument.result_rewrites.is_empty());
    let input_substitutions = aggregate_argument
        .substitutions
        .iter()
        .filter(|substitution| {
            matches!(substitution.source.root, ContentPlaceRoot::Parameter { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(input_substitutions.len(), 2);
    assert!(
        input_substitutions
            .iter()
            .all(|substitution| substitution.target.segments.is_empty())
    );
    let mut two_calls = compositions
        .iter()
        .filter(|composition| composition.state_symbol == state_symbol("two_calls"))
        .collect::<Vec<_>>();
    two_calls.sort_by_key(|composition| composition.statement_index);
    assert_eq!(two_calls.len(), 2);
    for (index, composition) in two_calls.into_iter().enumerate() {
        assert_eq!(composition.statement_index, index);
        assert_eq!(composition.call_ordinal, 0);
        assert_eq!(composition.input_claim_identities.len(), 2);
        assert_eq!(composition.result_rewrites.len(), 2);
        let outer = if index == 0 { "first" } else { "second" };
        assert!(composition.result_rewrites.iter().all(|rewrite| matches!(
            rewrite.target.segments.as_slice(),
            [ContentPlaceSegment::Field(field), ContentPlaceSegment::Field(_)]
                if field.name == outer
        )));
    }
    let forward_double = compositions
        .iter()
        .filter(|composition| composition.state_symbol == state_symbol("forward_double"))
        .collect::<Vec<_>>();
    assert_eq!(forward_double.len(), 2);
    assert!(
        forward_double.iter().all(|composition| {
            composition.source_callable == state_symbol("two_calls")
                && composition.source_derivation_depth == 1
                && composition.result_rewrites.len() == 2
        }),
        "forwarded rows: {forward_double:#?}"
    );
}

#[test]
fn checked_facts_compose_partitions_through_exact_array_and_case_arguments() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Pair {
            left: Region in Owned;
            right: Region in Owned;
        }
        data SumPair {
            case Pair(left: Region in Owned, right: Region in Owned);
            case Mirror(left: Region in Owned, right: Region in Owned);
        }
        boundary trait ArraySplitter {
            machine partition(pair: [Region in Owned; 2]) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair[0])),
                    Owned::content(old(&pair[1])),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        boundary trait CaseSplitter {
            machine partition(pair: SumPair) -> Pair
            ensures
                separate(
                    Owned::content(old(&pair.left)),
                    Owned::content(old(&pair.right)),
                )
                == separate(
                    Owned::content(&result.left),
                    Owned::content(&result.right),
                );
        }
        data Main<'s> {
            array_splitter: &'s mut ArraySplitter;
            case_splitter: &'s mut CaseSplitter;
        }
        machine Main::array_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        reaches ArraySplitter {
            self.array_splitter.partition([left, right])
        }
        machine Main::case_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        reaches CaseSplitter {
            self.case_splitter.partition(SumPair::Pair {
                left: left,
                right: right,
            })
        }
        machine Main::wrong_case_argument(
            &mut self,
            left: Region in Owned,
            right: Region in Owned
        ) -> Pair
        reaches CaseSplitter {
            self.case_splitter.partition(SumPair::Mirror {
                left: left,
                right: right,
            })
        }
        machine Main::main(&mut self) {}
    "#;

    let checked_program = checked(source);
    let compositions = &checked_program
        .facts
        .qualifications
        .content
        .partition_compositions;
    assert_eq!(compositions.len(), 2, "compositions: {compositions:#?}");
    let state_symbol = |name: &str| {
        checked_program
            .machines()
            .iter()
            .flat_map(|machine| checked_program.machine_states(machine))
            .find(|state| state.name.as_str() == name)
            .expect("named state")
            .symbol
    };
    let array = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("array_argument"))
        .expect("fixed-array argument composition");
    let case = compositions
        .iter()
        .find(|composition| composition.state_symbol == state_symbol("case_argument"))
        .expect("active-case argument composition");

    for composition in [array, case] {
        assert_eq!(composition.input_claim_identities.len(), 2);
        assert!(composition.result_rewrites.is_empty());
        let input_substitutions = composition
            .substitutions
            .iter()
            .filter(|substitution| {
                matches!(substitution.source.root, ContentPlaceRoot::Parameter { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(input_substitutions.len(), 2);
        assert!(
            input_substitutions
                .iter()
                .all(|substitution| substitution.target.segments.is_empty())
        );
    }
    assert!(array.substitutions.iter().any(|substitution| matches!(
        substitution.source.segments.as_slice(),
        [ContentPlaceSegment::FixedIndex(0)]
    )));
    assert!(case.substitutions.iter().any(|substitution| matches!(
        substitution.source.segments.as_slice(),
        [ContentPlaceSegment::Case(case), ContentPlaceSegment::Field(field)]
            if case.name == "Pair" && field.name == "left"
    )));
    assert!(
        compositions
            .iter()
            .all(|composition| composition.state_symbol != state_symbol("wrong_case_argument"))
    );
}

#[test]
fn checked_facts_infer_exact_content_reshuffles_through_sum_case_paths() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Envelope {
            case Empty;
            case Present(region: Region in Owned);
        }

        data Main {}
        machine Main::forward(value: Envelope) -> Envelope {
            value
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [reshuffle] = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .as_slice()
    else {
        panic!("one active-payload reshuffle should be inferred");
    };
    for term in [
        reshuffle.plan.equation.left(),
        reshuffle.plan.equation.right(),
    ] {
        let ContentConservationTerm::Projection { subject, .. } = term else {
            panic!("an identity reshuffle must remain a direct projection equality");
        };
        assert!(matches!(
            subject.segments.as_slice(),
            [ContentPlaceSegment::Case(case), ContentPlaceSegment::Field(field)]
                if case.name == "Present"
                    && case.symbol.is_valid()
                    && field.name == "region"
                    && field.symbol.is_valid()
        ));
    }
}

#[test]
fn checked_facts_do_not_infer_content_for_fresh_claim_establishment() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Main {}
        machine Main::issue() -> Region in Owned {
            Region { length: 1 }
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .is_empty(),
        "fresh establishment requires a sealed introduction row, not an inferred reshuffle"
    );
}

#[test]
fn checked_facts_keep_independent_same_algebra_reshuffles_separate() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Pair {
            first: Region in Owned;
            second: Region in Owned;
        }

        data Main {}
        machine Main::swap(pair: Pair) -> Pair {
            Pair { first: pair.second, second: pair.first }
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let reshuffles = &checked.facts.qualifications.content.identity_reshuffles;
    assert_eq!(reshuffles.len(), 2, "one row per preserved claim identity");
    assert!(reshuffles.iter().all(|row| {
        !matches!(
            row.plan.equation.left(),
            ContentConservationTerm::Separate(_)
        ) && !matches!(
            row.plan.equation.right(),
            ContentConservationTerm::Separate(_)
        )
    }));
    let ownership = &checked.facts.flow.ownership;
    assert!(reshuffles.iter().all(|row| {
        ownership.segments.span_or_empty(row.input_segments)
            != ownership.segments.span_or_empty(row.output_segments)
    }));
}

#[test]
fn checked_facts_do_not_equate_distinct_content_projection_identities() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Left;
        domain Region::Right;
        machine Left::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }
        machine Right::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Main {}
        machine Main::retag(region: Region in Left) -> Region in Right {
            region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .is_empty(),
        "matching carrier and algebra cannot replace exact projection identity"
    );
}

#[test]
fn checked_facts_infer_reshuffles_from_ordinary_qualification_contracts() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length }
        }

        data Main {}
        machine Main::forward(region: Region) -> Region in Owned
        requires
            region in Region::Owned
        {
            region
        }
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    assert_eq!(
        checked
            .facts
            .qualifications
            .content
            .identity_reshuffles
            .len(),
        1,
        "an ordinary requires qualification should select the exact input projection"
    );
}
