# L0 Lexer performance experiments

One entry per experiment, newest last. Each says what was measured, what was
expected, what happened, and whether the change was kept.

## How these are measured

    cargo run --release -p omega-lexer --example lex_throughput -- --passes 9 <directories>

- **Workload:** the 19,442 `.omg` files under V1's `source`, `samples` and
  `tests/fixtures`. 26,441,134 bytes, 7,758,242 tokens, median file 760
  bytes. All held in memory before timing, so no disk time is included.
- **Machine:** AMD Ryzen 9 7950X, Windows 11, release build, single thread.
  Other work was running on the machine during every measurement.
- **Noise:** two runs of the same binary gave medians of 85 and 80 MB/s. The
  slowest pass in a run is typically 15 to 20 percent below the fastest.
  Differences smaller than that mean nothing.
- **Same output:** the tool prints a fingerprint of every token's kind and
  byte range. A change meant only to be faster must print the same one.

Token mix of the workload, which decides where time can go:

| Kind | Tokens | Share | Bytes | Share |
| --- | --- | --- | --- | --- |
| Whitespace | 2,620,860 | 33.8% | 4,841,412 | 18.3% |
| Punctuation | 2,620,515 | 33.8% | 3,030,061 | 11.5% |
| Word | 2,242,358 | 28.9% | 14,367,566 | 54.3% |
| Integer | 201,615 | 2.6% | 495,218 | 1.9% |
| LineComment | 41,416 | 0.5% | 3,047,618 | 11.5% |
| Quoted | 23,293 | 0.3% | 624,474 | 2.4% |
| Float | 8,297 | 0.1% | 54,226 | 0.2% |

## 1. First-byte dispatch for punctuation (2026-11-18): kept

**Baseline.** About 78 MB/s file by file, 38 ns per token. UTF-8 validation
alone runs at about 12,000 MB/s on the same files, so the up-front validation
pass is not the cost. At roughly 5 GHz, 38 ns is on the order of 190 cycles
per token, far more than cutting a token out of a byte string should need.

**Hypothesis.** `Punctuation::longest_match` tried all 59 spellings in order
for every punctuation token. A third of all tokens are punctuation, and the
commonest ones (`;`, `,`, brackets) are at the end of the list, so most
lookups ran 50 or more failed comparisons. Looking only at the spellings that
share the first byte, at most six, should remove most of the lexer's time.
Estimated result before running: 300 to 450 MB/s. Staying under about 150
would have rejected the hypothesis.

**Change.** A 128-entry table, built at compile time from the same spelling
list, maps a first byte to the range of spellings that begin with it.
`longest_match` searches only that range. Nothing else changed.

**Result.** Three rounds, alternating baseline and candidate, 9 passes each.
Median of the passes, file by file:

| Round | Baseline | Candidate |
| --- | --- | --- |
| 1 | 79 MB/s, 37.9 ns/token | 528 MB/s, 6.4 ns/token |
| 2 | 77 MB/s, 39.1 ns/token | 525 MB/s, 6.3 ns/token |
| 3 | 77 MB/s, 37.3 ns/token | 525 MB/s, 6.3 ns/token |

About 6.7 times faster, well outside the noise. The per-token figures are
from each run's fastest pass.

**Same output.** Fingerprint `b7778a2e5cd741fb` for baseline and candidate in
all six runs. All 47 tests pass, including the fixtures, the 132 spec
examples and the 60,000 random inputs.

**Decision.** Kept. The hypothesis was right and slightly underestimated.

**What the result says about what is next.** Lexing all files as one joined
buffer now runs at about 260 MB/s, half the file-by-file speed. The only
difference is that one 7.8-million-token list (248 MB) is grown and written
instead of many small ones that stay in cache. So for one large input the
lexer's cost is dominated by writing its 32-byte tokens to memory, not by
deciding what they are. That matters only for very large single files, which
this workload does not have (median 760 bytes), so it is not worth acting on
now. If it becomes worth it, the questions are whether a token needs 32
bytes, and whether whitespace tokens need to be stored at all.

Correction, same day: this entry first said 24 bytes and 186 MB. That was
assumed, not measured. `size_of::<Token>()` is 32: a 16-byte kind and a
16-byte span.

## 2. A floor, and the token-list reservation (2026-11-18): kept

