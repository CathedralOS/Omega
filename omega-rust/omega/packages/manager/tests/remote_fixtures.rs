use package_manager::declarations::{PackageKey, PackageName};
use package_manager::declarations::{extract_dependency_projection, extract_package_declaration};
use package_manager::resolution::graph::{PackageSourceClosureLimits, resolve_git_package_closure};
use package_manager::resolution::source::resolve_git_package_source;
use package_manager::review::SemanticBindingReview;
use package_manager::review::compile_resolved_package_reviews;
use package_source::PrimaryGitChoices;
use package_source::{
    GitSourceRequest, LocalSourceLimits, SourceLineage, SourceResolverStorage, resolve_git_source,
    resolve_local_source,
};
#[cfg(unix)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "remote_fixtures/fixture.rs"]
mod fixture;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemotePin {
    package: String,
    https_url: String,
    commit: String,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| {
            ancestor
                .join("tests/fixtures/packages/REMOTE_PINS.md")
                .is_file()
        })
        .expect("package-manager should live beneath the Omega workspace")
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
    assert_eq!(pins.len(), 12);
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
    verify_remote_pins(remote_pins(), target::TargetProfile::WindowsX64);
}

#[test]
fn remote_builds_pin_recorded_dependencies_without_sibling_paths() {
    use package_manager::declarations::{DependencySourceRequest, PackageSelection};
    let fixture = fixture::Fixture::new();
    let pins = remote_pins();
    for (name, dependencies) in [
        ("file-journal", &["host-services"][..]),
        ("process-exit", &["host-services"][..]),
        ("remote-journal", &["host-services"][..]),
        (
            "graph-workbench",
            &["arithmetic-kernels", "file-journal"][..],
        ),
    ] {
        let expected = fixture.expected_package(name);
        assert_eq!(
            extract_package_declaration(&expected)
                .unwrap()
                .name
                .as_str(),
            name
        );
        let requests = extract_dependency_projection(&expected).unwrap();
        assert_eq!(requests.len(), dependencies.len());
        let local = extract_dependency_projection(local_package_root(name)).unwrap();
        assert_eq!(local.len(), dependencies.len());
        for ((request, local), dependency) in requests.iter().zip(&local).zip(dependencies) {
            let pin = pins.iter().find(|pin| &pin.package == dependency).unwrap();
            let DependencySourceRequest::Git {
                explicit_alias,
                repository,
                revision,
                selection,
            } = request
            else {
                panic!("remote fixture must have pinned Git dependencies");
            };
            assert!(explicit_alias.is_none());
            assert_eq!(repository, &ssh_url(pin));
            assert_eq!(revision, &pin.commit);
            assert_eq!(selection, &PackageSelection::Root);
            assert!(
                matches!(local, DependencySourceRequest::Path { location, .. } if location == &format!("../{dependency}"))
            );
        }
    }
}

