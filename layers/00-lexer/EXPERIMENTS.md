# L0 Lexer performance experiments

One entry per experiment, newest last. Each says what was measured, what was
expected, what happened, and whether the change was kept.

## How these are measured

    cargo run --release -p omega-lexer --example lex_throughput -- --passes 9 <directories>

To compare two versions, do not time them by hand. From the repository root:

    cargo run --release -p compare-builds -- --package omega-lexer --tool lex_throughput -- --passes 7 <directories>

That builds the last commit and the working tree three ways each, runs them
interleaved, and ends with one sentence. `--baseline-ref` names another commit
and `--baseline-dir` a directory. It exists because of what experiments 8 to
10 ran into; experiment 12 shows it gives the right answer on two cases where
the answer is known.

- **Workload:** the 19,442 `.omg` files under V1's `source`, `samples` and
  `tests/fixtures`. 26,441,134 bytes, 7,758,242 tokens, median file 760
  bytes. All held in memory before timing, so no disk time is included.
- **Machine:** AMD Ryzen 9 7950X, Windows 11, release build, single thread.
  Other work was running on the machine during every measurement.
- **Noise:** two runs of the same binary gave medians of 85 and 80 MB/s. The
  slowest pass in a run is typically 15 to 20 percent below the fastest.
  Differences smaller than that mean nothing.
- **Placement:** two builds of the same logic can differ by up to 9 percent
  because of where the hot loop lands in the binary, so one build's number is
  good to about 5 percent either way. When two builds differ by less than 10
  percent, build both with `RUSTFLAGS="-C llvm-args=-align-all-functions=6"`
  and see whether the difference survives. Found in experiments 8 and 9; the
  experiments before them did not control for this.
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
## 6. Where reading a file goes (2026-11-18): measured, nothing changed in the lexer

**The question.** After experiment 4 the path from files to tokens waits on
reading. Where does the time for one small file go, and what gets around it?
A new tool measures it:

    cargo run --release -p omega-lexer --example read_breakdown -- <directories>

Same 19,442 files, warm file cache, fastest of 5 passes, two runs. This host
was busy and the runs differ by up to 20 percent, so the figures are ranges.

**One file on one thread costs about 28 to 34 microseconds**, in four
requests to the operating system:

| Request | Per file | Share |
| --- | --- | --- |
| Open | 13 to 16 us | about 48% |
| Ask the size | about 3 us | about 10% |
| Read | 9 to 12 us | about 33% |
| Close | about 3 us | about 10% |

None of this is the drive. The bytes are already in memory; this is the cost
of asking for them. 9 to 12 microseconds to read about 1.4 KB from the cache
is far more than copying those bytes takes. Windows real-time antivirus
protection is on for this machine, and it inspects files as they are opened
and read. Whether it accounts for the cost is **not measured**: seeing or
changing its exclusions needs administrator rights, and that is the owner's
call.

**Ways to read a file, one thread:**

| Strategy | Files/s | MB/s |
| --- | --- | --- |
| `std::fs::read` | 26,000 to 34,000 | 36 to 47 |
| Reuse one buffer, skip the size request | 29,000 to 34,000 | 39 to 46 |
| Size from the directory listing, one read | 37,500 to 40,500 | 51 to 55 |
| All files packed into one, reused buffer | (one file) | 6,600 to 8,900 |

- **Trimming requests helps a little.** Knowing the size from the listing and
  reading once is the best per-file strategy, perhaps 1.2 to 1.3 times
  `std::fs::read`. The gap is not much larger than the noise.
- **One packed file is more than a hundred times faster.** 26 MB in 3 to 4
  ms. This is the only measurement here that gets near what the drive can do.

**More threads stop helping at about 8.**

| Threads | Files/s |
| --- | --- |
| 1 | 33,000 to 38,000 |
| 2 | 59,000 to 69,000 |
| 4 | 85,000 to 98,000 |
| 8 | 74,000 to 88,000 |
| 16 | about 71,000 |
| 32 | 72,000 to 74,000 |

Reading peaks at 4 to 8 threads and is slower on 16 and 32. The threads are
waiting on each other inside the operating system, not on the drive. So the
best a per-file loader does here is roughly 90,000 files per second, about
125 MB/s on this corpus, whatever it does in its own code.

**What follows for V2.**

- A loader that reads file by file should use about 4 to 8 threads for
  reading, take sizes from the directory listing, and reuse its buffers. That
  is worth perhaps 3 times over one thread of `std::fs::read`, and no more.
