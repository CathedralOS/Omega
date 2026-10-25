use lowered_psi_to_lowered_psi::PsiOptimizationStageResult;

/// Retain invocation code or publish the source-free Psi/Omega artifact.
///
/// Unsupported checked constructs fail at lowering; there is no alternate
/// checked-tree backend to select as a fallback.
pub fn finalize_terminal_program(
    optimized: PsiOptimizationStageResult,
    publication: terminal_codec::TerminalPublication,
) -> Result<terminal_codec::TerminalProgramProduct, terminal_codec::CanonicalTerminalArtifactError>
{
    let (lowered, execution) = optimized.into_parts();
    let (module, debug_map, _) = lowered.into_parts();
    // Activation establishment is conditional, but its successful custody is
    // activation-local and ordered across the complete CFG. Decide that
    // obligation on the final pre-Terminal graph before either consumer can
    // execute it. Artifact encoding deliberately trusts producer foundations;
    // external decode and optional target optimization are not this admission.
    for machine in &module.machines {
        terminal_semantics::activation_storage::validate_machine(&module, machine).map_err(
            |reason| {
                terminal_codec::CanonicalTerminalArtifactError::Semantic(
                    terminal_codec::CodecError::MalformedStructuralFoundation(reason),
                )
            },
        )?;
    }
    match publication {
        terminal_codec::TerminalPublication::Portable => {
            let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
                module,
                execution,
                debug_map.as_ref(),
            )?;
            Ok(terminal_codec::TerminalProgramProduct::Portable(artifact))
        }
        terminal_codec::TerminalPublication::Invocation => {
            let code = terminal_codec::InvocationTerminalCode::new(module)
                .map_err(terminal_codec::CanonicalTerminalArtifactError::Semantic)?;
            Ok(terminal_codec::TerminalProgramProduct::Invocation(code))
        }
    }
}
