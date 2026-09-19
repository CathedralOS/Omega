fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(typed(source)?)
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("element_field_flow_probe.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

const DEFINITIONS: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
data Row [copy] { bytes: [u8; 4] in Utf8; tag: u64; }
"#;

fn assert_accepted(name: &str, source: &str) {
    if let Err(diagnostics) = check(source) {
        panic!(
            "{name}: expected acceptance, got:\n{}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn assert_rejected(name: &str, source: &str, expected: &[&str]) {
    match check(source) {
        Ok(_) => panic!("{name}: expected rejection containing {expected:?}, got acceptance"),
        Err(diagnostics) => {
            let messages = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>();
            for fragment in expected {
                assert!(
                    messages.iter().any(|message| message.contains(fragment)),
                    "{name}: no diagnostic contains {fragment:?}; got:\n{}",
                    messages.join("\n")
                );
            }
        }
    }
}

#[test]
fn element_copy_into_local() {
    assert_accepted(
        "element_copy_into_local",
        &format!(
            r#"{DEFINITIONS}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let copy: Row = rows[0];
                consume(copy);
            }}
        "#
        ),
    );
}

#[test]
fn element_view_into_local() {
    assert_accepted(
        "element_view_into_local",
        &format!(
            r#"{DEFINITIONS}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let view: &Row = &rows[0];
                consume(view);
            }}
        "#
        ),
    );
}

#[test]
fn mutable_call_preserves_untouched_field() {
    assert_accepted(
        "mutable_call_preserves_untouched_field",
        &format!(
            r#"{DEFINITIONS}
            machine touch(row: &mut Row) {{ row.tag = 1; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &mut [Row; 2]) {{
                touch(&mut rows[0]);
                consume(&rows[0]);
            }}
        "#
        ),
    );
}

/// The corruption is caught on the callee side: `poke` hands `row` back at
/// its return with the declared `Utf8` coverage retired, so its return
/// rejects with the exact place. The caller then relies on that guarantee
/// and its own `consume` call is accepted.
#[test]
fn mutable_call_without_ensures_on_written_field() {
    assert_rejected(
        "mutable_call_writes_field_no_ensures",
        &format!(
            r#"{DEFINITIONS}
            machine poke(row: &mut Row) {{ row.bytes[0] = 255; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &mut [Row; 2]) {{
                poke(&mut rows[0]);
                consume(&rows[0]);
            }}
        "#
        ),
        &["for return from poke", "row.bytes requires"],
    );
}

#[test]
fn returned_element() {
    assert_accepted(
        "returned_element",
        &format!(
            r#"{DEFINITIONS}
            machine pick(rows: &[Row; 2]) -> Row {{ rows[0] }}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let chosen: Row = pick(rows);
                consume(chosen);
            }}
        "#
        ),
    );
}

#[test]
fn returned_element_direct_argument() {
    assert_accepted(
        "returned_element_direct_argument",
        &format!(
            r#"{DEFINITIONS}
            machine pick(rows: &[Row; 2]) -> Row {{ rows[0] }}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                consume(pick(rows));
            }}
        "#
        ),
    );
}

#[test]
fn returned_element_local_write_retires_fact() {
    // The `Row` annotation on `chosen` cannot restore the field fact that the
    // `chosen.bytes` write retired: mutation invalidation stays authoritative.
    assert_rejected(
        "returned_element_local_write_retires_fact",
        &format!(
            r#"{DEFINITIONS}
            machine pick(rows: &[Row; 2]) -> Row {{ rows[0] }}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let mut chosen: Row = pick(rows);
                chosen.bytes[0] = 255;
                consume(chosen);
            }}
        "#
        ),
        &["parameter row.bytes requires"],
    );
}

#[test]
fn local_array_whole_call() {
    assert_accepted(
        "local_array_whole_call",
        &format!(
            r#"{DEFINITIONS}
            machine consume_all(rows: &[Row; 2]) {{ }}
            machine caller() {{
                let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
                consume_all(&rows);
            }}
        "#
        ),
    );
}

#[test]
fn returned_element_with_ensures() {
    assert_accepted(
        "returned_element_ensures",
        &format!(
            r#"{DEFINITIONS}
            machine pick(rows: &[Row; 2]) -> Row ensures result.bytes in Utf8 {{ rows[0] }}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let chosen: Row = pick(rows);
                consume(chosen);
            }}
        "#
        ),
    );
}