**A floor to judge against.** The tool gained a measurement that looks every
byte up in a class table and counts class changes, storing nothing. It runs at
about 3,500 MB/s on the same files, which is 0.28 ns per byte. The lexer at
about 520 MB/s costs 1.9 ns per byte. Tokens here average 3.4 bytes, so the
difference is about 5.6 ns of work per token that is not reading bytes: it is
choosing the rule, building the token and storing it. Per-token overhead, not
per-byte scanning, is where the remaining time is.

**Hypothesis.** `scan_all` reserved room for one token per 4 bytes. The
workload has one per 3.4, so most files overflowed the reservation and the
list grew once, copying every token already stored. Reserving one per 3 bytes
should remove that copy. Estimated before running: 5 to 15 percent file by
file, more on a large input. No gain on the joined buffer would have rejected
it.

**Change.** `len / 4 + 16` became `len / 3 + 16`. Nothing else.

**Result.** Four rounds, alternating, 9 passes each. Median of the passes:

| Round | File by file, before | after | Joined buffer, before | after |
| --- | --- | --- | --- | --- |
| 1 | 408 MB/s | 546 | 215 MB/s | 364 |
| 2 | 487 | 517 | 227 | 353 |
| 3 | 526 | 518 | 256 | 354 |
| 4 | 493 | 528 | 249 | 357 |

- **Joined buffer: about 45 percent faster**, in every round and far outside
  the noise.
- **File by file: inconclusive.** The candidate was ahead in three rounds and
  behind in one, and round 1's baseline was visibly disturbed by other work on
  the machine. Any real gain is under about 5 percent, which this host cannot
  resolve. The 5 to 15 percent estimate was not confirmed.

**Same output.** Fingerprint `b7778a2e5cd741fb` in all eight runs.

**Decision.** Kept. It helps large inputs clearly, cannot hurt small ones, and
costs one changed constant. It reserves about 10.7 bytes of token storage per
source byte, up from 8.

## 3. Punctuation chosen by a hand-written match (2026-11-18): kept

**Hypothesis.** After experiment 1 a punctuation token still went through a
range lookup, an iterator and a variable-length comparison. Deciding on the
first three bytes with a `match` should compile to jumps. Estimated before
running: 15 to 30 percent file by file. Under 5 percent would have rejected
it.

**Change.** `Punctuation::longest_match` is now a `match` on the first two
bytes, with the third consulted for `<<` and `..`. The spelling list stays as
the definition of the right answer, and a test compares the match against a
search of that list on every input of up to four bytes built from the bytes
that could tell them apart (over 50,000 inputs). The first-byte range table
from experiment 1 is gone, since the match replaces it.

**Result.** Five rounds of 15 passes, with the order of baseline and candidate
swapped each round. Median of the passes, file by file:

| Round | Baseline | Candidate |
| --- | --- | --- |
| 1 | 524 MB/s | 578 |
| 2 | 531 | 577 |
| 3 | 532 | 571 |
| 4 | 537 | 616 |
| 5 | 556 | 586 |

The candidate won every round, by 5 to 15 percent and about 9 percent on
average. Per token went from about 6.3 to 5.8 ns. That is a real gain and
smaller than estimated: the lookup was already cheap after experiment 1.

**Same output.** Fingerprint `b7778a2e5cd741fb` in all ten runs; 47 tests pass.

**Decision.** Kept, narrowly. The cost is that a punctuation spelling now
lives in two places, the list and the match. The comparison test fails if
they ever differ, which is what makes that acceptable.

### Diagnostic: how much does storing tokens cost?

A throwaway build constructed every token but did not store it. It is not a
valid lexer and was reverted; it only measures what storage costs.

| | Normal | Tokens not stored |
| --- | --- | --- |
| File by file | about 586 MB/s | about 678 MB/s |
| One joined buffer | about 370 MB/s | about 742 MB/s |

- **Small files:** storing tokens costs about 14 percent.
- **One large input:** storing tokens costs half the time.
- **The ceiling without storage is about 700 MB/s**, still five times below
  the 3,500 MB/s byte-classification floor. So on small files most of the
  remaining time is neither reading bytes nor storing tokens. It is the
  scanner's own decision work: about 4 of the 5.8 ns per token.

## 4. The real path, and reusing the token list (2026-11-18): kept

