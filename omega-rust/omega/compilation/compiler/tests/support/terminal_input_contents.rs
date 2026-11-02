//! Supply authored scalar/case entry contents to real source-produced code.

use super::result_observation::{case_id, resolve_path};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
    TerminalStructuralContents, TerminalStructuralInputs, TerminalStructuralValue,
};

fn describe(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    expected: &str,
    contents: &[&str],
) -> Result<(), String> {
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).map_err(describe)?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        let entry = program.entry().map_err(describe)?;
        let [parameter] = entry.structural_parameters else {
            return Err("one structural entry required".into());
        };
        let root = parameter.structural_type;
        let arguments = [TerminalStructuralValue::minted(791, root, Vec::new())];
        let mut values = Vec::new();
        for contents in contents {
            let (path, value) = contents.split_once('=').ok_or("path=value required")?;
            let path = resolve_path(&program, Some(root), path)?;
            let contents = if let Some(name) = value.strip_prefix("case:") {
                TerminalStorageContents::Case(case_id(&program, root, &path, name)?)
            } else if let Ok(value) = value.parse::<bool>() {
                TerminalStorageContents::Scalar {
                    value: TerminalScalarValue::Boolean(value),
                    stored_scalar: ScalarType::Boolean,
                }
            } else {
                let (value, width) = value.split_once('@').ok_or("value@width required")?;
                let integer =
                    IntegerType::new(IntegerSign::Unsigned, width.parse().map_err(describe)?)
                        .map_err(describe)?;
                TerminalStorageContents::Scalar {
                    value: TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(value.parse().map_err(describe)?),
                    },
                    stored_scalar: ScalarType::Integer(integer),
                }
            };
            values.push(TerminalBoundaryStorageValue { path, contents });
        }
        values.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        let supplied = [TerminalStructuralContents {
            argument_index: 0,
            values,
        }];
        if let Some(reason) = expected.strip_prefix("reject:") {
            reject(
                &program,
                TerminalStructuralInputs {
                    arguments: &arguments,
                    contents: &supplied,
                    ..Default::default()
                },
                100_000,
                &reason.replace('_', " "),
            )?;
            continue;
        }
        let expected: u128 = expected.parse().map_err(describe)?;
        execute(&program, &arguments, &supplied, expected)?;
        // A well-formed changed input is accepted, but the guest must detect
        // its changed canary. This challenges the runtime oracle itself.
        let mut changed = supplied.clone();
        let before = resolve_path(&program, Some(root), "before")?;
        let scalar = changed[0]
            .values
            .iter_mut()
            .find(|value| value.path == before)
            .ok_or("before canary missing")?;
        let TerminalStorageContents::Scalar {
            value:
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                },
            ..
        } = &mut scalar.contents
        else {
            return Err("before canary must be unsigned".into());
        };
        *value += 1;
        execute(&program, &arguments, &changed, 3)?;

        for mutation in [
            "missing",
            "duplicate",
            "inactive",
            "unknown-case",
            "stored-width",
            "logical-type",
            "out-of-bounds",
            "duplicate-root",
            "zero-overlap",
            "storage-limit",
        ] {
            let mut changed = supplied.to_vec();
            match mutation {
                "missing" => {
                    changed[0].values.pop();
                }
                "duplicate" => {
                    let duplicate = changed[0].values[0].clone();
                    changed[0].values.push(duplicate);
                }
                "inactive" => {
                    let (pair_path, _) = contents
                        .iter()
                        .filter_map(|value| value.split_once('='))
                        .find(|(_, value)| *value == "case:Pair")
                        .ok_or("payload case required")?;
                    let path = resolve_path(&program, Some(root), pair_path)?;
                    let case = case_id(&program, root, &path, "Scalar")?;
                    changed[0]
                        .values
                        .iter_mut()
                        .find(|value| value.path == path)
                        .ok_or("case missing")?
                        .contents = TerminalStorageContents::Case(case);
                }
                "unknown-case" => {
                    let value = changed[0]
                        .values
                        .iter_mut()
                        .find(|value| matches!(value.contents, TerminalStorageContents::Case(_)))
                        .ok_or("case missing")?;
                    value.contents = TerminalStorageContents::Case(
                        semantic_vocabulary::StructuralCaseId::new(u64::MAX).unwrap(),
                    );
                }
                "stored-width" => {
                    let value = changed[0]
                        .values
                        .iter_mut()
                        .find(|value| value.path == before)
                        .ok_or("canary missing")?;
                    let TerminalStorageContents::Scalar { stored_scalar, .. } = &mut value.contents
                    else {
                        return Err("scalar required".into());
                    };
                    *stored_scalar =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
                }
                "logical-type" => {
                    let value = changed[0]
                        .values
                        .iter_mut()
                        .find(|value| value.path == before)
                        .ok_or("canary missing")?;
                    let TerminalStorageContents::Scalar { value, .. } = &mut value.contents else {
                        return Err("scalar required".into());
                    };
                    *value = TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                        value: IntegerValue::Signed(19),
                    };
                }
                "out-of-bounds" => {
                    let value = changed[0]
                        .values
                        .iter_mut()
                        .find(|value| {
                            matches!(value.contents, TerminalStorageContents::Scalar { .. })
                                && value
                                    .path
                                    .iter()
                                    .any(|step| matches!(step, Path::FixedIndex(_)))
                        })
                        .ok_or("indexed value missing")?;
                    let step = value
                        .path
                        .iter_mut()
                        .find(|step| matches!(step, Path::FixedIndex(_)))
                        .ok_or("index missing")?;
                    *step = Path::FixedIndex(u64::MAX);
                    changed[0]
                        .values
                        .sort_unstable_by(|left, right| left.path.cmp(&right.path));
                }
                "duplicate-root" => {
                    changed.push(changed[0].clone());
                }
                _ => {}
            }
            let intended = if mutation == "storage-limit" {
                "AllowanceExhausted(LiveStorage)"
            } else if mutation == "unknown-case" {
                "describe complete entry contents"
            } else {
                "StructuralContentsArgumentInvalid"
            };
            reject(
                &program,
                TerminalStructuralInputs {
                    arguments: &arguments,
                    contents: &changed,
                    zero_initialized_arguments: if mutation == "zero-overlap" {
                        &[0]
                    } else {
                        &[]
                    },
                    ..Default::default()
                },
                if mutation == "storage-limit" {
                    1
                } else {
                    100_000
                },
                intended,
            )
            .map_err(|error| format!("{mutation}: {error}"))?;
        }
    }
    Ok(())
}