#[test]
fn returned_element_with_ensures_unproved() {
    assert_rejected(
        "returned_element_ensures_unproved",
        &format!(
            r#"{DEFINITIONS}
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine pick(rows: &mut [Row; 2]) -> Row ensures result.bytes in Utf8 {{
                corrupt(&mut rows[0].bytes);
                rows[0]
            }}
        "#
        ),
        &[
            "cannot prove ensures contract for exit from pick",
            "cannot prove default-domain field requirement for return from pick",
        ],
    );
}

#[test]
fn return_corrupted_element() {
    // The corrupted element's field fact was retired by the `corrupt` call;
    // the nominal return annotation cannot restore it.
    assert_rejected(
        "return_corrupted_element",
        &format!(
            r#"{DEFINITIONS}
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine pick(rows: &mut [Row; 2]) -> Row {{
                corrupt(&mut rows[0].bytes);
                rows[0]
            }}
        "#
        ),
        &["cannot prove default-domain field requirement for return from pick"],
    );
}

#[test]
fn named_transition_corrupted_element() {
    assert_rejected(
        "named_transition_corrupted_element",
        &format!(
            r#"{DEFINITIONS}
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine caller(rows: &mut [Row; 2]) {{
                corrupt(&mut rows[0].bytes);
                transition true {{
                    true -> inner(rows[0])
                    false -> {{}}
                }}
                state inner(row: Row) {{ }}
            }}
        "#
        ),
        &["parameter row.bytes requires"],
    );
}

/// Element field domains seed at the whole-extent coordinate, so a runtime
/// `rows[index]` subject narrows coverage from the extent row: for a fixed
/// array the `u64 [0..2]` index range discharges the bounds proof and the
/// element's declared `Utf8` coverage reaches the `consume` argument.
#[test]
fn runtime_index_read() {
    assert_accepted(
        "runtime_index_read",
        &format!(
            r#"{DEFINITIONS}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2], index: u64 [0..2]) {{
                consume(rows[index]);
            }}
        "#
        ),
    );
}

/// Field coverage reaches the runtime-indexed slice element the same way;
/// the remaining rejection is the bounds proof, not the field domain: the
/// `u64 [0..2]` index range says nothing about `rows.len`, so an unknown
/// slice length cannot admit the access.
#[test]
fn slice_param_runtime_index() {
    assert_rejected(
        "slice_param_runtime_index",
        &format!(
            r#"{DEFINITIONS}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row], index: u64 [0..2]) {{
                consume(rows[index]);
            }}
        "#
        ),
        &["cannot prove index `index` is within unknown slice length of `rows`"],
    );
}

#[test]
fn result_local_from_call() {
    assert_accepted(
        "result_local_from_call",
        &format!(
            r#"{DEFINITIONS}
            machine pick(rows: &[Row; 2]) -> Row ensures result.bytes in Utf8 {{ rows[0] }}
            machine caller(rows: &[Row; 2]) -> Row ensures result.bytes in Utf8 {{
                pick(rows)
            }}
        "#
        ),
    );
}

#[test]
fn returned_array_element_field() {
    // A fixed array of nominal elements returned by value owes each element's
    // declared fields; the caller consumes `grid[1].bytes` exactly.
    assert_accepted(
        "returned_array_element_field",
        &format!(
            r#"{DEFINITIONS}
            machine both(rows: &[Row; 2]) -> [Row; 2] {{ [rows[0], rows[1]] }}
            machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let pair: [Row; 2] = both(rows);
                consume(pair[1]);
            }}
        "#
        ),
    );
}

#[test]
fn slice_view_element() {
    assert_accepted(
        "slice_view_element",
        &format!(
            r#"{DEFINITIONS}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &[Row; 2]) {{
                let view: &[Row] = rows;
                consume(&view[0]);
            }}
        "#
        ),
    );
}

/// The same contract holds on a named-transition edge: whole-extent element
/// coverage flows into the `inner` argument, so the only remaining rejection
/// is the unprovable index bound against the unknown slice length.
#[test]
fn named_transition_runtime_index() {
    assert_rejected(
        "named_transition_runtime_index",
        &format!(
            r#"{DEFINITIONS}
            machine caller(rows: &[Row], index: u64 [0..2]) {{
                transition index < 2 {{
                    true -> inner(rows[index])
                    false -> {{}}
                }}
                state inner(row: Row) {{ }}
            }}
        "#
        ),
        &["cannot prove index `index` is within unknown slice length of `rows`"],
    );
}