**The question.** Jarod's target is the drive, not a percentage: the source
is on a Samsung 9100 PRO, an NVMe drive rated for sequential reads far above
anything measured here, and the lexer should not be what holds a compile
below it. So this experiment measures the whole path from files to tokens,
with a new tool:

    cargo run --release -p omega-lexer --example lex_pipeline -- <directories>

It times finding the files, reading them, lexing them from memory, and
reading then lexing each file on one thread, each on 1 thread and on all 32
hardware threads (16 cores). Reads come from a warm file cache, which is the
best case for reading.

**Baseline, before any change.** Fastest of 5 passes:

| Step | Threads | Time | MB/s |
| --- | --- | --- | --- |
| Find the files | 1 | 1,211 ms | |
| Read | 1 | 748 ms | 35 |
| Read | 32 | 265 ms | 100 |
| Lex from memory | 1 | 45 ms | 589 |
| Lex from memory | 32 | 23 ms | 1,142 |
| Read then lex | 1 | 736 ms | 36 |
| Read then lex | 32 | 275 ms | 96 |

Two things stood out.

- **The lexer is not the bottleneck.** Lexing is 45 ms of a 736 ms path on
  one thread. Opening 19,442 small files is the cost, and it tops out near
  100 MB/s even from a warm cache on 32 threads. The drive's speed is not
  involved at all; this is the cost of opening files, not of moving bytes.
- **Lexing did not scale.** 32 threads were only twice as fast as one.

**Hypothesis for the scaling.** `lex` returned a new token list for every
file. With every thread allocating and freeing a list per file, the threads
queue for the memory allocator. Reusing one list per thread should restore
scaling. Under 2 GB/s on 32 threads would have rejected it.

**Change.** `lex_into(source, &mut tokens)` empties and refills a list the
caller owns. `lex` is now a thin wrapper around it. On a refusal the list is
left empty. Two tests cover the reuse and the refusal.

A flaw in the tool was fixed at the same time: the directory walk asked the
file system about every entry twice. That is why "find the files" drops below.

**Result.** Two runs, fastest of 5 passes each; the in-memory lexing steps go
over the files 20 times per pass so that they last long enough to time.

| Step | Threads | Time | MB/s |
| --- | --- | --- | --- |
| Find the files | 1 | 330 ms | |
| Read | 1 | 571 to 624 ms | 42 to 46 |
| Read | 32 | 263 ms | 100 |
| Lex, new list per file | 1 | 44 to 46 ms | 579 to 597 |
| Lex, new list per file | 32 | 24 to 25 ms | 1,047 to 1,092 |
| Lex, one list per thread | 1 | 42 ms | 630 |
| Lex, one list per thread | 32 | 2.7 ms | 9,636 to 9,770 |
| Read then lex | 1 | 616 to 648 ms | 41 to 43 |
| Read then lex | 32 | 255 to 267 ms | 99 to 104 |

- **On all threads, lexing went from about 1.07 to about 9.7 GB/s**, nine
  times faster, and now scales 15 times from 1 thread to 16 cores.
- **On one thread it gained about 5 percent**, 597 to 630 MB/s.
- **The real path did not move**: about 100 MB/s on 32 threads, because it
  was never waiting for the lexer.

**Same output.** `lex` and `lex_into` are compared token for token in the
tests; all 50 tests pass.

**Decision.** Kept. This is the change that lets the lexer use the machine.

**What limits the path now.** Reading: 263 ms to open and read 19,442 files
on 32 threads against 2.7 ms to lex them. To get anywhere near the drive, the
work is in how source files are found and loaded, which is the step before
this layer:

- Most of a small file's cost is opening it. Whether antivirus scanning is
  part of that on this machine is not measured; an exclusion for the source
  tree, or a Dev Drive, would show it.
- A project's sources could be read from one packed file, or the listing and
  contents cached between builds, so that a build opens a handful of files
  and not thousands.
- The cold case, straight from the drive, is not measured at all.
## 5. Tightening the scanner (2026-11-18): two parts kept, one rejected

**What was known.** The storage diagnostic in experiment 3 put about 4 of
5.8 ns per token in the scanner's own decision work. A new measurement in the
throughput tool split that further: the same line of code repeated, which a
branch predictor learns almost perfectly, cost 3.8 ns per token against 5.8 on
real files. So roughly a third of the per-token cost was branches guessed
wrong, and the rest was straight-line work.

