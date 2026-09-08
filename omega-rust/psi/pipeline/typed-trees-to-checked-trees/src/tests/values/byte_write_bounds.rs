use super::*;

fn source(increment: u64) -> String {
    format!(
        r#"
        machine fill(out: &mut [u8], byte: u8) {{
            transition {{ _ -> scan(out, 0, byte) }}
            state scan(out: &mut [u8], position: u64, byte: u8) {{
                transition position < out.len {{
                    true -> store(out, position, byte)
                    false -> done()
                }}
            }}
            state store(out: &mut [u8], position: u64, byte: u8) {{
                out[position] = byte;
                transition {{ _ -> scan(out, position + {increment}, byte) }}
            }}
            state done() {{}}
        }}
    "#
    )
}

#[test]
fn byte_store_preserves_transferred_strict_length_bound_for_one_step() {
    let program = source(1);
    lower_typed_trees(typed_trees(&program))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn byte_store_does_not_strengthen_the_guard_distance() {
    let diagnostics = lower_typed_trees(typed_trees(&source(2)))
        .expect_err("a strict length guard permits one step, not two");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow")),
        "{diagnostics:#?}"
    );
}

#[test]
fn byte_store_cannot_restore_a_length_relation_retired_by_its_rhs_call() {
    for (helper, accepted) in [
        ("machine touch(out: &mut [u8]) -> u8 { 0 }", true),
        (
            r#"
            machine touch(out: &mut [u8]) -> u8 {
                transition out.len > 0 { true -> write(out) false -> 0 }
                state write(out: &mut [u8]) -> u8 { out[0] = 0; 0 }
            }
        "#,
            false,
        ),
    ] {
        let program = format!(
            "{helper}\n{}",
            source(1).replace("out[position] = byte;", "out[position] = touch(out);")
        );
        match lower_typed_trees(typed_trees(&program)) {
            Ok(_) => assert!(accepted, "overlapping RHS call retained its prior relation"),
            Err(diagnostics) => {
                assert!(!accepted, "{diagnostics:#?}");
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("may overflow")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}
