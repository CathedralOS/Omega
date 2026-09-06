use super::*;

fn check_loop(carrier: &str, policy: &str, operation: &str, guard: &str, accepted: bool) {
    let source = format!(
        "data Main {{ items: [u8; 4]; index: {carrier} in {policy}; }}
        machine Main::run(&mut self) {{
            self.index = 2;
            transition {{ _ -> fill() }}
            state fill(&mut self) {{
                let value: u8 = self.items[self.index];
                self.index = self.index {operation};
                transition {guard} {{ true -> fill() _ -> {{}} }}
            }}
        }}"
    );
    check_source(&source, accepted);
}

fn check_source(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(accepted, "unproved loop index accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("index") && diagnostic.message.contains("prove")
                }),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn bounded_wrapping_steps_keep_their_monotone_index_facts() {
    check_loop("i32", "Wrapping", "+ 1", "self.index < 4", true);
    check_loop("u32", "Wrapping", "- 1", "self.index > 0", true);
}

#[test]
fn wrapping_increment_cannot_mint_a_nonnegative_loop_invariant() {
    check_loop("i32", "Wrapping", "+ 2147483647", "self.index < 4", false);
}

#[test]
fn wrapping_decrement_cannot_mint_an_upper_loop_invariant() {
    check_loop("u32", "Wrapping", "- 3", "self.index > 0", false);
}

#[test]
fn nonwrapping_policies_keep_direction_on_normal_completion() {
    for policy in ["Saturating", "Trapping"] {
        check_loop("i32", policy, "+ 2147483647", "self.index < 4", true);
        check_loop("u32", policy, "- 3", "self.index > 0", true);
    }
}

#[test]
fn wrapping_proofs_follow_every_update_in_the_statement_prefix() {
    check_loop(
        "i32",
        "Wrapping",
        "+ 1; self.index = self.index + 1",
        "self.index < 4",
        true,
    );
    check_loop(
        "i32",
        "Wrapping",
        "+ 2; self.index = self.index + 2147483644",
        "self.index < 4",
        false,
    );
}

#[test]
fn wrapping_proofs_follow_named_state_arrivals() {
    for (update, accepted) in [("+ 1", true), ("+ 2147483647", false)] {
        check_source(
            &format!(
                "data Main {{ items: [u8; 4]; index: i32 in Wrapping; }}
            machine Main::run(&mut self) {{
                self.index = 2;
                transition {{ _ -> fill() }}
                state fill(&mut self) {{
                    let value: u8 = self.items[self.index];
                    transition {{ _ -> advance() }}
                }}
                state advance(&mut self) {{
                    self.index = self.index {update};
                    transition self.index < 4 {{ true -> fill() _ -> {{}} }}
                }}
            }}"
            ),
            accepted,
        );
    }
}

#[test]
fn unsigned_wrapping_steps_use_their_arrival_floor() {
    for (floor, accepted) in [(2, true), (1, false)] {
        check_source(
            &format!(
                "data Main {{ items: [u8; 4]; index: u32 in Wrapping; }}
            machine Main::run(&mut self) {{
                self.index = 3;
                transition {{ _ -> fill() }}
                state fill(&mut self) {{
                    let value: u8 = self.items[self.index];
                    self.index = self.index - 2;
                    transition self.index >= {floor} {{ true -> fill() _ -> {{}} }}
                }}
            }}"
            ),
            accepted,
        );
    }
}

#[test]
fn wrapping_updates_use_the_live_false_branch_of_an_earlier_exit() {
    for (floor, accepted) in [(2, true), (1, false)] {
        check_source(
            &format!(
                "data Main {{ items: [u8; 4]; index: u32 in Wrapping; flag: bool; }}
            machine Main::run(&mut self) {{
                self.index = 3;
                transition {{ _ -> fill() }}
                state fill(&mut self) {{
                    let value: u8 = self.items[self.index];
                    transition {{
                        self.index < {floor} -> done()
                        self.flag -> decrease()
                        _ -> decrease()
                    }}
                }}
                state decrease(&mut self) {{
                    self.index = self.index - 2;
                    transition {{ _ -> fill() }}
                }}
                state done(&mut self) {{}}
            }}"
            ),
            accepted,
        );
    }
}
