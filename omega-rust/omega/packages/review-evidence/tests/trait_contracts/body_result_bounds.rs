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
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("guarded table reads and every ladder return satisfy their bounds");
    assert!(
        checked
            .custody
            .execution()
            .contract_entailment_stand_downs
            .as_slice()
            .is_empty(),
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

#[test]
fn widened_reads_and_unrelated_calls_close_fresh_package_obligations() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data ByteBuffer { bytes: [u8; 2]; }
pub machine ByteBuffer::byte_at(&mut self, index: u64) -> u64
    ensures 0 <= result && result < 256
{
    transition index < 2 {
        true -> hit(index)
        _ -> miss()
    }
    state hit(&mut self, index: u64) -> u64 { self.bytes[index] as u64 }
    state miss(&mut self) -> u64 { 0 }
}
pub machine ByteBuffer::slice_byte(&mut self, values: &[u8], position: u64) -> u64
    requires position < values.len
    ensures 0 <= result && result < 256
{
    values[position] as u64
}
pub data Region { base: u64; size: u64; }
data EndAddress { value: u64; }
machine Region::end_address(&self) -> EndAddress {
    EndAddress { value: ((self.base as u64 in Saturating) + (self.size as u64 in Saturating)) as u64 }
}
pub machine Region::contains(&self, address: u64) -> u64
    ensures 0 <= result && result < 2
{
    transition { _ -> check(address) }
    state check(&self, address: u64) -> u64 {
        let end: EndAddress = self.end_address();
        transition address >= self.base && address <= end.value {
            true -> (1)
            false -> (0)
        }
    }
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("ordinary return bounds survive indexed widening and unrelated calls");
    assert!(
        checked
            .custody
            .execution()
            .contract_entailment_stand_downs
            .as_slice()
            .is_empty()
    );
    let projection = project_checked_package_review(&checked)
        .expect("fresh capture independently checks the ordinary return bodies");
    assert!(projection.contract_entailment_open_obligations().is_empty());
    assert!(
        projection
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
    let results = reconstruct_ordinary_package_obligation_results(&checked)
        .expect("package replay closes the exact return guarantees");
    assert!(results.open_contract_entailment_obligations().is_empty());
    assert!(
        results
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
}
