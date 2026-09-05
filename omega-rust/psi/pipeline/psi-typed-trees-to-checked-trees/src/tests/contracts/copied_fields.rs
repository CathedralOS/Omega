use super::*;

const DEFINITIONS: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
data Payload { bytes: [u8; 4]; }
data Record { payload: Payload; tag: u64; }
"#;

#[test]
fn whole_copy_preserves_live_nested_predicates_and_disjoint_writes() {
    for copy in ["target = source;", "target.payload = source.payload;"] {
        let source = format!(
            r#"{DEFINITIONS}
            machine copy(target: &mut Record, source: Record)
            requires source.payload.bytes in Utf8
            ensures target.payload.bytes in Utf8 {{ {copy} target.tag = 1; }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{copy}: {diagnostics:#?}"));
    }
}

#[test]
fn whole_copy_cannot_use_another_sources_predicate() {
    let source = format!(
        r#"{DEFINITIONS}
        machine copy(target: &mut Record, source: Record, other: Record)
        requires source.payload.bytes in Utf8
        ensures target.payload.bytes in Utf8 {{ target = other; }}
    "#
    );
    assert!(lower_typed_trees(parse_typed_trees(&source)).is_err());
}

#[test]
fn mutations_retire_copied_nested_predicates() {
    for mutation in [
        "target.payload.bytes[0] = 255;",
        "let alias: &mut [u8; 4] = &mut target.payload.bytes; alias[0] = 255;",
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine copy(target: &mut Record, source: Record)
            requires source.payload.bytes in Utf8
            ensures target.payload.bytes in Utf8 {{ target = source; {mutation} }}
        "#
        );
        assert!(
            lower_typed_trees(parse_typed_trees(&source)).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn whole_copy_cannot_restore_an_invalidated_source_predicate() {
    let source = format!(
        r#"{DEFINITIONS}
        machine copy(target: &mut Record, source: &mut Record)
        requires source.payload.bytes in Utf8
        ensures target.payload.bytes in Utf8 {{
            source.payload.bytes[0] = 255;
            target = source;
        }}
    "#
    );
    assert!(lower_typed_trees(parse_typed_trees(&source)).is_err());
}
