//! Explicit checks of the objects from one completed native compilation.

use diagnostics::Diagnostic;
use native_artifact::{FinishedNativeImage, RequestedNativeArtifact};
use published_object_verification::{self as verify, NativeStageReplay};

/// One registered stage-output check, with its reported scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StageValidator {
    /// Rejoin the retained target execution plan to its optimized abstract operations.
    TargetOperationCorrespondence,
    /// Compare fixed-frame machine output with its original allocation,
    /// encoding and baseline layout inputs.
    FixedFrameCorrespondence,
    /// Replay the selected-instruction rewrites and match their recorded results.
    SelectedInstructionRewriteReplay,
}

impl StageValidator {
    /// Every registered validator, in report order.
    pub const ROSTER: &'static [Self] = &[
        Self::TargetOperationCorrespondence,
        Self::FixedFrameCorrespondence,
        Self::SelectedInstructionRewriteReplay,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::TargetOperationCorrespondence => "TargetOperationCorrespondence",
            Self::FixedFrameCorrespondence => "FixedFrameCorrespondence",
            Self::SelectedInstructionRewriteReplay => "SelectedInstructionRewriteReplay",
        }
    }

    /// The stage output the validator checks.
    pub const fn stage(self) -> &'static str {
        match self {
            Self::TargetOperationCorrespondence => "target operations",
            Self::FixedFrameCorrespondence => "fixed-frame machine code",
            Self::SelectedInstructionRewriteReplay => "selected-instruction rewrite determinism",
        }
    }

    pub fn from_exact_name(name: &str) -> Option<Self> {
        Self::ROSTER
            .iter()
            .copied()
            .find(|validator| validator.name() == name)
    }

    fn check(
        self,
        image: &FinishedNativeImage,
        replay: &NativeStageReplay,
    ) -> Result<bool, Diagnostic> {
        let (artifact, runtime) = image.parts();
        let object = match artifact {
            RequestedNativeArtifact::Direct(native) => native.object(),
            RequestedNativeArtifact::DynamicElf(native) => native.object(),
        };
        let mut applicable = match self {
            Self::TargetOperationCorrespondence => {
                verify::verify_retained_target_operations(object).map(|()| true)?
            }
            Self::FixedFrameCorrespondence => {
                verify::verify_retained_fixed_frame(object, replay).map(|()| true)?
            }
            Self::SelectedInstructionRewriteReplay => {
                verify::verify_retained_selected_optimization(object, replay)?
            }
        };
        for product in runtime
            .into_iter()
            .flat_map(|runtime| runtime.component_products())
        {
            let component = product.image();
            let originals = || {
                replay
                    .component(component.slot_commitment())
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "component audit requires retained originals (ReplayUnavailable)",
                        )
                    })
            };
            applicable |= match self {
                Self::TargetOperationCorrespondence => {
                    verify::verify_retained_target_operations(component.body()).map(|()| true)?
                }
                Self::FixedFrameCorrespondence => {
                    verify::verify_body_fixed_frame(component.body(), originals()?)
                        .map(|()| true)?
                }
                Self::SelectedInstructionRewriteReplay => {
                    verify::verify_body_selected_optimization(component.body(), originals()?)?
                }
            };
        }
        Ok(applicable)
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

    pub(super) fn check(
        &self,
        target: target::TargetProfile,
        image: &FinishedNativeImage,
        replay: &NativeStageReplay,
    ) -> Result<Vec<StageValidationRow>, NativeValidationError> {
        let mut rows = Vec::new();
        for &validator in StageValidator::ROSTER {
            if !self.every && !self.named.contains(&validator) {
                continue;
            }
            let result = match validator.check(image, replay) {
                Ok(true) => StageValidationResult::Passed,
                Ok(false) => {
                    StageValidationResult::Inapplicable("no selected-instruction rewrite rule ran")
                }
                Err(diagnostic) => StageValidationResult::Failed(diagnostic.to_string()),
            };
            rows.push(StageValidationRow {
                validator,
                target: target.target_name(),
                result,
            });
        }

        if rows
            .iter()
            .any(|row| matches!(row.result, StageValidationResult::Failed(_)))
        {
            return Err(NativeValidationError::Failed(rows));
        }
        for row in &rows {
            if self.named.contains(&row.validator)
                && let StageValidationResult::Inapplicable(reason) = row.result
            {
                return Err(NativeValidationError::Unavailable(Diagnostic::error(
                    format!(
                        "stage validator `{}` cannot run for target {}: {reason}",
                        row.validator.name(),
                        row.target
                    ),
                )));
            }
        }
        Ok(rows)
    }
}

/// Failed correspondence is a compiler defect. An unavailable explicitly requested
/// check is an invocation error. Neither error releases the native image.
#[derive(Debug)]
pub enum NativeValidationError {
    Compilation(Vec<Diagnostic>),
    Unavailable(Diagnostic),
    Failed(Vec<StageValidationRow>),
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
            StageValidationResult::Passed => {
                write!(formatter, "passed")?;
                if self.validator == StageValidator::SelectedInstructionRewriteReplay {
                    write!(
                        formatter,
                        " (producer repeatability only; independent rewrite validation not performed)"
                    )?;
                }
                Ok(())
            }
            StageValidationResult::Failed(reason) => write!(formatter, "failed: {reason}"),
            StageValidationResult::Inapplicable(reason) => {
                write!(formatter, "inapplicable: {reason}")
            }
        }
    }
}
