//! Meaningful optimizer-entrance and semantic-ladder checks.

use std::fs;

use crate::Audit;

use super::bounds::PREFERRED_ENTRANCE_LINES;
use super::inventory::RULE_STAGES;

mod requirements;

pub(super) use requirements::is_required_coordination_entrance;
use requirements::*;

pub(crate) fn check(audit: &mut Audit) {
    let repository = &audit.repository;
    let source_lines = &audit.source_lines;
    let violations = &mut audit.violations;

    for stage in RULE_STAGES {
        let Some(lines) = source_lines.get(stage.entrance) else {
            violations.insert(format!("missing rule-stage entrance: {}", stage.entrance));
            continue;
        };
        if *lines > PREFERRED_ENTRANCE_LINES {
            violations.insert(format!(
                "rule-stage entrance exceeds {PREFERRED_ENTRANCE_LINES} lines: {} ({lines})",
                stage.entrance
            ));
        }
        match fs::read_to_string(repository.join(stage.entrance)) {
            Ok(contents) if contents.contains(stage.coordination_marker) => {}
            Ok(_) => {
                violations.insert(format!(
                    "rule-stage entrance became a re-export wall: {} lacks `{}`",
                    stage.entrance, stage.coordination_marker
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "cannot read rule-stage entrance {}: {error}",
                    stage.entrance
                ));
            }
        }
        for next_rung in stage.next_rungs {
            if !repository.join(next_rung).exists() {
                violations.insert(format!(
                    "rule-stage entrance {} lost next rung: {next_rung}",
                    stage.entrance
                ));
            }
        }
    }

    for entrance in REQUIRED_COORDINATION_ENTRANCES {
        if !source_lines.contains_key(entrance.path) {
            violations.insert(format!(
                "missing required optimizer coordination entrance: {}",
                entrance.path
            ));
            continue;
        }
        match fs::read_to_string(repository.join(entrance.path)) {
            Ok(contents) if contents.contains(entrance.coordination_marker) => {}
            Ok(_) => {
                violations.insert(format!(
                    "optimizer entrance became a re-export wall: {} lacks `{}`",
                    entrance.path, entrance.coordination_marker
                ));
            }
            Err(error) => {
                violations.insert(format!(
                    "cannot read required optimizer entrance {}: {error}",
                    entrance.path
                ));
            }
        }
    }

    for path in REQUIRED_FIXED_VIEW_COPY_CODEC_LEAVES {
        if !source_lines.contains_key(*path) {
            violations.insert(format!(
                "fixed-view-copy codec lost a named semantic leaf: {path}"
            ));
        }
    }

    for path in REQUIRED_MANIFEST_LEAVES {
        if !source_lines.contains_key(*path) {
            violations.insert(format!(
                "optimization manifest lost a named semantic leaf: {path}"
            ));
        }
    }

    for (family, paths) in [
        (
            "optimization-unit identity encoding",
            REQUIRED_OPERATION_ENCODING_LEAVES,
        ),
        ("SCCP validation", REQUIRED_SCCP_VALIDATION_LEAVES),
        (
            "independent live-range replay",
            REQUIRED_LIVE_RANGE_REPLAY_LEAVES,
        ),
        (
            "independent GVN expression keys",
            REQUIRED_GVN_EXPRESSION_KEY_LEAVES,
        ),
        (
            "structural-catalog tests",
            REQUIRED_STRUCTURAL_CATALOG_TEST_LEAVES,
        ),
        ("transformation ledger", REQUIRED_LEDGER_LEAVES),
        (
            "register-allocation tests",
            REQUIRED_REGISTER_ALLOCATION_TEST_LEAVES,
        ),
        (
            "selected-lowering tests",
            REQUIRED_SELECTED_LOWERING_TEST_LEAVES,
        ),
        (
            "conditional-control lowering",
            REQUIRED_CONDITIONAL_CONTROL_LEAVES,
        ),
        (
            "provider-execution settlement",
            REQUIRED_PROVIDER_SETTLEMENT_LEAVES,
        ),
        (
            "pre-allocation machine-effect codec",
            REQUIRED_PRE_ALLOCATION_EFFECT_CODEC_LEAVES,
        ),
        ("AArch64 MOVN proposal", REQUIRED_MOVN_COMPUTE_LEAVES),
        ("extracted focused tests", REQUIRED_EXTRACTED_TEST_LEAVES),
        (
            "exact arithmetic translation",
            REQUIRED_TRANSLATION_ARITHMETIC_LEAVES,
        ),
        (
            "translation error taxonomy",
            REQUIRED_TRANSLATION_ERROR_LEAVES,
        ),
        (
            "selected-block validation",
            REQUIRED_SELECTED_BLOCK_VALIDATION_LEAVES,
        ),
    ] {
        for path in paths {
            if !source_lines.contains_key(*path) {
                violations.insert(format!("{family} lost a named semantic leaf: {path}"));
            }
        }
    }

    let codec_root = "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/";
    let codec_entrance = format!("{codec_root}mod.rs");
    for path in source_lines
        .keys()
        .filter(|path| path.starts_with(codec_root))
    {
        let Ok(contents) = fs::read_to_string(repository.join(path)) else {
            continue;
        };
        if path != &codec_entrance
            && (contents.contains("const MAGIC") || contents.contains("const VERSION"))
        {
            violations.insert(format!(
                "fixed-view-copy protocol admission escaped its sole codec entrance: {path}"
            ));
        }
    }
}
