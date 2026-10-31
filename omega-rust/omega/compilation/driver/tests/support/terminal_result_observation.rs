//! Observe real completed results from borrowed and transported source-free code.

use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerValue, StructuralTypeId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionStatus, TerminalInstantiation, TerminalProgram, TerminalScalarValue,
    TerminalStorageContents, TerminalStorageObservationError as ObservationError,
    TerminalStorageObservationValue as ObservationValue,
};
use terminal_psi::{StructuralFieldDeclaration, StructuralTypeShape};

fn describe(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    oracles: &[&str],
) -> Result<(), String> {
    if oracles.is_empty() {
        return Err("result observation needs authored oracles".into());
    }
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).map_err(describe)?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        let root = program
            .entry()
            .map_err(describe)?
            .result
            .structural()
            .map(|result| result.structural_type);
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(
                1_000_000, 64, 64, 100_000, 100_000, 100_000, 100_000, 100_000,
            )
            .ok_or("invalid observer limits")?,
            measurement,
        )
        .map_err(describe)?;
        let account = sponsor
            .begin_invocation(measurement, 1_000_000)
            .map_err(describe)?;
        let mut instance = program
            .instantiate(TerminalInstantiation {
                account: Some(&account),
                ..Default::default()
            })
            .map_err(describe)?;
        if instance.observe_result(&[]) != Err(ObservationError::NormalReturnRequired) {
            return Err("result was observable before execution".into());
        }
        let mut fuel =
            TerminalFuelMeter::with_invocation(account.clone(), Some(0)).map_err(describe)?;
        if !matches!(
            instance.invoke(&mut fuel).map_err(describe)?,
            TerminalExecutionStatus::SponsorExhausted(_)
        ) || instance.observe_result(&[]) != Err(ObservationError::NormalReturnRequired)
        {
            return Err("result observation ignored a fuel pause".into());
        }
        fuel.replenish(1_000_000).map_err(describe)?;
        let result = instance.invoke(&mut fuel).map_err(describe)?;
        if oracles == ["crash"] {
            if !matches!(result, TerminalExecutionStatus::Crashed(_))
                || instance.observe_result(&[]) != Err(ObservationError::NormalReturnRequired)
            {
                return Err(format!("crashed result became observable: {result:?}"));
            }
        } else {
            if !matches!(result, TerminalExecutionStatus::Complete(_)) {
                return Err(format!("result did not complete: {result:?}"));
            }
            let usage = *account.snapshot().map_err(describe)?.record();
            for _ in 0..2 {
                for oracle in oracles {
                    let (path, expected) = oracle
                        .split_once('=')
                        .ok_or("result oracle requires path=value")?;
                    let path = resolve_path(&program, root, path)?;
                    let observed = instance.observe_result(&path);
                    match expected {
                        "unknown" if observed == Err(ObservationError::UnknownPath) => {}
                        "nonstructural"
                            if observed == Err(ObservationError::NotStructuralResult) => {}
                        expected if expected.starts_with("case:") => {
                            let Ok(ObservationValue::Storage(TerminalStorageContents::Case(case))) =
                                observed
                            else {
                                return Err(format!("expected {oracle}, observed {observed:?}"));
                            };
                            let case_name = case_name(&program, root, &path, case)?;
                            if case_name != &expected[5..] {
                                return Err(format!("{oracle}: observed case {case_name}"));
                            }
                        }
                        expected if expected.starts_with("bytes:") => {
                            let Ok(ObservationValue::Bytes(bytes)) = observed else {
                                return Err(format!("expected {oracle}, observed {observed:?}"));
                            };
                            let actual = (0..bytes.len())
                                .map(|offset| bytes.read(offset))
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(describe)?;
                            if actual != expected.as_bytes()[6..] {
                                return Err(format!("{oracle}: observed bytes {actual:?}"));
                            }
                        }
                        expected => {
                            let Ok(ObservationValue::Storage(TerminalStorageContents::Scalar {
                                value,
                                stored_scalar,
                            })) = observed
                            else {
                                return Err(format!("expected {oracle}, observed {observed:?}"));
                            };
                            let TerminalScalarValue::Integer {
                                value: IntegerValue::Unsigned(actual),
                                ..
                            } = value
                            else {
                                return Err(format!("{oracle}: unexpected scalar {value:?}"));
                            };
                            let (expected, width) = expected
                                .split_once('@')
                                .ok_or("scalar oracle requires value@stored-bits")?;
                            let width: u16 = width.parse().map_err(describe)?;
                            if stored_scalar.as_integer().is_none_or(|integer| {
                                integer.bits() != width
                                    || integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                            }) {
                                return Err(format!(
                                    "{oracle}: wrong stored scalar {stored_scalar:?}"
                                ));
                            }
                            if actual != expected.parse::<u128>().map_err(describe)? {
                                return Err(format!("{oracle}: observed {actual}"));
                            }
                        }
                    }
                }
            }
            if *account.snapshot().map_err(describe)?.record() != usage {
                return Err("observing a result changed its accounting".into());
            }
        }
        drop(result);
        instance.close().map_err(describe)?;
        if instance.observe_result(&[]) != Err(ObservationError::Closed) {
            return Err("result observation survived instance close".into());
        }
    }
    Ok(())
}

