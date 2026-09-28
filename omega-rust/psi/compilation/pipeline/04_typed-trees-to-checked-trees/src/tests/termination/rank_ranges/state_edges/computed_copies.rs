use crate::tests::front_end::typed_program;

#[test]
fn explicit_self_occurrences_cannot_hide_beside_descending_edges() {
    for source in [
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining > 0 {
             true -> walk(remaining - 1) false -> self
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining == 0 {
             true -> self false -> walk(remaining - 1)
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 {
             transition { _ -> finish(remaining) }
             state finish(result: u32 [0..=5]) { transition { _ -> self } }
         }",
    ] {
        let diagnostics =
            crate::checks::termination::check_machine_termination(&typed_program(source))
                .expect_err("every self occurrence requires its own strict descent");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove rank range")
                    || diagnostic
                        .message
                        .contains("cannot prove the `terminates by` ranking")
            }),
            "{source}\n{diagnostics:#?}"
        );
        assert!(
            super::super::lower_typed_trees(
                typed_program(source),
                &crate::CheckingRequest::settled()
            )
            .is_err()
        );
    }
}
