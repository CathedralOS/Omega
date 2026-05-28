# Source Files To Tokens

[Pipeline](../pipeline.md) | Previous: none | Next: [Tokens To Syntax Trees](tokens_to_syntax_trees.md)

This stage turns loaded source text into token streams while preserving source identity for diagnostics and later artifacts.

## Stage Contract

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

## Implementation Map

- `lexer.rs` owns token dispatch, source-span slicing, token construction, comments, whitespace, identifiers, keywords, and punctuation.
- `lexer/numbers.rs` owns numeric literal scanning and lexical metadata such as base, suffix presence, and incomplete numeric parts.
- `lexer/strings.rs` owns cooked/raw string scanning, escape validation, and decoded string token text.
- `lex_error.rs` owns lexical diagnostics before later stages know enough to report semantic errors.
- `lexer/tests.rs` owns stage examples and coverage; behavior tests should not grow inside the dispatch file.

## Semantic Ownership

- Places: not known; source spans are text coordinates, not program places.
- Values: not known; literal text may be decoded, but typed values are created later.
- Facts: not known; numeric/string metadata is lexical payload only.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: not known.
- Transitions: not known.
- Effects: not known.
- Boundary edges: not known; `boundary` is only token text here.

## Ownership Rules

- Must preserve byte spans and source text slices faithfully enough for diagnostics and later lowering.
- Must classify tokens only by spelling-level rules.
- Must not own language meaning, import semantics, symbol resolution, type facts, proof facts, borrow facts, effects, or boundary authority.

## Known Gaps

None; this stage should stay intentionally boring.
