//! Stage-specific single-owner protocol invariants.

use std::fs;

use crate::Audit;

pub(super) fn check(audit: &mut Audit) {
    check_build_optimization_vocabulary(audit);

    let codec_root = "omega-rust/omega/representations/register-homes/src/register_homes/recovery/fixed_view_copy/codec/";
    let codec_entrance = format!("{codec_root}mod.rs");
    for path in audit
        .source_files
        .iter()
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

fn check_build_optimization_vocabulary(audit: &mut Audit) {
    let fragments = "omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/source_assembly/build_vocabulary/fragments.rs";
    // `97de35d903cc` (COORDINATOR-SCOPE-AUDIT F1) moved the toolchain prelude
    // out of the crate's entry file and into its own domain file beside
    // `build_vocabulary`, leaving `source_assembly.rs` the coordinator. The
    // sole-owner invariant is unchanged — only which file holds the slots.
    let build_prelude = "omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/source_assembly/build_prelude.rs";
    let mut files = Vec::new();
    for root in [
        "omega-rust/omega/pipeline/source-files-to-assembled-syntax/src",
        "omega-rust/omega/compiler/compiler/src",
    ] {
        let absolute_root = audit.repository.join(root);
        if let Err(error) = super::super::inventory::collect_rust_files(&absolute_root, &mut files)
        {
            audit.violations.insert(format!(
                "failed to inventory compiler optimization vocabulary: {error}"
            ));
            return;
        }
    }
    for marker in [
        "pub data Optimization {",
        "pub machine Optimizations::enable",
    ] {
        let owners = files
            .iter()
            .filter_map(|file| {
                let contents = fs::read_to_string(file).ok()?;
                contents.contains(marker).then(|| {
                    super::super::inventory::repository_relative_path(&audit.repository, file)
                        .unwrap_or_else(|_| file.display().to_string())
                })
            })
            .collect::<Vec<_>>();
        if owners != [fragments] {
            audit.violations.insert(format!(
                "compiler optimization vocabulary marker `{marker}` must be owned only by {fragments}; found {owners:?}"
            ));
        }
    }

    match fs::read_to_string(audit.repository.join(build_prelude)) {
        Ok(contents) => {
            for slot in [
                "// compiler-owned optimization declarations",
                "// compiler-owned optimization enable machine",
                "// compiler-owned optimization report machine",
            ] {
                let count = contents.matches(slot).count();
                if count != 1 {
                    audit.violations.insert(format!(
                        "the sole build prelude must contain exactly one `{slot}` slot; found {count} in {build_prelude}"
                    ));
                }
            }
        }
        Err(error) => {
            audit.violations.insert(format!(
                "cannot read compiler build-prelude owner {build_prelude}: {error}"
            ));
        }
    }
}
