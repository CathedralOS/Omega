//! Execute source-authored project requests through the public Terminal SDK.

use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalStructuralInputs,
    TerminalStructuralValue,
};

pub(super) fn observe(artifact: &terminal_codec::CanonicalTerminalArtifact) -> Result<(), String> {
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program
        .entry()
        .map_err(|error| format!("entry: {error:?}"))?;
    let [parameter] = entry.structural_parameters else {
        return Err("project request entry needs its original Build receiver".into());
    };
    let original = TerminalStructuralValue::minted(955, parameter.structural_type, Vec::new());
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: std::slice::from_ref(&original),
                zero_initialized_arguments: &[0],
                ..Default::default()
            },
            ..Default::default()
        })
        .map_err(|error| format!("instantiate: {error:?}"))?;
    let result = instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .map_err(|error| format!("project requests: {error:?}"))?;
    if result != TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) {
        return Err(format!("project requests did not return Unit: {result:?}"));
    }
    if instance.effects().len() != 2 {
        return Err(format!(
            "expected two executed project requests, observed {}",
            instance.effects().len()
        ));
    }
    instance
        .close()
        .map_err(|error| format!("close: {error:?}"))?;
    Ok(())
}
