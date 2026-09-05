//! Graph membership of owners embedded in genuine compiled policy identities.

use super::*;
use package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use package_evidence::record::{PackagePolicyBaseline, PackagePolicyCallableRole};

fn resolve_chain(tree: &TempTree) -> ResolvedPackageSourceClosure {
    let sources = tree.path("sources");
    for (directory, dependencies, source) in [
        (
            "root",
            "    builder.depend_as(\"middle\", Source::Path { location: \"../middle\" });\n",
            "use middle::main;\npub machine carry(value: Wrapper) -> Wrapper { value }\n",
        ),
        (
            "middle",
            "    builder.depend_as(\"types\", Source::Path { location: \"../types\" });\n",
            concat!(
                "use types::main;\n",
                "pub data Wrapper { token: Token; }\n",
                "pub machine identity(value: Token) -> Token { value }\n",
            ),
        ),
        ("types", "", "pub data Token { value: u64; }\n"),
    ] {
        let member = sources.join(directory);
        write_member(&member, "package", directory, dependencies);
        fs::write(member.join("main.omg"), source).unwrap();
    }
    let storage = SourceResolverStorage::for_hardened_base(tree.path("cache")).unwrap();
    resolve_external_local_project_closure_with_storage(
        sources.join("root"),
        ExternalSourceContext::derive(b"policy-owner-membership-chain"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap()
}

fn hex_digest(value: semantic_vocabulary::PackageKeyIdentity) -> String {
    use std::fmt::Write;
    let mut text = String::new();
    for byte in value.digest() {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}

fn string_scalar(value: &str) -> String {
    use std::fmt::Write;
    let mut text = String::from("string \"");
    for byte in value.bytes() {
        match byte {
            b'"' => text.push_str("\\\""),
            b'\\' => text.push_str("\\\\"),
            0x20..=0x7e => text.push(char::from(byte)),
            _ => write!(&mut text, "\\x{byte:02x}").unwrap(),
        }
    }
    text.push_str("\"\n");
    text
}

#[test]
fn lock_rejects_absent_owners_inside_canonical_types_and_callable_coordinates() {
    let tree = TempTree::new();
    let closure = resolve_chain(&tree);
    let target = TargetProfile::WindowsX64;
    let reviews =
        compile_resolved_package_reviews(&closure.for_exact_target(target), &tree.path("build"))
            .unwrap();
    let source = subject_for(&closure, target);
    let baselines: Vec<_> = source
        .packages()
        .iter()
        .map(|package| reviews.review(package.key()).unwrap().policy().clone())
        .collect();
    let middle = source
        .packages()
        .iter()
        .position(|package| package.key().name().as_str() == "middle")
        .unwrap();
    let type_owner = source
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "types")
        .unwrap()
        .key()
        .identity();
    let owner_text = hex_digest(type_owner);
    let absent = semantic_vocabulary::PackageKeyIdentity::from_digest([239; 32]).unwrap();
    assert!(
        source
            .packages()
            .iter()
            .all(|package| package.key().identity() != absent)
    );
    let absent_text = hex_digest(absent);
    let callable = baselines[middle]
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.role() == PackagePolicyCallableRole::Public)
        .unwrap();
    let type_identity = callable.parameters()[0].type_identity().canonical();
    let callable_identity = callable.identity().path();
    assert!(type_identity.contains(&owner_text));
    assert!(callable_identity.contains(&owner_text));
    let decisions = HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex()
        ),
        &source,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let lock = PackageLock::from_targets(vec![
        PackageLockTarget::from_parts(source.clone(), baselines.clone(), decisions.clone())
            .unwrap(),
    ])
    .unwrap();
    let lock_text = lock.canonical_text().unwrap();
    assert_eq!(
        PackageLock::recover_text(&lock_text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );

    let original_policy = baselines[middle].canonical_text().unwrap();
    for identity in [type_identity, callable_identity] {
        // Match complete canonical string scalars, including byte escapes.
        // A type scalar must not also alter a substring inside its callable
        // coordinate; all exact repeated coordinate occurrences stay joined.
        let changed = identity.replace(&owner_text, &absent_text);
        let original_scalar = string_scalar(identity);
        assert!(original_policy.contains(&original_scalar));
        let candidate_text = original_policy.replace(&original_scalar, &string_scalar(&changed));
        assert_ne!(candidate_text, original_policy);
        let candidate = PackagePolicyBaseline::recover_text(
            &candidate_text,
            PackagePolicyTextRecoveryLimits::default(),
        )
        .expect("standalone policy structure does not require a source graph");
        assert_eq!(candidate.package(), baselines[middle].package());
        let candidate_callable = candidate
            .callables()
            .callables()
            .iter()
            .find(|callable| callable.role() == PackagePolicyCallableRole::Public)
            .unwrap();
        if identity == type_identity {
            assert_eq!(candidate_callable.identity().path(), callable_identity);
            assert_eq!(
                candidate_callable.parameters()[0]
                    .type_identity()
                    .canonical(),
                changed
            );
        } else {
            assert_eq!(candidate_callable.identity().path(), changed);
            assert_eq!(
                candidate_callable.parameters()[0]
                    .type_identity()
                    .canonical(),
                type_identity
            );
        }
        let mut altered = baselines.clone();
        altered[middle] = candidate;
        assert!(matches!(
            PackageLockTarget::from_parts(source.clone(), altered, decisions.clone()),
            Err(PackageLockError::PolicySourceMembership(_))
        ));
        let old_section = format!("baseline {}\n{original_policy}", original_policy.len());
        let new_section = format!("baseline {}\n{candidate_text}", candidate_text.len());
        assert_eq!(lock_text.matches(&old_section).count(), 1);
        let altered_lock = lock_text.replacen(&old_section, &new_section, 1);
        assert!(matches!(
            PackageLock::recover_text(&altered_lock, PackageLockRecoveryLimits::default()),
            Err(PackageLockError::PolicySourceMembership(_))
        ));
    }
}