// The authored oracle resolves labels once against the artifact's own table.
// The SDK receives declaration IDs; it never interprets these test labels.
fn resolve_path(
    program: &TerminalProgram<'_>,
    root: Option<StructuralTypeId>,
    text: &str,
) -> Result<Vec<Path>, String> {
    if text == "$" {
        return Ok(Vec::new());
    }
    let mut declaration = program
        .structural_type(root.ok_or("nonstructural path")?)
        .ok_or("oracle structural type is absent")?;
    let mut payload: Option<&[StructuralFieldDeclaration]> = None;
    let mut path = Vec::new();
    for part in text.split('/') {
        if let Some(name) = part.strip_prefix('@') {
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return Err(format!("case on a non-sum in {text}"));
            };
            let case = cases
                .iter()
                .find(|case| program.module().identities.text(case.identity) == Some(name))
                .ok_or("unknown authored case")?;
            path.push(Path::Case(case.id));
            payload = Some(&case.fields);
        } else if let Ok(index) = part.parse::<u64>() {
            let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
                return Err(format!("index on a non-array in {text}"));
            };
            path.push(Path::FixedIndex(index));
            declaration = program
                .structural_type(element)
                .ok_or("oracle array element is absent")?;
            payload = None;
        } else {
            let fields = payload
                .take()
                .or_else(|| declaration.shape.direct_fields())
                .ok_or("field on a non-record")?;
            let field = fields
                .iter()
                .find(|field| program.module().identities.text(field.identity) == Some(part))
                .ok_or("unknown authored field")?;
            path.push(Path::Field(field.id));
            if let Some(child) = field.field_type.as_structural() {
                declaration = program
                    .structural_type(child)
                    .ok_or("oracle field type is absent")?;
            }
        }
    }
    Ok(path)
}

fn case_name<'program>(
    program: &'program TerminalProgram<'_>,
    root: Option<StructuralTypeId>,
    path: &[Path],
    selected: semantic_vocabulary::StructuralCaseId,
) -> Result<&'program str, String> {
    let mut declaration = program
        .structural_type(root.ok_or("case requires structural result")?)
        .ok_or("oracle case type is absent")?;
    let mut payload: Option<&[StructuralFieldDeclaration]> = None;
    for part in path {
        match part {
            Path::Case(case) => {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return Err("case path is not a sum".into());
                };
                payload = Some(
                    &cases
                        .iter()
                        .find(|candidate| candidate.id == *case)
                        .ok_or("case path is absent")?
                        .fields,
                );
            }
            Path::Field(field) => {
                let fields = payload
                    .take()
                    .or_else(|| declaration.shape.direct_fields())
                    .ok_or("field path is not a record")?;
                let field = fields
                    .iter()
                    .find(|candidate| candidate.id == *field)
                    .ok_or("field path is absent")?;
                declaration = program
                    .structural_type(
                        field
                            .field_type
                            .as_structural()
                            .ok_or("case path ended at scalar")?,
                    )
                    .ok_or("oracle nested case type is absent")?;
            }
            Path::FixedIndex(_) => {
                let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
                    return Err("case path is not an array".into());
                };
                declaration = program
                    .structural_type(element)
                    .ok_or("oracle case array element is absent")?;
            }
            _ => return Err("unexpected case oracle path".into()),
        }
    }
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return Err("observed case is not a sum".into());
    };
    let case = cases
        .iter()
        .find(|case| case.id == selected)
        .ok_or("observed case is not declared")?;
    program
        .module()
        .identities
        .text(case.identity)
        .ok_or_else(|| "case name is absent".into())
}
