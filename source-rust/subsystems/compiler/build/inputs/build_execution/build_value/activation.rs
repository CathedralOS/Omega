//! Admit the toolchain declaration for the Build value the host issues.
//!
//! A Build script runs against one compiler-owned `BuildPlan` value: the host
//! builds it, hands the script `@mutable BuildPlan`, and decodes what the script left
//! in it by field. Two things must hold before that is sound.
//!
//! The first is the vocabulary the host decodes: the program declares the
//! toolchain's `BuildPlan` exactly once. Its deployment features and optimizations
//! must use the corresponding toolchain declarations.
//!
//! The second is that source cannot construct, store or return a `BuildPlan`
//! value of its own, because the host would decode a forged one as if it had
//! issued it. That rule belongs to `BuildPlan`'s declaration: its one `seal`
//! field is the toolchain's `BuildSeal`, which is not `[copy]`, in the
//! toolchain's domain that an access policy governs
//! ([access policies](../../../../../../../../wiki/spec/resources/access.md)). The
//! lowered-form checker admits no source issuer for such a domain and no
//! zero-established or literal value of a record holding one, so the only
//! `BuildPlan` a script holds is the one it was lent. That the declaration has
//! this shape is checked here; the checker judges the program against it.

use diagnostics::Diagnostic;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::data::{DataDefinition, DataMember};
use source_compiler::representations::source_state::domain::DomainIssuance;
use source_compiler::representations::source_state::name::Identifier;
use source_compiler::representations::source_state::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use super::is_exact_toolchain_build_prelude_data;

/// Return the toolchain Build declaration after checking its fields and issuance seal.
pub(super) fn admit_toolchain_build_activation(
    typed: &TypedTrees,
) -> Result<&DataDefinition, Vec<Diagnostic>> {
    let refused = |message: String| vec![Diagnostic::error(message)];
    let mut builds = typed.data_definitions().iter().filter(|definition| {
        is_exact_toolchain_build_prelude_data(typed, definition.symbol, build_declarations::BUILD_TYPE_NAME)
    });
    let Some(build) = builds.next() else {
        return Err(refused(
            "Build evaluation requires the toolchain-provided BuildPlan declaration".into(),
        ));
    };
    if builds.next().is_some() {
        return Err(refused(
            "the program declares more than one toolchain Build activation".into(),
        ));
    }
    require_toolchain_field(
        typed,
        build,
        "x86_deployment_features",
        "X86DeploymentFeatures",
        &[],
    )?;
    require_toolchain_field(typed, build, "optimizations", "Optimizations", &[])?;
    let [storage] = build.lifetime_parameters.as_slice() else {
        return Err(refused(
            "toolchain BuildPlan must declare its storage lifetime".into(),
        ));
    };
    require_toolchain_field(
        typed,
        build,
        "product",
        "BuildProduct",
        std::slice::from_ref(storage),
    )?;
    let mut seals = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "seal" => Some(field),
            _ => None,
        });
    let (Some(seal), None) = (seals.next(), seals.next()) else {
        return Err(refused(
            "toolchain BuildPlan must declare exactly one `seal` field".into(),
        ));
    };
    if !is_issued_seal(typed, seal.type_reference) {
        return Err(refused(
            "toolchain BuildPlan.seal must be the toolchain BuildSeal, not `[copy]`, in the toolchain domain an access policy governs".into(),
        ));
    }
    Ok(build)
}

fn require_toolchain_field(
    typed: &TypedTrees,
    build: &DataDefinition,
    name: &str,
    expected_type: &str,
    lifetimes: &[Identifier],
) -> Result<(), Vec<Diagnostic>> {
    let mut fields = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => Some(field),
            _ => None,
        });
    let (Some(field), None) = (fields.next(), fields.next()) else {
        return Err(vec![Diagnostic::error(format!(
            "toolchain BuildPlan must declare exactly one `{name}` field"
        ))]);
    };
    let carrier = match typed
        .type_reference_table
        .type_reference(field.type_reference)
    {
        TypeReferenceNode::Named { symbol, .. } if lifetimes.is_empty() => Some(*symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } if arguments.is_empty() && lifetime_arguments == lifetimes => Some(*base_symbol),
        _ => None,
    };
    let exact = carrier
        .is_some_and(|symbol| is_exact_toolchain_build_prelude_data(typed, symbol, expected_type));
    if !exact {
        return Err(vec![Diagnostic::error(format!(
            "toolchain BuildPlan.{name} must have the exact toolchain {expected_type} type"
        ))]);
    }
    Ok(())
}

