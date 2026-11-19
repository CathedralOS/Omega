# L1 Parser: speed

How fast the parser is, where its time goes, and every attempt to make it
faster, kept or not.

## How it is measured

    cargo run --release -p omega-parser --example parse_throughput -- --files

To compare two versions, do not time them by hand. From the repository root:

    cargo run --release -p compare-builds -- --package omega-parser --tool parse_throughput -- --files

That builds the last commit and the working tree three ways each, runs them
interleaved, and ends with one sentence on `parse_min_ms`. It is the answer
to the warning under "Noise" below; the lexer's experiments 8 to 12 are why
it exists. `--passes N` sets how many times the tool repeats the measurement
(it was `--rounds` before the tool moved onto the shared `measure` crate).

**Workload.** The harness reads the current accept fixtures on each run. The
measurements below predate the `@` syntax and used 22 fixtures laid end to end,
12,747 bytes, lexed and parsed 5,265 times: 64 MB in all. That source is denser than real code, 2.3 bytes
per token against about 5 in V1's source, so the number to carry forward is
time per token; MB/s on this workload understates the parser.

**Two shapes of the same bytes.** In those runs, `--files` parsed 5,265 small files,
which is what a compiler does. Without it they are one 64 MB file. The two
behave differently: a 64 MB file has 870 MB of tokens, so each pass over them
waits on memory, while a 12 KB file's tokens stay in the cache. Experiments
are judged on `--files`; the single file is a check that nothing depends on
the input being small.

**Same output.** The harness prints a fingerprint of the tree, a hash of
every event and token index. A change is only compared if its fingerprint
matches the baseline's, so every experiment below built the same tree.

**The floor.** The harness also times a loop that does the least any parser
over these tokens could: skip whitespace and comments, and write one event
per token that is left. The gap between that and the parse time is what the
grammar costs.

**Noise.** Each figure is the best of nine rounds. Run to run, the same
binary's parse time on `--files` varies by 0.3 ms in 122, a quarter of a
percent, so differences of 1 percent are real. One warning: two builds of
the *lexer* from identical source have differed by 7 percent in the same
harness, apparently from where the linker put the code. A small difference
between two binaries is not necessarily caused by the change between them.

Machine: Ryzen 9 7950X, one thread, Windows real-time protection on.

## Baseline, 2026-11-18

Commit `883e0a0`.

| Shape | Lex | Parse | Floor | Parse per meaningful token |
| --- | --- | --- | --- | --- |
| 5,265 files of 12 KB | 135 ms | 122.3 ms, 523 MB/s | 16 ms | 7.08 ns |
| One 64 MB file | 136 ms | 151 ms, 424 MB/s | 50 ms | 8.7 ns |

The tree is 39.0 M events for 17.3 M meaningful tokens, 2.26 events per
token. An event is one byte and a token index four, so the tree takes 1.6
bytes per byte of this source.

Read on the small files: of 122 ms, 16 ms is the floor and 106 ms is the
grammar. The parser is already slightly faster than the lexer on these
bytes. On one thread that is 140 million tokens a second.

## Where it stands, 2026-11-18

After experiments 3, 4 and 5, commit `4b31150`. Same tree as the baseline,
event for event.

| Measure | Baseline | Now | Change |
| --- | --- | --- | --- |
| Parse, 64 MB as 5,265 files, one thread | 122.3 ms, 523 MB/s | 112.0 ms, 571 MB/s | 8.4% faster |
| Per meaningful token | 7.08 ns | 6.48 ns | |
| Parse alone on 32 threads | not measured | 8.1 GB/s | |
| Lex, same bytes, one thread | 135 ms | 135 ms | not touched |

These figures were taken before the branch was rebased onto the restyled
lexer. After it, the same parser source measures 109 ms in three alternating
runs against the pre-rebase binary's 111.5 ms, with the same fingerprint. The
only change is the lexer's accessor names, so that 2% is the kind of
difference a rebuilt binary shows, and the table is left as measured.

