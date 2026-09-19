//! Mathematical `let`/`boundary let` declarations already resolve, but
//! typed-tree elaboration is a later PROOF-CONTRACT-MIGRATION leg. Until it
//! lands, the boundary must refuse loudly rather than silently dropping a
//! resolved declaration.

use crate::lowerer::tests::lower_source;

#[test]
fn let_definition_refuses_loudly_until_elaboration_exists() {
    let diagnostic =
        lower_source("let double(x: u64): u64 = x;").expect_err("typed elaboration is pending");
    assert!(
        diagnostic
            .message
            .contains("typed-tree elaboration for them is not implemented yet"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
    assert!(diagnostic.message.contains("PROOF-CONTRACT-MIGRATION"));
}

#[test]
fn boundary_let_refuses_loudly_until_elaboration_exists() {
    let diagnostic = lower_source("boundary let choose(inhabited: u64): u64;")
        .expect_err("typed elaboration is pending");
    assert!(
        diagnostic.message.contains("PROOF-CONTRACT-MIGRATION"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}