/// Whether `reference` is what keeps source from producing a `BuildPlan`: the
/// prelude's own `BuildSeal`, which cannot be duplicated, in exactly one
/// domain, the prelude's own, whose issuers are those its access policy
/// plans. The checker admits no source issuer for such a domain, so the only
/// `BuildPlan` a script can hold is the one it was lent.
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
            && definition.properties.multiplicity != language_core::Multiplicity::Unrestricted
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
public data Optimizations { human_report: u8; selected: [u8; 1]; }
public trait AccessPolicy { machine plan() -> u64; }
public data BuildSeal { }
BuildSealAccess satisfies AccessPolicy { machine plan() -> u64 { 0u64 } }
public domain BuildSeal::Issued access BuildSealAccess;
public data BuildProduct<'storage> { case Unselected; case Package(name: @'storage [u8]); }
public data BuildPlan<'storage> {
    product: BuildProduct<'storage>;
    seal: BuildSeal in Issued;
    x86_deployment_features: X86DeploymentFeatures;
    optimizations: Optimizations;
    identifier: @'storage [u8];
}
"#;

    /// `build` typed beside `prelude`, which is registered the way the
    /// toolchain registers its Build prelude.
    fn typed(build: &str, prelude: Option<&str>) -> TypedTrees {
        let mut sources = source_compiler::representations::source_inputs::SourceMap::default();
        let mut loaded = Vec::new();
        let files = [
            Some((
                "package/build.omg",
                build,
                source_admission::SourceOrigin::User,
            )),
            prelude.map(|text| {
                (
                    "<build-prelude>",
                    text,
                    source_admission::SourceOrigin::Toolchain,
                )
            }),
        ];
        for (path, text, origin) in files.into_iter().flatten() {
            let file = sources.add_checked_instance(
                path.into(),
                text,
                if origin == source_admission::SourceOrigin::Toolchain {
                    path
                } else {
                    "package"
                }
                .into(),
                None,
                origin,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Build,
            );
            loaded.push(source_admission::LoadedSource {
                source_id: file.source_id,
                path: file.path.clone(),
                source: Arc::clone(&file.source),
                origin: Some(file.origin),
                scope: file.dependency_scope,
            });
        }
        let lexed =
            source_compiler::test_support::lexing::lex(loaded.into_iter().collect()).unwrap();
        let (pages, _) = source_compiler::test_support::parsing::parse(&lexed, Default::default())
            .unwrap()
            .into_parts();
        let mut pages = pages.into_iter();
        let mut syntax = pages.next().unwrap();
        for page in pages {
            syntax.adopt_page(page);
        }
        let mut request = source_compiler::test_support::resolution::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        let resolved = source_compiler::test_support::resolution::resolve(request).unwrap();
        source_compiler::test_support::typing::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn refusal(typed: &TypedTrees) -> String {
        let refused = admit_toolchain_build_activation(typed).unwrap_err();
        assert_eq!(refused.len(), 1);
        refused[0].message.clone()
    }

    #[test]
    fn a_program_with_the_sealed_toolchain_build_has_an_activation() {
        let typed = typed("machine build_entry() { }", Some(PRELUDE));
        assert_eq!(
            admit_toolchain_build_activation(&typed)
                .unwrap()
                .name
                .as_str(),
            "BuildPlan"
        );
    }

    #[test]
    fn a_source_owned_build_cannot_receive_the_host_value() {
        for source in [
            "data BuildPlan { marker: u64; }",
            "data Optimizations { human_report: u8; } data BuildPlan { optimizations: Optimizations; }",
        ] {
            let own = typed(&format!("{source}\nmachine build_entry() {{ }}"), None);
            assert_eq!(
                refusal(&own),
                "Build evaluation requires the toolchain-provided BuildPlan declaration"
            );
        }
    }

    #[test]
    fn the_toolchain_build_requires_its_own_optimization_type() {
        let missing = PRELUDE.replace("    optimizations: Optimizations;\n", "");
        assert_eq!(
            refusal(&typed("machine build_entry() { }", Some(&missing))),
            "toolchain BuildPlan must declare exactly one `optimizations` field"
        );
        let wrong = PRELUDE.replace(
            "optimizations: Optimizations;",
            "optimizations: X86DeploymentFeatures;",
        );
        assert_eq!(
            refusal(&typed("machine build_entry() { }", Some(&wrong))),
            "toolchain BuildPlan.optimizations must have the exact toolchain Optimizations type"
        );
    }

    #[test]
    fn the_build_product_must_be_the_toolchain_sum_in_its_own_storage() {
        let wrong = PRELUDE.replace(
            "product: BuildProduct<'storage>;",
            "product: @'storage BuildProduct<'storage>;",
        );
        assert_eq!(
            refusal(&typed("machine build_entry() { }", Some(&wrong))),
            "toolchain BuildPlan.product must have the exact toolchain BuildProduct type"
        );
    }

    /// The vocabulary is judged before the value's protection, so a prelude
    /// the host could not decode is reported as that.
    #[test]
    fn a_toolchain_build_without_the_decoded_vocabulary_is_refused_as_that() {
        let missing = typed(
            "machine build_entry() { }",
            Some("public data BuildPlan { marker: u64; }"),
        );
        assert_eq!(
            refusal(&missing),
            "toolchain BuildPlan must declare exactly one `x86_deployment_features` field"
        );
        let foreign = typed(
            "machine build_entry() { }",
            Some("public data BuildPlan { x86_deployment_features: u64; }"),
        );
        assert_eq!(
            refusal(&foreign),
            "toolchain BuildPlan.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type"
        );
    }

    /// The seal is what keeps source from producing a `BuildPlan`, so a prelude
    /// whose `BuildPlan` has none, or one source could make or duplicate, is
    /// refused before anything runs. Each prelude changes one declaration
    /// of `PRELUDE`.
    #[test]
    fn a_toolchain_build_that_source_could_produce_is_refused_as_that() {
        const SEAL: &str = "toolchain BuildPlan.seal must be the toolchain BuildSeal, not `[copy]`, in the toolchain domain an access policy governs";
        let unsealed = PRELUDE.replace("    seal: BuildSeal in Issued;\n", "");
        assert_ne!(unsealed, PRELUDE);
        assert_eq!(
            refusal(&typed("machine build_entry() { }", Some(&unsealed))),
            "toolchain BuildPlan must declare exactly one `seal` field"
        );
        for (declared, changed) in [
            // The bare carrier: a literal makes one.
            ("seal: BuildSeal in Issued;", "seal: BuildSeal;"),
            // No policy: anybody may add the domain.
            ("Issued access BuildSealAccess;", "Issued;"),
            // A seal that copies out of the lent `BuildPlan`.
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
