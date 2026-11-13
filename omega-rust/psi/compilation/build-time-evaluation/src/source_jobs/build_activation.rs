//! Whether a program has the toolchain's Build activation, and whether it
//! may run.
//!
//! A Build script runs against one compiler-owned `Build` value: the host
//! builds it, hands the script `&mut Build`, and decodes what the script left
//! in it by field. Two things must hold before that is sound.
//!
//! The first is the vocabulary the host decodes: the program declares the
//! toolchain's `Build` exactly once, with the one `x86_deployment_features`
//! field of the toolchain's own type. That is checked here.
//!
//! The second is that source cannot construct, store or return a `Build`
//! value of its own, because the host would decode a forged one as if it had
//! issued it. The stage that walked source for those shapes was deleted with
//! the old checker and is not rebuilt: the rule belongs to `Build`'s
//! declaration, as a routed domain whose issuance admits nobody
//! ([domains](../../../../../wiki/spec/language/domains.md): an empty
//! `Issuance::Only` list "permits nobody"). Neither the parser nor the
//! lowered-form checker has that mechanism yet, so every program that has a
//! Build activation is refused here, by that reason. Removing that refusal is
//! the last step once `Build` is declared that way and the checker judges it.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::TypeReferenceNode;

use super::is_exact_toolchain_build_prelude_data;

/// What stops every Build activation today. One text, so the refusal a user
/// reads and the tests name the same reason.
pub const BUILD_VALUE_UNPROTECTED: &str = "no product route yet: Build activation: the compiler-owned `Build` value is not yet protected from forging by its declaration";

/// `Ok(false)`: the program declares no toolchain `Build`, so it has no
/// activation and no compiler-owned value to protect. A program that does is
/// refused: for its vocabulary when that is not the toolchain's, and
/// otherwise for `BUILD_VALUE_UNPROTECTED`.
pub fn admit_toolchain_build_activation(typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    if build_activation_symbol(typed)?.is_none() {
        return Ok(false);
    }
    Err(vec![Diagnostic::error(BUILD_VALUE_UNPROTECTED)])
}

/// The toolchain's `Build` declaration, when the program has one, after
/// checking the part of its vocabulary the host decodes by exact type.
fn build_activation_symbol(typed: &TypedTrees) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
    let refused = |message: String| vec![Diagnostic::error(message)];
    let mut builds = typed.data_definitions().iter().filter(|definition| {
        is_exact_toolchain_build_prelude_data(typed, definition.symbol, "Build")
    });
    let Some(build) = builds.next() else {
        return Ok(None);
    };
    if builds.next().is_some() {
        return Err(refused(
            "the program declares more than one toolchain Build activation".into(),
        ));
    }
    let mut features = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "x86_deployment_features" => {
                Some(field)
            }
            _ => None,
        });
    let (Some(field), None) = (features.next(), features.next()) else {
        return Err(refused(
            "toolchain Build must declare exactly one `x86_deployment_features` field".into(),
        ));
    };
    let exact = matches!(
        typed.type_reference_table.type_reference(field.type_reference),
        TypeReferenceNode::Named { symbol, .. }
            if is_exact_toolchain_build_prelude_data(typed, *symbol, "X86DeploymentFeatures")
    );
    if !exact {
        return Err(refused(
            "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type".into(),
        ));
    }
    Ok(Some(build.symbol))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const PRELUDE: &str = r#"
public data X86DeploymentFeatures { avx2: bool; }
public data Build<'storage> {
    x86_deployment_features: X86DeploymentFeatures;
    identifier: &'storage [u8];
}
"#;

    /// `build` typed beside `prelude`, which is registered the way the
    /// toolchain registers its Build prelude.
    fn typed(build: &str, prelude: Option<&str>) -> TypedTrees {
        let mut sources = source::SourceMap::default();
        let mut syntax: Option<syntax_trees::SyntaxTrees> = None;
        let files = [
            Some(("package/build.omg", build, source::SourceOrigin::User)),
            prelude.map(|text| ("<build-prelude>", text, source::SourceOrigin::Toolchain)),
        ];
        for (path, text, origin) in files.into_iter().flatten() {
            let id = sources
                .add_checked_instance(
                    path.into(),
                    text,
                    "package".into(),
                    None,
                    origin,
                    source::SourceResolutionStratum::Base,
                    source::DependencyScope::Build,
                )
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            match &mut syntax {
                None => {
                    syntax = Some(
                        tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap(),
                    );
                }
                Some(syntax) => {
                    tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
                }
            }
        }
        let mut request =
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
        request.sources = Some(Arc::new(sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn refusal(typed: &TypedTrees) -> String {
        let refused = admit_toolchain_build_activation(typed).unwrap_err();
        assert_eq!(refused.len(), 1);
        refused[0].message.clone()
    }

    #[test]
    fn a_program_with_the_toolchain_build_is_refused_for_the_unprotected_value() {
        let typed = typed("machine build_entry() { }", Some(PRELUDE));
        assert_eq!(refusal(&typed), BUILD_VALUE_UNPROTECTED);
    }

    /// A `Build` the program declares itself is no compiler-owned value: the
    /// host issues none for it, so there is no activation to admit or refuse.
    #[test]
    fn a_program_without_the_toolchain_build_has_no_activation() {
        let own = typed(
            "data Build { marker: u64; }\nmachine build_entry() { }",
            None,
        );
        assert_eq!(admit_toolchain_build_activation(&own), Ok(false));
    }

    /// The vocabulary is judged before the value's protection, so a prelude
    /// the host could not decode is reported as that.
    #[test]
    fn a_toolchain_build_without_the_decoded_vocabulary_is_refused_as_that() {
        let missing = typed(
            "machine build_entry() { }",
            Some("public data Build { marker: u64; }"),
        );
        assert_eq!(
            refusal(&missing),
            "toolchain Build must declare exactly one `x86_deployment_features` field"
        );
        let foreign = typed(
            "machine build_entry() { }",
            Some("public data Build { x86_deployment_features: u64; }"),
        );
        assert_eq!(
            refusal(&foreign),
            "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type"
        );
    }
}
