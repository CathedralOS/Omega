use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_program(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved index accepted: {source}"),
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

fn loop_source(declaration: &str) -> String {
    format!(
        "{declaration}
        data Main {{ items: [u8; 4]; index: i32 in Wrapping; }}
        machine Main::run(&mut self) {{
            self.index = 0;
            transition {{ _ -> fill() }}
            state fill(&mut self) {{
                let value: u8 = self.items[self.index];
                self.index = self.index + 1;
                transition self.index < 4 {{ true -> fill() _ -> {{}} }}
            }}
        }}"
    )
}

#[test]
fn loop_bounds_retain_comparison_and_increment_meaning() {
    for declaration in [
        "operator < i32::custom(left: i32 in Wrapping, right: i32 in Wrapping) -> bool;",
        "operator + i32::custom(left: i32 in Wrapping, right: i32 in Wrapping) -> i32 in Wrapping;",
    ] {
        check(&loop_source(declaration), false);
    }
}
