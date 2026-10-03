use crate::tests::front_end::typed_program;

#[test]
fn non_decreasing_named_edges_cannot_hide_beside_descending_edges() {
    for source in [
        "machine walk(remaining: u32) -> u32
         requires remaining <= 5
         terminates by remaining -> Nat::Descending;
         { transition remaining > 0 {
             true -> walk(remaining - 1) false -> walk(remaining)
         } }",
        "machine walk(remaining: u32) -> u32
         requires remaining <= 5
         terminates by remaining -> Nat::Descending;
         { transition remaining == 0 {
             true -> walk(remaining) false -> walk(remaining - 1)
         } }",
        "machine walk(remaining: u32) -> u32
         requires remaining <= 5
         terminates by remaining -> Nat::Descending;
         {
             transition { _ -> finish(remaining) }
             state finish(result: u32) { transition { _ -> finish(result) } }
         }",
    ] {
        let diagnostics =
            crate::checks::termination::check_machine_termination(&typed_program(source))
                .expect_err("every recursive edge requires its own strict descent");
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