Three changes were tried one at a time, each against the one before, in five
alternating rounds of 15 passes. Figures are the median pass, file by file.

**A. Pick each token's rule from a byte-class table: kept.** The main loop
now looks the first byte up in a 256-entry table and handles whitespace, words
and punctuation in place, with the position in a local. Numbers, quoted
literals and block comments still go to the existing methods. The raw-literal
check, which ran twice for every word starting with `r`, now runs only for the
one-letter word `r`. Guess: 10 to 25 percent.

| | Before | After |
| --- | --- | --- |
| Real files | 578 MB/s, 5.8 ns/token | 616 MB/s, 5.3 ns/token |
| One line repeated | 3.8 ns/token | 3.1 ns/token |

About 7 percent on real files and 18 percent on the predictable input. The
straight-line work shrank as intended; on real files the wrong guesses, which
this did not touch, hid most of it.

**B. Find the end of a word without a data-dependent loop: kept.** A word's
length differs every time, so a loop that exits at the end of the word is
guessed wrong about once per word. `find_word_end` looks eight bytes up at
once, packs the answers into a mask and reads the length off it. Guess: about
10 percent.

| | Before (A) | After (B) |
| --- | --- | --- |
| Real files | 642 MB/s, 5.2 ns/token | 745 MB/s, 4.5 ns/token |

About 16 percent, in every round. This is the first estimate of a small gain
that came in low.

**C. The same window for whitespace runs: rejected.** Guess: a few percent.

| | Before (B) | After (C) |
| --- | --- | --- |
| Real files | 740 MB/s | 626 MB/s |

15 percent slower in every round. Most whitespace runs are one byte, so the
plain loop exits at once and is guessed right, while the window always pays
for eight lookups. Reverted; a comment at the site says why.

**Same output.** Fingerprint `b7778a2e5cd741fb` in all thirty runs. 53 tests
pass, with three new ones tying the class table to the punctuation list.

**Where the lexer stands after A and B.** Fastest of 5 passes:

| | 1 thread | 32 threads |
| --- | --- | --- |
| All 19,442 files | 746 MB/s | about 11 GB/s |
| V1's `source` tree only (301 files, 13 KB average) | 832 MB/s | about 11 GB/s |
## How the estimates have done

| Experiment | Estimated | Measured |
| --- | --- | --- |
| 1. First-byte dispatch | 4 to 6 times | 6.7 times |
| 2. Larger reservation | 5 to 15 percent file by file | under 5 percent, unresolved |
| 3. Punctuation match | 15 to 30 percent | about 9 percent |
| 4. Reuse the token list | over 2 GB/s on 32 threads | about 9.7 GB/s |
| 5A. Class-table dispatch | 10 to 25 percent | about 7 percent |
| 5B. Branch-free word end | about 10 percent | about 16 percent |
| 5C. Branch-free whitespace | a few percent faster | 15 percent slower |

Guesses about straight-line work have run high. The two that came in low, 4
and 5B, both removed a wait (for the allocator, for a wrong branch guess)
rather than work. Worth remembering when picking the next one.
## Candidates not yet tried

Ranked for the file-by-file workload after the diagnostic above.

1. **The remaining wrong guesses.** Real files still cost 4.5 ns per token
   against 3.0 on a predictable input. What is left is mostly the choice of
   rule itself, which follows the token sequence. Not obviously removable
   without a different design, such as classifying a whole block of bytes at
   once with vector instructions. Guess: unknown.
2. **A smaller token.** 32 bytes holds two `usize` offsets and a 16-byte
   kind. Since tokens tile the source, a token's end is the next token's
   start, so start plus kind fits in 8 bytes. Worth at most the 14 percent
   that storage costs on small files, and up to half on one large input.
   This changes the type the parser consumes, so it should be decided with
   the parser.
3. **Not storing whitespace.** A third of tokens are whitespace that a parser
   skips. This changes contract rule R2 and is an owner decision.
4. **Lexing files in parallel.** The workload is many small independent
   files. This belongs to whatever calls the lexer, not to the lexer.

Not worth trying: SIMD scanning. Words average 6.4 bytes and whitespace 1.9,
too short for vector code to pay off; only comments are long, and they are
11.5 percent of bytes. Dropping the separate UTF-8 pass would save at most 4
percent.