# Source Files To Tokens

[Pipeline](../pipeline.md) | Previous: none | Next: [Tokens To Syntax Trees](tokens_to_syntax_trees.md)

This stage turns loaded source text into token streams while preserving source identity for diagnostics and later artifacts.

## Stage Contract

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

## Implementation Map

The Psi product role owns this stage; its eventual hosted source belongs under
`source/compiler/omega/psi/`. The current Rust realization is:

- `source/compiler/rust/psi/foundation/psi-source` contains loaded-source records and maps,
  source identities, byte-span coordinates, and source-backed text.
- `source/compiler/rust/psi/foundation/psi-diagnostics` contains source-addressable
  diagnostics, and `source/compiler/rust/psi/foundation/psi-source-loader` implements root-file
  loading.
- `source/compiler/rust/psi/representations/psi-tokens` contains token kinds, text, and
  streams.
- `source/compiler/rust/psi/pipeline/psi-source-files-to-tokens` contains the implementation
  files below. Every workspace harness uses this Psi stage directly.
- `lexer.rs` owns token dispatch, source-span slicing, token construction, comments, whitespace, identifiers, keywords, and punctuation.
- `lexer/numbers.rs` owns numeric literal scanning and lexical metadata such as base, suffix presence, and incomplete numeric parts.
- `lexer/strings.rs` owns cooked/raw string scanning and escape validation while advancing the lexer cursor.
- `lexer/strings/decode.rs` owns decoded string token text, including cooked escapes and raw string body extraction.
- `lex_error.rs` owns lexical diagnostics before later stages know enough to report semantic errors.
- `lexer/tests.rs` owns stage examples and coverage; behavior tests should not grow inside the dispatch file.

## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Not known; source spans are text coordinates, not program places. |
| Values | Not known; literal text may be decoded, but typed values are created later. |
| Facts | Not known; numeric/string metadata is lexical payload only. |
| Loans | Not known. |
| Moves | Not known. |
| Drops | Not known. |
| Calls | Not known. |
| Transitions | Not known. |
| Reach | Not known. |
| Boundary edges | Not known; `boundary` is only token text here. |

## Ownership Rules

- Must preserve byte spans and source text slices faithfully enough for diagnostics and later lowering.
- Must classify tokens only by spelling-level rules.
- Must not own language meaning, import semantics, symbol resolution, type facts, proof facts, borrow facts, reach, or boundary authority.
