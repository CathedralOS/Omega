//! Authored source helpers declare an exact live-cell peak and result oracle.
//! Execute with that ceiling and one cell less, through ordinary public
//! instantiation and fuel pauses. No synthesized IR or private storage access.
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
    EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEntryStorageValue, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstanceCloseError, TerminalInstanceError, TerminalInstantiation,
    TerminalInterpretError, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
    TerminalStructuralInputs, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

fn describe(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[derive(Clone, Copy, Debug)]
enum ExpectedResult {
    Unit,
    Unsigned(u64),
}

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    cells: &str,
    value: &str,
    initial: Option<&str>,
) -> Result<(), String> {
    let peak: u64 = cells.parse().map_err(describe)?;
    let expected = if value == "unit" {
        ExpectedResult::Unit
    } else {
        ExpectedResult::Unsigned(value.parse().map_err(describe)?)
    };
    if initial.is_some() && matches!(expected, ExpectedResult::Unit) {
        return Err("primitive entry writeback requires an unsigned result oracle".to_owned());
    }
    if peak < 2 {
        return Err("storage peak must allow a positive short budget".to_owned());
    }
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).map_err(describe)?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        observe_program(program, peak, expected, initial)?;
    }
    Ok(())
}

fn observe_program(
    program: TerminalProgram<'_>,
    peak: u64,
    expected: ExpectedResult,
    initial: Option<&str>,
) -> Result<(), String> {
    let short = peak - 1;
    let mut arguments = Vec::new();
    let mut primitives = Vec::new();
    if let Some(initial) = initial {
        let entry = program.entry().map_err(describe)?;
        let [parameter] = entry.structural_parameters else {
            return Err(
                "primitive storage fixture requires one structural entry argument".to_owned(),
            );
        };
        arguments.push(TerminalStructuralValue::minted(
            79,
            parameter.structural_type,
            Vec::new(),
        ));
        primitives.push(TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).map_err(describe)?,
                value: IntegerValue::Unsigned(initial.parse::<u32>().map_err(describe)?.into()),
            },
        });
    }
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    for ceiling in [peak, short] {
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(100_000, 8, 2, 2, ceiling, 1000, 1000, 1000)
                .ok_or("invalid storage fixture limits")?,
            measurement,
        )
        .map_err(describe)?;
        let account = sponsor
            .begin_invocation(measurement, 100_000)
            .map_err(describe)?;
        let mut instance = program
            .instantiate(TerminalInstantiation {
                account: Some(&account),
                structural_inputs: TerminalStructuralInputs {
                    arguments: &arguments,
                    primitive_values: &primitives,
                    ..Default::default()
                },
                ..Default::default()
            })
            .map_err(describe)?;
        let mut meter =
            TerminalFuelMeter::with_invocation(account.clone(), Some(0)).map_err(describe)?;
        let result = loop {
            match instance.invoke(&mut meter) {
                Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {
                    meter.replenish(1).map_err(describe)?
                }
                result => break result,
            }
        };
        if ceiling == peak {
            let matches = match (&result, expected) {
                (
                    Ok(TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)),
                    ExpectedResult::Unit,
                ) => true,
                (
                    Ok(TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                        TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(observed),
                            ..
                        },
                    ))),
                    ExpectedResult::Unsigned(expected),
                ) => *observed == u128::from(expected),
                _ => false,
            };
            if !matches {
                return Err(format!("expected {expected:?}, observed {result:?}"));
            }
            if sponsor.peak_live_cells() != peak {
                return Err(format!(
                    "expected peak {peak}, observed {}",
                    sponsor.peak_live_cells()
                ));
            }
            if initial.is_some()
                && !matches!(
                    instance.observe_borrowed_entry(0, &[]).map_err(describe)?,
                    TerminalEntryStorageValue::Storage(TerminalStorageContents::Scalar {
                        value: TerminalScalarValue::Integer { value: IntegerValue::Unsigned(observed), .. }, ..
                    }) if matches!(expected, ExpectedResult::Unsigned(expected) if observed == u128::from(expected))
                )
            {
                return Err(
                    "original primitive entry backing differs from the scalar result".to_owned(),
                );
            }
            instance.close().map_err(describe)?;
        } else {
            let Err(refusal) = result else {
                return Err(format!(
                    "expected live-storage refusal at {ceiling} cells, observed {result:?}"
                ));
            };
            let TerminalInstanceError::Execution(mut cause) = refusal.clone() else {
                return Err(format!("expected execution refusal, observed {refusal:?}"));
            };
            while let TerminalInterpretError::ExecutionSite { cause: inner, .. }
            | TerminalInterpretError::StructuralReplacement { cause: inner, .. } = cause
            {
                cause = *inner;
            }
            if !matches!(
                cause,
                TerminalInterpretError::Storage(EvaluationAccountingError::AllowanceExhausted(
                    EvaluationResource::LiveStorage
                ))
            ) {
                return Err(format!(
                    "expected live-storage refusal, observed {refusal:?}"
                ));
            }
            if instance.result().is_some()
                || instance.close() != Err(TerminalInstanceCloseError::Busy)
            {
                return Err(
                    "storage refusal published a result or discarded a live activation".to_owned(),
                );
            }
            let usage = *account.snapshot().map_err(describe)?.record();
            if instance.invoke(&mut meter) != Err(refusal)
                || *account.snapshot().map_err(describe)?.record() != usage
            {
                return Err("storage refusal replayed or changed usage".to_owned());
            }
        }
        drop(instance);
        if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
            return Err("activation teardown retained storage".to_owned());
        }
    }
    Ok(())
}
