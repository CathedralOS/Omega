//! Source-produced results exercise cumulative publication separately from live
//! storage: exact/short grants, sibling invocations, cached observations and
//! sticky refusal. Both borrowed and loaded artifacts use the public SDK.
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
    EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::IntegerValue;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstanceCloseError,
    TerminalInstanceError, TerminalInstantiation, TerminalInterpretError, TerminalProgram,
    TerminalScalarValue,
};

fn describe(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    cells: &str,
    bytes: &str,
    oracle: &[&str],
) -> Result<(), String> {
    let cells: u64 = cells.parse().map_err(describe)?;
    let bytes: u64 = bytes.parse().map_err(describe)?;
    if cells == 0 {
        return Err("result envelope requires a cell".into());
    }
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).map_err(describe)?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        let mut arguments = Vec::new();
        let mut byte_arrays = Vec::new();
        if let ["structural", "input-bytes", bytes] = oracle {
            let entry = program.entry().map_err(describe)?;
            let [parameter] = entry.structural_parameters else {
                return Err("expected one byte argument".into());
            };
            arguments.push(terminal_interpreter::TerminalStructuralValue::minted(
                79,
                parameter.structural_type,
                Vec::new(),
            ));
            byte_arrays.push(terminal_interpreter::TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: Vec::new(),
                bytes: bytes.as_bytes().to_vec(),
            });
        }
        let mut ceilings = vec![(cells, bytes.max(1), true)];
        if cells > 1 {
            ceilings.push((cells - 1, bytes.max(1), false));
        }
        if bytes > 1 {
            ceilings.push((cells, bytes - 1, false));
        }
        for (cell_limit, byte_limit, first_succeeds) in ceilings {
            let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            };
            let sponsor = BuildEvaluationSponsor::new_with_measurement(
                BuildEvaluationSponsorLimits::new(
                    100_000, 8, 2, 2, 100_000, 100_000, cell_limit, byte_limit,
                )
                .ok_or("invalid limits")?,
                measurement,
            )
            .map_err(describe)?;
            for invocation in 0..3 {
                let account = sponsor
                    .begin_invocation(measurement, 100_000)
                    .map_err(describe)?;
                let mut instance = program
                    .instantiate(TerminalInstantiation {
                        account: Some(&account),
                        structural_inputs: terminal_interpreter::TerminalStructuralInputs {
                            arguments: &arguments,
                            byte_arrays: &byte_arrays,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .map_err(describe)?;
                let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0))
                    .map_err(describe)?;
                let result = loop {
                    match instance.invoke(&mut meter) {
                        Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {
                            meter.replenish(1).map_err(describe)?
                        }
                        result => break result,
                    }
                };
                if first_succeeds && invocation == 0 {
                    let Ok(TerminalExecutionStatus::Complete(ref value)) = result else {
                        return Err(format!("expected completed result: {result:?}"));
                    };
                    verify_result(value, oracle)?;
                    let retained = value.clone();
                    let retained_cells = match &retained {
                        TerminalExecutionResult::ScalarArray(value) => value.value.elements().len(),
                        TerminalExecutionResult::ScalarCase(value) => value.value().fields.len(),
                        _ => 0,
                    } as u64;
                    let usage = *account.snapshot().map_err(describe)?.record();
                    if instance.result() != Some(value)
                        || instance.invoke(&mut meter) != result
                        || *account.snapshot().map_err(describe)?.record() != usage
                    {
                        return Err("cached result changed value or charged again".into());
                    }
                    if sponsor.live_payload_bytes() < bytes {
                        return Err("completed result released retained byte backing".into());
                    }
                    drop(result);
                    instance.close().map_err(describe)?;
                    if sponsor.live_storage_cells() != retained_cells {
                        return Err("inspection payload lost or duplicated its live reservation after close".into());
                    }
                    drop(retained);
                } else {
                    let Err(ref refusal) = result else {
                        return Err(format!("expected result allowance refusal: {result:?}"));
                    };
                    let TerminalInstanceError::Execution(mut cause) = refusal.clone() else {
                        return Err(describe(refusal));
                    };
                    while let TerminalInterpretError::ExecutionSite { cause: inner, .. }
                    | TerminalInterpretError::StructuralReplacement {
                        cause: inner, ..
                    } = cause
                    {
                        cause = *inner;
                    }
                    if !matches!(
                        cause,
                        TerminalInterpretError::Storage(
                            EvaluationAccountingError::AllowanceExhausted(
                                EvaluationResource::ResultCustody
                            )
                        )
                    ) {
                        return Err(format!("wrong refusal: {refusal:?}"));
                    }
                    if instance.result().is_some()
                        || instance.close() != Err(TerminalInstanceCloseError::Busy)
                    {
                        return Err("refusal published or discarded result custody".into());
                    }
                    if sponsor.live_payload_bytes() < bytes {
                        return Err("refusal discarded retained backing".into());
                    }
                    let usage = *account.snapshot().map_err(describe)?.record();
                    if instance.invoke(&mut meter) != Err(refusal.clone())
                        || *account.snapshot().map_err(describe)?.record() != usage
                    {
                        return Err("refusal replayed or changed usage".into());
                    }
                }
                drop(instance);
                if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
                    return Err("teardown retained live storage".into());
                }
                let expected_cells = if first_succeeds { cells } else { 0 };
                let expected_bytes = if first_succeeds { bytes } else { 0 };
                if sponsor.consumed_result_storage_cells() != expected_cells
                    || sponsor.consumed_result_payload_bytes() != expected_bytes
                {
                    return Err(format!(
                        "expected cumulative ({expected_cells}, {expected_bytes}), observed ({}, {})",
                        sponsor.consumed_result_storage_cells(),
                        sponsor.consumed_result_payload_bytes()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn verify_result(result: &TerminalExecutionResult, oracle: &[&str]) -> Result<(), String> {
    let values = match (result, oracle) {
        (TerminalExecutionResult::Unit, ["unit"]) => return Ok(()),
        (TerminalExecutionResult::Structural(_), ["structural", ..]) => return Ok(()),
        (TerminalExecutionResult::Scalar(value), ["scalar", ..]) => vec![*value],
        (TerminalExecutionResult::ScalarArray(result), ["array", ..]) => {
            result.value.elements().to_vec()
        }
        (TerminalExecutionResult::ScalarCase(result), ["case", ..]) => result
            .value()
            .fields
            .iter()
            .map(|(_, value)| *value)
            .collect(),
        _ => return Err(format!("expected {oracle:?}, observed {result:?}")),
    };
    let expected = oracle[1..]
        .iter()
        .map(|value| value.parse::<u128>().map_err(describe))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != expected.len() || values.iter().zip(expected).any(|(value, expected)| !matches!(value, TerminalScalarValue::Integer { value: IntegerValue::Unsigned(actual), .. } if *actual == expected)) {
        return Err(format!("expected {oracle:?}, observed {result:?}"));
    }
    Ok(())
}
