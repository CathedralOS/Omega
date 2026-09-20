//! Rule-owning entrances, their sole catalogs, next semantic rungs, and the
//! consumers that keep each stage's produced route from becoming an orphan
//! output (see `omega-rust/pipeline.md` placement rules).

use std::fs;

use crate::Audit;

use super::super::inventory::RULE_STAGES;

pub(super) fn check(audit: &mut Audit) {
    for stage in RULE_STAGES {
        if !audit.source_files.contains(stage.entrance) {
            audit
                .violations
                .insert(format!("missing rule-stage entrance: {}", stage.entrance));
            continue;
        }
        match fs::read_to_string(audit.repository.join(stage.entrance)) {
            Ok(contents) if contents.contains(stage.coordination_marker) => {}
            Ok(_) => {
                audit.violations.insert(format!(
                    "rule-stage entrance became a re-export wall: {} lacks `{}`",
                    stage.entrance, stage.coordination_marker
                ));
            }
            Err(error) => {
                audit.violations.insert(format!(
                    "cannot read rule-stage entrance {}: {error}",
                    stage.entrance
                ));
            }
        }
        for next_rung in stage.next_rungs {
            if !audit.repository.join(next_rung).exists() {
                audit.violations.insert(format!(
                    "rule-stage entrance {} lost next rung: {next_rung}",
                    stage.entrance
                ));
            }
        }
        for consumer in stage.consumers {
            match fs::read_to_string(audit.repository.join(consumer)) {
                Ok(contents) if contents.contains(stage.output_marker) => {}
                Ok(_) => {
                    audit.violations.insert(format!(
                        "orphan stage output: {} produces `{}` but consumer {} no longer names it",
                        stage.entrance, stage.output_marker, consumer
                    ));
                }
                Err(error) => {
                    audit.violations.insert(format!(
                        "cannot read consumer {consumer} of {}: {error}",
                        stage.entrance
                    ));
                }
            }
        }
    }
}
