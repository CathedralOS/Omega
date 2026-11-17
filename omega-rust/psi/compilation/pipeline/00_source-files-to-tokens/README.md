# Lexical scanning

Start at [lexer.rs](lexer.rs). It turns loaded UTF-8 source into ordered token
streams. Its only public operation is `lex(LoadedSources) -> Result<LexedSources, LexError>`.
The private scanner publishes each token, and `lex_next_token` dispatches its spelling. The cursor, small lexical
error type, comments, identifiers and punctuation scanning stay with that flow.

Follow the two substantial spelling domains under `literals/`:

- [numbers.rs](literals/numbers.rs) recognizes radix, fractional, exponent and suffix boundaries.
- [quoted_bytes.rs](literals/quoted_bytes.rs) validates quoting and escapes, then decodes bytes.

The [source-compilation coordinator](../../source-compiler/src/source_parsing.rs) owns file
loading before this stage receives text. [Token vocabulary](../../representations/tokens/lexemes/kinds.rs)
owns the shared keyword and punctuation definitions. Ordinary tokens borrow the
source; quoted literals use the token stream's byte buffer. Whitespace and
comments retain exact spans, including LF after a line comment.

Behavior tests live under `tests/`. They check exact source coverage, token
boundaries, accepted and refused spellings, decoded bytes, and batch order.
The source-loading test lives with the source-compilation coordinator. Small
fixture adapters in [tests/support/frontend.rs](../../../../../tests/support/frontend.rs)
construct complete stage inputs; they expose no additional production route.

## Benchmark scanning through its consumer

[benchmarks/lexer_timing.rs](benchmarks/lexer_timing.rs) is a custom benchmark
program. Run it with the repository's host budget, from the checkout root,
passing an absolute source path:

```text
mbx bench -p source-files-to-tokens --bench lexer_timing -- lex 100 <absolute-source-path>
mbx bench -p source-files-to-tokens --bench lexer_timing -- parse 100 <absolute-source-path>
```

Cargo runs benchmarks from the package directory; absolute paths avoid changing
the meaning of an input path. Additional source paths join the input batch. `lex` measures loaded-source
batch construction and tokenization; `parse` also constructs and drops syntax
trees. File reads and the complete token fingerprint are outside the timed loop.
Each result reports bytes processed, elapsed time, and a checksum containing the
fingerprint and consumed token/root counts.

For paired comparisons, build without running with
`mbx bench -p source-files-to-tokens --bench lexer_timing --no-run`. Retain the
executable at the printed path and use [omega-bench](../../../../../tools/benchmark/README.md)'s `reported_work`
plans. Keep input files and executables unchanged during a run. The
[initial comparison](../../../../../wiki/drafts/measurements/rust_lexer_2026_11_16.md)
records the measured changes and host limitations.
