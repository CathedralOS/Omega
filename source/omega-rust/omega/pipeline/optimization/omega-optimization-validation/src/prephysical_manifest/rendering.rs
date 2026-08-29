use std::fmt::Write;

use super::*;

impl PrePhysicalOptimizationManifest {
    /// Deterministic human projection. Rendering is deliberately downstream of
    /// the structured record and cannot affect optimization decisions or bytes.
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega pre-physical optimization manifest").unwrap();
        writeln!(output, "stage: pre-physical abstract plan").unwrap();
        writeln!(
            output,
            "physical data: unavailable before physical realization"
        )
        .unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "source Terminal Psi: {}",
            hex(self.psi.program_fingerprint.as_bytes())
        )
        .unwrap();
        writeln!(output, "initial unit: {}", hex(&self.initial_unit.bytes())).unwrap();
        writeln!(output, "final unit: {}", hex(&self.final_unit.bytes())).unwrap();
        writeln!(output, "projection: {}", hex(&self.projection.bytes())).unwrap();
        writeln!(
            output,
            "rule set: {}",
            hex(&self.identity_bundle.rule_set().bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target-neutral cost model: {}",
            hex(&self.identity_bundle.target_cost_model().bytes())
        )
        .unwrap();
        writeln!(
            output,
            "decision log: {}",
            self.identity_bundle
                .decision_log()
                .map(|identity| hex(&identity.bytes()))
                .unwrap_or_else(|| "absent".into())
        )
        .unwrap();
        writeln!(
            output,
            "transformation ledger: {}",
            hex(&self.transformation_ledger.identity().bytes())
        )
        .unwrap();
        let selected = self
            .selections
            .as_slice()
            .iter()
            .map(|selection| selection.build_case_name())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "requested selections: {selected}").unwrap();
        let psi_selected = self
            .psi_selections
            .as_slice()
            .iter()
            .map(|selection| selection.build_case_name())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "completed Psi selections: {psi_selected}").unwrap();
        writeln!(output, "passes: {}", self.pass_manifests.len()).unwrap();
        for (pass_index, pass) in self.pass_manifests.iter().enumerate() {
            writeln!(
                output,
                "pass[{pass_index}]: identity={}, input={}, output={}, rules={}, decisions={}",
                hex(&pass.pass().bytes()),
                hex(&pass.input().bytes()),
                hex(&pass.output().bytes()),
                pass.ordered_rules().len(),
                pass.decisions().len(),
            )
            .unwrap();
            for (rule_index, rule) in pass.ordered_rules().iter().enumerate() {
                writeln!(output, "  rule[{rule_index}]: {}", hex(&rule.bytes())).unwrap();
            }
            for (decision_index, decision) in pass.decisions().iter().enumerate() {
                writeln!(
                    output,
                    "  decision[{decision_index}]: candidate={}, rule={}, verdict={:?}, validator={}, facts={}",
                    hex(&decision.candidate().bytes()),
                    hex(&decision.rule().bytes()),
                    decision.verdict(),
                    decision
                        .validator()
                        .map(|identity| hex(&identity.bytes()))
                        .unwrap_or_else(|| "absent".into()),
                    decision.consumed_facts().len(),
                )
                .unwrap();
                for fact in decision.consumed_facts() {
                    writeln!(output, "    fact: {}", render_fact(*fact)).unwrap();
                }
            }
        }
        let (applied, skipped, rejected) = decision_counts(&self.pass_manifests);
        writeln!(
            output,
            "candidate verdicts: applied={applied}, skipped={skipped}, rejected={rejected}"
        )
        .unwrap();
        writeln!(
            output,
            "work usage: rules={}, candidates={}, validations={}, commits={}, iterations={}",
            self.usage.rule_evaluations,
            self.usage.candidates,
            self.usage.validation_steps,
            self.usage.commits,
            self.usage.iterations,
        )
        .unwrap();
        writeln!(
            output,
            "source structure: functions={}, blocks={}, nodes={}",
            self.source_statistics.functions,
            self.source_statistics.blocks,
            self.source_statistics.nodes,
        )
        .unwrap();
        writeln!(
            output,
            "optimized structure: functions={}, blocks={}, nodes={}",
            self.optimized_statistics.functions,
            self.optimized_statistics.blocks,
            self.optimized_statistics.nodes,
        )
        .unwrap();
        let realized = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| rewrite.disposition.is_realized())
            .count();
        let proven_unreachable = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| !rewrite.disposition.is_realized())
            .count();
        let proven_unreachable_sources = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| !rewrite.disposition.is_realized())
            .map(|rewrite| rewrite.sources.len())
            .sum::<usize>();
        writeln!(
            output,
            "provenance/fuel records: transformations={}, realized={}, proven-unreachable={}, proven-unreachable-sources={}",
            self.transformation_ledger.records().len(),
            realized,
            proven_unreachable,
            proven_unreachable_sources,
        )
        .unwrap();
        for (record_index, record) in self.transformation_ledger.records().iter().enumerate() {
            writeln!(
                output,
                "ledger[{record_index}]: candidate={}, rule={}, input={}, output={}",
                hex(&record.candidate.bytes()),
                hex(&record.rule.bytes()),
                hex(&record.input.bytes()),
                hex(&record.output.bytes()),
            )
            .unwrap();
            for rewrite in &record.provenance {
                render_provenance_rewrite(&mut output, rewrite);
            }
        }
        output
    }
}

