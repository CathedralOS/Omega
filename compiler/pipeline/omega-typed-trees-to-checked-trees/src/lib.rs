use omega_checked_trees::Program;
use omega_core::diagnostics::Diagnostic;

pub fn lower_typed_trees(program: &omega_typed_trees::Program) -> Result<Program, Diagnostic> {
    Ok(program.clone())
}

pub fn lower_typed_program(program: &omega_typed_trees::Program) -> Result<Program, Diagnostic> {
    lower_typed_trees(program)
}
