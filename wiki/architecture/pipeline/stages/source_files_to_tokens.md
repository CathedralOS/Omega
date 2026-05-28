# Source Files To Tokens

[Pipeline](../pipeline.md) | Previous: none | Next: [Tokens To Syntax Trees](tokens_to_syntax_trees.md)

This stage turns loaded source text into token streams while preserving source identity for diagnostics and later artifacts.

## Stage Contract

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

## Semantic Ownership

- Places: not known.
- Values: not known.
- Facts: not known.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: not known.
- Transitions: not known.
- Effects: not known.
- Boundary edges: not known, except `boundary` as token text.

## Ownership Rules

Must not own: language meaning, import semantics, symbol resolution.

## Known Gaps

None; this stage should stay intentionally boring.
