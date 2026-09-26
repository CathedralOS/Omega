use crate::CheckingRequest;
use crate::flow::check_against_whole_pass as lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str, accepted: bool, rejection: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved contract accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection:?}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn tail_call_return_guarantees_do_not_leak_to_sibling_branches() {
    for (fallback, accepted) in [(7, true), (0, false)] {
        check(
            &format!(
                "machine produce() -> u64 ensures result == 7 {{ 7 }}
                 machine main(selected: bool) -> u64 ensures result == 7 {{
                     transition selected {{ true -> produce() false -> {fallback} }}
                 }}"
            ),
            accepted,
            "ensures",
        );
    }
}

#[test]
fn named_states_cannot_recover_entry_domains_after_mutating_calls() {
    for parameter in ["bytes", "destination"] {
        check(
            &format!(
                r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine consume(bytes: &[u8; 4]) requires bytes in Utf8 {{}}
            machine main(bytes: &mut [u8; 4])
            requires bytes in Utf8
            {{
                corrupt(bytes);
                transition {{ _ -> finish(bytes) }}
                state finish({parameter}: &mut [u8; 4]) {{ consume({parameter}); }}
            }}
        "#
            ),
            false,
            "requires",
        );
    }
}
