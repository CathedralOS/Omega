use super::*;
use compiler::compile_to_checked_with_packages;
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use std::path::PathBuf;

struct Source(PathBuf);
impl Drop for Source {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn attached_alias_requires_exact_owner_field_and_nonempty_source_span() {
    let directory = Source(std::env::temp_dir().join(format!(
            "omega-member-alias-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir(&directory.0).unwrap();
    std::fs::write(
        directory.0.join("main.omg"),
        r#"
pub data Owner { value: u64; other: u64; }
pub data Other { value: u64; }
pub machine Owner::check(&self) {}
pub machine Other::check(&self) {}
"#,
    )
    .unwrap();
    std::fs::write(
        directory.0.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    )
    .unwrap();
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([41; 32]).unwrap();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "review-fixture",
            directory.0.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(
        &directory.0.join("main.omg"),
        Some("windows_x86_64"),
        inputs,
    )
    .unwrap();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::check")
        .unwrap();
    let entry = &checked.machine_states(machine)[0];
    let context = ContractProjectionContext {
        subject_kind: "alias test", subject_name: "Owner::check",
        owner: ContractProofFactOwner::Machine { machine_symbol: machine.symbol },
        point: facts::ProgramPoint::Machine { machine_symbol: machine.symbol },
        parameters: checked.state_parameters(entry), domain_symbol: None, data_symbol: None,
        lifetime_binders: &[], lifetime_substitutions: &[],
        selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let symbol = |path: &str| {
        checked
            .symbols
            .symbols()
            .nodes()
            .iter()
            .find_map(|(handle, _)| {
                (checked.symbols.display_path(handle, "::") == path).then_some(handle)
            })
            .unwrap()
    };
    let expected = symbol("Owner::value");
    let selected = symbol("Owner::check::value");
    assert!(attached_field(&checked, &context, expected, selected));
    assert!(!attached_field(
        &checked,
        &context,
        expected,
        symbol("Other::check::value")
    ));
    // Same owner and valid field declarations are insufficient: the other
    // seeded field points at a different exact source span in the same file.
    assert!(!attached_field(
        &checked,
        &context,
        expected,
        symbol("Owner::check::other")
    ));
    assert!(!attached_field(
        &checked,
        &context,
        symbol("Owner::other"),
        selected
    ));
    assert_ne!(
        checked.symbols.symbol_source_span(expected),
        checked
            .symbols
            .symbol_source_span(symbol("Owner::check::other"))
    );
}
