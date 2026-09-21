use crate::CompileReport;
use compilation_report::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    ProductionArtifactIdentity,
};
use diagnostics::Diagnostic;
use target::TargetProfile;

/// An ordinary target product or its diagnostics. None identifies target-neutral
/// checking/Terminal production, never an implicit native target.
#[derive(Debug)]
pub struct CompileTargetOutcome {
    target: Option<TargetProfile>,
    result: Result<CompileReport, Vec<Diagnostic>>,
}

impl CompileTargetOutcome {
    pub(in crate::compiler) fn new(
        target: Option<TargetProfile>,
        result: Result<CompileReport, Vec<Diagnostic>>,
    ) -> Self {
        Self { target, result }
    }
    pub const fn target_profile(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn report(&self) -> Option<&CompileReport> {
        self.result.as_ref().ok()
    }
    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        self.result.as_ref().err().map(Vec::as_slice)
    }
    pub const fn succeeded(&self) -> bool {
        self.result.is_ok()
    }
    pub fn into_result(self) -> Result<CompileReport, Vec<Diagnostic>> {
        self.result
    }
}

/// One outcome per configuration, in canonical target order, including
/// failures, plus the optional batch manifest binding that explicit set and
/// each child's commitment/outcome. The collection grants no deployment,
/// platform-support, test, or audit claim.
#[derive(Debug)]
pub struct CompileOutcomes {
    outcomes: Box<[CompileTargetOutcome]>,
    batch_manifest: BatchCompilationManifest,
    #[cfg(test)]
    prepared_terminal_native_input_count: usize,
}

impl CompileOutcomes {
    pub(in crate::compiler) fn new(
        outcomes: Vec<CompileTargetOutcome>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let rows = outcomes
            .iter()
            .map(|outcome| {
                batch_child_outcome(outcome).map(|child| BatchChildRow::new(outcome.target, child))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch_manifest = BatchCompilationManifest::new(rows)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        Ok(Self {
            outcomes: outcomes.into_boxed_slice(),
            batch_manifest,
            #[cfg(test)]
            prepared_terminal_native_input_count: 0,
        })
    }
    pub fn outcomes(&self) -> &[CompileTargetOutcome] {
        &self.outcomes
    }
    /// The invocation's batch manifest: it binds the explicit configured
    /// target set and each child's commitment/outcome in canonical order —
    /// never a completeness, support, test, or deployment claim.
    pub const fn batch_manifest(&self) -> &BatchCompilationManifest {
        &self.batch_manifest
    }
    pub fn into_outcomes(self) -> Box<[CompileTargetOutcome]> {
        self.outcomes
    }

    /// The strongest commitment a child's retained report carries: its own
    /// production manifest when package-aware custody exists, else the
    /// produced artifact identity, else the check-only marker.
    fn report_commitment(report: &CompileReport) -> BatchChildCommitment {
        if let Some(manifest) = report.production_manifest() {
            return BatchChildCommitment::Manifest(manifest.identity());
        }
        if let Some(native) = report.retained_native_artifact() {
            return BatchChildCommitment::Artifact(ProductionArtifactIdentity::Native(
                native.identity(),
            ));
        }
        if let Some(artifact) = report.artifact() {
            return BatchChildCommitment::Artifact(ProductionArtifactIdentity::Terminal(
                artifact.manifest().identity(),
            ));
        }
        if let Some(outputs) = report.build_outputs() {
            return BatchChildCommitment::Artifact(ProductionArtifactIdentity::BuildOutputs(
                *outputs.identity(),
            ));
        }
        BatchChildCommitment::Checked
    }

    /// Explicit extraction for consumers that requested exactly one product.
    /// Never discard a sibling failure or silently select the first target.
    pub fn into_single_report(self) -> Result<CompileReport, Vec<Diagnostic>> {
        if self.outcomes.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "expected one compilation outcome, received {}",
                self.outcomes.len()
            ))]);
        }
        self.outcomes
            .into_vec()
            .pop()
            .ok_or_else(|| vec![Diagnostic::error("compilation outcome is missing")])?
            .into_result()
    }
    pub(in crate::compiler) fn with_prepared_terminal_native_input_count(
        self,
        count: usize,
    ) -> Self {
        #[cfg(test)]
        {
            let mut outcomes = self;
            outcomes.prepared_terminal_native_input_count = count;
            outcomes
        }
        #[cfg(not(test))]
        {
            let _ = count;
            self
        }
    }
    #[cfg(test)]
    pub(in crate::compiler) const fn prepared_terminal_native_input_count(&self) -> usize {
        self.prepared_terminal_native_input_count
    }
}

fn batch_child_outcome(
    outcome: &CompileTargetOutcome,
) -> Result<BatchChildOutcome, Vec<Diagnostic>> {
    match &outcome.result {
        Ok(report) => Ok(BatchChildOutcome::Succeeded {
            commitment: CompileOutcomes::report_commitment(report),
        }),
        Err(diagnostics) => BatchChildOutcome::rejected(diagnostics)
            .map_err(|message| vec![Diagnostic::error(message)]),
    }
}
