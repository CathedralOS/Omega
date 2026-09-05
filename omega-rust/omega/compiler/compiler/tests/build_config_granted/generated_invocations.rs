use super::*;

fn compile_generated_invocation(
    label: &str,
    generated_source: &str,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let project = Project::new(label);
    project.write(
        "main.omg",
        "boundary trait Console { machine write(value: i32) reaches Console; }\n\
         boundary trait FilesystemHost { machine touch() reaches FilesystemHost; }\n\
         boundary trait Network { machine connect() reaches Network; }\n\
         machine touch_files() { FilesystemHost::touch(); }\n\
         machine middle() { touch_files(); }\n\
         machine declared_files() reaches FilesystemHost {}\n\
         machine reach_helper() { declared_files(); }\n\
         machine declared_network() reaches Network {}\n\
         machine network_helper() { declared_network(); }\n",
    );
    let escaped = generated_source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    project.write(
        "build.omg",
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.package("generated-invocations");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "{escaped}");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}}
"#
        ),
    );
    let session = Project::new(&format!("{label}-session"));
    let session_root = std::fs::canonicalize(&session.root).unwrap();
    let sponsor = FilesystemSponsor::new(&session_root).unwrap();
    let build_dir = session_root.join("output");
    let bound = sponsor.bind_path(&build_dir).unwrap();
    let prepared = sponsor.prepare_create_directory(&bound).unwrap();
    std::fs::create_dir(&build_dir).unwrap();
    prepared.commit().unwrap();
    set_canonical_source_tree_permissions(&project.root, true);
    compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.main(),
        &build_dir,
        Some(target::TargetProfile::LinuxX64.target_name()),
        package_inputs(&project.root),
        sponsor,
    )
}

#[test]
fn generated_invocation_reaches_final_effect_checking() {
    let checked = compile_generated_invocation(
        "generated-invocation-valid",
        "pub machine generated() reaches FilesystemHost invokes FilesystemHost; { middle(); }\n",
    )
    .expect("generated invocation uses the ordinary transitive effect checker");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .unwrap();
    let filesystem = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "FilesystemHost")
        .unwrap();
    assert_eq!(
        checked
            .typed
            .service_reach_rows
            .services(machine.service_reach_row),
        &[checked
            .typed
            .service_reaches
            .id_for_symbol(filesystem.symbol)
            .unwrap()],
    );
    let [invocation] = checked.typed.machine_invokes(machine) else {
        panic!("one generated invocation");
    };
    assert_eq!(
        invocation.target,
        typed_trees::signature::AuthoredInvocationTarget::Service(filesystem.symbol)
    );
}

#[test]
fn generated_invocations_combine_retained_services_in_final_checking() {
    let checked = compile_generated_invocation(
        "generated-invocation-combined",
        "pub machine generated() reaches Console + FilesystemHost invokes Console; invokes FilesystemHost; { Console::write(7); middle(); }\n",
    )
    .expect("new reach combination receives ordinary final checking");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .unwrap();
    let console = checked
        .typed
        .service_reaches
        .id_for_name("Console")
        .unwrap();
    let filesystem = checked
        .typed
        .service_reaches
        .id_for_name("FilesystemHost")
        .unwrap();
    assert_eq!(
        checked
            .typed
            .service_reach_rows
            .services(machine.service_reach_row),
        &[console, filesystem]
    );
    assert_eq!(checked.typed.machine_invokes(machine).len(), 2);
}

#[test]
fn generated_combined_ceiling_still_rejects_an_undeclared_transitive_service() {
    let diagnostics = compile_generated_invocation(
        "generated-invocation-combined-false-reach",
        "pub machine generated() reaches Console + FilesystemHost invokes Console; invokes FilesystemHost; { Console::write(7); middle(); network_helper(); }\n",
    )
    .expect_err("new combined row cannot hide a third reachable service");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("machine `generated`"), "{messages}");
    assert!(
        messages.contains("reaches undeclared service `Network`"),
        "{messages}"
    );
}

#[test]
fn generated_invocation_cannot_hide_transitive_filesystem_invocation_under_console_ceiling() {
    let diagnostics = compile_generated_invocation(
        "generated-invocation-false-ceiling",
        "pub machine generated() reaches Console invokes Console; { middle(); }\n",
    )
    .expect_err("generated -> middle -> touch_files must exceed the Console-only ceiling");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("machine `generated`"), "{messages}");
    assert!(
        messages.contains("omits `invokes FilesystemHost;`"),
        "{messages}"
    );
}

#[test]
fn generated_invocation_cannot_hide_transitive_filesystem_reach_under_console_ceiling() {
    let diagnostics = compile_generated_invocation(
        "generated-invocation-false-reach",
        "pub machine generated() reaches Console invokes Console; { reach_helper(); }\n",
    )
    .expect_err(
        "generated -> reach_helper -> declared_files exceeds the Console-only reach ceiling",
    );
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("machine `generated`"), "{messages}");
    assert!(
        messages.contains("reaches undeclared service `FilesystemHost`"),
        "{messages}"
    );
}
