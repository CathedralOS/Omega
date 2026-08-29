use omega_package_manager::identity::{PackageKey, PackageName};
use omega_package_manager::manifest::{extract_dependency_projection, extract_package_declaration};
use omega_package_manager::resolution::{
    PackageSourceClosureLimits, resolve_git_package_closure_with_storage,
    resolve_git_package_source_with_storage,
};
use omega_package_manager::review::compile_resolved_package_reviews;
use omega_package_source::{
    GitSourceRequest, LocalSourceLimits, SourceLineage, SourceResolverStorage,
    resolve_git_source_with_storage, resolve_local_source,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemotePin {
    package: String,
    https_url: String,
    commit: String,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("omega-package-manager should live under the Omega workspace")
        .to_path_buf()
}

fn remote_pins() -> Vec<RemotePin> {
    let pins =
        std::fs::read_to_string(workspace_root().join("tests/fixtures/packages/REMOTE_PINS.md"))
            .expect("REMOTE_PINS.md should be readable");
    pins.lines().filter_map(parse_pin_line).collect::<Vec<_>>()
}

fn parse_pin_line(line: &str) -> Option<RemotePin> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.contains("https://github.com/CathedralOS/") {
        return None;
    }
    let columns = trimmed
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();
    if columns.len() != 3 {
        return None;
    }
    Some(RemotePin {
        package: trim_code(columns[0]).to_owned(),
        https_url: trim_code(columns[1]).to_owned(),
        commit: trim_code(columns[2]).to_owned(),
    })
}

fn trim_code(value: &str) -> &str {
    value.trim().trim_matches('`')
}

fn local_package_root(package: &str) -> PathBuf {
    workspace_root()
        .join("tests/fixtures/packages")
        .join(package)
}

fn ssh_url(pin: &RemotePin) -> String {
    format!("git@github.com:CathedralOS/{}.git", pin.package)
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-remote-fixtures-{name}-{}-{stamp}",
        std::process::id()
    ))
}

#[test]
fn remote_fixture_pins_are_exact_and_match_local_package_names() {
    let pins = remote_pins();
    assert_eq!(pins.len(), 11);
    let mut packages = BTreeSet::new();
    for pin in &pins {
        PackageName::parse(&pin.package).expect("remote fixture package names must be kebab-case");
        assert!(pin.https_url.ends_with(&format!("/{}", pin.package)));
        assert_eq!(
            SourceLineage::git(&ssh_url(pin)).expect("SSH fixture locator must define lineage"),
            SourceLineage::git(&pin.https_url)
                .expect("REMOTE_PINS HTTPS locator must define lineage"),
            "{} SSH and HTTPS locators must normalize to one lineage",
            pin.package
        );
        assert_eq!(pin.commit.len(), 40);
        assert!(pin.commit.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(local_package_root(&pin.package).join("build.omg").is_file());
        assert!(local_package_root(&pin.package).join("main.omg").is_file());
        let declared = extract_package_declaration(local_package_root(&pin.package))
            .expect("local fixture must declare its package identity");
        assert_eq!(declared.name.as_str(), pin.package);
        assert!(packages.insert(pin.package.clone()));
    }
}

#[ignore = "requires network access plus private CathedralOS GitHub repository access over SSH"]
#[test]
fn remote_fixture_pins_resolve_to_local_fixture_contents() {
    let cache = temp_root("cache");
    std::fs::create_dir_all(&cache).expect("cache root should be creatable");
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create remote fixture resolver storage");
    for pin in remote_pins() {
        let request = GitSourceRequest::new(ssh_url(&pin), Some(pin.commit.clone()))
            .expect("remote fixture request must be valid");
        let expected_lineage = SourceLineage::git(&pin.https_url)
            .expect("REMOTE_PINS HTTPS locator must define canonical lineage");
        let expected_key = PackageKey::new(
            PackageName::parse(&pin.package).expect("remote fixture package name"),
            expected_lineage.clone(),
        );
        let resolved =
            resolve_git_source_with_storage(&request, &storage, LocalSourceLimits::default())
                .unwrap_or_else(|error| {
                    panic!(
                        "remote fixture {} at {} should resolve: {error}",
                        pin.package,
                        ssh_url(&pin)
                    )
                });
        let local = resolve_local_source(
            local_package_root(&pin.package),
            LocalSourceLimits::default(),
        )
        .expect("local fixture should resolve");

        assert_eq!(
            resolved.commit(),
            pin.commit,
            "{} commit drift",
            pin.package
        );
        assert_eq!(
            resolved.local().content_identity,
            local.content_identity,
            "{} content drift",
            pin.package
        );
        assert_eq!(
            resolved.local().file_count,
            local.file_count,
            "{} file-count drift",
            pin.package
        );
        assert_eq!(
            resolved.local().byte_count,
            local.byte_count,
            "{} byte-count drift",
            pin.package
        );

        let declared = resolve_git_package_source_with_storage(
            &request,
            &storage,
            LocalSourceLimits::default(),
        )
        .unwrap_or_else(|error| {
            panic!(
                "remote fixture {} should bind its declared identity: {error}",
                pin.package
            )
        });
        assert_eq!(declared.key().name().as_str(), pin.package);
        assert_eq!(
            declared.dependency_requests(),
            extract_dependency_projection(local_package_root(&pin.package))
                .expect("local fixture dependency projection should close"),
            "{} dependency projection drift",
            pin.package
        );
        assert_eq!(declared.source().commit(), pin.commit);
        assert_eq!(
            declared.source().local().content_identity,
            local.content_identity,
            "{} declared-source content drift",
            pin.package
        );

        if !declared.dependency_requests().is_empty() {
            assert_eq!(
                pin.package, "graph-workbench",
                "only the workspace-graph fixture may require sibling package custody"
            );
            continue;
        }

        let closure = resolve_git_package_closure_with_storage(
            &request,
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap_or_else(|error| {
            panic!(
                "remote fixture {} should resolve its package closure: {error}",
                pin.package
            )
        });
        assert_eq!(request.lineage(), &expected_lineage);
        assert_eq!(closure.graph().root(), &expected_key);
        assert_eq!(
            closure.source_requests().root().selected().key(),
            &expected_key,
            "{} root request must select the transport-normalized package key",
            pin.package
        );
        let custody = closure
            .custody(&expected_key)
            .expect("resolved root package must retain source custody");
        assert_eq!(custody.key(), &expected_key);
        assert_eq!(custody.resolution(), declared.resolution());

        let compiler_build = cache.join("compiler-build");
        let reviews = compile_resolved_package_reviews(&closure, "windows_x64", &compiler_build)
            .unwrap_or_else(|error| {
                panic!(
                    "remote fixture {} should compile through package-aware review: {error:#?}",
                    pin.package
                )
            });
        let issued = reviews
            .review(&expected_key)
            .expect("compiler review must retain the exact normalized root package key");
        assert_eq!(issued.key(), &expected_key);
        assert_eq!(issued.key().source_lineage(), &expected_lineage);
        assert_eq!(issued.resolution(), custody.resolution());
        assert_eq!(issued.projection().package(), expected_key.identity());
        assert_ne!(issued.source_consumption_commitment().digest(), [0; 32]);
        assert!(
            std::fs::read_dir(&compiler_build)
                .expect("compiler review build root remains readable")
                .next()
                .is_none(),
            "{} compiler review must dispose its private build session",
            pin.package
        );
    }
    let _ = std::fs::remove_dir_all(cache);
}
