# Rust lexical scanning

This stage turns valid UTF-8 source into tokens with exact byte spans. Ordinary
spellings borrow the source; quoted literals decode into the token stream's byte
buffer. Whitespace and comments retain their spans, including LF after a line
comment. The parser decides which tokens affect its grammar.

Start at `src/lexer.rs`: `fill_tokens` publishes each token and `lex_next_token`
dispatches its spelling. Follow the reader's question from there:

| Question | Owner |
| --- | --- |
| How are files read? | `src/load_sources.rs` |
| How are independent sources scheduled and errors ordered? | `src/lex_sources.rs` |
| Which token starts here, and how do comments nest? | `src/lexer.rs` |
| How do scans keep absolute byte positions while skipping comment text? | `src/source_cursor.rs` |
| Where do digits, fractions, exponents and suffixes end? | `src/numeric_literals.rs` |
| Which escapes are legal, and what bytes do they produce? | `src/quoted_literals.rs` |
| What does a lexical failure retain? | `src/lex_error.rs` |
| Which keyword or punctuation spelling matches? | [Token vocabulary](../../representations/tokens/src/token_kind.rs), with `keywords.rs` and `punctuation.rs` beside it |

Tests live under `tests/`, separate from production code. They cover exact source
coverage, token boundaries, accepted and refused spellings, decoded bytes, and
source ownership. The newline regression reconstructs the source from token
spans; a token-kind-only assertion would miss the dropped byte.

## Measure scanning through its real consumer

Build the timing example with the repository's host budget:

```text
mbx build --release -p source-files-to-tokens --example lexer_timing
target/release/examples/lexer_timing lex 100 source/library/core/math.omg
target/release/examples/lexer_timing parse 100 source/library/core/math.omg
```

On Windows, append `.exe`. Additional source paths join the same input batch.
`lex` measures source-retaining tokenization and buffer recycling; `parse` also
constructs and drops syntax trees. File reads and the complete token fingerprint
are outside the timed loop. Each result reports bytes processed, elapsed time,
and a checksum that includes the fingerprint and consumed token/root counts.

Use [omega-bench](../../../../../tools/benchmark/README.md)'s `reported_work`
plans to compare retained baseline and candidate executables with alternating
pairs, a fixed checksum, and resource capture. Keep source inputs and executables
unchanged throughout a run. The [initial comparison](../../../../../wiki/drafts/measurements/rust_lexer_2026_11_16.md)
records isolated changes, their combined result, and the host limitations.
