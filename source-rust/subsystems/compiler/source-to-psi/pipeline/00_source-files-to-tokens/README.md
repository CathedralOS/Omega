# Lexical scanning

Start at [src/lib.rs](src/lib.rs). It turns loaded UTF-8 source into ordered token
streams. The [streaming parser](../../driver/parse_source_batch.rs) calls
`lex_file` for each file and recycles token buffers between files.
`lex(LoadedSources)` returns a complete token batch for consumers that retain it.
The [source-files representation](../../representations/source-files/source_files.rs) carries the captured text, origin and scope.

Run the scanner and batch tests with `mbx test --locked -p source-lexer`.
Inside `src/lib.rs`, `ByteScanner::scan_token` dispatches on the first byte.
The scanner writes each token directly into the parser’s reusable stream. Its
offsets refer to the original UTF-8 file.

Follow the two substantial spelling domains under `literals/`:

- [numbers.rs](src/literals/numbers.rs) recognizes radix, fractional, exponent and suffix boundaries.
- [quoted_bytes.rs](src/literals/quoted_bytes.rs) validates quoting and escapes, then decodes bytes.

[Token vocabulary](../../representations/tokens/lexemes/kinds.rs)
owns the shared keyword and punctuation definitions. Ordinary tokens borrow the
source; quoted literals use the token stream's byte buffer. Whitespace and
comments retain exact spans, including LF after a line comment.

Behavior tests live under `tests/`. They check exact source coverage, token
boundaries, accepted and refused spellings, decoded bytes, and batch order.
The source-loading test lives with the source-compilation coordinator. The
[shared lexer fixture](../../../../../../tests/support/lexing.rs)
calls the ordinary batch lexer on retained text.

## Benchmark scanning through its consumer

[benchmarks/lexer_timing.rs](benchmarks/lexer_timing.rs) is a custom benchmark
program. Run it with the repository's host budget, from the checkout root,
passing an absolute source path:

```text
mbx bench -p source-compiler --features test-support --bench lexing_lexer_timing -- lex 100 <absolute-source-path>
mbx bench -p source-compiler --features test-support --bench lexing_lexer_timing -- parse 100 <absolute-source-path>
```

Cargo runs benchmarks from the package directory; absolute paths avoid changing
the meaning of an input path. Additional source paths join the input batch. `lex` measures loaded-source
batch construction and tokenization; `parse` also constructs and drops syntax
trees. File reads and the complete token fingerprint are outside the timed loop.
One persistent invocation worker supplies the compiler's large parsing stack;
worker creation is outside timing.
Each result reports bytes processed, elapsed time, and a checksum containing the
fingerprint and consumed token/root counts.

For paired comparisons, build without running with
`mbx bench -p source-compiler --features test-support --bench lexing_lexer_timing --no-run`. Retain the
executable at the printed path and use [omega-bench](../../../../../../_recycling/tools/benchmark/README.md)'s `reported_work`
plans. Keep input files and executables unchanged during a run. The
[initial comparison](../../../../../../wiki/drafts/measurements/rust_lexer_2026_11_16.md)
records the measured changes and host limitations.
