use crate::tests::front_end::typed_program;

thread_local! {
    pub(in crate::flow) static STATE_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(in crate::flow) static WHOLE_PASS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static SWEEP_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) };
}

/// Mirror of `crate::lower_typed_trees` that also checks the incremental
/// flow pass against the whole-pass reference under the same request.
pub(crate) fn check_against_whole_pass(
    program: typed_trees::TypedTrees,
    request: &crate::CheckingRequest<'_>,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let reference_program = program.clone();
    let result = crate::lower_typed_trees(program, request);
    let reference =
        with_whole_pass_reference(|| crate::lower_typed_trees(reference_program, request));
    assert_eq!(
        result, reference,
        "complete checked facts or diagnostics changed"
    );
    result
}

#[test]
fn exhausted_sweep_budget_discards_provisional_constants() {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SWEEP_LIMIT.set(self.0);
        }
    }
    let source = reverse_chain_source(12, 0).replace(
        "machine chain() -> u8",
        "machine chain() -> u8 ensures result == 3",
    );
    let program = typed_program(&source);
    check_against_whole_pass(program.clone(), &crate::CheckingRequest::settled())
        .expect("the complete fixed point proves 3");
    let _restore = Restore(SWEEP_LIMIT.replace(1));
    let diagnostics = check_against_whole_pass(program, &crate::CheckingRequest::settled())
        .expect_err("fallback must discard every provisional constant");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ensures"))
    );
}

fn with_whole_pass_reference<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            WHOLE_PASS_REFERENCE.set(self.0);
        }
    }
    let _restore = Restore(WHOLE_PASS_REFERENCE.replace(true));
    run()
}

fn reverse_chain_source(length: usize, independent: usize) -> String {
    use std::fmt::Write;
    let mut source = String::from("machine chain() -> u8 { transition { _ -> step0(3) }\n");
    for index in (0..length).rev() {
        write!(source, "state step{index}(value: u8) -> u8 {{ ").unwrap();
        if index + 1 == length {
            source.push_str("value");
        } else {
            write!(source, "transition {{ _ -> step{}(value) }}", index + 1).unwrap();
        }
        source.push_str(" }\n");
    }
    source.push_str("}\n");
    for index in 0..independent {
        writeln!(source, "machine independent{index}() -> u8 {{ 7 }}").unwrap();
    }
    source
}

#[test]
fn large_parameter_counter_converges_without_proving_a_false_exit() {
    let source = r#"
        machine count() -> u64 {
            transition { _ -> advance(0) }
            state advance(value: u64) -> u64 {
                let next: u64 = ((value as u64 in Wrapping) + 1) as u64;
                transition value < 100000 {
                    true -> advance(next)
                    _ -> finish(value)
                }
            }
            state finish(value: u64) -> u64 { value }
        }
    "#;
    STATE_BUILDS.set(0);
    let checked =
        check_against_whole_pass(typed_program(source), &crate::CheckingRequest::settled())
            .expect("the guarded counter checks");
    let program = &checked.typed;
    let state = program
        .machine_states(&program.machines()[0])
        .iter()
        .find(|state| program.symbols.name(state.symbol) == "advance")
        .unwrap();
    let parameter = program.state_parameters(state)[0].symbol;
    let semantic = &checked.facts.semantic;
    let settled = semantic.facts.iter().find_map(|(_, fact)| {
        if !matches!(fact.point, facts::ProgramPoint::State { state_symbol, .. } if state_symbol == state.symbol) {
            return None;
        }
        let facts::FactPlace::Place(place) = fact.place else { return None; };
        if semantic.places.get(place).root != facts::PlaceRoot::Symbol(parameter) { return None; }
        let facts::FactPayload::AssignedIntegerBounds { bounds } = fact.payload else { return None; };
        Some(semantic.integer_ranges.get(bounds))
    }).expect("convergence must retain a settled interval; exhausted-budget fallback erases it");
    assert!(settled.minimum <= numerics::bignum::BigInt::zero());
    assert!(settled.maximum >= numerics::bignum::BigInt::from_u64(100000));
    assert!(
        STATE_BUILDS.get() < 250,
        "must not enumerate counter values"
    );
    let false_claim = source.replace(
        "machine count() -> u64 {",
        "machine count() -> u64 ensures result < 100000 {",
    );
    let diagnostics = check_against_whole_pass(
        typed_program(&false_claim),
        &crate::CheckingRequest::settled(),
    )
    .expect_err("early provisional bounds cannot prove the false exit");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ensures"))
    );
}