- To get near the drive, a build has to open few files. That means a packed
  copy of the sources, with a cheap way to notice which files have changed.
  The walk that finds the files costs 330 ms by itself, so "check every file"
  is not cheap either; this needs designing, and is recorded as an owner
  question in PLAN.md.
- Lexing is no longer close to mattering: 2.4 ms on all threads against at
  least 200 ms to read the same files one by one.

### With real-time antivirus protection off

Jarod turned Windows real-time protection off and the same measurements were
rerun, same day, same files, two runs. `Get-MpComputerStatus` confirmed
real-time and on-access protection were both off.

| Request | Protection on | Protection off |
| --- | --- | --- |
| Open | 13 to 16 us | 12.9 us |
| Ask the size | about 3 us | 2.8 to 2.9 us |
| Read | 9 to 12 us | 5.6 to 5.8 us |
| Close | about 3 us | 2.7 to 3.1 us |
| All four | 28 to 34 us | 24.3 to 24.5 us |

| | Protection on | Protection off |
| --- | --- | --- |
| Best per-file strategy, one thread | 37,500 to 40,500 files/s | 40,000 to 48,000 |
| Peak across threads | 85,000 to 98,000 files/s at 4 | 103,000 to 122,000 at 4 |
| 32 threads | 72,000 to 74,000 files/s | 78,000 to 80,000 |
| One packed file, reused buffer | 6.6 to 8.9 GB/s | 9.2 to 9.7 GB/s |

- **Antivirus was about a fifth of the per-file cost**, 4 to 9 microseconds
  out of roughly 30. Nearly all of it was in the read step, which halved.
- **Opening the file is untouched** at about 13 microseconds and is now more
  than half of what is left. That cost belongs to Windows and the file
  system, not to antivirus.
- **Threads still stop helping after 4.** The peak moved up about 25 percent
  and 16 and 32 threads are still slower than 4, so the contention is not
  antivirus either.
- **The two runs agree to within 1 percent on the per-request figures**, where
  the protection-on runs differed by 20 percent. Some of the earlier noise was
  the antivirus itself.

So the conclusion stands with better numbers: a per-file loader is capped
near 110,000 to 120,000 files per second with protection off, about 150 MB/s
on this corpus, against 9 GB/s for one packed file.

**Not measured:** a cold cache; a Dev Drive or a folder exclusion in place of
turning protection off altogether; opening files relative to an already-open
directory, which skips walking the path each time and needs calls the
standard library does not expose. With open now the largest remaining cost,
that last one is the obvious next thing to try for per-file loading.
## 7. Cheaper routes to opening a file (2026-11-18): rejected

**Hypothesis.** With antivirus off, opening a file is about 13 microseconds
and more than half of reading it. The standard library opens by full path
through the Win32 layer, so the kernel walks every directory from the drive
letter down each time. Opening relative to a handle on the file's own
directory leaves nothing to walk and should be clearly cheaper. No measurable
difference would reject it.

**How it was tested.** A new tool opens the same 19,442 files four ways:

    cargo run --release -p omega-lexer --example open_breakdown -- <directories>

1. `File::open` with the full path.
2. `NtOpenFile` with the full path, which skips the Win32 layer.
3. `NtOpenFile` relative to one handle on the top of the tree.
4. `NtOpenFile` relative to a handle on the file's own directory.

Routes 2 to 4 call ntdll directly, since the standard library cannot name a
starting directory. Before timing, 512 files were read through every route
and compared byte for byte with `std::fs::read`; all matched. Real-time
antivirus was off. Fastest of 5 passes, two runs, one thread:

| Route | Open + close | Open + read + close |
| --- | --- | --- |
| 1. `File::open`, full path | 16.6 to 17.7 us | 22.9 to 26.3 us |
| 2. `NtOpenFile`, full path | 17.6 to 19.5 us | 20.2 to 20.6 us |
| 3. `NtOpenFile`, from the tree root | 16.3 to 19.3 us | 26.1 to 27.1 us |
| 4. `NtOpenFile`, from its own directory | 16.8 to 18.6 us | 20.7 to 21.8 us |

**Result.** No route is faster. Every one costs 16 to 20 microseconds to open
and close a file, and the differences between routes are smaller than the
differences between two runs of the same route. Across threads the two
extremes are also the same: both peak near 112,000 files per second at 4
threads and fall back to about 87,000 on 16 and 32.

