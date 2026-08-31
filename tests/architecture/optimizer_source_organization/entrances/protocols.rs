//! Stage-specific single-owner protocol invariants.

use std::fs;

use crate::Audit;

pub(super) fn check(audit: &mut Audit) {
    let codec_root = "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/";
    let codec_entrance = format!("{codec_root}mod.rs");
    for path in audit
        .source_lines
        .keys()
        .filter(|path| path.starts_with(codec_root))
    {
        let Ok(contents) = fs::read_to_string(audit.repository.join(path)) else {
            continue;
        };
        if path != &codec_entrance
            && (contents.contains("const MAGIC") || contents.contains("const VERSION"))
        {
            audit.violations.insert(format!(
                "fixed-view-copy protocol admission escaped its sole codec entrance: {path}"
            ));
        }
    }
}
