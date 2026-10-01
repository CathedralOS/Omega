//! Authored `<'a>` machine binders erase at checking: the composed plan's
//! type identities and custody paths never spell the lifetime name, so a
//! lifetime-declared machine admits the same graph route as its plain twin.
//! Generic `<T>` binders still need instantiation machinery and stay out.
use crate::{TerminalMachineSelection, lower_machine};

fn view_return_decline(binder: &str, result: &str) -> &'static str {
    let checked = crate::front_end::checked_program(&format!(
        r#"
            data Store {{ bytes: [u8; 64]; }}

            machine Store::view{binder}(&self, n: u64) -> {result} {{
                transition (n <= self.bytes.len) {{
                    true -> (self.bytes[0..n])
                    false -> (self.bytes[0..64])
                }}
            }}
        "#
    ));
    match lower_machine(&checked, TerminalMachineSelection::Name("Store::view")) {
        Err(crate::lowering_error::LoweringError::Unsupported(message)) => message,
        Err(error) => panic!("unexpected decline: {error:?}"),
        Ok(_) => panic!("borrowed view results still need the view return arm"),
    }
}

#[test]
fn lifetime_declared_view_return_reaches_same_frontier_as_plain_twin() {
    // `&'a [u8]` results admit the same plan route as `&[u8]` results: the
    // shared frontier is the borrowed-view return arm's producer, not the
    // binder declaration.
    let lifetime = view_return_decline("<'a>", "&'a [u8]");
    assert_eq!(lifetime, view_return_decline("", "&[u8]"));
    assert_eq!(lifetime, "ordered return lost its structural producer");
}
