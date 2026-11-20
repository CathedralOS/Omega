use omega_lowering::{LoweredPackage, LoweringError, lower_package};
use omega_preparation::prepare_source_package;
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
    let lowered = lower_package(&prepared)?;
    for machine in &lowered.machines {
        control_flow::check_operation_ownership(&machine.body).expect("lowered operation ownership");
    }
    Ok(lowered)
}
