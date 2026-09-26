use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

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
        match lower_typed_trees(typed_program(&program), &CheckingRequest::settled()) {
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