**Decision.** Rejected. The cost of opening a file here is not the path walk
and not the Win32 layer. It is what Windows and the file system do to create
and tear down an open file, and no choice of route from user code avoids it.

Two side facts from this run:

- The tree has 9,620 directories for 19,442 files, about 2 files each, and
  opening a directory costs 13 to 15 microseconds, the same as a file. So a
  loader that keeps directory handles pays for 9,620 extra opens up front.
- The tool stays in the repository so the same comparison can be rerun on a
  different file system, such as a Dev Drive.

**What this leaves.** For reading file by file there is nothing further to
take out from the caller's side: about 20 microseconds per file, peaking near
110,000 to 120,000 files per second at 4 threads. The only measured way past
that is to open fewer files, which is owner question 4 in PLAN.md.

## 8. Restyling to Squalr's conventions (2026-11-18): kept, and a flaw found in how we measure

**What changed.** No algorithm. The crate was rewritten in the style of the
Squalr codebase: one type per file, private fields behind `get_` accessors,
`Token::new` in place of a struct literal in the hot loop, and methods on the
small `Copy` enums taking `&self` where they had taken `self`. Afterwards the
files were moved into folders, which changes no code at all.

**Hypothesis.** None of this should be measurable, since every accessor and
constructor is a few instructions and is inlined. A repeatable difference
larger than the noise would reject it.

**What happened.** Every build printed the same fingerprint. File by file,
median MB/s over interleaved rounds:

| Build | Medians |
| --- | --- |
| Before the restyle | 742, 743, 739 |
| Restyled, `&self` on the enums | 713, 711, 712 |
| Restyled, `self` by value | 718, 734, 724, 729, 728 |
| The same code moved into folders | 690, 684, 677, 681 |

The last row is the important one. Moving files cannot make the lexer 8
percent slower, so the measurement was showing something other than the code.

**The flaw.** It is where the build happens to place the hot loop. Building
with every function aligned to 64 bytes
(`RUSTFLAGS="-C llvm-args=-align-all-functions=6"`) takes that luck away, and
then the old code and the reorganised code are the same: 703, 706 and 693
against 716, 714 and 704. The old code loses about 6 percent under that flag,
which is to say its usual build was a lucky one.

So two builds of this lexer can differ by 6 to 9 percent with nothing changed
that matters. That is larger than several effects recorded above, and the
"Noise" note at the top only covered two runs of one binary. Experiments 2, 3
and 5A are the ones small enough to be worth re-measuring with alignment held
equal. Experiments 1, 4 and 5B are far larger than this and stand. (Experiment
3 was re-measured this way in experiment 10, and it stands too.)

**`&self` against `self`, measured again.** With alignment held equal, four
rounds gave 717, 717, 715 and 704 by value against 705, 699, 699 and 697 by
reference. About 2 percent, in the same direction every round, not the 4
percent the first comparison suggested. The scanner asks every punctuation
token for the length of its spelling, and punctuation is a third of all
tokens, so `Punctuation::get_spelling` is the likely place.

**Decision.** The restyle and the folders are kept. Methods on `Punctuation`,
`TokenKind`, `IntegerRadix` and `LexErrorKind` take `self` by value, which is the one
place this crate does not follow Squalr.

**What this leaves.** An ordinary release build of the reorganised crate has
measured between 680 and 720 MB/s, moving each time the folders were renamed,
where the old one measured about 740. That gap is placement. Aligning loops to 64 bytes as well as functions
(`-C llvm-args=-align-loops=64`) gave 719, 723 and 712 against 719, 731 and
737 for the old build in the same rounds, on a machine that was busier than
usual. Experiment 9 tests that properly, and it does not hold up.

## 9. Alignment flags as a fix for placement (2026-11-18): rejected

**Hypothesis.** If luck in where the hot loop lands is worth 6 to 9 percent,
then forcing the alignment should take the luck out and leave every build at
the speed of a lucky one. Specifically, aligning functions and loops to 64
bytes should measure about 740 MB/s whatever the folders are called. A forced
build that is slower than an ordinary one would reject it.

**How it was tested.** Three copies of the crate with the same code and
different folder names, so that the logic is identical and only placement can
differ. Each was built under nine settings, giving 27 binaries, all printing
the same fingerprint. Every setting is a `-C llvm-args=` flag. File by file,
7 passes, three or four rounds interleaved across settings and layouts.

Another build was running on the machine for part of this and pulled whole
rounds down by as much as 15 percent. So the table gives the fastest pass of
each run, which other work can lower but not raise, and takes the middle
round. The settings were measured in two sweeps, each with its own ordinary
build beside it; the ordinary build read 728, 733 and 730 in the other sweep.