fn decision_counts(manifests: &[OptimizationPassManifestRecord]) -> (usize, usize, usize) {
    manifests
        .iter()
        .flat_map(OptimizationPassManifestRecord::decisions)
        .fold(
            (0, 0, 0),
            |(applied, skipped, rejected), decision| match decision.verdict() {
                OptimizationCandidateVerdict::Applied => (applied + 1, skipped, rejected),
                OptimizationCandidateVerdict::Skipped(_) => (applied, skipped + 1, rejected),
                OptimizationCandidateVerdict::Rejected(_) => (applied, skipped, rejected + 1),
            },
        )
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn render_fact(fact: OptimizationFactReference) -> String {
    match fact {
        OptimizationFactReference::ScalarConstant(identity) => {
            format!("scalar-constant:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::AcceptedObligation(identity) => {
            format!("accepted-obligation:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::OwnershipFrontier(identity) => {
            format!("ownership-frontier:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::ValueRange(identity) => {
            format!("value-range:{}", hex(&identity.bytes()))
        }
    }
}

fn render_provenance(provenance: PsiProvenance) -> String {
    match provenance {
        PsiProvenance::Operation(operation) => format!("operation:{}", operation.get()),
        PsiProvenance::Edge(edge) => format!("edge:{}", edge.get()),
    }
}

fn render_provenance_rewrite(output: &mut String, rewrite: &ProvenanceRewrite) {
    let (label, site) = match rewrite.disposition {
        ProvenanceDisposition::RealizedAt(site) => ("realized-at", site),
        ProvenanceDisposition::ProvenUnreachableAt(site) => ("proven-unreachable-at", site),
    };
    writeln!(
        output,
        "  input: {}",
        render_realization_site(rewrite.input)
    )
    .unwrap();
    writeln!(output, "  {label}: {}", render_realization_site(site)).unwrap();
    for source in &rewrite.sources {
        writeln!(output, "    source: {}", render_provenance(*source)).unwrap();
    }
    for fuel in &rewrite.fuel {
        match rewrite.disposition {
            ProvenanceDisposition::RealizedAt(_) => writeln!(
                output,
                "    source-scheduled-fuel: {} units={} runtime-charge={}",
                render_provenance(fuel.site),
                fuel.units,
                fuel.units,
            ),
            ProvenanceDisposition::ProvenUnreachableAt(_) => writeln!(
                output,
                "    source-scheduled-fuel: {} units={} runtime-charge=none reason=proven-unreachable",
                render_provenance(fuel.site),
                fuel.units,
            ),
        }
        .unwrap();
    }
}

fn render_realization_site(site: PsiRealizationSite) -> String {
    match site {
        PsiRealizationSite::Node(location) => format!(
            "node:machine={},block={},node={}",
            location.machine.get(),
            location.block.get(),
            location.node
        ),
        PsiRealizationSite::Edge { machine, edge } => {
            format!("edge:machine={},edge={}", machine.get(), edge.get())
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
