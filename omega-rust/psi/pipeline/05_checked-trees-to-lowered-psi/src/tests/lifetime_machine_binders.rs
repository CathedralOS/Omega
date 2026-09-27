//! Authored `<'a>` machine binders erase at checking: the composed plan's
//! type identities and custody paths never spell the lifetime name, so a
//! lifetime-declared machine admits the same graph route as its plain twin.
//! Generic `<T>` binders still need instantiation machinery and stay out.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn lifetime_declared_view_return_reaches_same_frontier_as_plain_twin() {
    // `&'a [u8]` results admit the same plan route as `&[u8]` results: the
    // shared frontier is the borrowed-view return destination arm, not the
    // binder declaration.
    let checked = crate::front_end::checked_program(
        r#"
            data Store { bytes: [u8; 64]; }

            machine Store::view<'a>(&self, n: u64) -> &'a [u8] {
                transition (n <= self.bytes.len) {
                    true -> (self.bytes[0..n])
                    false -> (self.bytes[0..64])
                }
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Store::view"))
        .expect_err("borrowed view results still need the view destination arm");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::Unsupported(message)
                if message.contains("ordered structural destination")
        ),
        "unexpected decline: {error:?}"
    );
}