| Setting | Flag | Three layouts, fastest pass, MB/s |
| --- | --- | --- |
| Ordinary build | none | 734, 731, 739 |
| Blocks to 16 bytes | `-align-all-blocks=4` | 739, 734, 729 |
| Functions and loops to 64 | `-align-all-functions=6 -align-loops=64` | 717, 715, 706 |
| Branches inside 32-byte lines | `-x86-branches-within-32B-boundaries` | 705, 711, 707 |
| Functions and loops to 32 | `-align-all-functions=5 -align-loops=32` | 703, 698, 713 |
| Blocks to 32 bytes | `-align-all-blocks=5` | 697, 704, 704 |
| Blocks after a jump to 32 | `-align-all-nofallthru-blocks=5` | 702, 711, 681 |
| Functions to 64 | `-align-all-functions=6` | 692, 700, 694 |
| Loops to 64 | `-align-loops=64` | 689, 710, 679 |

**Result.** Rejected. No setting is faster than an ordinary build. Aligning
blocks to 16 bytes ties it, and everything else is 2 to 6 percent slower. The
setting proposed at the end of experiment 8 lands at about 713, not 740.

Forcing alignment does make the three layouts agree with each other, but so
did the ordinary build this time, whose medians in the undisturbed sweep were
721, 720 and 724. The 9 percent swing in
experiment 8 was between builds made in different directories, and it did not
come back here. Collected over the day, ordinary builds of this same logic
have measured 683, about 700 to 720, about 720 three times over, 737 and 749.
The middle is about 720. The 749 that everything was being compared against
was the old code's lucky build, and 683 was an unlucky one.

**Decision.** No build flag is added. An ordinary release build is as fast as
any forced alignment we tried, and usually faster.

**What this leaves.** Two things worth keeping:

- `-align-all-functions=6` is still the right check when two builds of the
  same logic differ by less than 10 percent. It will not tell you how fast the
  lexer is, since it costs about 5 percent itself. It tells you whether the
  difference is in the code.
- A single build's number is good to about 5 percent either way. The size of
  the swing says the hot loop is dense with branches whose cost depends on
  their addresses. Making the loop less sensitive to that would be a change to
  the loop, which is candidate 1 below, not a flag.

## 10. Is the hand-written punctuation match still worth it? (2026-11-18): yes, kept

**Why ask.** Experiment 3 kept the hand-written match for a 9 percent gain,
and experiment 8 showed that placement alone can move a build by that much.
The match costs a second copy of the punctuation table, about 120 lines with
the search and the test that keep the two in step. If the gain was luck, that
is 120 lines and a duplicate we could drop.

**Hypothesis.** The gain is real. Taking the match out and searching only the
spellings that share a first byte, as experiment 1 did, should be at least 5
percent slower on every layout, with and without forced alignment. Less than
that, or a mixed result, would mean the match does not earn its place.

**How it was tested.** As in experiment 9: three copies of the crate that
differ only in folder names, each built plainly and with functions aligned to
64 bytes, each binary in its own build folder. Fastest pass of 7, three
rounds interleaved, the middle round shown. Every build printed the same
fingerprint.

| | Layout 1 | Layout 2 | Layout 3 |
| --- | --- | --- | --- |
| With the match, plain | 691 | 690 | 688 |
| Without it, plain | 585 | 587 | 592 |
| With the match, aligned | 708 | 713 | 693 |
| Without it, aligned | 627 | 641 | 633 |

**Result.** Confirmed. Without the match the lexer is 15 percent slower in a
plain build and 10 percent slower aligned, on all three layouts. That is more
than experiment 3 measured, and it is not placement.

**Decision.** The match stays, and so does the test that checks it against
the spelling list. Experiment 3 stands.

A note on method. The first attempt at this built every variant into one
shared build folder, and Cargo quietly reused the first binary for all of
them, because the copies had the same file names and older timestamps. The
giveaway was a build that finished in a hundredth of a second. Each variant
now gets its own build folder, and the binaries are checked to be different
before anything is timed.

## 11. Every case of the loop yields its kind and its end (2026-11-18): kept

**Why.** For reading, not for speed. The loop in `scan_all` set the scan
position from inside each case, and the three slow paths (numbers, quoted
literals, block comments) each needed three lines to hand the position to a
method and take it back. The loop was harder to read than what it does.

