# L1 Parser: speed

The parser has been measured once and not yet tuned. This file holds the
baseline and what is known about where its time goes, so that tuning starts
from a number and a list, as the lexer's did.

## Workload

There is no large body of V2 source. V1's source tree is in V1's syntax,
which this grammar refuses on purpose (arms without commas, bare `<` generic
calls). So the workload is this crate's own accept fixtures, laid end to end
and repeated to 64 MB: `cargo run --release -p omega-parser --example
parse_throughput`.

That source is denser than real code: 2.3 bytes per token, short names, few
comments. V1's source averaged about 5 bytes per token. Per-token cost is
therefore the number to carry forward; MB/s on this workload understates what
the parser would show on ordinary source.

Machine: Ryzen 9 7950X, one thread. Windows real-time protection on.

## Baseline, 2026-11-18

| Step | Time for 64 MB | Rate | Per token |
| --- | --- | --- | --- |
| Lex | 139 to 142 ms | 452 to 461 MB/s | 5.1 ns per token (27.3 M tokens) |
| Parse | 154 to 167 ms | 383 to 416 MB/s | 8.9 to 9.7 ns per meaningful token (17.3 M) |
| Both | 293 to 309 ms | 207 to 219 MB/s | |

The tree is 39.1 M events for 17.3 M meaningful tokens, 2.26 events per
token. At one byte per event plus a four-byte index per token, the tree
takes 1.6 bytes per byte of this source.

Two runs, each the best of seven rounds, with another session compiling in the same
checkout at the time. They differ by 8 percent; no proper A/A noise
measurement has been taken.

## What this says

- Parsing costs about as much as lexing on the same bytes. Before this
  measurement the lexer was the only number, and it was easy to assume the
  parser would be several times slower. It is not.
- One thread parses about 110 million tokens a second. Nothing here is
  parallel yet: files are independent, so the lexer's result (about 15 times
  faster on 32 threads) should carry over, but that has not been measured for
  the parser.
- It is not known how far this is from what the hardware allows. No floor has
  been measured. The obvious floor is a loop that only walks the meaningful
  tokens and writes one event each, which would show how much of the 9 ns
  is the grammar and how much is the cursor.

## Candidates, not tried

Ordered by how much I expect each to matter. These are guesses until
measured; the lexer's ledger shows my guesses run high.

| # | Idea | Why it might help |
| --- | --- | --- |
| 1 | A compact array built once per file: one byte of kind per meaningful token, plus a small number for each word the grammar tests for. | Every `at_keyword` today reads the 32-byte token, then reads the word's bytes out of the source and compares them. A statement start runs about ten of those. With a keyword number per token each becomes one byte compare, and the source is never touched during parsing. |
| 2 | Stop inserting into the event list. `open_before` shifts every event after the insertion point; each binary operator, call, field access and cast does one. | A link ("my parent starts N events back") written in place and resolved in one pass at the end removes the shifting. It matters most on long expressions. |
| 3 | Build the meaningful-token index during lexing instead of in a second pass over all tokens. | The parser's first act is a pass over 27 M tokens to drop the 10 M that are whitespace. The lexer already knows which they are. |
| 4 | Check the nesting counter less often. | It is incremented and decremented in every prefix, type and pattern. Probably small. |

## Ledger

No experiments yet.
