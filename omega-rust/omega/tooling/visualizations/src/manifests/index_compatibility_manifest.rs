//! Indexed-domain compatibility conditions and their retained discharge routes.

use crate::encoding::manifest_coordinates::{
    exact_program_point_label, qualification_symbol_label,
};
use crate::encoding::manifest_values::push_json_string;
use checked_trees::CheckedTrees;

/// Public PDI3 compatibility surface. The named condition and its exact
/// discharge route are retained independently of indexed-domain identity.
pub fn index_compatibility_manifest_json(program: &CheckedTrees) -> String {
    use checked_trees::IndexCompatibilityDischarge;

    let mut rows = program
        .facts
        .index_compatibility
        .conditions
        .iter()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.name.cmp(&right.name));

    let mut json = String::from("{\n  \"index_compatibility\": [");
    for (index, condition) in rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let (route, operation_count, evidence_facts) = match &condition.discharge {
            IndexCompatibilityDischarge::ClosedEvaluation => {
                ("closed_evaluation", None, Vec::new())
            }
            IndexCompatibilityDischarge::LicensedNormalization { operation_count } => {
                ("licensed_normalization", Some(*operation_count), Vec::new())
            }
            IndexCompatibilityDischarge::EstablishedLocalFacts { facts } => {
                ("established_local_fact", None, facts.clone())
            }
        };
        json.push_str("\n    {\n      \"name\": ");
        push_json_string(&mut json, &condition.name);
        json.push_str(",\n      \"program_point\": ");
        push_json_string(
            &mut json,
            &exact_program_point_label(program, condition.point),
        );
        json.push_str(",\n      \"family\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, condition.family),
        );
        json.push_str(",\n      \"actual_instance\": ");
        json.push_str(&condition.actual_instance.0.to_string());
        json.push_str(",\n      \"expected_instance\": ");
        json.push_str(&condition.expected_instance.0.to_string());
        json.push_str(",\n      \"actual_expression\": ");
        push_json_string(&mut json, &condition.actual_label);
        json.push_str(",\n      \"expected_expression\": ");
        push_json_string(&mut json, &condition.expected_label);
        json.push_str(",\n      \"discharge\": ");
        push_json_string(&mut json, route);
        json.push_str(",\n      \"operation_count\": ");
        match operation_count {
            Some(count) => json.push_str(&count.to_string()),
            None => json.push_str("null"),
        }
        json.push_str(",\n      \"evidence_facts\": [");
        for (index, fact) in evidence_facts.iter().enumerate() {
            if index > 0 {
                json.push_str(", ");
            }
            json.push_str(&fact.arena_index().to_string());
        }
        json.push(']');
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}