#[ignore = "requires private CathedralOS GitHub repository access over SSH for the complete closure"]
#[test]
fn refreshed_authority_pins_resolve_and_review_over_ssh() {
    let pins = remote_pins()
        .into_iter()
        .filter(|pin| {
            matches!(
                pin.package.as_str(),
                "host-services" | "file-journal" | "process-exit"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(pins.len(), 3);
    verify_remote_pins(pins, target::TargetProfile::LinuxX64);
}

fn verify_remote_pins(pins: Vec<RemotePin>, target: target::TargetProfile) {
    let fixture = fixture::Fixture::new();
    let cache = fixture.path("cache");
    std::fs::create_dir_all(&cache).expect("cache root should be creatable");
    let storage = SourceResolverStorage::for_hardened_base(&cache, PrimaryGitChoices::default())
        .expect("create remote fixture resolver storage");
    for pin in pins {
        let request = GitSourceRequest::new(ssh_url(&pin), Some(pin.commit.clone()))
            .expect("remote fixture request must be valid");
        let expected_root = fixture.expected_package(&pin.package);
        verify_remote_pin(
            &pin,
            &request,
            &pin.commit,
            &expected_root,
            &storage,
            &cache,
            target,
        );
    }
}

fn verify_remote_pin(
    pin: &RemotePin,
    request: &GitSourceRequest,
    expected_commit: &str,
    expected_root: &Path,
    storage: &SourceResolverStorage,
    cache: &Path,
    target: target::TargetProfile,
) {
    let expected_lineage = SourceLineage::git(&pin.https_url)
        .expect("REMOTE_PINS HTTPS locator must define canonical lineage");
    let expected_key = PackageKey::new(
        PackageName::parse(&pin.package).expect("remote fixture package name"),
        expected_lineage.clone(),
    );
    let resolved = resolve_git_source(request, storage, LocalSourceLimits::default())
        .unwrap_or_else(|error| {
            panic!(
                "remote fixture {} at {} should resolve: {error}",
                pin.package,
                ssh_url(pin)
            )
        });
    let local = resolve_local_source(expected_root, LocalSourceLimits::default())
        .expect("local fixture should resolve");

    assert_eq!(
        resolved.commit(),
        expected_commit,
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

    let declared = resolve_git_package_source(request, storage, LocalSourceLimits::default())
        .unwrap_or_else(|error| {
            panic!(
                "remote fixture {} should bind its declared identity: {error}",
                pin.package
            )
        });
    assert_eq!(declared.key().name().as_str(), pin.package);
    assert_eq!(
        declared.product_dependency_requests(),
        extract_dependency_projection(expected_root)
            .expect("local fixture dependency projection should close"),
        "{} dependency projection drift",
        pin.package
    );
    assert_eq!(declared.source().commit(), expected_commit);
    assert_eq!(
        declared.source().local().content_identity,
        local.content_identity,
        "{} declared-source content drift",
        pin.package
    );

    let closure = resolve_git_package_closure(
        request,
        storage,
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
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(target),
        &compiler_build,
        SemanticBindingReview::Explicit(&[]),
    )
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

// The pinned SSH flows above stay `#[ignore]`d: they need live access to the
// private CathedralOS mirrors. This section replays the same acquisition,
// closure, and review path on every host. Each pin's recorded remote tree is
// committed under <root>/CathedralOS/<package>.git, and the child process runs
// with GIT_SSH_COMMAND pointed at a shim that redirects the scp-like locator's
// upload-pack request to that mirror. Dependency commits recorded in
// REMOTE_PINS.md cannot be reproduced locally, so mirror trees pin the
// dependencies' mirror heads instead; the recorded dependency pins remain
// asserted by remote_builds_pin_recorded_dependencies_without_sibling_paths
// and by the ignored live-network tests.

#[cfg(unix)]
const MIRROR_ROOT_ENV: &str = "OMEGA_REMOTE_FIXTURE_MIRROR_ROOT";

#[cfg(unix)]
#[test]
fn recorded_remote_pins_replay_through_local_ssh_transport() {
    replay_mirrored_pins(
        remote_pins(),
        target::TargetProfile::WindowsX64,
        "recorded_remote_pins_replay_through_local_ssh_transport",
    );
}

#[cfg(unix)]
#[test]
fn refreshed_authority_pins_replay_through_local_ssh_transport() {
    let pins = remote_pins()
        .into_iter()
        .filter(|pin| {
            matches!(
                pin.package.as_str(),
                "host-services" | "file-journal" | "process-exit"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(pins.len(), 3);
    replay_mirrored_pins(
        pins,
        target::TargetProfile::LinuxX64,
        "refreshed_authority_pins_replay_through_local_ssh_transport",
    );
}

#[cfg(not(unix))]
#[test]
#[ignore = "local SSH transport mirrors require the Unix test-only SSH transport"]
fn remote_fixture_mirrors_require_unix_shell() {}

#[cfg(unix)]
fn replay_mirrored_pins(pins: Vec<RemotePin>, target: target::TargetProfile, test: &'static str) {
    if let Some(root) = std::env::var_os(MIRROR_ROOT_ENV) {
        verify_mirrored_pins(&pins, target, Path::new(&root));
        return;
    }
    let fixture = fixture::Fixture::new();
    let root = fixture.path("mirrors");
    std::fs::create_dir_all(&root).expect("create mirror root");
    create_mirrors(&root, &fixture, &pins);
    let calls = root.join("transport-calls");
    std::fs::write(&calls, "").expect("create transport call counter");
    // The host git configuration may rewrite the scp-like locators (for
    // example through url.insteadOf into an HTTPS proxy); isolate the child so
    // acquisition really exercises the SSH transport shim.
    let git_config = root.join("git-config");
    std::fs::write(&git_config, "").expect("create isolated Git configuration");
    let shim = root.join("ssh-transport.sh");
    std::fs::write(&shim, ssh_transport_shim(&root, &calls)).expect("write SSH transport shim");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &child_filter(test), "--nocapture"])
        .env(MIRROR_ROOT_ENV, &root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &git_config)
        .env(
            "GIT_SSH_COMMAND",
            format!("sh {}", shell_quote(&shim.to_string_lossy())),
        )
        .env("GIT_SSH_VARIANT", "simple")
        .output()
        .expect("run remote-fixture child");
    assert!(
        output.status.success(),
        "{test}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
        "child must execute exactly {test}: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let calls = std::fs::read_to_string(&calls).expect("read transport call counter");
    assert!(
        calls.lines().count() >= pins.len(),
        "every remote pin must acquire through the SSH transport shim"
    );
}

// The child's test name is `remote_fixtures::<test>` under the suite binary;
// `module_path!()` carries the crate prefix, so keep only the topic segment.
#[cfg(unix)]
fn child_filter(test: &str) -> String {
    let topic = module_path!()
        .rsplit("::")
        .next()
        .expect("module path carries a topic segment");
    format!("{topic}::{test}")
}

#[cfg(unix)]
fn verify_mirrored_pins(pins: &[RemotePin], target: target::TargetProfile, root: &Path) {
    let fixture = fixture::Fixture::new();
    let cache = fixture.path("cache");
    std::fs::create_dir_all(&cache).expect("cache root should be creatable");
    let storage = SourceResolverStorage::for_hardened_base(&cache, PrimaryGitChoices::default())
        .expect("create remote fixture resolver storage");
    let heads = pins
        .iter()
        .map(|pin| {
            (
                pin.package.clone(),
                git_stdout(&mirror_repository(root, pin), &["rev-parse", "HEAD"]),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for pin in pins {
        let head = heads[&pin.package].clone();
        let request = GitSourceRequest::new(ssh_url(pin), Some(head.clone()))
            .expect("mirror request must be valid");
        let expected_root = fixture.expected_package(&pin.package);
        let substitutions = mirror_substitutions(pin, pins, &heads);
        if !substitutions.is_empty() {
            adapt_mirror_build(&expected_root.join("build.omg"), &substitutions);
        }
        verify_remote_pin(
            pin,
            &request,
            &head,
            &expected_root,
            &storage,
            &cache,
            target,
        );
    }
}

#[cfg(unix)]
fn create_mirrors(root: &Path, fixture: &fixture::Fixture, pins: &[RemotePin]) {
    let mut heads = BTreeMap::new();
    let mut pending = pins.iter().collect::<Vec<_>>();
    while !pending.is_empty() {
        let mut deferred = Vec::new();
        let mut progressed = false;
        for pin in pending {
            if remote_git_dependencies(&pin.package)
                .iter()
                .all(|dependency| heads.contains_key(dependency))
            {
                let head = create_mirror(root, fixture, pin, pins, &heads);
                heads.insert(pin.package.clone(), head);
                progressed = true;
            } else {
                deferred.push(pin);
            }
        }
        assert!(progressed, "recorded remote dependencies must not cycle");
        pending = deferred;
    }
}

#[cfg(unix)]
fn create_mirror(
    root: &Path,
    fixture: &fixture::Fixture,
    pin: &RemotePin,
    pins: &[RemotePin],
    heads: &BTreeMap<String, String>,
) -> String {
    let repository = mirror_repository(root, pin);
    std::fs::create_dir_all(&repository).expect("create mirror repository");
    copy_fixture_tree(&fixture.expected_package(&pin.package), &repository);
    let substitutions = mirror_substitutions(pin, pins, heads);
    if !substitutions.is_empty() {
        adapt_mirror_build(&repository.join("build.omg"), &substitutions);
    }
    run_git(&repository, &["init", "--quiet"]);
    run_git(
        &repository,
        &["config", "uploadpack.allowAnySHA1InWant", "true"],
    );
    run_git(&repository, &["config", "uploadpack.allowFilter", "true"]);
    run_git(&repository, &["add", "."]);
    run_git(
        &repository,
        &[
            "-c",
            "user.name=Omega Tests",
            "-c",
            "user.email=omega@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "recorded remote fixture",
        ],
    );
    git_stdout(&repository, &["rev-parse", "HEAD"])
}

// The override build declarations are the only fixture content carrying Git
// dependencies; local fixtures use sibling Path dependencies instead.
#[cfg(unix)]
fn remote_git_dependencies(package: &str) -> Vec<String> {
    use package_manager::declarations::DependencySourceRequest;
    let overrides = workspace_root()
        .join("tests/fixtures/package-remotes")
        .join(package);
    if !overrides.join("build.omg").is_file() {
        return Vec::new();
    }
    extract_dependency_projection(&overrides)
        .expect("recorded remote build declaration must parse")
        .iter()
        .map(|request| match request {
            DependencySourceRequest::Git { repository, .. } => repository
                .strip_prefix("git@github.com:CathedralOS/")
                .and_then(|name| name.strip_suffix(".git"))
                .expect("remote fixture dependencies stay inside the fixture organization")
                .to_owned(),
            DependencySourceRequest::Path { .. } => {
                panic!("recorded remote fixture dependencies must be pinned Git sources")
            }
        })
        .collect()
}

#[cfg(unix)]
fn mirror_substitutions<'a>(
    pin: &RemotePin,
    pins: &'a [RemotePin],
    heads: &'a BTreeMap<String, String>,
) -> Vec<(&'a str, &'a str)> {
    remote_git_dependencies(&pin.package)
        .into_iter()
        .map(|name| {
            let dependency = pins
                .iter()
                .find(|candidate| candidate.package == name)
                .unwrap_or_else(|| panic!("{name} mirror must be replayed beside its dependents"));
            let head = heads
                .get(&name)
                .unwrap_or_else(|| panic!("dependency mirror {name} must exist first"));
            (dependency.commit.as_str(), head.as_str())
        })
        .collect()
}

// Recorded remote content predates the qualified provider-operand spelling
// that the local fixtures were realigned to; replay applies the same
// realignment when the retired operand is present.
#[cfg(unix)]
const RETIRED_PROVIDER_OPERAND_REWRITES: &[(&str, &str)] = &[(
    "builder.select_provider<Console, ConsoleNativeProvider>()",
    "builder.select_provider<host_services::Console, host_services::ConsoleNativeProvider>()",
)];

#[cfg(unix)]
fn adapt_mirror_build(build_path: &Path, substitutions: &[(&str, &str)]) {
    let mut build = std::fs::read_to_string(build_path).expect("read mirror build declaration");
    let mut changed = false;
    for (recorded, head) in substitutions {
        assert!(
            build.contains(recorded),
            "remote build declaration must pin {recorded}"
        );
        build = build.replace(recorded, head);
        changed = true;
    }
    for (retired, current) in RETIRED_PROVIDER_OPERAND_REWRITES {
        if build.contains(retired) {
            build = build.replace(retired, current);
            changed = true;
        }
    }
    if changed {
        std::fs::write(build_path, build).expect("write mirror build declaration");
    }
}

#[cfg(unix)]
fn mirror_repository(root: &Path, pin: &RemotePin) -> PathBuf {
    let locator = ssh_url(pin);
    let path = locator
        .split_once(':')
        .expect("scp-like SSH locator carries a repository path")
        .1;
    root.join(path)
}

#[cfg(unix)]
fn copy_fixture_tree(source: &Path, destination: &Path) {
    for entry in std::fs::read_dir(source).expect("list remote expectation tree") {
        let entry = entry.expect("read remote expectation entry");
        let target = destination.join(entry.file_name());
        if entry
            .metadata()
            .expect("remote expectation metadata")
            .is_dir()
        {
            std::fs::create_dir_all(&target).expect("create mirror directory");
            copy_fixture_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy remote expectation file");
        }
    }
}

#[cfg(unix)]
fn ssh_transport_shim(root: &Path, calls: &Path) -> String {
    format!(
        concat!(
            "#!/bin/sh\n",
            "printf 'call\\n' >> {calls}\n",
            "for last do :; done\n",
            "case $last in\n",
            "  \"git-upload-pack '\"*\"'\")\n",
            "    path=${{last#\"git-upload-pack '\"}}\n",
            "    path=${{path%\"'\"}}\n",
            "    ;;\n",
            "  *) exit 2 ;;\n",
            "esac\n",
            "exec git upload-pack {root}\"/$path\"\n",
        ),
        calls = shell_quote(&calls.to_string_lossy()),
        root = shell_quote(&root.to_string_lossy()),
    )
}

#[cfg(unix)]
fn run_git(repository: &Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .expect("spawn fixture Git");
    assert!(
        output.status.success(),
        "fixture Git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn git_stdout(repository: &Path, arguments: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .expect("spawn fixture Git");
    assert!(
        output.status.success(),
        "fixture Git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("fixture Git output is UTF-8")
        .trim()
        .to_owned()
}

#[cfg(unix)]
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}