**Change.** Each case now evaluates to a pair, the token's kind and the
offset where it ends, and the position is advanced in one place after the
match. The slow-path methods take the token's start and return the same pair,
so each of them is one line in the loop. No rule changed.

**Hypothesis.** No measurable difference: the same decisions in the same
order. A loss of more than 2 percent under forced alignment would reject it.

**Result.** Measured with experiment 10, same method. "Shared" is the same
change with the two identical punctuation cases moved into one function.

| | Layout 1 | Layout 2 | Layout 3 |
| --- | --- | --- | --- |
| Before, plain | 691 | 690 | 688 |
| After, plain | 722 | 723 | 727 |
| After, shared, plain | 716 | 714 | 716 |
| Before, aligned | 708 | 713 | 693 |
| After, aligned | 754 | 748 | 753 |
| After, shared, aligned | 741 | 739 | 736 |

Faster, which was not the aim: about 5 percent plain and 7 percent aligned,
on every layout. We did not look for the cause. The likely one is that the
position now lives in a local for the whole loop and the scanner's own copy
is written only on the slow paths.

Sharing the punctuation case costs about 1.5 percent of that in all six
comparisons, so the two cases stay written out, with a comment saying why.

**Same output.** Fingerprint `b7778a2e5cd741fb` in every run; all 49 tests
pass.

**Decision.** Kept. The scanner is about 20 lines shorter and the loop reads
as a list of cases.

## 12. Does the comparison tool tell the truth? (2026-11-18): yes on both known cases

**Why.** Experiments 8 to 10 were measured by hand, and the hand work went
wrong three ways: placement read as a real effect, one binary reused for every
variant, and a busy machine dragging medians down. `compare-builds`, under
`tools/`, fixes the procedure in a program. Before trusting its verdicts we
gave it two comparisons whose answers we already knew.

**What it does.** It builds each version three ways, each in a folder of its
own: plainly, with every function in a different place (a flag that changes
nothing but the hash inside each symbol name), and with functions aligned to
64 bytes. It checks the binaries differ, runs them interleaved, takes each
one's best round, and gives a verdict on the first number the tool reports. A
difference counts only if all three builds show it and it is over 2 percent.

**Case 1: the same source against a copy of itself.** The right answer is no
difference. File by file, fastest pass, three rounds of 7 passes:

| Build | Copy | Working tree | Change |
| --- | --- | --- | --- |
| Plain | 785.5 | 779.9 | 0.7% slower |
| Reshuffled | 794.2 | 783.3 | 1.4% slower |
| Aligned | 737.1 | 743.9 | 0.9% faster |

Verdict printed: no difference that every build agrees on. That 1.4 percent is
where the 2 percent floor comes from.

**Case 2: a baseline with the hand-written punctuation match taken out.**
Experiment 10 measured that by hand as 13 to 18 percent slower.

| Build | Without the match | Working tree | Change |
| --- | --- | --- | --- |
| Plain | 646.1 | 781.0 | 20.9% faster |
| Reshuffled | 645.9 | 782.6 | 21.2% faster |
| Aligned | 648.8 | 732.6 | 12.9% faster |

Verdict printed: the candidate is faster in every build, by 12.9% to 21.2%.
The fingerprint was the same in all 18 runs of both cases.

**What else it showed.** Only the headline number is steady enough to judge.
In case 1, with identical source, the one-line-repeated figure differed by
13.7 percent in one build and the joined-buffer figure by 6.2 percent in
another. Those rows are printed, and nothing is concluded from them.

It also had the shared-folder mistake in it at first, in a new form: a
baseline taken from an older directory could have been handed the previous
baseline's binary. Its build folders for the baseline are now emptied at the
start of every run.

**Decision.** Use it for every comparison from here on. To be compared, a
layer's tool has to end with a RESULT line from the `measure` crate, so the
oldest commit this layer can be compared against is the one that adds it.

## 13. What the token reservation costs on Windows (2026-11-18)

On commit `3badc18`, a fresh token buffer requests one slot per three input
bytes plus 16. An 8 MiB block comment requests 2,796,218 slots before the
scanner knows that the entire file is one token. On this 64-bit build each
slot is 32 bytes, so the vector reports 85.33 MiB of capacity.

