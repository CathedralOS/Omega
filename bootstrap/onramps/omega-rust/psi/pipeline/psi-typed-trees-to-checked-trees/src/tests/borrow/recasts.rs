use super::checks::check_program;

#[test]
fn mutable_whole_place_recast_retains_source_loan() {
    let source = r#"
        data Cell {
            value: u64;
        }

        machine Cell::exercise(&mut self) {
            let view: &mut f64 = &mut self.value as &mut f64;
            self.value = 1;
            view = 2.0;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a mutable whole-place recast must retain its source");
    assert_conflict(&diagnostics, "self.value", "view");
}

#[test]
fn shared_whole_place_recast_retains_source_loan() {
    let source = r#"
        data Cell {
            value: u64;
        }

        machine observe(value: &f64) {
        }

        machine Cell::exercise(&mut self) {
            let view: &f64 = &self.value as &f64;
            self.value = 1;
            observe(view);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a shared whole-place recast must retain its source");
    assert_conflict(&diagnostics, "self.value", "view");
}

#[test]
fn whole_member_recast_keeps_disjoint_sibling_writable() {
    let source = r#"
        data Pair {
            left: u64;
            right: u64;
        }

        machine Pair::exercise(&mut self) {
            let view: &mut f64 = &mut self.left as &mut f64;
            self.right = 1;
            view = 2.0;
        }
    "#;

    check_program(source).expect("a whole-member recast must not capture a disjoint sibling");
}

#[test]
fn whole_member_recast_rejects_overlapping_write() {
    let source = r#"
        data Pair {
            left: u64;
            right: u64;
        }

        machine Pair::exercise(&mut self) {
            let view: &mut f64 = &mut self.left as &mut f64;
            self.left = 1;
            view = 2.0;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a whole-member recast must retain the exact field loan");
    assert_conflict(&diagnostics, "self.left", "view");
}

fn assert_conflict(diagnostics: &[psi_diagnostics::Diagnostic], source: &str, owner: &str) {
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(&format!(
            "mutates `{source}` while local borrow `{owner}` is still active"
        )),
        "expected recast loan conflict, got:\n{combined}"
    );
}
