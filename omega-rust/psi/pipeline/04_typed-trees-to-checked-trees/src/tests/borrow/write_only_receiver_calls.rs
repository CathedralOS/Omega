use crate::tests::front_end::checked_program_result;

mod indexed;
mod projections;

fn caller_source(root: &str, body: &str, methods: &str) -> String {
    let signature = if root == "self" {
        "Record::exercise(&write self, replacement: u16)"
    } else {
        "exercise(destination: &write Record, replacement: u16)"
    };
    format!(
        "data Record {{ value: u16; }}
         {methods}
         machine {signature} {{ {body} }}"
    )
}

fn reject_source(source: &str, expected: &[&str]) {
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("source must fail semantic checking: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            expected
                .iter()
                .all(|fragment| diagnostic.message.contains(fragment))
        }),
        "expected diagnostic containing {expected:?}: {diagnostics:#?}\nsource:\n{source}"
    );
}

fn reject_receiver_requirement(receiver: &str) {
    for root in ["destination", "self"] {
        // An empty body still requires the declared receiver authority.
        let source = caller_source(
            root,
            &format!("{root}.exercise_access();"),
            &format!("machine Record::exercise_access({receiver}) {{}}"),
        );
        reject_source(
            &source,
            &[&format!("calls through write-only parameter `{root}`")],
        );
        let source = caller_source(
            root,
            &format!("let result: u16 = {root}.exercise_access();"),
            &format!("machine Record::exercise_access({receiver}) -> u16 {{ 17 }}"),
        );
        reject_source(
            &source,
            &[
                &format!("reads write-only parameter `{root}`"),
                "never observation",
            ],
        );
    }
}

#[test]
fn write_only_receiver_cannot_satisfy_mutable_receiver_requirement() {
    reject_receiver_requirement("&mut self");
}
