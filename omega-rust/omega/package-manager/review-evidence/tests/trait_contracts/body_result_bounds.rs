use crate::support::*;
use compiler::CheckedCompileRequest;

// These are ordinary executable bodies, not proofs by an authored assumption.
// Squalr's scale tables and magnitude ladders need the same exact judgment to
// survive fresh package capture and obligation reconstruction after checking.
#[test]
fn body_result_bounds_close_package_obligations_without_assumption_certificates() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Scale {}
pub machine Scale::table(position: u64) -> u64
    requires position <= 2
    ensures 1 <= result && result <= 18446744073709551615
{
    let scales: [u64; 3] = [1, 1000, 1000000];
    transition scales[position] >= 1 {
        true -> (scales[position])
        false -> (1)
    }
}
pub machine Scale::place(value: u64) -> u64
    ensures 0 <= result && result <= 6
{
    transition value >= 1000000 {
        true -> (6)
        false -> smaller(value)
    }
    state smaller(value: u64) -> u64 {
        transition value >= 1000 {
            true -> (3)
            false -> smallest(value)
        }
    }
    state smallest(value: u64) -> u64 {
        transition value >= 1 {
            true -> (1)
            false -> (0)
        }
    }
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("guarded table reads and every ladder return satisfy their bounds");
    assert!(
        checked.custody.contract_entailment_stand_downs().is_empty(),
        "source checking must judge both complete bodies, not defer their proofs"
    );
    let projection = project_checked_package_review(&checked)
        .expect("fresh package capture independently rechecks the same bodies");
    assert!(projection.contract_entailment_open_obligations().is_empty());
    assert!(
        projection
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
    let results = reconstruct_ordinary_package_obligation_results(&checked)
        .expect("ordinary package replay retains the actual body judgments");
    assert!(results.open_contract_entailment_obligations().is_empty());
    assert!(
        results
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
}
