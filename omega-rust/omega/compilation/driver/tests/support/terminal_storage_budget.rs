//! Authored source helpers declare an exact live-cell peak and scalar oracle.
//! Execute with that ceiling and one cell less, through ordinary public
//! instantiation and fuel pauses. No synthesized IR or private storage access.
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
    value: &str,
) -> Result<(), String> {
    let peak: u64 = cells.parse().map_err(describe)?;
    let expected: u64 = value.parse().map_err(describe)?;
    if peak < 2 {
        return Err("storage peak must allow a positive short budget".to_owned());
    }
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).map_err(describe)?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        observe_program(program, peak, expected)?;
    }
    Ok(())
}

fn observe_program(program: TerminalProgram<'_>, peak: u64, expected: u64) -> Result<(), String> {
    let short = peak - 1;
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
            if !matches!(result, Ok(TerminalExecutionStatus::Complete(
                TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(observed), ..
                })
            )) if observed == u128::from(expected))
            {
                return Err(format!("expected scalar {expected}, observed {result:?}"));
            }
            if sponsor.peak_live_cells() != peak {
                return Err(format!(
                    "expected peak {peak}, observed {}",
                    sponsor.peak_live_cells()
                ));
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
