use omega_packages::{
    GitSourceSpec, LocalSourceLimits, PackageName, extract_package_declaration, resolve_git_source,
    resolve_local_source,
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
        .nth(6)
        .expect("omega-packages should live under the Omega workspace")
        .to_path_buf()
}

fn remote_pins() -> Vec<RemotePin> {
    let pins = std::fs::read_to_string(workspace_root().join("fixtures/packages/REMOTE_PINS.md"))
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
    workspace_root().join("fixtures/packages").join(package)
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
    assert_eq!(pins.len(), 8);
    let mut packages = BTreeSet::new();
    for pin in &pins {
        PackageName::parse(&pin.package).expect("remote fixture package names must be kebab-case");
        assert!(pin.https_url.ends_with(&format!("/{}", pin.package)));
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
    for pin in remote_pins() {
        let resolved = resolve_git_source(
            &GitSourceSpec {
                url: ssh_url(&pin),
                rev: Some(pin.commit.clone()),
            },
            &cache,
            LocalSourceLimits::default(),
        )
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

        assert_eq!(resolved.commit, pin.commit);
        assert_eq!(resolved.local.content_identity, local.content_identity);
        assert_eq!(resolved.local.file_count, local.file_count);
        assert_eq!(resolved.local.byte_count, local.byte_count);
    }
    let _ = std::fs::remove_dir_all(cache);
}
