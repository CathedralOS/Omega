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
