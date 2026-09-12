use super::*;
use crate::CheckedCompileRequest;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn compile(source: &str) -> super::super::CheckedCompilation {
    let root = std::env::temp_dir().join(format!(
        "omega-selected-const-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("main.omg"), source).unwrap();
    let result = super::super::compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("macos_arm64"),
    ));
    fs::remove_dir_all(root).unwrap();
    result.expect("selected semantic evaluation")
}

const SOURCE: &str = r#"
use omega::language::core::float_operations;
machine multiply(left:f64, right:f64) -> f64 { left * right }
machine length() -> u64 {
    let product:f64 = multiply(2.0, 3.0);
    transition product == 6.0 { true -> 4 _ -> 5 }
}
data Main { bytes:[u8;length()]; }
"#;

#[test]
fn selected_float_fold_replays_exact_provider_operator_and_operand_custody() {
    let checked = compile(SOURCE);
    let custody = &checked.const_evaluation;
    assert_eq!(custody.folds.len(), 1);
    assert!(custody.operators.len() >= 2);
    let replay = |candidate: &SelectedConstEvaluation| {
        candidate.validate(&checked.program, &checked.selected_provider_plans, None)
    };
    assert!(replay(custody).is_ok());
    let mut wrong_provider = custody.clone();
    wrong_provider.operators[0].provider = CheckedProviderPlanCommitment::from_digest([99; 32]);
    assert!(replay(&wrong_provider).is_err());
    let mut wrong_requirement = custody.clone();
    wrong_requirement.operators[0].requirement = wrong_requirement.operators[1].requirement;
    assert!(replay(&wrong_requirement).is_err());
    let mut wrong_operands = custody.clone();
    wrong_operands.operators[0].operands.swap(0, 1);
    assert!(replay(&wrong_operands).is_err());
    let mut missing = custody.clone();
    missing.operators.clear();
    assert!(replay(&missing).is_err());
    let mut duplicate = custody.clone();
    duplicate.operators.push(duplicate.operators[0]);
    assert!(replay(&duplicate).is_err());
    let mut duplicate_origin = custody.clone();
    let mut substituted = duplicate_origin.operators[0];
    if let checked_trees::CheckedValueOrigin::StateStatement {
        statement_index, ..
    } = &mut substituted.origin
    {
        *statement_index += 1;
    }
    duplicate_origin.operators.push(substituted);
    assert!(
        build_time_evaluation::validate_selected_operators(
            &checked.program.typed,
            &duplicate_origin.operators
        )
        .is_err()
    );
    let mut wrong_type = checked.program.clone();
    let owner = wrong_type
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap()
        .clone();
    let member = wrong_type
        .typed
        .tables
        .data_members
        .get_mut(owner.members.start());
    let typed_trees::data::DataMember::Field(field) = member else {
        panic!("bytes field");
    };
    field.type_reference = typed_trees::types::TypeReferenceHandle::invalid();
    assert!(
        custody
            .validate(&wrong_type, &checked.selected_provider_plans, None)
            .is_err()
    );
}

#[test]
fn selected_float_evaluation_uses_f32_rounding_and_right_operand_policy() {
    let checked = compile(
        r#"
use omega::language::core::float_operations;
machine narrow_length() -> u64 {
    let left:f32 = 16777216.0;
    let right:f32 = 1.0;
    transition left + right == 16777216.0 { true -> 4 _ -> 5 }
}
machine saturated_length() -> u64 {
    let right:f64 in Saturating = 2.0;
    transition 1.7976931348623157e308 * right == 1.7976931348623157e308 { true -> 4 _ -> 5 }
}
data Narrow { bytes:[u8;narrow_length()]; }
data Saturated { bytes:[u8;saturated_length()]; }
"#,
    );
    for owner in ["Narrow", "Saturated"] {
        let data = checked
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == owner)
            .unwrap();
        let typed_trees::data::DataMember::Field(field) = &checked.data_members(data)[0] else {
            panic!("bytes field");
        };
        assert!(
            checked
                .type_reference_table
                .fixed_array_lengths()
                .any(|(handle, length)| handle == field.type_reference
                    && *length == typed_trees::types::FixedArrayLength::Literal(4)),
            "{owner}"
        );
    }
}
