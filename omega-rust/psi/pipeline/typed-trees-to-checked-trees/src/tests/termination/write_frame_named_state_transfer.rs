use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};

fn program(source: &str) -> typed_trees::TypedTrees {
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn visible_paths(paths: Option<Vec<String>>) -> Option<Vec<String>> {
    paths.map(|paths| {
        let mut paths = paths
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    })
}

fn entry_frame(source: &str) -> Option<Vec<String>> {
    let program = program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    visible_paths(
        resolver
            .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
            .into_complete_paths(),
    )
}

fn assert_run_frame(source: &str, expected: &[&str]) {
    let expected = Some(
        expected
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(entry_frame(source), expected);
}

// A reference actual carried through a transition arm substitutes the target
// state's parameter writes into the caller namespace.
#[test]
fn named_state_reference_actual_substitutes_target_writes() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine Main::run(&mut self) {
            transition { _ -> step(&mut self.value) }

            state step(&mut self, alias: &mut u64) {
                consume(alias);
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.value"],
    );
}

// The same substitution covers a direct write through the state parameter.
#[test]
fn named_state_parameter_write_substitutes_the_actual() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine Main::run(&mut self) {
            transition { _ -> step(&mut self.value) }

            state step(&mut self, alias: &mut u64) {
                alias = 1;
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.value"],
    );
}

// Divergent arms handing different actuals into the same named state union the
// candidate origins on that state's frame.
#[test]
fn named_state_divergent_arms_union_their_actuals() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine Main::run(&mut self) {
            transition self.tag { 0 -> step(&mut self.value) _ -> step(&mut self.other) }

            state step(&mut self, alias: &mut u64) {
                consume(alias);
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.other", "self.value"],
    );
}

// A divergent exclusive-alias local handed to a named state still unions its
// proven referent set; unproven divergent actuals stay opaque.
#[test]
fn named_state_divergent_local_unions_its_referents() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 { match tag { 0 -> a, _ -> b } }
        machine Main::run(&mut self) {
            let alias: &mut u64 = pick(&mut self.value, &mut self.other, self.tag);
            transition { _ -> step(alias) }

            state step(&mut self, leaf: &mut u64) {
                consume(leaf);
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.other", "self.value"],
    );
}

// A single-origin local binding transfers its exact referent the same way.
#[test]
fn named_state_local_alias_transfers_its_origin() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine Main::run(&mut self) {
            let alias: &mut u64 = &mut self.value;
            transition { _ -> step(alias) }

            state step(&mut self, leaf: &mut u64) {
                consume(leaf);
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.value"],
    );
}

// Forwarding a reference parameter through a second named state composes the
// substitutions back to the caller's storage.
#[test]
fn named_state_chained_transfer_composes_substitutions() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine Main::run(&mut self) {
            transition { _ -> step(&mut self.value) }

            state step(&mut self, alias: &mut u64) {
                transition { _ -> deeper(alias) }
            }

            state deeper(&mut self, leaf: &mut u64) {
                consume(leaf);
                transition { _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.value"],
    );
}

// A cyclic named transition forwarding the write-capable parameter positionally
// keeps the caller's exact storage through the permuted frame solve.
#[test]
fn named_state_cyclic_transfer_forwards_the_parameter() {
    assert_run_frame(
        r#"
        data Main { value: u64; other: u64; tag: u64; }
        machine consume(value: &mut u64) { value = 1; }
        machine Main::run(&mut self) {
            transition { _ -> step(&mut self.value) }

            state step(&mut self, alias: &mut u64) {
                consume(alias);
                transition self.tag { 0 -> step(alias) _ -> finish() }
            }

            state finish(&mut self) {}
        }
        "#,
        &["self.value"],
    );
}
