use semantic_vocabulary::compiler_evaluation::CompilerEvaluationEffects;

pub fn require_compiler_evaluation_floor(
    machine: &str,
    effects: &CompilerEvaluationEffects,
    closure_violation: Option<&str>,
    selection_violation: Option<&str>,
) -> Result<(), String> {
    let mut violations = Vec::new();
    if !effects.services.is_empty() {
        violations.push(format!("service reach [{}]", effects.services.join(", ")));
    }
    if effects.may_suspend {
        violations.push("may suspend".to_owned());
    }
    if effects.may_block {
        violations.push("may block".to_owned());
    }
    violations.extend(closure_violation.map(str::to_owned));
    violations.extend(selection_violation.map(str::to_owned));
    if violations.is_empty() {
        return Ok(());
    }
    Err(format!(
        "machine `{machine}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
        violations.join("; ")
    ))
}
