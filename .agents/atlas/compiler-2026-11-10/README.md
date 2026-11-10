# Compiler atlas, 2026-11-10

This folder is a written record of what every line of the Rust compiler does, made by reading all of
`omega-rust/` once at commit `457adf4a11f`. Use it to find out what a file or crate does, how it decides,
and where the same work is written more than once, without re-reading the code yourself. It describes one
commit and goes stale as the code moves, so check a claim against the code before acting on it.

## How to use it

- **Start from a digest.** `digests/area_NN.md` condenses one part of the compiler (the table below says
  which). Each digest has the same sections: what the area does, how it decides, the same job in more than
  one place, fixed lists of shapes, accepts when it does not know, silent drops, unused or unwired, stated
  limits, disagreements between records, and what the digest writer checked in the code.
- **Go to a record for detail.** `units.tsv` lists which unit read which file and line range. The record
  for unit 57 is `records/unit_057.md`. Thirteen units ran out of room; the ranges they left were read in a
  second pass, listed in `second_pass_ranges.tsv`, with records named `records/unit_NNN_gap_MMM.md`.
- **Open a cited line as it was.** Citations are `file:line` at `457adf4a11f`. Use
  `git show 457adf4a11f:<path>` when the file has changed since. In the records, `<checkout>` stands for the
  repository root at that commit.

| Digest | Covers | Units |
| --- | --- | --- |
| `area_01` | Command line, build system, compiler coordination, provider realization | 1, 12, 13, 23, 24 |
| `area_02` | Back end: native artifacts, calling conventions, executable images | 2 to 6 |
| `area_03` | Back end: instruction sets, machine emission, plans, register environment | 7 to 11 |
| `area_04` | Omega pipeline stages 00 to 09 | 14 to 22 |
| `area_05` | Omega representations and foundation | 25 to 31 |
| `area_06` | Terminal runtime and packages | 32 to 37 |
| `area_07` | Psi build-time evaluation and stages 00 to 04 | 38 to 45 |
| `area_08` | Psi stage 05: borrow ledger and the checks | 46 to 55 |
| `area_09` | Psi stage 05: execution plans, flow, published facts, values | 56 to 65 |
| `area_10` | Psi stages 06 to 08 | 66 to 72 |
| `area_11` | Proof, proof admission, psi-compiler, checked trees | 73 to 78 |
| `area_12` | Psi representations: syntax, resolved trees, Terminal Psi codec and semantics, typed trees | 79 to 87 |
| `area_13` | Validation, first half | 88 to 96 |
| `area_14` | Validation, second half, and terminal installation | 97 to 105 |
| `area_15` | Psi foundation and the two interpreters | 106 to 113 |

## How far to trust it

The records say what readers saw. They are not a list of proven defects.

- **Tested claims.** Each digest writer opened at least eight of its area's claims in the code: 401 in all.
  329 matched as stated, 38 were partly right, 33 were wrong and 1 could not be checked. Each digest corrects
  the wrong ones it found, under "Disagreements" and "What I checked myself".
- **What the wrong ones were.** Ten said nothing calls a function when something does. Twelve were a wrong
  count or a wrong file or line. Five said two pieces of code do the same job when they do different jobs.
  Four feared an accepting path that is in fact closed. Two were other mistakes. So treat every "unused" and
  "same job" entry as a lead to confirm.
- **"Accepts when it does not know".** Readers classed each such place as `stated` (a comment at the site
  gives the reason), `traced` (the reader followed the callers) or `open` (no reason given and not traced).
  No reader showed a wrongly accepted program reaching an `open` place. They are places to examine.
- **Words to keep.** "suspected", "inferred" and "not traced" in a record mean the reader did not read the
  other site or did not follow the value to its end. Do not quote such a claim as established.
- **Counts are of places cited, not of defects.** A decision keyed on a name can be correct when a check on
  who owns the declaration stands in front of it, and many are guarded that way.

## What it found, in numbers

The digests' per-crate counts add up to:

- **2,004 places where a decision is keyed on a name, a printed label or compiler-built text** instead of an
  identity. Validation has 508, Psi stage 05 has 227, stage 02 has 155, typed trees 127, stage 06 has 91,
  build-time evaluation 65, provider planning 61.
- **930 places that drop something silently**: a step that cannot plan, resolve or lower something produces
  no row and no diagnostic. Validation has 143, Psi stage 05 has 129, typed trees 61, stage 06 has 56.

The digests also record, per area, the jobs written more than once, the ordered lists of special shapes
that decide acceptance, the checks that are a second hand-written copy of what they check, and the public
code with no production caller.

## How it was made

1. **Units.** The 2,211,695 lines of Rust in the 126 crates under `omega-rust/` were cut into 113 units of
   at most 22,000 lines, in path order. Files longer than that were split by line range.
2. **First pass.** One reader per unit (Claude Sonnet 5.5, high effort) read every line and wrote a record
   following `reader_brief.md`: purpose, shape, inputs and outputs, how it decides, cases by shape, the same
   job elsewhere, unused code with search counts, stated limits, accepting paths, then coverage, five things
   a maintainer must know, and what the reader was unsure about. Readers were told to record, not to judge
   or recommend.
3. **Second pass.** Thirteen readers reported ranges they had not read, 133,176 lines in all. Thirty-one
   more readers each took at most 6,000 of those lines.
4. **Digests.** Fifteen writers (Claude Fable 5.2) each read every record of one area, merged them, and
   opened claims in the code as described above.

Nothing was built, compiled or run. A pilot on one unit compared Sonnet 5.5 at high effort with Opus 5.5 at
medium: accuracy at the cited lines was the same, Sonnet recorded more and finished sooner, and Opus judged
better which accepting paths were real risks. The brief was changed after the pilot to ask for a search of
outside users of anything documented as shared, and for the `stated` / `traced` / `open` classes.

## Known gaps

- 59 of the 2,147 file rows are not named in their unit's record. All are in the package and review-evidence
  crates, 41 of them in unit 36. The reader may have described them as a group by folder; this was not
  confirmed.
- Unit 12's reader reported 106 fewer lines than its unit holds. Its record does cover the 106-line file.
- Only `omega-rust/` was read. The Omega-written compiler under `source/`, the bundled library, the
  bootstrap chain, the tests, the samples and the wiki were not.
- Lines inside a unit's range that a reader said it read were not independently confirmed as read, beyond
  the tested claims above.

## Layout

- `README.md`: this file.
- `reader_brief.md`: the instructions every reader followed.
- `units.tsv`: unit, path, first line, last line, line count, for the first pass.
- `second_pass_ranges.tsv`: second-pass number, original unit, path, first line, last line, line count.
- `records/`: 113 first-pass records and 31 second-pass records.
- `digests/`: 15 area digests.
- `boundaries/`: three notes written after the digests, one per stretch of the
  pipeline, on what each stage hands the next, what is trusted, what is checked
  and whether the check is independent. Each claim is marked as opened in the
  code or taken from a record. They correct two digest claims: PE output is
  partly re-read, and program-entry binding works for hosted targets.
