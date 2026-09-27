use crate::tests::front_end::checked_program_result;

fn check(source: &str, rejection: Option<&str>) {
    let _ = checked_fixture(source, rejection);
}

fn checked_fixture(source: &str, rejection: Option<&str>) -> Option<checked_trees::CheckedTrees> {
    match checked_program_result(source) {
        Ok(checked) => {
            assert!(rejection.is_none(), "expected {rejection:?}\n{source}");
            Some(checked)
        }
        Err(diagnostics) => {
            let Some(rejection) = rejection else {
                panic!("check: {diagnostics:#?}\nsource:\n{source}");
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}\nsource:\n{source}"
            );
            if rejection == "creates local borrow" {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("is still active")),
                    "expected active-loan conflict: {diagnostics:#?}\n{source}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .all(|diagnostic| !diagnostic.message.contains("cannot prove")),
                    "bounds must validate before borrow rejection: {diagnostics:#?}\n{source}"
                );
            }
            None
        }
    }
}

fn window_source(scalar: &str, requirement: &str, body: &str, access: &str) -> String {
    let requirement = if requirement.is_empty() {
        String::new()
    } else {
        format!("requires {requirement};")
    };
    format!(
        "machine window(items: &[i32; 4], mut original: {scalar}) -> u64
         {requirement} {{ {body} let view: &[i32] = items[{access}]; view.len }}"
    )
}

#[test]
fn inclusive_normalization_overflow_never_wraps_into_a_valid_window() {
    for (access, rejection) in [
        ("0..=9223372036854775807", "inclusive end overflow"),
        ("0..9223372036854775807", "end bound"),
        ("0..=4", "end bound"),
    ] {
        check(
            &window_source("u64", "original <= 4", "", access),
            Some(rejection),
        );
    }
    check(&window_source("u64", "original <= 4", "", "0..4"), None);
    check(&window_source("u64", "original <= 4", "", "0..=3"), None);
}

#[test]
fn arithmetic_snapshot_cannot_copy_a_pre_assignment_lower_bound() {
    for before_mutation in [false, true] {
        let capture = "let cut: i64 = original - 1;";
        let mutation = "original = replacement;";
        let body = if before_mutation {
            format!("{capture} {mutation}")
        } else {
            format!("{mutation} {capture}")
        };
        let source = format!(
            "machine window(items: &[i32; 4], mut original: i64 [0..=5], replacement: i64 [0..=4]) -> u64
             requires 0 <= original - 1 && original - 1 <= 4
                 && 0 <= replacement && replacement <= 4;
             {{ {body} let view: &[i32] = items[0..cut]; view.len }}"
        );
        check(&source, (!before_mutation).then_some("cannot prove"));
    }
}

#[test]
fn explicit_owned_receiver_result_preserves_captured_count_across_later_call() {
    check(
        r#"
data Storage { bytes: [u8; 8]; }
data Engine { storage: Storage; }
data Arenas<'scope> { bytes: &'scope mut [u8]; }
machine Storage::borrow_arenas<'scope>(&mut self) -> Arenas<'scope> {
 Arenas { bytes: &mut self.bytes }
}
machine noop() -> u64 { 0 }
data Producer { value: u64; }
machine Producer::copy_modules(&mut self, output: &mut [u8; 8]) -> u64
ensures 0 <= result && result < 9;
{
    transition { _ -> copy(output, 0) }
    state copy(&mut self, output: &mut [u8; 8], position: u64) -> u64
    requires 0 <= position && position < 9;
    {
        transition position < self.value && position < 8 {
            true -> store(output, position)
            false -> (position)
            _ -> (position)
        }
    }
    state store(&mut self, output: &mut [u8; 8], position: u64) -> u64
    requires 0 <= position && position < 8;
    {
        output[position] = 1;
        transition { _ -> copy(output, position + 1) }
    }
}
machine consume(values: &[u8]) -> u64 { values.len }
machine demonstrate(producer: &mut Producer, engine: &mut Engine) -> u64 {
    let mut output: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
    let count: u64 = producer.copy_modules(&mut output);
    let arenas: Arenas = (&mut engine.storage).borrow_arenas();
    let ignored: u64 = noop();
    consume(&output[0..count])
}
"#,
        None,
    );
}

#[test]
fn explicit_owned_receiver_results_keep_backing_storage_writes() {
    use validation::CallFrameResolver;

    for (parameter, receiver, expected) in [
        ("mut storage: Storage", "&mut storage", "storage.bytes"),
        (
            "engine: &mut Engine",
            "&mut engine.storage",
            "engine.storage.bytes",
        ),
        ("stores: &mut [Storage; 2]", "&mut stores[0]", "stores"),
    ] {
        let program = checked_fixture(
            &format!(
                "data Storage {{ bytes: [u8; 8]; }}
             data Engine {{ storage: Storage; }}
             data Arenas<'scope> {{ bytes: &'scope mut [u8; 8]; }}
             machine Storage::borrow_arenas<'scope>(&mut self) -> Arenas<'scope> {{
                 Arenas {{ bytes: &mut self.bytes }}
             }}
             machine exercise({parameter}) {{
                 let arenas: Arenas = ({receiver}).borrow_arenas();
                 arenas.bytes[0] = 1;
             }}"
            ),
            None,
        )
        .unwrap()
        .typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "exercise")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let assignment = &program.statement_table.statements(state.statement_nodes)[1];
        let paths = CallFrameResolver::new(&program)
            .unwrap()
            .assignment_write_frame(machine, assignment)
            .into_complete_paths()
            .unwrap_or_else(|| panic!("complete returned-origin frame for {receiver}"));
        assert!(
            paths.iter().any(|path| path == expected),
            "{receiver}: {paths:?}"
        );
        if expected == "stores" {
            assert!(
                !paths.iter().any(|path| path.starts_with("stores.")),
                "{paths:?}"
            );
        }
    }
}
