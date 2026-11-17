# Rust lexer measurements, 2026-11-16

Direct punctuation dispatch, keyword string matching, and bulk comment scanning
reduced lexing time by 71.8% and lexing plus parsing time by 5.7% on six selected
source files on one Windows x86-64 host. These are phase measurements, not a
claim about whole-compiler throughput or other hosts. Token storage is unchanged.

The baseline already repairs line-comment LF retention and separates lexical
responsibilities and tests. Every compared variant produces identical token
kinds, byte spans and decoded literal bytes. All six authored inputs also parse.

## Isolated changes

Each cell is the median paired change in elapsed time, followed by its 95%
paired-bootstrap interval. Negative means less time. Intervals spanning zero do
not establish a difference. These first comparisons use one warmup and eight
alternating AB/BA pairs.

| Change from corrected baseline | Six-file lexing | Six-file lexing plus parsing | Matching stress input |
| --- | --- | --- | --- |
| punctuation | -48.0% [-50.5, -42.5] | -4.1% [-8.5, -1.2] | -74.9% [-76.3, -64.5] |
| keywords | -21.0% [-27.4, -14.1] | +1.2% [-1.0, +7.1] | -53.4% [-54.1, -52.5] |
| comments | -10.2% [-27.1, +5.3] | +2.9% [+0.7, +4.1] | -97.9% [-98.0, -97.8] |

The punctuation change dispatches on the first byte, using the same vocabulary
that supplies diagnostic spellings. Keyword matching is generated from one
spelling list. The comment change replaces the peekable character-index cursor
with a character cursor that can skip byte runs, using `memchr` for LF and
`memchr2` for slash/asterisk candidates while preserving nested comments. Its
measurement includes that required cursor change. No custom SIMD kernel was added.

Keyword-only changes also affected some punctuation timings. These compare
compiled lexer variants, so inlining and code layout effects are included;
they do not measure the isolated latency of one lookup function.

## Combined result and comment tradeoff

Longer follow-up runs used the final combined source. The baseline comparison
used 16 pairs for lexing and 24 for lexing plus parsing. Both reused the same
frozen inputs and correctness oracle.

| Comparison | Paired time change, 95% interval |
| --- | --- |
| Baseline to combined: lexing | -71.8% [-74.2, -67.2] |
| Baseline to combined: lexing plus parsing | -5.7% [-6.8, -4.8] |
| Add bulk comments after other changes: lexing | -3.5% [-15.3, +0.6] |
| Add bulk comments after other changes: lexing plus parsing | +1.8% [-3.0, +4.5] |
| Add bulk comments: long comments | -98.5% [-99.1, -97.7] |
| Add bulk comments: identifier control | -4.7% [-5.6, -2.7] |
| Add bulk comments: quoted-byte control | -26.3% [-27.0, -24.8] |

Keep all three. Punctuation and keyword changes clearly help ordinary source
lexing. Bulk scanning has a large benefit on long comments and the comment-rich
small input; its incremental effect on the six-file batch remains inconclusive.
The isolated comment variant initially increased lexing plus parsing time by
2.9%; adding it after the other changes did not establish a regression. The
combined version improves that boundary overall. The identifier and string
controls also found no incremental regression in these follow-up runs.

Process peak resident memory was captured by the same runner. It includes input
loading, token fingerprinting, and runtime overhead, rather than isolating token
storage. No token-memory optimization is claimed.

## Inputs and reproducibility

The six authored files total 151,835 bytes. Each was copied byte-for-byte from
base `bad196d5fa5433e17f157cca80a521fe17456565` before timing:

- `samples/cli/basics/print_number/main.omg`
- `samples/cli/basics/standalone/main.omg`
- `source/library/core/math.omg`
- `source/library/core/compact_binary.omg`
- `source/library/core/float_operations.omg`
- `source/library/core/numeric_conversion.omg`

Separate token-only stress inputs repeat punctuation families, identifiers and
keywords, long UTF-8 comments with nested block delimiters, and quoted bytes with
escapes. They are not runnable Omega programs. The small-file control is the
existing `standalone/main.omg`, whose explanatory comments dominate its text.
Stress results are cost attribution, not substitutes for the authored-source run.

Host: Intel Core i9-12900K, 16 physical / 24 logical CPUs, Windows build 26200,
`x86_64-pc-windows-msvc`. Toolchain: `rustc 1.100.0-nightly (a69a63265 2026-09-03)`,
LLVM 23.1.1, repository `nightly-2026-09-04`, mbx 1.29.2, Cargo release profile
with the checkout's `.cargo/config.toml`. Builds used Idle priority and the shared
18-CPU budget. Timed runs used Idle priority pinned to logical CPU 8 (`0x100`);
this session ran no builds during measurement. The workstation was shared, so
paired intervals and observed variability matter. ARM64 and other operating
systems remain unmeasured.

The kept [timing example](../../../omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/examples/lexer_timing.rs)
reads inputs and fingerprints every token before timing. The timed region includes
source-retaining tokenization, output consumption and scratch recycling; `parse`
also creates, consumes and drops syntax trees. File reads, token fingerprinting
and process startup are excluded from the reported phase time. The runner also
captures process-wide resource observations. The checksum binds the complete
untimed token observation and timed token/root counts.

Build and run commands:

```text
mbx build --release -p source-files-to-tokens --example lexer_timing
mbx build --release -p omega-bench --bin omega-bench
build/lexer-review/baseline.exe lex 148 <six frozen paths>
target/release/omega-bench.exe compare build/lexer-review/plans/authored-lex-punctuation.json build/lexer-review/results/authored-lex-punctuation
```

Each retained plan specifies the exact paths, repetition count, byte count and
checksum. The experiment directory `build/lexer-review/` retains input copies,
source variant snapshots, executables, plans, journals, resource observations,
bootstrap summaries and the sequential measurement scripts. The baseline is the
base above plus the newline correction, organization changes and common timing
example; every isolated optimization starts from that corrected baseline.
The final binary differs from the initial combined binary only by test-only
cleanup and debug assertions, which are disabled in these release measurements.

| Executable | SHA-256 |
| --- | --- |
| `baseline.exe` | `5fe5f2b5b54a4f3b473b95290b7475bf702d31d20078d850064f39e384b107dd` |
| `punctuation.exe` | `6181adb901fd93b05cd9b319b239711040236cec46792b7e900ef49c61888e85` |
| `keywords.exe` | `f902d36933d28863ffc76c0fb91c306c7de02615aa03fc0119a6c14a36486bc6` |
| `comments.exe` | `a03156d4c7ccf1dfcd61ec5bfda55021a4bc5c138ca1ccb00a1728b7a87c66c9` |
| `combined.exe` | `d34cd27d9d4646ce41dcd202e10a7b8327017d7d5d6e255e3c07e640fd1d71f4` |
| `no-comments.exe` | `f3c4e3219db96e945d3a49cbc09bfaa48e5856a4d3241f9a7d91c0c7e3e2bae0` |
| `final.exe` | `5801b60cd05533dfd30727a00f5e86fb6045e7c261f84b9bd6588eb7ff8b2f15` |
