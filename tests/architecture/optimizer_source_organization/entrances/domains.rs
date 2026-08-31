//! Domain-grouped executable joins and semantic ladders.

use std::fs;

use crate::Audit;

use super::requirements::{EXECUTABLE_ENTRANCE_DOMAINS, SEMANTIC_LADDER_DOMAINS};

pub(super) fn check(audit: &mut Audit) {
    for domain in EXECUTABLE_ENTRANCE_DOMAINS {
        debug_assert!(!domain.name.is_empty());
        for entrance in domain.entrances {
            if !audit.source_lines.contains_key(entrance.path) {
                audit.violations.insert(format!(
                    "missing required optimizer coordination entrance: {}",
                    entrance.path
                ));
                continue;
            }
            match fs::read_to_string(audit.repository.join(entrance.path)) {
                Ok(contents) if contents.contains(entrance.coordination_marker) => {}
                Ok(_) => {
                    audit.violations.insert(format!(
                        "optimizer entrance became a re-export wall: {} lacks `{}`",
                        entrance.path, entrance.coordination_marker
                    ));
                }
                Err(error) => {
                    audit.violations.insert(format!(
                        "cannot read required optimizer entrance {}: {error}",
                        entrance.path
                    ));
                }
            }
        }
    }

    for domain in SEMANTIC_LADDER_DOMAINS {
        debug_assert!(!domain.name.is_empty());
        for ladder in domain.ladders {
            for path in ladder.paths {
                if !audit.source_lines.contains_key(*path) {
                    audit.violations.insert(format!(
                        "{} lost a named semantic leaf: {path}",
                        ladder.family
                    ));
                }
            }
        }
    }
}
