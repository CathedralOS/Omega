//! Exhaustive, source-local classification of optimizer module entrances.

use std::fs;

use crate::Audit;

use super::entrances::is_required_coordination_entrance;
use super::inventory::RULE_STAGES;

const CRATE_MAP_MARKER: &str = "Optimizer module role: crate map.";
const STAGE_GROUP_MARKER: &str = "Optimizer module role: stage group.";
const EXECUTABLE_ENTRANCE_MARKER: &str = "Optimizer module role: executable entrance.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModuleRole {
    CrateMap,
    StageGroup,
    ExecutableEntrance,
}

impl ModuleRole {
    const fn marker(self) -> &'static str {
        match self {
            Self::CrateMap => CRATE_MAP_MARKER,
            Self::StageGroup => STAGE_GROUP_MARKER,
            Self::ExecutableEntrance => EXECUTABLE_ENTRANCE_MARKER,
        }
    }
}

fn is_module_map(path: &str) -> bool {
    path.ends_with("/lib.rs") || path.ends_with("/mod.rs")
}

fn expected_role(path: &str) -> ModuleRole {
    if RULE_STAGES.iter().any(|stage| stage.entrance == path)
        || is_required_coordination_entrance(path)
    {
        ModuleRole::ExecutableEntrance
    } else if path.ends_with("/lib.rs") {
        ModuleRole::CrateMap
    } else {
        ModuleRole::StageGroup
    }
}

fn declared_roles(contents: &str) -> Vec<ModuleRole> {
    [
        ModuleRole::CrateMap,
        ModuleRole::StageGroup,
        ModuleRole::ExecutableEntrance,
    ]
    .into_iter()
    .filter(|role| contents.contains(role.marker()))
    .collect()
}

pub(crate) fn check(audit: &mut Audit) {
    for path in audit.source_lines.keys().filter(|path| is_module_map(path)) {
        let contents = match fs::read_to_string(audit.repository.join(path)) {
            Ok(contents) => contents,
            Err(error) => {
                audit
                    .violations
                    .insert(format!("cannot read optimizer module map {path}: {error}"));
                continue;
            }
        };
        let declared = declared_roles(&contents);
        if declared.len() != 1 {
            audit.violations.insert(format!(
                "optimizer module map must declare exactly one role marker: {path}"
            ));
            continue;
        }

        let expected = expected_role(path);
        if declared[0] != expected {
            audit.violations.insert(format!(
                "optimizer module map has role {:?}, expected {:?}: {path}",
                declared[0], expected
            ));
        }
    }
}
