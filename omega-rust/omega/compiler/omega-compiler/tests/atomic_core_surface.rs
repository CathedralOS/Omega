//! Exact source-to-compiler agreement for the public atomic outcome family.
//!
//! The Rust schema is descriptive only: importing the ordinary core source
//! must still produce the same generic nominal declarations and payload rows.

use omega_compiler::compile_to_checked;
use psi_language_core::atomic::AtomicCompareExchangeOutcomeIdentity;
use psi_source::SourceOrigin;
use psi_typed_trees::data::{DataMember, TypeParameterKind};
use std::fs;
use std::path::{Path, PathBuf};

struct TemporaryProgram(PathBuf);

impl TemporaryProgram {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("omega-atomic-core-surface-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create atomic core-surface fixture");
        fs::write(
            directory.join("main.omg"),
            r#"use omega::language::core::layout;

data Main {
}

machine Main::main(&mut self) {
}
"#,
        )
        .expect("write atomic core-surface fixture");
        Self(directory)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TemporaryProgram {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn imported_core_atomic_outcomes_match_the_compiler_owned_schema() {
    let fixture = TemporaryProgram::new();
    let checked = compile_to_checked(&fixture.main(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "the normative core atomic outcome surface should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    for identity in AtomicCompareExchangeOutcomeIdentity::ALL {
        let matches = checked
            .typed
            .data_definitions()
            .iter()
            .filter(|definition| definition.name.as_str() == identity.name())
            .collect::<Vec<_>>();
        let [definition] = matches.as_slice() else {
            panic!(
                "core import must load exactly one authored `{}` declaration, found {}",
                identity.name(),
                matches.len()
            );
        };
        assert!(
            definition.is_public,
            "{} must remain public",
            identity.name()
        );
        assert_eq!(
            definition.generic_instance,
            None,
            "{} must be the authored generic nominal, not a synthesized instance",
            identity.name()
        );

        let [parameter] = checked.typed.data_type_parameters(definition) else {
            panic!(
                "{} must have exactly one generic parameter",
                identity.name()
            );
        };
        assert_eq!(parameter.name.as_str(), "T");
        assert_eq!(parameter.kind, TypeParameterKind::Type);

        let source = checked
            .typed
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| checked.typed.symbols.source_file(span))
            .unwrap_or_else(|| panic!("{} must retain authored source custody", identity.name()));
        assert_eq!(source.origin, SourceOrigin::Toolchain);
        assert!(
            source.path.ends_with(Path::new("core/layout.omg")),
            "{} must be owned by the normative core layout source, got {}",
            identity.name(),
            source.path.display()
        );

        let members = checked.typed.data_members(definition);
        assert_eq!(
            members.len(),
            identity.cases().len(),
            "{} case count must match its compiler schema",
            identity.name()
        );
        for (tag, (member, expected)) in members.iter().zip(identity.cases()).enumerate() {
            let DataMember::Variant(variant) = member else {
                panic!("{} must remain a flat sum", identity.name());
            };
            assert_eq!(variant.name.as_str(), expected.case.name());
            assert_eq!(usize::from(expected.tag), tag);

            let payload = checked.typed.data_payload_fields(variant);
            match expected.payload {
                Some(expected_payload) => {
                    let [field] = payload else {
                        panic!(
                            "{}::{} must carry exactly one payload",
                            identity.name(),
                            expected.case.name()
                        );
                    };
                    assert_eq!(field.name.as_str(), expected_payload.field_name());
                    assert_eq!(
                        checked.typed.type_reference_symbol(field.type_reference),
                        parameter.symbol,
                        "{}::{} payload must be the sole T parameter",
                        identity.name(),
                        expected.case.name()
                    );
                }
                None => assert!(
                    payload.is_empty(),
                    "{}::{} must carry no payload",
                    identity.name(),
                    expected.case.name()
                ),
            }
        }
    }
}
