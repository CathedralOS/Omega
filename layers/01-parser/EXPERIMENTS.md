# L1 Parser: speed

How fast the parser is, where its time goes, and every attempt to make it
faster, kept or not.

## How it is measured

    cargo run --release -p omega-parser --example parse_throughput -- --files

**Workload.** There is no large body of V2 source. V1's source tree is in
V1's syntax, which this grammar refuses on purpose. So the workload is this
crate's 22 accept fixtures laid end to end, 12,747 bytes, lexed and parsed
5,265 times: 64 MB in all. That source is denser than real code, 2.3 bytes
per token against about 5 in V1's source, so the number to carry forward is
time per token; MB/s on this workload understates the parser.

**Two shapes of the same bytes.** `--files` parses them as 5,265 small files,
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

Commit `ca8eec8`.

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

What my guess got wrong in experiment 1: I priced the classification at
nothing, because it is "one pass". It was the most expensive pass in the
parser. I also guessed the wrong cost inside the grammar; it was the spelling
comparisons, not the token reads.

## Candidates, not tried

| # | Idea | Why it might help | What the findings above say |
| --- | --- | --- | --- |
| A | Ask about spellings less often: test the cheap thing first. A prefix word (`move`, `suspend`, `block`) needs a word after it, so check that before comparing three spellings; in the binary-operator test, look for punctuation before comparing with `in`. | Up to the 29 ms that spelling comparisons cost, though only part of it is avoidable. | Supported: this is the one cost the measurements isolated. |
| B | Return a small error. Every grammar function returns `Result<_, ParseError>`, 40 bytes, through memory. Keeping the error in the parser and returning a marker would let results travel in registers. | Every call in the grammar pays for it. | Unknown; nothing measured yet. |
| C | Stop inserting into the event list. `open_before` shifts the events of the child being wrapped; each binary operator, call, field access, cast and statement does one. For statements the wrapper can simply be opened first. | A call into `memmove` per wrapped node. | Unknown. |
| D | Have the lexer hand over what the parser's setup pass computes: the list of meaningful tokens, and for each word which keyword it is spelled like. | The lexer already has each word's bytes in hand and knows which tokens are trivia. Experiment 1 shows the grammar would run 27% faster if keyword identity were free. | Supported, but the cost moves to the lexer and has to be measured there. Needs the lexer to be free to change. |
| E | Check the nesting counter less often. | It is incremented and decremented in every prefix, type and pattern. | Probably small. |
