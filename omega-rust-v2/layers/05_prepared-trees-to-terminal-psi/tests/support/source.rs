use omega_lowering::{LoweredPackage, LoweringError, lower_package};
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, resolve_source_package};
pub mod control_flow;

pub fn resolve<'text>(files: &[(&'text str, &'text str)]) -> SourcePackage<'text> {
    resolve_named("app", files).unwrap()
}

fn resolve_named<'text>(
    package: &'text str,
    files: &[(&'text str, &'text str)],
) -> Result<SourcePackage<'text>, omega_resolver::NameError> {
    let syntax: Vec<_> = files
        .iter()
        .map(|(_, source)| {
            let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&syntax)
        .map(|(&(logical_path, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    resolve_source_package(&sources)
}

// Package identities can appear inside rendered types. Obtain the reference diagnostic
// under the package command's identity, and require it to come from the same decision.
fn package_command_reference(
    files: &[(&str, &str)],
    original: &LoweringError,
    owner: Option<[u8; 32]>,
) -> Result<String, String> {
    let mut package = resolve_named("root", files).map_err(|error| format!("resolution: {}", error.message))?;
    if let Some(owner) = owner {
        package.bind_package_key("root", owner)?;
    }
    let mut prepared = prepare_source_package(&package).map_err(|error| format!("preparation: {}", error.message))?;
    if owner.is_some() {
        omega::check_pending_policy_constructions(&mut prepared).map_err(|error| error.to_string())?;
    }
    let error = lower_package(&prepared).err().ok_or("lowering produced Psi")?;
    let expected_owner = if original.site.package == "app" {
        "root"
    } else {
        original.site.package.as_str()
    };
    if error.site.package != expected_owner || error.site.file != original.site.file || error.site.span != original.site.span {
        return Err(format!("different diagnostic site: {:?}", error.site));
    }
    #[cfg(feature = "diagnostic-origins")]
    if error.diagnostic_origin() != original.diagnostic_origin() {
        return Err(format!("different lowering decision: {}", error.diagnostic_origin()));
    }
    Ok(error.message)
}

#[allow(dead_code)] // Handoff tests that enter with a prepared product do not lower source.
pub fn lower(files: &[(&str, &str)]) -> Result<LoweredPackage, LoweringError> {
    let package = resolve(files);
    let prepared = prepare_source_package(&package).expect("the source reaches L5 after preparation");
    lower_and_report(&prepared, files, "source", None)
}

#[allow(dead_code)] // Only handoff tests enter with a prepared product.
pub fn lower_prepared(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
) -> Result<LoweredPackage, LoweringError> {
    lower_and_report(prepared, files, "prepared", None)
}

struct ExpectedRefusal<'a> {
    file: &'a str,
    span: std::ops::Range<usize>,
    message: &'a str,
    cli_message: &'a str,
    package_owner: Option<[u8; 32]>,
}

/// Locate the authored occurrence before consulting either compiler return.
#[allow(dead_code)]
pub fn unique_span(
    source: &str,
    text: &str,
) -> std::ops::Range<usize> {
    assert!(!text.is_empty(), "an empty expected occurrence needs an explicit byte range");
    let start = source
        .find(text)
        .unwrap_or_else(|| panic!("expected occurrence absent: {text:?}"));
    let next = start + text.chars().next().unwrap().len_utf8();
    assert!(!source[next..].contains(text), "ambiguous expected occurrence: {text:?}");
    start..start + text.len()
}

#[allow(dead_code)]
pub fn span_within(
    source: &str,
    context: &str,
    text: &str,
) -> std::ops::Range<usize> {
    let outer = unique_span(source, context);
    let inner = unique_span(context, text);
    outer.start + inner.start..outer.start + inner.end
}

#[allow(dead_code)]
pub fn refuse(
    files: &[(&str, &str)],
    file: &str,
    span: std::ops::Range<usize>,
    message: &str,
) -> LoweringError {
    refuse_with_cli_message(files, file, span, message, message)
}

#[allow(dead_code)]
pub fn refuse_with_cli_message(
    files: &[(&str, &str)],
    file: &str,
    span: std::ops::Range<usize>,
    message: &str,
    cli_message: &str,
) -> LoweringError {
    let package = resolve(files);
    let prepared = prepare_source_package(&package).expect("the source reaches L5 after preparation");
    let expected = ExpectedRefusal {
        file,
        span,
        message,
        cli_message,
        package_owner: None,
    };
    lower_and_report(&prepared, files, "source", Some(&expected)).expect_err("expected L5 to refuse")
}

#[allow(dead_code)]
pub fn refuse_prepared(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
    file: &str,
    span: std::ops::Range<usize>,
    message: &str,
) -> LoweringError {
    refuse_prepared_with_cli_message(prepared, files, file, span, message, message)
}

#[allow(dead_code)]
pub fn refuse_prepared_with_cli_message(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
    file: &str,
    span: std::ops::Range<usize>,
    message: &str,
    cli_message: &str,
) -> LoweringError {
    let expected = ExpectedRefusal {
        file,
        span,
        message,
        cli_message,
        package_owner: None,
    };
    lower_and_report(prepared, files, "prepared", Some(&expected)).expect_err("expected L5 to refuse")
}

/// Exercise the same default Build capture and owner binding as source lower --package-root.
#[allow(dead_code)]
pub fn lower_built_fixture(
    directory: &str,
    file: &str,
    source: &str,
) -> (Vec<omega_typed::TypingObligation>, LoweredPackage) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(directory);
    let build = omega_toolchain::DefaultBuild::capture(&root).expect("capture the fixture package");
    assert_eq!(build.source_bytes(std::path::Path::new(file)).unwrap().as_ref(), source.as_bytes());
    let files = [(file, source)];
    let mut resolved = resolve(&files);
    resolved.bind_package_key("app", build.package_key()).unwrap();
    let mut prepared = prepare_source_package(&resolved).expect("the captured package prepares");
    let product = omega::lower_prepared_package(&mut prepared);
    let lowered = report_product(&prepared, &files, "package", None, product).expect("the captured package lowers");
    (prepared.typed().obligations.clone(), lowered)
}

fn lower_and_report(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
    entry: &str,
    expected: Option<&ExpectedRefusal<'_>>,
) -> Result<LoweredPackage, LoweringError> {
    report_product(prepared, files, entry, expected, lower_package(prepared))
}

fn report_product(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
    entry: &str,
    expected: Option<&ExpectedRefusal<'_>>,
    product: Result<LoweredPackage, LoweringError>,
) -> Result<LoweredPackage, LoweringError> {
    if let Some(expected) = expected {
        let error = product.as_ref().expect_err("expected L5 to refuse");
        let text = files
            .iter()
            .find(|(file, _)| *file == expected.file)
            .expect("the expected refusal names an input file")
            .1;
        assert!(text.get(expected.span.clone()).is_some(), "expected byte span must select UTF-8 source");
        let actual_span = error.site.span.get_start_offset()..error.site.span.get_end_offset();
        assert_eq!(
            (error.site.package.as_str(), error.site.file.as_str(), actual_span, error.message.as_str()),
            ("app", expected.file, expected.span.clone(), expected.message)
        );
    }
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        let thread = std::thread::current();
        let test = thread.name().unwrap_or("<unnamed>");
        let counts = tree_handoff_consumer::obligation_report::counts(prepared)
            .into_iter()
            .map(|(kind, n)| format!("{kind}={n}"))
            .collect::<Vec<_>>()
            .join(",");
        let names = files
            .iter()
            .map(|(path, _)| *path)
            .collect::<Vec<_>>()
            .join(",");
        let detail = match &product {
            Ok(_) => {
                let typed = prepared.typed();
                let files: std::collections::BTreeSet<_> = typed
                    .resolved()
                    .trees()
                    .map(|tree| (tree.file.package, tree.file.path))
                    .collect();
                let local: std::collections::BTreeSet<_> = typed
                    .obligations
                    .iter()
                    .filter(|row| files.contains(&(row.site.package.as_str(), row.site.file.as_str())))
                    .map(|row| tree_handoff_consumer::obligation_report::name(&row.kind))
                    .collect();
                let kinds = local.into_iter().collect::<Vec<_>>().join(",");
                let frame = format!("produced Psi\tlocal-kinds-v1\t{kinds}");
                let checksum = receipt_checksum(&format!("{test}\t{names}\t{counts}\t{frame}"));
                format!("{frame}\t{checksum:08x}")
            }
            Err(error) => {
                #[cfg(feature = "diagnostic-origins")]
                let suffix = origin_suffix(error.diagnostic_origin());
                #[cfg(not(feature = "diagnostic-origins"))]
                let suffix = "";
                let site = &error.site;
                let start = site.span.get_start_offset();
                let end = site.span.get_end_offset();
                let message = error.message.replace(['\r', '\n', '\t'], " ");
                let source = files
                    .iter()
                    .find(|(file, _)| site.package == "app" && *file == site.file);
                let location = source.and_then(|(_, text)| {
                    let selected = text.get(start..end)?;
                    let pos = omega_lexer::LineIndex::new(text.as_bytes()).get_position(start);
                    Some(format!("{}\t{}\t{}", pos.get_line(), pos.get_column(), selected.escape_debug()))
                });
                let field = spec_corpus::diagnostic_field;
                let selected = source
                    .and_then(|(_, text)| text.get(start..end))
                    .unwrap_or_default();
                let (reference, root_message) = match package_command_reference(files, error, expected.and_then(|e| e.package_owner)) {
                    Ok(message) => ("root", message),
                    Err(detail) => ("unreproduced", detail),
                };
                let mut inputs = vec![
                    field(&error.message),
                    field(selected),
                    reference.into(),
                    field(&root_message),
                ];
                for &(path, source) in files {
                    inputs.push(field(path));
                    inputs.push(field(source));
                }
                let entry = if let Some(expected) = expected {
                    inputs.extend([
                        field(expected.file),
                        expected.span.start.to_string(),
                        expected.span.end.to_string(),
                        field(expected.message),
                        field(expected.cli_message),
                    ]);
                    format!("{entry}+expected-v1")
                } else {
                    entry.to_owned()
                };
                // One frame per actual call, including repeated obligations at the same site.
                // Imported rows retain their owning package; their source need not be local.
                for obligation in &prepared.typed().obligations {
                    let at = &obligation.site;
                    inputs.extend([
                        tree_handoff_consumer::obligation_report::name(&obligation.kind).into(),
                        field(&at.package),
                        field(&at.file),
                        at.span.get_start_offset().to_string(),
                        at.span.get_end_offset().to_string(),
                    ]);
                }
                let position = location.unwrap_or_else(|| "\t\t".into());
                let position = format!("{}\t{}\t{start}\t{end}\t{position}", site.package, site.file);
                let frame = format!("{entry}+sites-v1\t{}", inputs.join("\t"));
                let checksum = receipt_checksum(&format!("{test}\t{position}\t{frame}"));
                format!("refused: {message} at {}\t{position}{suffix}\treplay-v1\t{frame}\t{checksum:08x}", site.file)
            }
        };
        println!("OMEGA_FIXTURE\t{test}\t{names}\t{counts}\t{detail}");
    }
    let lowered = product?;
    for machine in &lowered.machines {
        control_flow::check_operation_ownership(&machine.body).expect("lowered operation ownership");
    }
    Ok(lowered)
}

// Adler-32 detects accidental edits (including plausible in-range span substitutions).
// It is a receipt integrity check, not authentication.
fn receipt_checksum(record: &str) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in record.bytes() {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(feature = "diagnostic-origins")]
fn origin_suffix(origin: &std::panic::Location<'_>) -> String {
    const PREFIX: &str = "layers/05_prepared-trees-to-terminal-psi/";
    let path = origin.file().replace('\\', "/");
    let relative = path
        .strip_prefix(PREFIX)
        .or_else(|| path.rsplit_once(&format!("/{PREFIX}")).map(|(_, tail)| tail))
        .unwrap_or(&path);
    assert!(relative.starts_with("src/") && relative.ends_with(".rs"), "non-L5 origin {path}");
    format!("\t{PREFIX}{relative}\t{}\t{}", origin.line(), origin.column())
}
