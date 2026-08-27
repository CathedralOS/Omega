use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const FILESYSTEM_HOST_SOURCE: &str = "../../../../../../source/library/std/filesystem_host.omg";

struct Operation {
    name: String,
    variant: String,
    operands: Vec<&'static str>,
    result: &'static str,
}

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies CARGO_MANIFEST_DIR"),
    );
    let source_path = manifest_dir.join(FILESYSTEM_HOST_SOURCE);
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", source_path.display()));
    let operations = parse_operations(&source_path, &source);
    let generated = render_operations(&operations);
    let output_path = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("filesystem_host_operations.rs");
    fs::write(&output_path, generated)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", output_path.display()));
}

fn parse_operations(source_path: &Path, source: &str) -> Vec<Operation> {
    let mut operations = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let Some(signature) = line.trim().strip_prefix("machine ") else {
            continue;
        };
        let context = || format!("{}:{}", source_path.display(), line_index + 1);
        let (signature, reach) = signature
            .split_once(" reaches ")
            .unwrap_or_else(|| panic!("{}: filesystem operation has no reach clause", context()));
        assert_eq!(
            reach,
            "FilesystemHost;",
            "{}: filesystem operation must reach the canonical trait",
            context()
        );
        let open = signature
            .find('(')
            .unwrap_or_else(|| panic!("{}: filesystem operation has no argument list", context()));
        let result_separator = signature
            .rfind(") -> ")
            .unwrap_or_else(|| panic!("{}: filesystem operation has no result type", context()));
        let name = &signature[..open];
        assert!(
            is_snake_identifier(name),
            "{}: noncanonical filesystem operation name `{name}`",
            context()
        );
        let operands = &signature[open + 1..result_separator];
        let operands = if operands.is_empty() {
            Vec::new()
        } else {
            operands
                .split(',')
                .map(|operand| {
                    let (_, authored_type) = operand.split_once(':').unwrap_or_else(|| {
                        panic!("{}: unnamed filesystem operand `{operand}`", context())
                    });
                    operand_kind(authored_type.trim(), &context())
                })
                .collect()
        };
        let authored_result = &signature[result_separator + ") -> ".len()..];
        let result = match authored_result {
            "i32" => "I32",
            "i64" => "I64",
            other => panic!("{}: unsupported filesystem result `{other}`", context()),
        };
        operations.push(Operation {
            name: name.to_owned(),
            variant: rust_variant(name),
            operands,
            result,
        });
    }
    assert!(
        !operations.is_empty(),
        "{} declares no FilesystemHost operations",
        source_path.display()
    );
    assert!(
        operations.len() <= usize::from(u16::MAX),
        "{} declares too many FilesystemHost operations",
        source_path.display()
    );
    for (index, operation) in operations.iter().enumerate() {
        assert!(
            !operations[..index]
                .iter()
                .any(|prior| prior.name == operation.name || prior.variant == operation.variant),
            "{} declares duplicate or Rust-colliding operation `{}`",
            source_path.display(),
            operation.name
        );
    }
    operations
}

fn operand_kind(authored_type: &str, context: &str) -> &'static str {
    match authored_type {
        "&[u8] in Path" => "PathBytes",
        "&[u8]" => "Bytes",
        "i32" => "I32",
        "u32" => "U32",
        "i64" => "I64",
        "u64" => "U64",
        "&mut [u8]" => "MutableBytes",
        "&mut i64" => "MutableI64",
        other => panic!("{context}: unsupported filesystem operand type `{other}`"),
    }
}

fn is_snake_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !name.contains("__")
        && !name.ends_with('_')
}

fn rust_variant(name: &str) -> String {
    if name == "get_osfhandle" {
        return "GetOsfHandle".to_owned();
    }
    let mut variant = String::new();
    for word in name.split('_') {
        let mut characters = word.chars();
        variant.extend(characters.next().into_iter().flat_map(char::to_uppercase));
        variant.extend(characters);
    }
    variant
}

fn render_operations(operations: &[Operation]) -> String {
    let mut generated = String::from(
        "// @generated from source/library/std/filesystem_host.omg by psi-checked-interpreter/build.rs.\n",
    );
    generated.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(u16)]\npub(super) enum FilesystemHostOperation {\n",
    );
    for (index, operation) in operations.iter().enumerate() {
        writeln!(generated, "    {} = {},", operation.variant, index + 1).unwrap();
    }
    generated.push_str("}\n\nimpl FilesystemHostOperation {\n");

    writeln!(
        generated,
        "    #[cfg(test)]\n    pub(super) const ALL: [Self; {}] = [",
        operations.len()
    )
    .unwrap();
    for operation in operations {
        writeln!(generated, "        Self::{},", operation.variant).unwrap();
    }
    generated.push_str("    ];\n\n");

    generated.push_str(
        "    pub(super) fn from_canonical_name(name: &str) -> Option<Self> {\n        Some(match name {\n",
    );
    for operation in operations {
        writeln!(
            generated,
            "            {:?} => Self::{},",
            operation.name, operation.variant
        )
        .unwrap();
    }
    generated.push_str("            _ => return None,\n        })\n    }\n\n");
    generated.push_str(
        "    pub(super) const fn operation_tag(self) -> u16 {\n        self as u16\n    }\n\n",
    );

    generated.push_str(
        "    pub(super) const fn operand_kinds(self) -> &'static [FilesystemHostOperandKind] {\n        use FilesystemHostOperandKind as K;\n        match self {\n",
    );
    for operation in operations {
        let operands = operation
            .operands
            .iter()
            .map(|kind| format!("K::{kind}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            generated,
            "            Self::{} => &[{}],",
            operation.variant, operands
        )
        .unwrap();
    }
    generated.push_str("        }\n    }\n\n");

    generated.push_str(
        "    pub(super) const fn result_kind(self) -> FilesystemHostResultKind {\n        use FilesystemHostResultKind as R;\n        match self {\n",
    );
    for operation in operations {
        writeln!(
            generated,
            "            Self::{} => R::{},",
            operation.variant, operation.result
        )
        .unwrap();
    }
    generated.push_str("        }\n    }\n\n");

    generated.push_str(
        "    pub(super) const fn canonical_name(self) -> &'static str {\n        match self {\n",
    );
    for operation in operations {
        writeln!(
            generated,
            "            Self::{} => {:?},",
            operation.variant, operation.name
        )
        .unwrap();
    }
    generated.push_str("        }\n    }\n}\n");
    generated
}
