use omega_lexer::{LineIndex, SourceSpan};
use omega_preparation::{CheckedPreparationEvidence, PreparationAdvanceError, PreparationProgress, PreparationRound, PreparationStep};
use omega_resolver::{ParsedSource, StageSource};

pub fn lower(files: &[(&str, Vec<u8>)]) -> Result<omega_psi::LoweredPackage, String> {
    let diagnostic = |stage: &str, file: &str, span: SourceSpan, message: String| {
        let location = files
            .iter()
            .find(|(path, _)| *path == file)
            .map(|(_, bytes)| {
                let position = LineIndex::new(bytes).get_position(span.get_start_offset());
                format!("{file}:{}:{}", position.get_line(), position.get_column())
            })
            .unwrap_or_else(|| format!("<package>/{file}"));
        format!("{stage}\t{location}\t{message}")
    };
    let mut token_files = Vec::new();
    for (path, bytes) in files {
        token_files.push(omega_lexer::lex(bytes).map_err(|err| diagnostic("L0", path, err.get_span(), err.to_string()))?);
    }
    let mut trees = Vec::new();
    for ((path, bytes), tokens) in files.iter().zip(&token_files) {
        trees.push(omega_parser::parse_file(bytes, tokens).map_err(|err| diagnostic("L1", path, err.get_span(), err.to_string()))?);
    }
    let mut sources = Vec::new();
    for (((path, bytes), tokens), tree) in files.iter().zip(&token_files).zip(&trees) {
        let text = std::str::from_utf8(bytes).map_err(|err| format!("input\t{path}\t{err}"))?;
        sources.push(StageSource {
            package: "root",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        });
    }
    let resolved = omega_toolchain::resolve_source_package(&sources).map_err(|err| diagnostic("L2", &err.file, err.span, err.message))?;
    let typing = |err: omega_typed::TypeError| diagnostic("L3", &err.file, err.span, err.message);
    let mut round = PreparationRound::begin(&resolved, &[], None).map_err(typing)?;
    let prepared = loop {
        let evaluated = round
            .evaluate(CheckedPreparationEvidence::default())
            .map_err(|err| diagnostic("L4", &err.file, err.span, err.message))?;
        match round.advance(evaluated).map_err(|err| match err {
            PreparationAdvanceError::Typing(err) => diagnostic("L3/L4 resume", &err.file, err.span, err.message),
            PreparationAdvanceError::DifferentRound => unreachable!("evaluated the same round"),
        })? {
            PreparationStep::Evaluate(next) => round = *next,
            PreparationStep::Finished(PreparationProgress::Complete(product)) => break product,
            PreparationStep::Finished(PreparationProgress::OpenTarget(open)) => {
                let demand = &open.demands()[0].diagnostic;
                return Err(diagnostic("setup", &demand.file, demand.span, demand.message.clone()));
            }
        }
    };
    omega_lowering::lower_package(&prepared).map_err(|err| diagnostic("L5", &err.site.file, err.site.span, err.message))
}
