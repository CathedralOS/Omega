//! Generate ordinary target values and their checked protocol conversion from
//! the same catalog used by executed Build rows. No source prefix is involved.

use language_semantics::declaration_selection::ProductTarget;
use std::fmt::Write;

pub(super) fn install(prelude: &str) -> String {
    let mut declarations = String::from("public data BuildTarget [copy] {\n");
    for target in ProductTarget::ALL {
        writeln!(declarations, "    case {};", target.source_case()).unwrap();
    }
    declarations.push_str("}\n");
    for target in ProductTarget::ALL {
        writeln!(
            declarations,
            "public const {}: BuildTarget = BuildTarget::{};",
            target.canonical_name(),
            target.source_case()
        )
        .unwrap();
    }
    declarations.push_str(
        "public machine build_selection_target(target: BuildTarget) -> u64 {\n    match target {\n",
    );
    for target in ProductTarget::ALL {
        writeln!(
            declarations,
            "        BuildTarget::{} -> {}u64",
            target.source_case(),
            target.code()
        )
        .unwrap();
    }
    declarations.push_str("    }\n}\n");
    prelude.replace("// compiler-owned target declarations", &declarations)
}
