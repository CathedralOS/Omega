//! Machine and trait-default bodies: `parse_body` reads one body as a
//! statement sequence (`sequence`) of states (`states`), rewriting terminal
//! tail self-calls (`tail_calls`); `trait_default` reads a trait's default
//! machine body. `statements` owns the statement grammar and `transitions`
//! the transition blocks, their guards and targets.

pub(crate) mod parse_body;
pub(crate) mod sequence;
pub(crate) mod statements;
pub(crate) mod states;
pub(crate) mod tail_calls;
pub(crate) mod trait_default;
pub(crate) mod transitions;

/// Copy an expression-table identifier path into the statement arena —
/// the rebuild both statement-table and transition-target copies run,
/// each carrying its own overflow message.
pub(crate) fn copy_identifier_path_to_statement_table(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    overflow_message: &str,
) -> arena::HandleSpan<syntax_trees::identifier::Identifier> {
    let mut start = arena::Handle::invalid();
    let mut count = 0u32;

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count.checked_add(1).expect(overflow_message);
    }

    if count == 0 {
        arena::HandleSpan::empty()
    } else {
        arena::HandleSpan::from_parts(start, count)
    }
}
