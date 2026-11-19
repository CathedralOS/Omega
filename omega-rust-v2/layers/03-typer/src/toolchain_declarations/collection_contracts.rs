use crate::typing::type_stage::type_admitted_stage;
use crate::{TypedSignature, TypedStage};
use omega_resolver::{ParsedSource, StageSource};
use std::sync::OnceLock;

use super::source_package::TOOLCHAIN_PACKAGE;

/// Exact sealed declarations and their typed requires/ensures predicates.
pub fn collection_contracts() -> &'static TypedStage {
    static CONTRACTS: OnceLock<TypedStage> = OnceLock::new();
    CONTRACTS.get_or_init(|| {
        let text = include_str!("collection_access.omg");
        let tokens = omega_lexer::lex(text.as_bytes()).expect("toolchain collection tokens");
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).expect("toolchain collection grammar");
        let sources = [StageSource {
            package: TOOLCHAIN_PACKAGE,
            source: ParsedSource {
                logical_path: "indexing/collections.omg",
                text,
                tokens: &tokens,
                tree: &tree,
            },
        }];
        let resolved = omega_resolver::resolve_stage(&sources, &[]).expect("toolchain collection names");
        type_admitted_stage(&sources, &resolved, true, &crate::TypingEnvironment::default()).expect("toolchain collection contracts")
    })
}
pub(crate) fn signature(name: &str) -> &'static TypedSignature {
    collection_contracts()
        .signatures
        .iter()
        .find(|s| s.machine.path.rsplit("::").next() == Some(name))
        .expect("embedded collection primitive")
}
