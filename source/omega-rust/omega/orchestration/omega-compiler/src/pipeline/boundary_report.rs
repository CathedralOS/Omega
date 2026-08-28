mod builder;

use crate::pipeline::compile_options::CompileOptions;
use builder::{append_capability_blast_radius, build_boundary_report};
use omega_artifacts::ArtifactWriter;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;

/// Source-bound boundary observation retained until checked capability facts
/// can complete it. The driver never needs to courier the full syntax tree
/// across resolution, typing, and checking solely for this report.
pub(super) struct BoundaryReportObservation {
    report: omega_artifacts::BoundaryReport,
}

impl BoundaryReportObservation {
    pub(super) fn capture(syntax: &SyntaxTrees) -> Self {
        Self {
            report: build_boundary_report(syntax),
        }
    }

    pub(super) fn write_initial(
        &self,
        options: &CompileOptions,
        emit_auxiliary_artifacts: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        if !emit_auxiliary_artifacts {
            return Ok(());
        }
        self.write(options)
    }

    /// Consumes the retained source observation once checked facts are
    /// available. Capability validation remains unconditional even when report
    /// emission is suppressed.
    pub(super) fn settle_with_capabilities(
        self,
        options: &CompileOptions,
        checked: &CheckedTrees,
        emit_auxiliary_artifacts: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        let settled = self.into_checked_report(checked)?;
        if !emit_auxiliary_artifacts {
            return Ok(());
        }
        settled.write(options)
    }

    fn into_checked_report(mut self, checked: &CheckedTrees) -> Result<Self, Vec<Diagnostic>> {
        append_capability_blast_radius(&mut self.report, checked)?;
        Ok(self)
    }

    fn write(&self, options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
        let writer =
            ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
        writer
            .write_boundary_report(&self.report)
            .map_err(|diagnostic| vec![diagnostic])
    }
}

#[cfg(test)]
mod tests {
    use super::BoundaryReportObservation;
    use crate::pipeline::compile_options::CompileOptions;
    use psi_checked_trees::CheckedTrees;
    use psi_effects::{CapabilityFlowFact, CapabilityFlowKind};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbols::SymbolHandle;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use std::path::PathBuf;

    fn observation() -> BoundaryReportObservation {
        let source = r#"
            boundary operator Slice::index<T>(items: &[T], index: usize) -> T
            requires
                index < items.len;

            target native {
                host: omega::host {
                    os = darwin
                }
                boundary omega::host::contracts
                boundary unchecked invariant_proofs
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        BoundaryReportObservation::capture(&syntax)
    }

    #[test]
    fn checked_settlement_conserves_exact_source_rows() {
        let observation = observation();
        let source_report = observation.report.clone();

        let settled = observation
            .into_checked_report(&CheckedTrees::default())
            .expect("empty checked capability facts settle");

        assert_eq!(settled.report.targets, source_report.targets);
        assert_eq!(settled.report.contracts, source_report.contracts);
        assert_eq!(
            settled.report.unchecked_policies,
            source_report.unchecked_policies
        );
        assert_eq!(settled.report.capability_blast_radius.len(), 0);
    }

    #[test]
    fn output_suppression_still_validates_checked_capability_custody() {
        let mut checked = CheckedTrees::default();
        checked.facts.capabilities.flows.append(CapabilityFlowFact {
            kind: CapabilityFlowKind::Uses,
            capability_symbol: SymbolHandle::from_arena_index(1),
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: SymbolHandle::from_arena_index(3),
            statement_index: 0,
            call_ordinal: 0,
            via_state_symbol: SymbolHandle::invalid(),
        });
        let options = CompileOptions {
            root_path: PathBuf::from("suppressed-boundary-report.omg"),
            build_dir: None,
            target_name: None,
            write_output: false,
        };

        let diagnostics = observation()
            .settle_with_capabilities(&options, &checked, false)
            .expect_err("suppression must not bypass capability validation");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("has no exact boundary capability")
        }));
    }
}