fn reject(
    program: &TerminalProgram<'_>,
    structural_inputs: TerminalStructuralInputs<'_>,
    cells: u64,
    reason: &str,
) -> Result<(), String> {
    let sponsor = sponsor(cells)?;
    let account = sponsor
        .begin_invocation(measurement(), 1_000_000)
        .map_err(describe)?;
    let error = match program.instantiate(TerminalInstantiation {
        structural_inputs,
        account: Some(&account),
        ..Default::default()
    }) {
        Ok(_) => return Err("invalid complete input accepted".into()),
        Err(error) => describe(error),
    };
    if !error.contains(reason) {
        return Err(format!("expected {reason}, observed {error}"));
    }
    if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
        return Err("rejected input retained sponsored storage".into());
    }
    Ok(())
}

fn measurement() -> EvaluationMeasurementIdentity {
    EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    }
}

fn sponsor(cells: u64) -> Result<BuildEvaluationSponsor, String> {
    BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(1_000_000, 64, 64, 64, cells, 100_000, 100_000, 100_000)
            .ok_or("invalid input sponsor limits")?,
        measurement(),
    )
    .map_err(describe)
}

fn execute(
    program: &TerminalProgram<'_>,
    arguments: &[TerminalStructuralValue],
    contents: &[TerminalStructuralContents],
    expected: u128,
) -> Result<(), String> {
    let sponsor = sponsor(100_000)?;
    let account = sponsor
        .begin_invocation(measurement(), 1_000_000)
        .map_err(describe)?;
    let mut instance = program
        .instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments,
                contents,
                ..Default::default()
            },
            account: Some(&account),
            ..Default::default()
        })
        .map_err(describe)?;
    let mut meter = TerminalFuelMeter::with_invocation(account, Some(0)).map_err(describe)?;
    if !matches!(
        instance.invoke(&mut meter).map_err(describe)?,
        TerminalExecutionStatus::SponsorExhausted(_)
    ) {
        return Err("input execution ignored fuel pause".into());
    }
    meter.replenish(1_000_000).map_err(describe)?;
    let result = instance.invoke(&mut meter).map_err(describe)?;
    if !matches!(result, TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer { value: IntegerValue::Unsigned(actual), .. })) if actual == expected)
    {
        return Err(format!("expected scalar {expected}, observed {result:?}"));
    }
    drop(result);
    instance.close().map_err(describe)?;
    if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
        return Err("closing initialized input retained sponsored storage".into());
    }
    Ok(())
}
