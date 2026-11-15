/*

Independent stage validators requested with `--validator <ExactName>` or
`--every-validator` (wiki/spec/build/command_line.md#independent-stage-validators).

Default compilation never runs these: the stages do not re-verify their own
output. A request adds checking work inside one compilation and reports a row
per validator and target, and changes nothing about what the compilation
produces.

The roster is the fixed list `StageValidator::ROSTER`, in that order. A named
request must apply to this compilation: asking for a validator whose stage is
not produced (native target operations under `--check`) is an invocation
error, even when `--every-validator` was also given. `--every-validator` runs
every applicable validator and reports the rest as inapplicable with a reason;
an inapplicable validator is never reported as passed.

*/

use crate::{BuildError, NativeDestination, ProjectProduct};
use diagnostics::Diagnostic;

/// One registered independent stage validator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StageValidator {
    /// Rejoins each target's retained target operations to the optimized
    /// abstract operations they were lowered from
    /// (`target_operations_to_selected_instructions::verify_target_operation_plan_with_closed_module`).
    TargetOperationCorrespondence,
}

impl StageValidator {
    /// Every registered validator, in report order.
    pub const ROSTER: &'static [Self] = &[Self::TargetOperationCorrespondence];

    pub const fn name(self) -> &'static str {
        match self {
            Self::TargetOperationCorrespondence => "TargetOperationCorrespondence",
        }
    }

    /// The stage output the validator checks.
    pub const fn stage(self) -> &'static str {
        match self {
            Self::TargetOperationCorrespondence => "target operations",
        }
    }

    pub fn from_exact_name(name: &str) -> Option<Self> {
        Self::ROSTER
            .iter()
            .copied()
            .find(|validator| validator.name() == name)
    }

    /// Why this validator cannot run for `product`, or `None` when the
    /// product produces the stage it checks.
    fn inapplicable_reason(self, product: &ProjectProduct) -> Option<&'static str> {
        match (self, product) {
            (
                Self::TargetOperationCorrespondence,
                ProjectProduct::Native {
                    destination: NativeDestination::Published,
                    ..
                },
            ) => None,
            (Self::TargetOperationCorrespondence, ProjectProduct::Check) => {
                Some("--check produces no native target operations")
            }
            (Self::TargetOperationCorrespondence, ProjectProduct::Terminal)
            | (Self::TargetOperationCorrespondence, ProjectProduct::Inspection { .. }) => {
                Some("this product stops at Terminal Psi before target operations")
            }
            (
                Self::TargetOperationCorrespondence,
                ProjectProduct::Native {
                    destination: NativeDestination::RuntimeAdmission,
                    ..
                },
            ) => Some("runtime admission stops before target lowering"),
            (
                Self::TargetOperationCorrespondence,
                ProjectProduct::Native {
                    destination: NativeDestination::Retained,
                    ..
                },
            ) => Some("stage validators run only for published native compilation"),
        }
    }
}

/// The validators one compilation was asked to run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageValidatorRequest {
    named: Vec<StageValidator>,
    every: bool,
}

