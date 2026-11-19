//! An unused choice must still name inert storage. An inactive case may own
//! storage, so validate it even when the program never constructs the carrier.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

#[test]
fn build_checks_each_carrier_and_only_emits_it_for_a_value_demand() {
    let package = fixture_package_inputs::repo_root()
        .join("build")
        .join(format!("opaque-choice-{}", std::process::id()));
    std::fs::create_dir_all(&package).unwrap();
    let main = package.join("main.omg");
    for (field, valid, use_by_value) in [
        ("u64", true, false),
        ("@u64", false, false),
        ("u64", true, true),
    ] {
        let binding = if use_by_value {
            ""
        } else {
            "Build::bind(builder, linux_x86_64::ProgramEntry, start);"
        };
        std::fs::write(
            package.join("build.omg"),
            format!(
                r#"
machine build(builder: @mut Build) {{
    Build::application(builder, "opaque_choice");
    Build::select_representation<Token, TokenRepresentation>(builder);
    {binding}
}}"#
            ),
        )
        .unwrap();
        let relay = if use_by_value {
            "public machine relay(token: Token) -> Token { token }"
        } else {
            ""
        };
        std::fs::write(
            &main,
            format!(
                r#"
use omega::language::core::OpaqueRepresentation;
public boundary data Token [linear];
public data Storage {{ case Empty; case Full(value: {field}); }}
public TokenRepresentation satisfies OpaqueRepresentation<Token, Storage> {{}}
machine start() {{}}
{relay}
"#
            ),
        )
        .unwrap();
        let result = std::thread::scope(|scope| {
            compiler::checked::spawn_compile_worker(scope, || {
                let mut package =
                    fixture_package_inputs::reviewed_repository_fixture_package_inputs(
                        &main,
                        Some("linux_x86_64"),
                    )?;
                let product = fixture_package_inputs::check_fixture(
                    &mut package,
                    "linux_x86_64",
                    Default::default(),
                )?;
                let choices = &product.build().config.opaque_representation_selections;
                assert_eq!(choices.len(), 1);
                assert_ne!(choices[0].selected_application_commitment(), [0; 32]);
                let module = product.terminal_publication().accepted().unwrap().module();
                assert_eq!(
                    module.opaque_representations.len(),
                    usize::from(use_by_value)
                );
                if let Some(row) = module.opaque_representations.first() {
                    assert_eq!(
                        row.application.selected_application_commitment,
                        choices[0].selected_application_commitment()
                    );
                    assert!(
                        module
                            .structural_type(row.opaque)
                            .unwrap()
                            .owns_linear_claim
                    );
                }
                Ok::<(), Vec<diagnostics::Diagnostic>>(())
            })
            .unwrap()
            .join()
            .unwrap()
        });
        if valid {
            result.expect("a plain field is inert storage");
        } else {
            let reasons = result.expect_err("a borrowed field is not inert storage");
            assert!(
                reasons
                    .iter()
                    .any(|reason| reason.message.contains("fixed, inert storage")),
                "{reasons:?}"
            );
        }
    }
    std::fs::remove_dir_all(package).unwrap();
}
