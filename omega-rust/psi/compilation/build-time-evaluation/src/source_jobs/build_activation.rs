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
//! issued it. That rule belongs to `Build`'s declaration: its one `seal`
//! field is the toolchain's `BuildSeal`, which is not `[copy]`, in the
//! toolchain's domain that an access policy governs
//! ([access policies](../../../../../wiki/spec/resources/access.md)). The
//! lowered-form checker admits no source issuer for such a domain and no
//! zero-established or literal value of a record holding one, so the only
//! `Build` a script holds is the one it was lent. That the declaration has
//! this shape is checked here; the checker judges the program against it
//! (`compiler/tests/build_value_protection.rs` shows it on the real `Build`).
//!
//! Limit: the policy names no source issuer because no plan is evaluated
//! yet, not because its plan says so. Once plans are evaluated, the prelude's
//! `BuildSealAccess` plan must state an issuance with no routes.
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::domain::DomainIssuance;
use typed_trees::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use super::is_exact_toolchain_build_prelude_data;

/// `Ok(false)`: the program declares no toolchain `Build`, so it has no
/// activation and no compiler-owned value to protect. `Ok(true)`: it does,
/// with the vocabulary the host decodes and the sealed field that keeps
/// source from producing a `Build`. Anything else is refused by what is
/// missing.
pub fn admit_toolchain_build_activation(typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    Ok(build_activation_symbol(typed)?.is_some())
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
    let mut seals = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "seal" => Some(field),
            _ => None,
        });
    let (Some(seal), None) = (seals.next(), seals.next()) else {
        return Err(refused(
            "toolchain Build must declare exactly one `seal` field".into(),
        ));
    };
    if !is_issued_seal(typed, seal.type_reference) {
        return Err(refused(
            "toolchain Build.seal must be the toolchain BuildSeal, not `[copy]`, in the toolchain domain an access policy governs".into(),
        ));
    }
    Ok(Some(build.symbol))
}

/// Whether `reference` is what keeps source from producing a `Build`: the
/// prelude's own `BuildSeal`, which cannot be duplicated, in exactly one
/// domain, the prelude's own, whose issuers are those its access policy
/// plans. The checker admits no source issuer for such a domain, so the only
/// `Build` a script can hold is the one it was lent.
fn is_issued_seal(typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let types = &typed.type_reference_table;
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = types.type_reference(reference)
    else {
        return false;
    };
    let TypeReferenceNode::Named { symbol, .. } = types.type_reference(*base_type) else {
        return false;
    };
    let affine = typed.data_definitions().iter().any(|definition| {
        definition.symbol == *symbol
            && definition.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
    });
    let [TypeConstraintNode::Domain(domain)] = types.constraints(*constraints) else {
        return false;
    };
    affine
        && is_exact_toolchain_build_prelude_data(typed, *symbol, "BuildSeal")
        && domain.subject == DomainConstraintSubject::Declared
        && domain.arguments.is_empty()
        && typed.symbols.is_build_prelude_symbol(domain.symbol)
        && typed
            .domain_definition(domain.symbol)
            .is_some_and(|definition| matches!(definition.issuance(), DomainIssuance::Planned(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const PRELUDE: &str = r#"
public data X86DeploymentFeatures { avx2: bool; }
public trait AccessPolicy { machine plan() -> u64; }
public data BuildSeal { }
BuildSealAccess satisfies AccessPolicy { machine plan() -> u64 { 0u64 } }
public domain BuildSeal::Issued access BuildSealAccess;
public data Build<'storage> {
    seal: BuildSeal in Issued;
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

    /// The toolchain's `Build` with its sealed field is admitted: nobody in
    /// source can produce one, so the value the host decodes is the one it
    /// lent.
    #[test]
    fn a_program_with_the_sealed_toolchain_build_is_admitted() {
        let typed = typed("machine build_entry() { }", Some(PRELUDE));
        assert_eq!(admit_toolchain_build_activation(&typed), Ok(true));
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

    /// The seal is what keeps source from producing a `Build`, so a prelude
    /// whose `Build` has none, or one source could make or duplicate, is
    /// refused before anything runs. Each prelude changes one declaration
    /// of `PRELUDE`.
    #[test]
    fn a_toolchain_build_that_source_could_produce_is_refused_as_that() {
        const SEAL: &str = "toolchain Build.seal must be the toolchain BuildSeal, not `[copy]`, in the toolchain domain an access policy governs";
        let unsealed = PRELUDE.replace("    seal: BuildSeal in Issued;\n", "");
        assert_ne!(unsealed, PRELUDE);
        assert_eq!(
            refusal(&typed("machine build_entry() { }", Some(&unsealed))),
            "toolchain Build must declare exactly one `seal` field"
        );
        for (declared, changed) in [
            // The bare carrier: a literal makes one.
            ("seal: BuildSeal in Issued;", "seal: BuildSeal;"),
            // No policy: anybody may add the domain.
            ("Issued access BuildSealAccess;", "Issued;"),
            // A seal that copies out of the lent `Build`.
            ("data BuildSeal { }", "data BuildSeal [copy] { }"),
        ] {
            let prelude = PRELUDE.replace(declared, changed);
            assert_ne!(prelude, PRELUDE, "{declared}");
            assert_eq!(
                refusal(&typed("machine build_entry() { }", Some(&prelude))),
                SEAL,
                "{changed}"
            );
        }
    }
}
