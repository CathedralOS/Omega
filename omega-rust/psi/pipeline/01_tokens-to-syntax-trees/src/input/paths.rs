use source::{SourceSpan, Span};
use syntax_trees::identifier::Identifier;

pub(crate) fn join_path_identifier(members: &[Identifier]) -> Identifier {
    let mut name = String::new();

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    let first = members
        .first()
        .expect("joined machine path should contain a name")
        .source_span();
    let last = members
        .last()
        .expect("joined machine path should contain a name")
        .source_span();
    debug_assert_eq!(first.source_id, last.source_id);
    Identifier::new(
        name,
        SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end)),
    )
}

#[cfg(test)]
mod tests {
    use super::join_path_identifier;
    use source::{SourceId, SourceSpan, Span};
    use syntax_trees::identifier::Identifier;

    #[test]
    fn joined_machine_path_retains_authored_source_span() {
        let source = SourceId(7);
        let members = [
            Identifier::new("Provider", SourceSpan::new(source, Span::new(11, 19))),
            Identifier::new("first", SourceSpan::new(source, Span::new(21, 26))),
        ];

        let joined = join_path_identifier(&members);

        assert_eq!(joined.as_str(), "Provider::first");
        assert_eq!(
            joined.source_span(),
            SourceSpan::new(source, Span::new(11, 26))
        );
    }
}
