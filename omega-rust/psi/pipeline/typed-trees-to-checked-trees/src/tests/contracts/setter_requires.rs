//! A setter's `requires` bounds the value it stores: a field whose domain is
//! an interval accepts a parameter the contract confines to it. The contract
//! is an assumption only while the state prefix leaves its reads unchanged.

use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str) -> Result<(), Vec<String>> {
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .map(|_| ())
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect()
        })
}

fn rejects_the_store(source: &str) {
    let diagnostics = check(source).expect_err("the store must not prove");
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("cannot prove assignment value")),
        "{diagnostics:#?}"
    );
}

const FIELDS: &str = "
    domain i32::Level requires self >= 0 && self <= 1000;
    data Inner [copy] { value: i32 in Level; }
    data Outer [copy] { inner: Inner; }
";

#[test]
fn a_setter_requires_bounds_a_nested_interval_field_store() {
    check(&format!(
        "{FIELDS}
        data Main {{ state: Outer; }}
        machine Main::set(&mut self, v: i32)
        requires v >= 0 && v <= 1000;
        {{ self.state.inner.value = v; }}"
    ))
    .unwrap();
}

#[test]
fn a_requires_wider_than_the_field_does_not_prove_the_store() {
    rejects_the_store(&format!(
        "{FIELDS}
        data Main {{ state: Outer; }}
        machine Main::set(&mut self, v: i32)
        requires v >= 0 && v <= 1001;
        {{ self.state.inner.value = v; }}"
    ));
}

#[test]
fn a_write_to_the_required_place_retires_its_bound() {
    let program = |prefix: &str| {
        format!(
            "{FIELDS}
            data Main {{ source: i32; state: Outer; }}
            machine Main::copy(&mut self, other: i32)
            requires self.source >= 0 && self.source <= 1000;
            {{ {prefix} self.state.inner.value = self.source; }}"
        )
    };
    check(&program("")).unwrap();
    rejects_the_store(&program("self.source = other;"));
}
