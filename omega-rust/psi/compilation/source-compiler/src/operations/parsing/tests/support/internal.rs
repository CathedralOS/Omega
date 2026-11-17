use crate::parsing::*;
use diagnostics::Diagnostic;
use syntax_trees::ParsedSources;

pub(crate) fn parse_syntax_trees(tokens: &TokenStream<'_>) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub(crate) fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse_into(&mut syntax_trees, source_id, tokens)?;
    Ok(syntax_trees)
}

pub(crate) fn parse_sources(
    lexed: LexedSources,
    syntax_trees: &mut SyntaxTrees,
) -> Result<ParsedSources, Vec<Diagnostic>> {
    let options = ParseOptions {
        first_page_identity: u32::try_from(syntax_trees.page_count()).unwrap() + 1,
        ..Default::default()
    };
    let (pages, sources) = crate::parsing::parse(&lexed, options)
        .map_err(|error| vec![error.into_diagnostic()])?
        .into_parts();
    for page in pages {
        syntax_trees.adopt_page(page);
    }
    Ok(sources)
}

pub(crate) fn parse_source_units(
    lexed: LexedSources,
) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    crate::parsing::parse(
        &lexed,
        ParseOptions {
            layout: ParseLayout::IndependentUnits,
            ..Default::default()
        },
    )
    .map_err(|error| vec![error.into_diagnostic()])
}
