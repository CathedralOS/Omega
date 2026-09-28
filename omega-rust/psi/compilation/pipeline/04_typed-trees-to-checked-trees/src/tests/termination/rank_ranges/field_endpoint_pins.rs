use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

fn accepts(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_range(source: &str) {
    let diagnostics = lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .expect_err("computed field endpoints must form and preserve every exact input");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

/// A member chain through a stored shared reference reaches the referent's
/// declared bounds: `indirect.target.remaining` pins when the chain's prefix
/// is forwarded intact, and only then.
const PROJECTED_ENDPOINT: &str = r#"
    data Wrap { remaining: u64 [4..=500]; }
    data Indirect { target: &Wrap; }
    machine walk(n: u64 [0..=4], indirect: Indirect)
    terminates by n -> Nat::Descending in 0..=indirect.target.remaining;
    -> u64 {
        transition n > 0 {
            true -> walk(n - 1, Indirect { target: indirect.target })
            false -> n
        }
    }
"#;

#[test]
fn exclusive_borrow_endpoint_rejects_overlapping_writes_and_reseats() {
    let source = PROJECTED_ENDPOINT.replace("target: &Wrap;", "target: &mut Wrap;");
    for prefix in [
        "reset(indirect.target);",
        "let alias: &mut Wrap = indirect.target; reset(alias);",
    ] {
        let changed = format!(
            "machine reset(value: &mut Wrap) {{ value.remaining = 4; }} {}",
            source.replace(
                "        transition n > 0",
                &format!("        {prefix}\n        transition n > 0")
            ),
        );
        // With a constant endpoint these are lawful programs: this test must
        // witness endpoint preservation failure, not an invalid store or loan.
        accepts(&changed.replace("indirect.target.remaining;", "5;"));
        rejects_range(&changed);
    }
    let replacement = source
        .replace(
            "indirect: Indirect)",
            "indirect: Indirect, spare: &mut Wrap)",
        )
        .replace(
            "Indirect { target: indirect.target })",
            "Indirect { target: spare }, indirect.target)",
        );
    rejects_range(&replacement);
}

#[test]
fn member_chains_through_stored_references_supply_endpoint_bounds() {
    accepts(PROJECTED_ENDPOINT);
    // A different receiver's reference field does not preserve this input.
    rejects_range(
        &PROJECTED_ENDPOINT
            .replace(
                "indirect: Indirect)",
                "indirect: Indirect, spare: Indirect)",
            )
            .replace(
                "Indirect { target: indirect.target })",
                "Indirect { target: spare.target }, spare)",
            ),
    );
    // Rebinding the stored reference to another formal is a foreign referent.
    rejects_range(
        &PROJECTED_ENDPOINT
            .replace("indirect: Indirect)", "indirect: Indirect, spare: &Wrap)")
            .replace(
                "Indirect { target: indirect.target })",
                "Indirect { target: spare }, spare)",
            ),
    );
    // Write-only storage cannot supply an endpoint observation.
    let write_only =
        typed_program(&PROJECTED_ENDPOINT.replace("target: &Wrap;", "target: &write Wrap;"));
    let diagnostics = crate::checks::termination::check_machine_termination(&write_only)
        .expect_err("write-only endpoints cannot be read");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{diagnostics:#?}"
    );
}