Against the hardware: the drive reads a packed file at 6 to 9 GB/s (the
lexer's experiment 6). Parsing alone on every thread is now inside that
range on this dense source, and would be above it on ordinary source, which
has about half as many tokens per byte. The lexer is the slower of the two
layers on these bytes.

Of the 112 ms, 16 ms is the floor, the pass that drops whitespace and
comments, and 96 ms is the grammar. Experiment 1 measured what two more
things are worth if they could be had for nothing: the floor itself (16 ms)
and keyword identity (about 29 ms). Both are things the lexer has in hand
while it scans, which is candidate D below, and together they are 40% of the
parse. That is the largest known step left, and it is not the parser's to
take alone.

What is not known: how the remaining 96 ms divides. There is no sampling
profiler on this machine, and three of my guesses about where time went were
wrong. The next single-thread gain should start from a profile, not a guess.

## What the experiments have shown about where the time goes

Measured, not guessed:

- **Reading tokens is not the cost.** Replacing the 32-byte token with a
  one-byte kind, for every question the grammar asks, changed the grammar's
  time by nothing (106 ms both ways). The tokens of a 12 KB file are in the
  cache whatever their size.
- **Comparing word spellings is about a quarter of the grammar.** With every
  keyword test turned into a comparison of two small numbers, the grammar
  took 77 ms where it had taken 106. So about 29 ms is spent answering "is
  this word `machine`?" and its relatives.
- **A pass over the tokens ahead of the grammar is expensive in a way the
  grammar is not.** The grammar predicts its own branches, because it knows
  what it expects next. A pass that classifies tokens with no context
  mispredicts at every change of token kind. Sorting tokens into a one-byte
  kind cost 28 ms; looking each word up in the keyword table cost 78 ms.
- **A predictable sequence of comparisons beats one unpredictable lookup.**
  Asking "which keyword is this?" once per statement, by length and first
  letter, was slower than the eight comparisons it replaced.

## Ledger

| # | Hypothesis | Change | Parse, 5,265 files | Same tree | Decision |
| --- | --- | --- | --- | --- | --- |
| 1 | The grammar's many look-ahead tests are the cost; classify each token once (kind, and which keyword a word is spelled like) and every test becomes a comparison of small numbers. Guess: 20 to 40% faster. | A two-byte class per meaningful token, built in the setup pass. First with the keyword table as a `match` on the spelling, then as a lookup by length and first byte. | 122.3 ms to 170.2 ms, **39% slower**. Setup alone went from about 16 ms to 94 ms; the grammar from 106 ms to 77 ms. | Yes | **Rejected.** The grammar gain is real but the classification costs three times what it saves. With the `match` form it was worse still: the compiler tests all 38 spellings in turn. |
| 2 | Most of the gain needs only the kind, not the keyword: a one-byte kind built without reading the source, with spellings compared on demand. | One byte per meaningful token; `at(punctuation)` and friends read it; keyword tests read the spelling only if the token is a word. | 122.3 ms to 156.1 ms, **28% slower**. Setup 44 ms; grammar 106 ms, unchanged. | Yes | **Rejected.** The kind array buys the grammar nothing and its setup costs 28 ms. |
| 2b | Keep only the keyword type: a `Keyword` enum in place of strings, with one lookup per statement in place of several comparisons. | Baseline cursor, keyword-typed interface, `nth_keyword` at statement starts and in the clause loop. | 122.3 ms to 127.3 ms, **4% slower**. | Yes | **Rejected.** Nothing from experiments 1 and 2 is kept in the source. |

| 3 | Spelling comparisons are a quarter of the grammar, so do fewer: test the cheap condition first. | A prefix word is only looked for when a word follows; the binary-operator test looks for punctuation before comparing with `in`. | 122.1 ms to 121.3 ms, under 1% faster. | Yes | **Kept**, as part of 4. By itself it is inside what code placement can cause, so it shows that these comparisons were not the cost. |
| 4 | Wrapping a finished node shifts its events to make room; per kind of source, the constructs that wrap (binary expressions at 9.8 ns per token, statements at 7.9) cost the most, and those that do not (data at 5.6, types at 5.7) the least. | A statement is opened before its expression and given its kind at the end; a pattern that is one name takes a direct path. Same tree. | With 3: 122.3 ms to 118.7 ms, **3% faster**, in three alternating runs (118.2 to 119.1 against 122.0 to 124.9). | Yes | **Kept.** |

| 5 | Every grammar function returns `Result<_, ParseError>`. The error is 40 bytes, so each result goes through memory on every call to carry an error that almost never exists. | The error is recorded in the parser when found; functions return `Result<_, ParseFailed>`, where `ParseFailed` has no size. | 118.7 ms to 112.0 ms, **5.6% faster**, in three alternating runs (112.0 each against 118.4 to 119.1). | Yes | **Kept.** |

| 6 | The build could be leaving speed behind: 16 codegen units and no link-time optimization are Cargo's defaults. | Same source built with one codegen unit, and with one unit plus full link-time optimization. | One unit: 112.0 ms to 111.5 ms, nothing. With LTO: 108.1 ms, 3.5% faster, but the lexer in the same binary went from 138 ms to 142 ms. | Yes | **Not adopted.** Lexing and parsing together come out even, and the profile is a decision for the whole workspace. |
| 7 | The parser returns two fresh lists per file. The lexer's threads queued for the allocator until its list was made reusable; the parser's may too. | Measured only: `--threads` parses on 1 to 32 threads. | 598 MB/s on one thread, 6,740 on 16 (11.3 times), 8,144 on 32 (13.6 times). | n/a | **No change needed.** 16 cores with a second thread each rarely give more than this, so the allocator is not holding it back at this file size. |

What my guess got wrong in experiment 1: I priced the classification at
nothing, because it is "one pass". It was the most expensive pass in the
parser. I also guessed the wrong cost inside the grammar; it was the spelling
comparisons, not the token reads.

## 8. Build the meaningful-token index during lexing (2026-11-18)

On the `@` grammar at `a2fcd81`, the lexer still emits every token, including
comments and whitespace. The experiment also had it emit the `u32` positions
of meaningful tokens and transfer ownership of that vector into the parser.
The existing path builds this vector by walking the token list after lexing.
Keyword handling, grammar, tree events and token spans were identical.

Two placements were tested. Checking `is_trivia()` at the scanner's common
token-emission point made little difference on 13,445-byte files. Emitting
the index inside the word, punctuation, number and literal branches avoided
that second classification. In twelve balanced three-way blocks, the median
paired changes against the original release binary were −0.10% and −2.09%,
respectively. The branch version won all twelve blocks. Median stage times
on that version moved from 147.1 to 160.3 ms in lexing and from 111.5 to
93.0 ms in parsing. Changing the index's initial capacity alone gave no
reliable improvement in a separate eight-pair control.

| Same 64 MiB of source, one thread | Original median | Branch version median | Median paired time change |
| --- | ---: | ---: | ---: |
| 23 fixtures kept as individual files, repeated | 282.5 ms | 269.1 ms | −4.37% (7 of 7 faster) |
| 23 fixtures concatenated into 13,445-byte files, repeated | 258.4 ms | 253.2 ms | −2.09% (12 of 12 faster) |
| One contiguous 64 MiB file | 450.3 ms | 449.1 ms | +0.32% (2 of 5 faster) |

The middle row uses the existing `parse_throughput --files --passes 1`
harness. The other two rows time the full in-memory loop including tree
destruction, average two measured passes per invocation after two warmups,
and alternate separately built executables. The change column is computed
within pairs. An eight-pair same-path control on 13 KB files varied by a
median of 0.11%. Tests used rustc 1.100.0 on Windows and a Ryzen 9 7950X.

Both placements matched the original token vectors and index positions,
syntax trees and diagnostics on all 160 lexer/parser fixtures and 132 spec
examples, through the file and fragment entry points. The release benchmark
fingerprint was `ea73705f46fd5d3c` in all three builds.

**Decision.** The branch placement is a candidate for an opt-in handoff on
many small files. The large-file control showed no reliable gain. The
prototype reserved one index slot per three input bytes, which can overreserve
on comment-heavy source, and limited inputs to `u32::MAX` bytes. A production
API would need to preserve the existing token-index overflow behavior and
keep each index paired with its own tokens. These timings cover in-memory
L0 and L1 only.

## 9. Size the index and token buffers from observed work (2026-11-18)

The prototype in experiment 8 reserved 10.7 MiB of index capacity for an
8 MiB block comment containing zero meaningful tokens. The production lexer
also reserved 85.3 MiB of token capacity for that comment's single token.
These numbers measure vector capacity on the 64-bit test host. They exclude
the input buffer and do not measure resident memory.

Growing both vectors from empty on large inputs fixed the comment case but
cost about 15% on a dense 64 MiB file. Fixed initial caps of 4 KiB for the
index and 256 KiB for token storage still incurred that large-file cost.
Growing token capacity eightfold after observing 1,024 tokens also lost
13% in the large-file comparison.

The prototype used the existing token density estimate on sources
through 16 KiB and initially reserved at most 1,024
`u32` positions. It appended each meaningful token's position as it lexed
and transferred the vector into the parser. Above 16 KiB the scanner finished
the token crossing the first sixteenth of the file and estimated the remaining
token count from the density seen so far, allowing one eighth of headroom.
It continued from the next token. A single comment could exhaust the entire
input during the sample. The parser built its checked index over the emitted
tokens on this path, preserving its existing token-position overflow check.

Each row below processes the same 64 MiB of sorted V2 accept fixtures using
separately built release executables. Each invocation averages two measured
passes after two warmups. The table gives the median times over six paired
rounds including tree destruction, on the same Windows host as experiment 8.

| Workload | Previous | Sampled prototype | Median paired change |
| --- | ---: | ---: | ---: |
| 23 individual files repeated | 282.74 ms | 268.52 ms | −4.88% |
| 13,445-byte bundles repeated | 251.30 ms | 249.05 ms | −0.87% |
| One dense 64 MiB file | 447.98 ms | 394.15 ms | −12.15% |

All six rounds moved in the same direction on each workload. The bundle
result is inside the project's 2% noise threshold. On individual files the
lexer stage rose from 140.46 to 163.00 ms and the parser stage fell from
138.55 to 102.15 ms. On the dense file lexing fell from 302.67 to 249.92 ms.
On an 8 MiB block comment, a fresh token vector reserved 128 bytes and the
lexer allocated no index. Appending a space and `machine f() {}` took 512
bytes of token capacity for ten tokens, six meaningful.

The standalone V2 lexer on V1's source, samples and fixtures exposed the
tradeoff. Across plain, reshuffled and aligned builds, file-by-file throughput
fell 10.4–15.4%. Restoring the original full-file reservation and specializing
the shared scan loop narrowed this to 2.1–7.3% slower. Separating the scanner
entry points and explicitly inlining number scanning also regressed across
all three layouts. All variants produced the same V1 token fingerprint,
`389ed15fa61d1f87`. The project keeps the existing scanner and parser handoff.

**Parser-only change.** The parser's index used to start with room for half
the emitted tokens plus one. Many small files immediately outgrew it. In five
paired rounds on individual fixtures, reserving for two thirds of the tokens
reduced median combined time by 3.38%. Reserving for every token reduced it
by 5.85%. On one dense 64 MiB file, the latter estimate was 2.96% slower in
the median and increased the initial request on trivia-heavy sources.

We kept the full-token estimate only for inputs of up to 1,024 emitted tokens.
That asks for at most 4 KiB and avoids a second allocation on those files.
Above that limit the parser uses its original estimate. The lexer needs no
change. With the same 64 MiB of fixtures, three rounds of the repository's
`compare-builds` tool gave these best combined times for individual files:

| Build | Previous | Kept | Reduction |
| --- | ---: | ---: | ---: |
| Plain | 286.4 ms | 275.3 ms | 4.0% |
| Reshuffled | 285.8 ms | 278.5 ms | 2.6% |
| Aligned | 286.0 ms | 275.2 ms | 3.9% |

Parsing alone improved 8.5–11.2% on the individual fixtures. Combined times
on 13,445-byte bundles varied by at most 1.3% and times on the single 64 MiB
file varied by at most 0.5%. Both controls fall inside the project's 2%
noise threshold. The production lexer still makes the 85.3 MiB token
reservation for an 8 MiB block comment.

The in-tree benchmark now preserves original file boundaries with
`--individual` and pairs the lexer and parser times from each pass before
reporting the fastest combined time. Run the same corpus on another host with
`cargo run --release -p omega-parser --example parse_throughput -- --individual --passes 9`.

## Remaining candidates

A (fewer spelling tests) and B (a small error) became experiments 3 and 5.
C was done for statements and single-name patterns in experiment 4; what is
left of it is the first row here.

| # | Idea | Why it might help | What the findings above say |
| --- | --- | --- | --- |
| C | Stop shifting events for the nodes that still wrap their first child: binary operators, calls, field accesses, casts. A link ("my parent starts N events back") resolved in one pass at the end would do it, at the price of a second pass over the events. | A call into `memmove` per wrapped node. | In the spec examples about 4% of tokens begin such a node, so a few percent at most, less on source with fewer expressions. |
| D | Supply keyword identity along with the meaningful-token index measured in experiment 8. | The lexer has the spelling in hand. Experiment 1 timed the grammar with preclassified keywords. | The cost of keyword classification in the lexer and its effect on combined throughput remain unmeasured. |
| E | Check the nesting counter less often. | It is incremented and decremented in every prefix, type and pattern. | Probably small. |
| F | Let a caller hand the parser the lists to fill, as `lex_into` does, so a thread parsing many files allocates nothing after the first few. | Three allocations per file today. | Experiment 7 shows threads are not queuing for the allocator at 12 KB per file. Worth re-measuring on much smaller files. |
| G | A sampling profiler, which this machine does not have. | It would replace guessing about the 96 ms of grammar. | Three of the guesses in the ledger were wrong. |