impl StageValidatorRequest {
    /// Resolve exact names (repeats allowed and merged) and the
    /// `--every-validator` quantifier. An unknown name rejects.
    pub fn from_exact_names<'a>(
        names: impl IntoIterator<Item = &'a str>,
        every: bool,
    ) -> Result<Self, String> {
        let mut named = Vec::new();
        for name in names {
            let validator = StageValidator::from_exact_name(name).ok_or_else(|| {
                format!(
                    "unknown stage validator `{name}`; registered validators: {}",
                    StageValidator::ROSTER
                        .iter()
                        .map(|validator| validator.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
            if !named.contains(&validator) {
                named.push(validator);
            }
        }
        named.sort();
        Ok(Self { named, every })
    }

    /// Whether `--every-validator` asked for the whole roster.
    pub const fn enumerates_every(&self) -> bool {
        self.every
    }

    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.every
    }

    /// A named validator must apply to the requested product.
    pub(crate) fn require_named_applicable(
        &self,
        product: &ProjectProduct,
    ) -> Result<(), BuildError> {
        for validator in &self.named {
            if let Some(reason) = validator.inapplicable_reason(product) {
                return Err(BuildError::Diagnostics(vec![Diagnostic::error(format!(
                    "stage validator `{}` cannot run in this compilation: {reason}",
                    validator.name()
                ))]));
            }
        }
        Ok(())
    }

    /// Run the selected validators for one target's compile report, in roster
    /// order, each at most once.
    pub(crate) fn run(
        &self,
        product: &ProjectProduct,
        target: target::TargetProfile,
        report: Option<&compiler::CompileReport>,
    ) -> Result<Vec<StageValidationRow>, BuildError> {
        self.run_with(product, target, |validator| validator.check(report))
    }

    fn run_with(
        &self,
        product: &ProjectProduct,
        target: target::TargetProfile,
        check: impl Fn(StageValidator) -> StageValidationResult,
    ) -> Result<Vec<StageValidationRow>, BuildError> {
        let mut rows = Vec::new();
        for &validator in StageValidator::ROSTER {
            let named = self.named.contains(&validator);
            if !named && !self.every {
                continue;
            }
            let result = match validator.inapplicable_reason(product) {
                Some(reason) => StageValidationResult::Inapplicable(reason),
                None => check(validator),
            };
            if named && let StageValidationResult::Inapplicable(reason) = result {
                return Err(BuildError::Diagnostics(vec![Diagnostic::error(format!(
                    "stage validator `{}` cannot run for target {}: {reason}",
                    validator.name(),
                    target.target_name()
                ))]));
            }
            let row = StageValidationRow {
                validator,
                target: target.target_name(),
                result,
            };
            if matches!(row.result, StageValidationResult::Failed(_)) {
                return Err(BuildError::StageValidation(row));
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

impl StageValidator {
    fn check(self, report: Option<&compiler::CompileReport>) -> StageValidationResult {
        match self {
            Self::TargetOperationCorrespondence => {
                let object = report.and_then(|report| {
                    report
                        .retained_native_artifact()
                        .map(|artifact| artifact.object())
                        .or_else(|| {
                            report
                                .retained_dynamic_elf_artifact()
                                .map(|artifact| artifact.object())
                        })
                });
                let Some(object) = object else {
                    return StageValidationResult::Inapplicable(
                        "this build produces no native executable",
                    );
                };
                match image_emission::verify_retained_target_operations(object) {
                    Ok(()) => StageValidationResult::Passed,
                    Err(error) => StageValidationResult::Failed(error.to_string()),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageValidationResult {
    Passed,
    Failed(String),
    Inapplicable(&'static str),
}

/// One validator's result for one target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageValidationRow {
    pub validator: StageValidator,
    pub target: &'static str,
    pub result: StageValidationResult,
}

impl std::fmt::Display for StageValidationRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "stage validator {} (target {}, stage {}): ",
            self.validator.name(),
            self.target,
            self.validator.stage()
        )?;
        match &self.result {
            StageValidationResult::Passed => write!(formatter, "passed"),
            StageValidationResult::Failed(reason) => write!(formatter, "failed: {reason}"),
            StageValidationResult::Inapplicable(reason) => {
                write!(formatter, "inapplicable: {reason}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native() -> ProjectProduct {
        ProjectProduct::Native {
            destination: NativeDestination::Published,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
        }
    }

    fn host() -> target::TargetProfile {
        target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap()
    }

    #[test]
    fn exact_names_resolve_once_and_unknown_names_reject() {
        let request = StageValidatorRequest::from_exact_names(
            [
                "TargetOperationCorrespondence",
                "TargetOperationCorrespondence",
            ],
            true,
        )
        .unwrap();
        let rows = request
            .run_with(&native(), host(), |_| StageValidationResult::Passed)
            .unwrap();
        assert_eq!(rows.len(), 1, "a named and enumerated validator runs once");
        let error =
            StageValidatorRequest::from_exact_names(["NoSuchValidator"], false).unwrap_err();
        assert!(
            error.contains("unknown stage validator `NoSuchValidator`"),
            "{error}"
        );
        assert!(StageValidatorRequest::default().is_empty());
    }

    #[test]
    fn a_named_validator_without_its_stage_is_an_invocation_error() {
        let named =
            StageValidatorRequest::from_exact_names(["TargetOperationCorrespondence"], true)
                .unwrap();
        let error = named
            .require_named_applicable(&ProjectProduct::Check)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("--check produces no native target operations"),
            "{error}"
        );
        let every = StageValidatorRequest::from_exact_names([], true).unwrap();
        every
            .require_named_applicable(&ProjectProduct::Check)
            .unwrap();
        let rows = every
            .run_with(&ProjectProduct::Check, host(), |_| {
                panic!("an inapplicable validator must not run")
            })
            .unwrap();
        assert_eq!(
            rows[0].result,
            StageValidationResult::Inapplicable("--check produces no native target operations")
        );
        assert!(
            rows[0]
                .to_string()
                .ends_with("inapplicable: --check produces no native target operations")
        );
        // A named validator whose product has no native executable after
        // compilation is refused rather than reported as passed.
        assert!(
            named
                .run_with(&native(), host(), |_| StageValidationResult::Inapplicable(
                    "this build produces no native executable"
                ))
                .is_err()
        );
    }

    #[test]
    fn a_failing_validator_fails_the_compilation_as_a_compiler_defect() {
        let request = StageValidatorRequest::from_exact_names([], true).unwrap();
        let error = request
            .run_with(&native(), host(), |_| {
                StageValidationResult::Failed("changed operand".into())
            })
            .unwrap_err();
        assert!(matches!(error, BuildError::StageValidation(_)));
        assert_eq!(error.outcome(), compiler::CompilerRequestOutcome::Reject);
        let text = error.to_string();
        assert!(text.contains("stage validator TargetOperationCorrespondence (target linux_x86_64, stage target operations): failed: changed operand"), "{text}");
        assert!(
            text.contains("compiler defect, not an error in the source"),
            "{text}"
        );
    }
}
