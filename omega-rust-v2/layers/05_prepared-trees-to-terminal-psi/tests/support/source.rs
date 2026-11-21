use omega_lowering::{LoweredPackage, LoweringError, lower_package};
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, resolve_source_package};
pub mod control_flow;

pub fn resolve<'text>(files: &[(&'text str, &'text str)]) -> SourcePackage<'text> {
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
            package: "app",
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    resolve_source_package(&sources).unwrap()
}

pub fn lower(files: &[(&str, &str)]) -> Result<LoweredPackage, LoweringError> {
    let package = resolve(files);
    let prepared = prepare_source_package(&package).expect("the source reaches L5 after preparation");
    lower_prepared(&prepared, files)
}

pub fn lower_prepared(
    prepared: &PreparedTrees,
    files: &[(&str, &str)],
) -> Result<LoweredPackage, LoweringError> {
    let product = lower_package(prepared);
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
            Ok(_) => "produced Psi".to_owned(),
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
                format!(
                    "refused: {message} at {}\t{}\t{}\t{start}\t{end}\t{}{suffix}",
                    site.file,
                    site.package,
                    site.file,
                    location.unwrap_or_else(|| "\t\t".into())
                )
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

#[cfg(feature = "diagnostic-origins")]
fn origin_suffix(origin: &std::panic::Location<'_>) -> String {
    const PREFIX: &str = "layers/05_prepared-trees-to-terminal-psi/";
    let path = origin.file().replace('\\', "/");
    let relative = path.strip_prefix(PREFIX)
        .or_else(|| path.rsplit_once(&format!("/{PREFIX}")).map(|(_, tail)| tail))
        .unwrap_or(&path);
    assert!(relative.starts_with("src/") && relative.ends_with(".rs"), "non-L5 origin {path}");
    format!("\t{PREFIX}{relative}\t{}\t{}", origin.line(), origin.column())
}
