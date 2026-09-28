fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(
        typed(source)?,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
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

const HOLDER: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
data Holder { line: [u8; 4] in Utf8; i: u64; }
data Main { tag: u64; }
"#;

/// One corrupted edge poisons the join: `other.line[0] = 255` retires the
/// coverage `other.line` would have delivered, so `put` cannot promise
/// `buf.line` stays `Utf8`.
#[test]
fn named_mutable_argument_poisoned_edge_rejects() {
    assert_rejected(
        "named_mutable_argument_poisoned_edge_rejects",
        &format!(
            r#"{HOLDER}
            machine Main::run(&mut self) {{
                let mut buf: Holder;
                buf.line = "    ";
                buf.i = 0;
                let mut other: Holder;
                other.line = "    ";
                other.i = 0;
                other.line[0] = 255;
                transition self.tag == 0 {{ true -> put(&mut buf) _ -> put(&mut other) }}
                state put(&mut self, buf: &mut Holder) {{
                    transition buf.i < 4 {{ true -> write(buf) _ -> done() }}
                }}
                state write(&mut self, buf: &mut Holder) {{
                    buf.line[buf.i] = 46;
                }}
                state done(&mut self) {{}}
            }}
        "#
        ),
        &["parameter buf.line requires"],
    );
}

/// An argument the caller never wrote carries no incoming fact at all: no
/// zero-initialization credit is invented for `buf.line`, so the write in
/// `put` cannot justify the `write` edge's `Utf8` requirement.
#[test]
fn named_mutable_argument_without_facts_rejects() {
    assert_rejected(
        "named_mutable_argument_without_facts_rejects",
        &format!(
            r#"{HOLDER}
            machine Main::run(&mut self) {{
                let mut buf: Holder;
                buf.i = 0;
                transition buf.i < 4 {{ true -> put(&mut buf) _ -> done() }}
                state put(&mut self, buf: &mut Holder) {{
                    buf.line[buf.i] = 46;
                    transition buf.i < 4 {{ true -> write(buf) _ -> done() }}
                }}
                state write(&mut self, buf: &mut Holder) {{
                    buf.line[0] = 46;
                }}
                state done(&mut self) {{}}
            }}
        "#
        ),
        &["parameter buf.line requires"],
    );
}
