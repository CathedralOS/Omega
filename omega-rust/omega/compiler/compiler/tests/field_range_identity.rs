//! Index bounds belong to the selected receiver field, not the first field
//! with the same spelling elsewhere in the source closure.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn check(source: &str) -> Result<(), String> {
    check_files(&[("main.omg", source)])
}

fn check_files(sources: &[(&str, &str)]) -> Result<(), String> {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-field-range-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    let root = fixture.0.join("main.omg");
    for (name, source) in sources {
        fs::write(fixture.0.join(name), source).unwrap();
    }
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root,
            build_dir: Some(fixture.0.join("build")),
            target_name: None,
        })
        .with_requested_product(RequestedCompileProduct::Check),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .map(|_| ())
    .map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[test]
fn dependent_receiver_bounds_use_the_selected_declaration_in_either_import_order() {
    for (selected_limit, foreign_limit) in [(3, 4), (4, 3)] {
        let selected = format!(
            "module selected;
            pub data Holder {{ limit: u64 [0..={selected_limit}]; values: [u8; 4]; }}
            pub machine Holder::read(&self, index: u64 [0..=self.limit]) -> u8 {{
                self.values[index]
            }}"
        );
        let foreign = format!(
            "module foreign;
            pub data Holder {{ limit: u64 [0..={foreign_limit}]; values: [u8; 4]; }}"
        );
        for imports in ["use foreign; use selected;", "use selected; use foreign;"] {
            let root = format!("{imports} pub machine identity(value: u8) -> u8 {{ value }}");
            let result = check_files(&[
                ("main.omg", &root),
                ("selected.omg", &selected),
                ("foreign.omg", &foreign),
            ]);
            if selected_limit == 3 {
                result.expect("the selected declaration bounds every index below four");
            } else {
                let diagnostics = result.expect_err(
                    "an unrelated declaration cannot exclude the selected receiver's index four",
                );
                assert!(diagnostics.contains("cannot prove index"), "{diagnostics}");
            }
        }
    }
}

#[test]
fn unsigned_receiver_index_is_independent_of_foreign_field_order() {
    let selected = "pub data Main { index: u8; values: [u8; 256]; }";
    let unrelated = "pub data Other { index: i32; values: [u8; 1]; }";
    for declarations in [
        format!("{unrelated} {selected}"),
        format!("{selected} {unrelated}"),
    ] {
        check(&format!(
            "{declarations}
            pub machine Main::read(&self) -> u8 {{ self.values[self.index] }}"
        ))
        .expect("the selected u8 field fits its own 256-byte array");
    }
}

#[test]
fn foreign_field_bounds_cannot_prove_an_invalid_receiver_index() {
    for index in ["i32 [-1..=255]", "u16 [0..=256]"] {
        let unrelated = "pub data Other { index: u8 [0..=255]; values: [u8; 256]; }";
        let selected = format!("pub data Main {{ index: {index}; values: [u8; 256]; }}");
        for declarations in [
            format!("{unrelated} {selected}"),
            format!("{selected} {unrelated}"),
        ] {
            let diagnostics = check(&format!(
                "{declarations}
                pub machine Main::read(&self) -> u8 {{ self.values[self.index] }}"
            ))
            .expect_err("another declaration cannot prove this field's index bounds");
            assert!(
                diagnostics.contains("cannot prove index"),
                "{index}: {diagnostics}"
            );
        }
    }
}

#[test]
fn nested_and_borrowed_receiver_indexes_keep_their_declared_field_owner() {
    check(
        r#"
        pub data Other { index: i32; values: [u8; 1]; }
        pub data Record { index: u8; values: [u8; 256]; }
        pub data Main { record: Record; }
        pub machine Main::read(&self) -> u8 { self.record.values[self.record.index] }
        pub machine read(record: &Record) -> u8 { record.values[record.index] }
    "#,
    )
    .expect("nested and borrowed projections resolve through their receiver declaration");
}

#[test]
fn record_literal_collection_uses_its_declared_field_length() {
    check(
        r#"
        pub data Other { values: [u8; 1]; }
        pub data Record { values: [u8; 4]; }
        pub machine read(index: u8 [0..=3]) -> u8 {
            Record { values: [0, 1, 2, 3] }.values[index]
        }
    "#,
    )
    .expect("a literal receiver supplies its own collection declaration");
}
