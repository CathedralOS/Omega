//! Contract consumption at call boundaries: element-level field evidence
//! must survive a mutable call through a sibling element of the same slice
//! view, while a call that corrupts the consumed element still rejects.

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
            std::path::PathBuf::from("contracts_probe.omg"),
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
data Level [copy] { rooms: [Row; 2]; }
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

/// The flattened write frame reports `clear(&mut rooms[0])` as a write to
/// `rooms`; the structured mutation set keeps `rooms[0].tag`. The sibling
/// element's index bound and `bytes in Utf8` evidence must survive the call.
#[test]
fn call_through_view_element_keeps_sibling_element_coverage() {
    assert_accepted(
        "call_through_view_element_keeps_sibling_element_coverage",
        &format!(
            r#"{DEFINITIONS}
            machine clear(row: &mut Row) {{ row.tag = 0; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(level: &mut Level) {{
                let rooms: &mut [Row] = level.rooms.as_mut_slice();
                clear(&mut rooms[0]);
                consume(&rooms[1]);
            }}
        "#
        ),
    );
}

/// Writing an element's declared field retires that element's evidence only:
/// consuming the same view element after the mutating call must reject.
#[test]
fn call_through_view_element_retires_the_written_element() {
    assert_rejected(
        "call_through_view_element_retires_the_written_element",
        &format!(
            r#"{DEFINITIONS}
            machine poke(row: &mut Row) {{ row.bytes[0] = 255; }}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(level: &mut Level) {{
                let rooms: &mut [Row] = level.rooms.as_mut_slice();
                poke(&mut rooms[1]);
                consume(&rooms[1]);
            }}
        "#
        ),
        &["parameter row.bytes requires"],
    );
}
