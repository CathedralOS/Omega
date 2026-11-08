// Zeroed `Build.optimizations` field construction and post-evaluation counter extraction.
//
// Each optimization pass and the `human_report` request is represented as an integer
// activation counter in `Build.optimizations`. A counter value of `0` leaves the
// pass/report disabled, `1` enables it, and `> 1` or `< 0` rejects so duplicate
// or malformed enablement in `build.omg` fails closed.

use build_time_evaluation::BuildTimeValue;
use optimization_core::{Optimization, OptimizationSelections};

use super::vocabulary::OptimizationBuildVocabulary;

pub(super) fn zero_build_field(
    vocabulary: OptimizationBuildVocabulary,
) -> Option<(String, BuildTimeValue)> {
    (vocabulary == OptimizationBuildVocabulary::Canonical).then(|| {
        (
            "optimizations".to_owned(),
            BuildTimeValue::Struct {
                type_name: "Optimizations".to_owned(),
                fields: vec![
                    ("human_report".to_owned(), BuildTimeValue::Int(0)),
                    (
                        "selected".to_owned(),
                        BuildTimeValue::Array(vec![
                            BuildTimeValue::Int(0);
                            Optimization::ALL.len()
                        ]),
                    ),
                ],
            },
        )
    })
}

pub(super) fn extract(
    build: &BuildTimeValue,
    vocabulary: OptimizationBuildVocabulary,
) -> Result<
    (
        OptimizationSelections,
        optimization_core::OptimizationReportRequest,
    ),
    String,
> {
    if vocabulary == OptimizationBuildVocabulary::LegacyWithoutField {
        return Ok((
            OptimizationSelections::default(),
            optimization_core::OptimizationReportRequest::Suppressed,
        ));
    }
    let BuildTimeValue::Struct { fields, .. } = build else {
        return Err(format!("expected a Build struct, got {build:?}"));
    };
    let value = fields
        .iter()
        .find(|(field, _)| field == "optimizations")
        .map(|(_, value)| value)
        .ok_or_else(|| "the Build carries no `optimizations` field".to_owned())?;
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err("Build.optimizations is not an Optimizations value".to_owned());
    };
    if type_name != "Optimizations" {
        return Err(format!(
            "Build.optimizations has nominal type `{type_name}` instead of `Optimizations`"
        ));
    }
    let Some((_, report_count)) = fields.iter().find(|(field, _)| field == "human_report") else {
        return Err("Build.optimizations carries no `human_report` request counter".to_owned());
    };
    let BuildTimeValue::Int(report_count) = report_count else {
        return Err(format!(
            "Build.optimizations.human_report is not an integer request counter: {report_count:?}"
        ));
    };
    let report = match *report_count {
        0 => optimization_core::OptimizationReportRequest::Suppressed,
        1 => optimization_core::OptimizationReportRequest::EmitHumanText,
        count if count > 1 => {
            return Err("optimization human report is requested more than once".to_owned());
        }
        count => {
            return Err(format!(
                "Build.optimizations.human_report has invalid negative request count {count}"
            ));
        }
    };
    let Some((_, counters)) = fields.iter().find(|(field, _)| field == "selected") else {
        return Err("Build.optimizations carries no `selected` counter array".to_owned());
    };
    let BuildTimeValue::Array(counters) = counters else {
        return Err(format!(
            "Build.optimizations.selected is not a counter array: {counters:?}"
        ));
    };
    if counters.len() != Optimization::ALL.len() {
        return Err(format!(
            "Build.optimizations.selected has {} counters, expected {}",
            counters.len(),
            Optimization::ALL.len()
        ));
    }
    let mut selected = Vec::new();
    for (optimization, value) in Optimization::ALL.into_iter().zip(counters) {
        let name = optimization.build_case_name();
        let BuildTimeValue::Int(count) = value else {
            return Err(format!(
                "Build.optimizations.selected counter for `{name}` is not an integer: {value:?}"
            ));
        };
        match *count {
            0 => {}
            1 => selected.push(optimization),
            count if count > 1 => {
                return Err(format!(
                    "optimization `{}` is enabled more than once",
                    optimization.build_case_name()
                ));
            }
            count => {
                return Err(format!(
                    "Build.optimizations.selected counter for `{name}` has invalid negative count {count}"
                ));
            }
        }
    }
    Ok((
        OptimizationSelections::new(selected).map_err(|error| error.to_string())?,
        report,
    ))
}
