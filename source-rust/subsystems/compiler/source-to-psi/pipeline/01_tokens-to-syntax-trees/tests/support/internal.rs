use crate::*;

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
