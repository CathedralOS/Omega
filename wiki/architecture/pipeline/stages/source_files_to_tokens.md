# Source Files To Tokens

[Pipeline](../pipeline.md) | Previous: none | Next: [Tokens To Syntax Trees](tokens_to_syntax_trees.md)

This stage turns loaded source text into token streams while preserving source identity for diagnostics and later artifacts.

## Stage Contract

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

## Implementation Map

The Psi product role owns this stage; its hosted source belongs under
`source/psi/`. The current Rust realization is:

- `omega-rust/psi/foundation/source` contains loaded-source records and maps,
  source identities, byte-span coordinates, and source-backed text.
- `omega-rust/psi/foundation/diagnostics` contains source-addressable
  diagnostics. Source-closure discovery and loading remain at the
  package/compiler boundary; this lexical stage consumes the resulting loaded
  source records.
- `omega-rust/psi/representations/tokens` contains token kinds, text, and
  streams.
- `omega-rust/psi/pipeline/source-files-to-tokens` contains the implementation
  files below. Every workspace harness uses this Psi stage directly.
- `lexer.rs` owns token dispatch, source-span slicing, token construction,
  comments, the closed space/tab/CR/LF whitespace set, ASCII identifiers,
  keywords, and punctuation. Host Unicode classification is not language
  authority.
- `lexer/numbers.rs` owns numeric literal scanning and lexical metadata such as base, suffix presence, and incomplete numeric parts.
- `lexer/strings.rs` owns quoted byte-literal scanning and byte-level escape
  validation while advancing the lexer cursor. Raw-string and codepoint-escape
  spellings reject through the closed lexical-profile diagnostic.
- `lexer/strings/decode.rs` owns decoded string token bytes. Decoding copies
  literal-body source bytes and expands only the fixed byte escapes; it does not
  synthesize an encoding from Unicode scalar values.
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

## Lexical Profile Replay

Both maintained lexers implement Chapter 1's closed V1 profile. The generated
Unicode identifier table and direct comparator dependency are absent. A
versioned, semantic-free lexical observation preserves source bytes, decoded
literal bytes, token coordinates, and the unified profile diagnostic. The
product gate compares that observation byte-for-byte with the independently
maintained Rust observer for accepted and rejected profile cases; this is
differential evidence, not language authority.