**What Windows charged.** The release probe filled and touched the source
before the first reading. It then called the public `lex_into`, reused the
same vector for `machine f() {}`, and dropped it while keeping the source
alive. At each boundary the child paused on stdin while a parent refreshed
its Windows process counters. Across five fresh processes on Windows 11
build 26300, the median private-byte increase for the comment was **85.50
MiB**. The median working-set increase was **8 KiB** in the first batch and
**12 KiB** in a second five-process batch. The small follow-up
file retained the commit. After dropping the vector the median private-byte
delta returned to zero. An unterminated comment retained the same capacity
in the caller's empty output vector after refusal. The CLI uses `lex` with
a fresh vector, so its buffer lifetime ends with the file operation.

**How often the estimate misses.** Lexing each V1 file separately with a
fresh buffer accepted all 19,442 files (26,441,134 bytes). The initial
estimate was too small on 2,650 files and at least twice the emitted count
on 319 nonempty token lists. Final unused capacity, including any growth,
was 1,568 bytes at the median, 19,264 at p95 and 48,576 at p99. The maximum
was 301,152 bytes: the initial guess on `samples/cli/simulation/game_of_life/main.omg`
was 9,516 slots for 9,621 tokens, so the buffer grew to 19,032. All 23 V2
parser accept fixtures also lexed. Their median unused capacity was 3,168
bytes and their maximum was 12,320 bytes. These are per-file counts, with
one file live at a time.

**Two bounded reservations.** In isolated builds the scanner used either
`min(estimate, 32768)` or a smaller initial count chosen to grow back to
approximately the original estimate by doubling. For the latter, when the
estimate exceeded 32,768 slots, we divided it by the smallest power of two
that brought it under the cap, rounding up. Both changed only the initial
reservation. Everything else used the production lexer.

| Input | Production token capacity | Flat cap | Scaled cap |
| --- | ---: | ---: | ---: |
| One 8 MiB block comment, one token | 85.33 MiB | 1.00 MiB | 0.67 MiB |
| 8 MiB of 64-byte comment lines, 262,144 tokens including whitespace | 85.33 MiB | 8.00 MiB | 10.67 MiB |
| 8 MiB of repeated short machines, 5,592,404 tokens | 170.67 MiB | 256.00 MiB | 170.67 MiB |
| Repeated V2 accept fixtures, 8.00 MiB and 3,446,976 tokens | 170.69 MiB | 128.00 MiB | 170.70 MiB |

On the repeated short machines, the flat cap grew through powers of two to
8,388,608 slots. Its peak working set above the pre-lex reading was 256.05
MiB, versus 170.72 MiB in production and 171.38 MiB with the scaled cap.
Matching the final capacity did not recover the time spent growing the
vector. On the repeated V2 fixtures, lexing took **18.3% longer** with the
flat cap and **31.9% longer** with the scaled cap. On repeated short machines
the increases were 48.9% and 25.9%. Each timing is the median within-round
change across six rotated rounds, four timed lexes per process after two
warmups, using fresh token storage and excluding destruction. A second copy
of the same baseline invocation differed by −0.8% and −1.1% respectively
in the median. All six stress inputs that lexed had matching kind-and-span
fingerprints across the three builds; the unterminated comment produced the
same error and span.

On the V1 corpus, the largest file was 97,788 bytes, just below the flat
cap's cutoff. The allocation request was identical for every individual file.
`compare-builds` (three rounds, five passes, three layouts) found no
file-by-file throughput difference above the 2% threshold across all layouts.
On the same sources joined into one buffer, the flat cap reduced throughput
by 37.1–39.3% across all three layouts. The V1 fingerprint matched in all
18 runs (`389ed15fa61d1f87`).

**Decision.** Keep the production policy during this investigation. A large
sparse file consumes Windows commit even when almost none of its token
storage becomes resident. A reusable buffer can keep that charge between
files. Both tested caps address the sparse case at a substantial cost on
large dense inputs. The next candidate is estimating density from tokens
already emitted on large inputs, tested on its own against the standalone
lexer. [Parser experiment 9](../01-parser/EXPERIMENTS.md#9-size-the-index-and-token-buffers-from-observed-work-2026-11-18)
combined density sampling with an indexed handoff. Its regression cannot
by itself tell us the cost of sampling alone.

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
| 7. Open relative to a directory | clearly cheaper opens | no difference |
| 8. Restyle and folders | no difference | no difference in the code; up to 9 percent from placement |
| 9. Alignment flags | every build at 740 MB/s | none faster than an ordinary build; most 2 to 6 percent slower |
| 10. Punctuation match, re-measured | at least 5 percent slower without it | 10 to 15 percent slower without it |
| 11. Cases yield kind and end | no difference | 5 to 7 percent faster |

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
