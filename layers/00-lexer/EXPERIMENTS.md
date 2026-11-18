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
