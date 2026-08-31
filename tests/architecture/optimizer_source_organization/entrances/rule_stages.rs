//! Six rule-owning entrances, their sole catalogs, and next semantic rungs.

use std::fs;

use crate::Audit;

use super::super::bounds::PREFERRED_ENTRANCE_LINES;
use super::super::inventory::RULE_STAGES;

pub(super) fn check(audit: &mut Audit) {
    for stage in RULE_STAGES {
        let Some(lines) = audit.source_lines.get(stage.entrance) else {
            audit
                .violations
                .insert(format!("missing rule-stage entrance: {}", stage.entrance));
            continue;
        };
        if *lines > PREFERRED_ENTRANCE_LINES {
            audit.violations.insert(format!(
                "rule-stage entrance exceeds {PREFERRED_ENTRANCE_LINES} lines: {} ({lines})",
                stage.entrance
            ));
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
    }
}
