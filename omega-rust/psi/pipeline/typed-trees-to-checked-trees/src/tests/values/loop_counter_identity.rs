use super::*;

fn loop_source(receiver: &str) -> String {
    format!(
        "data Main {{ items: [u8; 4]; index: i32 in Saturating; }}
        machine Main::run(&mut self, other: &Main) {{
            self.index = 0;
            transition {{ _ -> fill(other) }}
            state fill(&mut self, other: &Main) {{
                let value: u8 = self.items[self.index];
                self.index = {receiver}.index + 1;
                transition self.index < 4 {{ true -> fill(other) _ -> {{}} }}
            }}
        }}"
    )
}

#[test]
fn saturating_self_counter_keeps_its_own_loop_bound() {
    let source = loop_source("self");
    lower_typed_trees(typed_trees(&source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

#[test]
fn another_receivers_field_cannot_establish_self_counter_monotonicity() {
    let source = loop_source("other");
    let diagnostics = lower_typed_trees(typed_trees(&source))
        .expect_err("another receiver's possibly negative index cannot establish self.index >= 0");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("index") && diagnostic.message.contains("prove")
        }),
        "{source}\n{diagnostics:#?}"
    );
}
