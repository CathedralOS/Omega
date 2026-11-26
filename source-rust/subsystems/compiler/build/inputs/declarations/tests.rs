/// The boundary requirements of `source` that do not write `crashes none`,
/// as `Trait::requirement`. The parser refuses a boundary requirement that
/// writes no `crashes` clause at all (effects.md: it "must likewise write
/// `crashes`"); this reading asks the stronger question the Build host needs,
/// that each operation publishes no crash route.
fn requirements_with_crash_routes(source: &str) -> Vec<String> {
    let parsed = source_syntax::parse_text(source).unwrap_or_else(|_| panic!("source parses"));
    let syntax = &parsed.syntax;
    let mut open = Vec::new();
    for item in syntax.root_items() {
        let syntax_trees::item::Item::Trait(definition) = item else {
            continue;
        };
        if !definition.is_boundary {
            continue;
        }
        for handle in syntax.items.state_signatures(definition.machines) {
            let requirement = syntax.items.state_signature(*handle);
            // Every host operation here completes or the host refuses the
            // whole activation; neither is a crash route of the Build
            // program, so each writes `crashes none`.
            if !requirement.declares_no_crash_route {
                open.push(format!(
                    "{}::{}",
                    definition.name.as_str(syntax),
                    requirement.name.as_str(syntax)
                ));
            }
        }
    }
    open
}

#[test]
fn successor_entry_selects_one_package_relative_omega_file() {
    use crate::BuildEntryPath;
    for path in ["configure.omg", "stages/compile.omg", "build.lock.omg"] {
        assert_eq!(BuildEntryPath::parse(path.into()).unwrap().as_str(), path);
    }
    for path in [
        "",
        "build",
        ".omg",
        "a/.omg",
        "stage/",
        "../outside.omg",
        "./main.omg",
        "/main.omg",
        "a//main.omg",
        r"C:\main.omg",
        "a/C:/main.omg",
        "build.omg/../main.omg",
    ] {
        assert!(BuildEntryPath::parse(path.into()).is_err(), "{path}");
    }
}

#[test]
fn every_build_host_operation_writes_its_crash_ceiling() {
    assert_eq!(
        requirements_with_crash_routes(&crate::build_prelude().declarations),
        Vec::<String>::new()
    );
    // The same reading reports a requirement that publishes a crash route.
    assert_eq!(
        requirements_with_crash_routes(
            "boundary trait Host {
             machine closed(value: u64) reaches none crashes none;
             machine trapping(value: u64) reaches none crashes Trap;
             }
"
        ),
        ["Host::trapping"]
    );
}

#[test]
fn build_prelude_uses_file_imports_and_free_machines() {
    let prelude = crate::build_prelude();
    for (namespace, text) in std::iter::once((None, prelude.declarations)).chain(
        prelude
            .modules
            .into_iter()
            .map(|(namespace, text)| (Some(namespace), text)),
    ) {
        let syntax = source_syntax::parse_text(&text)
            .expect("compiler-owned prelude source")
            .syntax;
        assert!(
            !text.lines().any(|line| line.trim_start().starts_with("module ")),
            "{namespace:?}"
        );
        for item in syntax.root_items() {
            assert!(
                !matches!(item, syntax_trees::item::Item::Use(syntax_trees::item::UseItem::Namespace { .. })),
                "{namespace:?}: a Build import must select a file"
            );
            if let syntax_trees::item::Item::Machine(machine) = item {
                assert!(
                    machine.attached_data.is_none(),
                    "{namespace:?}: a helper still names a data owner"
                );
            }
        }
    }
}
