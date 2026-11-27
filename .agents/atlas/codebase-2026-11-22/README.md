# Codebase atlas, 2026-11-22

This folder is a written record of what every line of the shipping compiler does, made by reading it once at
commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86` (main on 2026-11-22). It covers the Rust compiler, CLI,
interpreter and package manager under `source-rust/`, the repository tooling under `tools/`, and the
Omega-written compiler, application and bundled library under `source/`. Use it to find out what a crate or
file does, how it decides, and where the same work is written more than once, without re-reading the code.
It describes one commit and goes stale as the code moves, so check a claim against the code before acting
on it.

Track current repairs, earlier fixes and outstanding claims in the [Atlas repair queue](triage/README.md).

For the current CLI contract, see [standalone interpretation](../../../wiki/spec/build/embedding.md#standalone-interpretation).
The run comparisons recorded here used the [November 22 driver](https://github.com/CathedralOS/Omega/tree/4f3c255d05599c11b1ea8ac668635f15f2a6eb86/source-rust/application/src).

## Verified follow-ups

The counts below describe the November 22 survey. The area digests also record fixes verified after that revision.

- Replay three fixes for generated infinity text, changed-program selection and repeated build output directories. [Commands and timings](reproductions.md)
- Source stage 05 can pass a final guarded arm to the ordinary coverage proof. [Guarded matches and transitions](digests/03-source-stage-05.md#verified-follow-ups)
- Native plans preserve memory fences when reopened, and editing an existing spill slot invalidates its cached digest. [Native representations](digests/12-native-representations.md#verified-follow-ups)

## How to use it

- **Start from the crate map.** `crate-map/README.md` follows a program through the compiler end to end and
  says which crate does each step. `crate-map/<area>.md` places each crate of one area in the pipeline.
- **Then read the area's digest.** `digests/<area>.md` condenses every record for one area. Each digest has
  the same ten sections: what the area does, how it decides, the same job in more than one place, fixed
  lists of shapes, accepts when it does not know, silent drops, unused or unwired, stated limits,
  disagreements between records, and what the digest writer checked in the code.
- **Go to a record for detail.** `units.tsv` lists which unit read which file and line range; the record for
  unit 7 is `records/unit_007.md`. Ranges a first reader did not finish were read in a second pass, listed
  in `second_pass_ranges.tsv`, with records named `records/unit_NNN_gap_MMM.md` (NNN the original unit).
- **Read a boundary note for one stretch of the pipeline.** `boundaries/` holds three notes on what each
  stage hands the next, what the receiver trusts, what is checked, and whether that check is independent of
  the code that produced the value.
- **Open a cited line as it was.** Citations are `file:line` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.
  Use `git show 4f3c255d05599c11b1ea8ac668635f15f2a6eb86:<path>` when the file has changed since.

| Area | Covers | Units | Records merged | Checked: matched / partly / wrong |
| --- | --- | --- | --- | --- |
| `01-cli-foundation` | The `omega` binary and its commands, shared foundation crates, the public compiler entry | 1, 2, 5, 6, 50 | 6 | 16 / 3 / 1 |
| `02-source-stages-00-04` | Source stages 00 to 04: tokens, syntax trees, name resolution, typing, preparation | 51 to 58 | 12 | 12 / 2 / 2 |
| `03-source-stage-05` | Source stage 05: prepared typed trees to the lowered program | 58 to 63 | 6 | 13 / 2 / 3 |
| `04-source-stages-06-07` | Source stages 06 and 07: the lowered-form checker and Psi publication | 63 to 69 | 11 | 6 / 8 / 3 |
| `05-source-representations` | The forms the source stages exchange | 69 to 75 | 7 | 15 / 3 / 0 |
| `06-source-driver-build` | Source driver and compile-time evaluation; Build planning, execution and settings | 2 to 5, 50, 51, 69, 75 | 9 | 12 / 2 / 2 |
| `07-psi-semantics` | Psi semantics: storage flow, descriptors, replay, proofs, continuation checks | 41 to 50 | 17 | 11 / 2 / 3 |
| `08-psi-program` | The portable Psi program, artifact encoding, identities, interfaces | 37 to 41, 50 | 7 | 13 / 3 / 2 |
| `09-native-stages-00-03` | Native stages 00 to 03: admitted inputs, target binding, abstract operations | 6 to 8 | 3 | 13 / 4 / 3 |
| `10-native-stages-04-10` | Native stages 04 to 10: target operations, instruction selection, register allocation, framing | 8 to 16 | 10 | 12 / 4 / 2 |
| `11-native-stages-11-15` | Native stages 11 to 15: encoding, layout, placement, objects, images | 16 to 24 | 18 | 12 / 2 / 1 |
| `12-native-representations` | The forms the native stages exchange | 24 to 33 | 16 | 11 / 1 / 2 |
| `13-native-targets-driver` | Native targets and ABIs, the native driver and its admission crates | 6, 24, 33 to 37 | 7 | 12 / 4 / 2 |
| `14-interpreter` | The Psi interpreter, its host services and memory | 76 to 81 | 9 | 13 / 4 / 2 |
| `15-package-manager` | Package acquisition, graphs, review and update | 75, 81 to 84 | 5 | 15 / 5 / 2 |
| `16-tools` | Repository tooling: `xtask` and the benchmarks | 89, 90 | 2 | 15 / 4 / 1 |
| `17-omega-source` | The Omega-written compiler and application, and the bundled library packages | 84 to 89 | 6 | 15 / 3 / 3 |

A record that spans two areas is counted once in each area's digest, using only its part for that area.

| Boundary note | Stretch | Handoffs | Claims opened in the code / taken from a record |
| --- | --- | --- | --- |
| `boundaries/front_half.md` | Source stages 00 to 07 and what stage 07 publishes (areas 02 to 06) | 8 | 108 / 15 |
| `boundaries/psi_and_native.md` | Published Psi through native stage 15's image (areas 07 to 13) | 17 | 170 / 41 |
| `boundaries/outputs_and_drivers.md` | CLI, Build, interpreter, package manager, tools, Omega-written compiler (areas 01, 06, 14 to 17) | 21 | 185 / 66 |

## How far to trust it

The records say what readers saw. They are not a list of proven defects.

- **Tested claims.** Each digest writer opened at least eight of its area's claims in the code, choosing the
  ones most likely to mislead: 306 in all. 216 matched as stated, 56 were partly right and 34 were wrong.
  Each digest corrects the wrong ones it found, in its sections 9 and 10.
- **What the wrong ones were.** The digest writers' reports name these kinds: "unused" claims for functions
  that do have callers (for example `save_shared_psi`, `source_snapshot::capture`,
  `retained_package_review_numeric_requirements`, `stage_aggregate_layout`); wrong counts and wrong line
  numbers, many of them in the crate map (stage sizes, dependency counts, test counts, the `OMGTRO` object
  format version); a condition read backwards (`require_body_completion` in the Omega-written typer); and
  "same job" pairs that do different jobs. Treat every "unused", "same job" and "only caller" entry as a
  lead to confirm.
- **"Accepts when it does not know".** Readers classed each such place as `stated` (a comment at the site
  gives the reason), `traced` (the reader followed the callers) or `open` (no reason given and not traced).
  Digest writers closed some `open` entries by finding the refusal elsewhere (for example the
  `static_reach.rs:311-313` and `external_providers.rs:140-144` entries in digest 04). An `open` entry is a
  place to examine, not a shown hole.
- **Words to keep.** "suspected", "inferred" and "not traced" mean the reader did not read the other site or
  did not follow the value to its end. Do not quote such a claim as established.
- **Counts are of places cited, not of defects.** A decision keyed on a name can be correct when a check on
  who owns the declaration stands in front of it.

## What it found, in numbers

These are counts of list entries in the 17 digests, made by a script over the digest files. One entry may
cite several places, and the counts measure what was written down, not how many defects exist.

| Digest section | Entries |
| --- | --- |
| 3. The same job in more than one place | 481, plus 8 paragraph groups in digest 10 |
| 4. Fixed lists of shapes | 388, plus 10 paragraph groups in digest 12 |
| 5. Accepts when it does not know | 969 |
| 6. Silent drops | 252 |
| 7. Unused or unwired | 244 |

Of the 969 accept-when-unknown entries, 612 carry the class word `open`, 295 `stated` and 241 `traced`; an
entry that names two classes (for example "stated for the limit, open beyond it") counts under both. Digest
12 has the most entries in section 5 (96), then 02 (87), 06 (81) and 08 (78).

Decisions keyed on spelling (a name, a display path, a string compare or `format!` text) are counted per
crate in each digest's section 2, but not in one form, so they are not summed here. Digests that give
totals include area 02 (lexer 3, parser 20, resolution 45, typing 30, preparation 2), area 15
(package-manager about 23, source-acquisition about 9, review-evidence about 11), area 16 (xtask about 37,
omega-bench about 11) and area 17 (97 lines in 21 files call `spans_equal` or `span_spells`).

Findings that the digests and boundary notes opened in the code and that a reader new to the code should
see first:

- The proof-certificate writer emits rule tags 28 to 32 and primitive judgments 4 and 5, which the reader
  refuses (`psi/artifact/callable-encoding/src/certificates.rs:219-362` against `:453-676`); evidence seals
  in a `PSIART` artifact go through the same reader. Whether any program produces those rules was not
  traced (digest 08, unit 037).
- At the survey revision, the CLI could report agreement when the native and Psi runs
  printed different output. [#5188](https://github.com/CathedralOS/Omega/pull/5188) fixed the CLI
  comparison for single runs and input cases. That check required both stdout and stderr to match byte for byte.
  The separate [Build test comparison](../../../wiki/spec/build/testing.md#comparing-native-and-psi-tests)
  was specified in [#5187](https://github.com/CathedralOS/Omega/pull/5187).
- The compiler entry accepts its own output with `HostProducerTrust::accept("current compiler invocation")`
  on every compilation (`compiler.rs:103-109`), and the Build program reaches the interpreter with no
  separate authorization step (`build/evaluation/activation.rs:405-410`) (`boundaries/outputs_and_drivers.md`).
  Both match the stated design: AGENTS.md "Verify at trust boundaries" says default compilation does not
  re-verify its own stage output, and the ratified `psi-execution-admission` decision
  (`wiki/spec/terminal-psi/product.md#publication-and-separate-consumption`) attaches trust to the producing
  process. They are recorded here as trust points, not as holes.
- Digest 08 notes that the Psi wire writes format marker 290 (`psi/artifact/wire/src/semantic_module.rs:4205`)
  while the encoding page says 291. At this commit that was planned work, not a stale page: `TASKS.md` listed
  writing marker 291 as open, and CathedralOS/Omega#5132 later moved the code to 291.
- Five source handoffs, 00 to 01 through 04 to 05, have no check independent of the producing code
  (`boundaries/front_half.md`).
- In the Omega-written checker, `proves` drops goals once 128 are pending and then reports the expression
  proved (`source/subsystems/compiler/source_to_psi/pipeline/typed_trees_to_checked_trees/facts.omg:268-288,341-343`,
  which digest 17 writes as `check/facts.omg`); whether a program reaches that depth was not traced
  (digest 17).

Paths above are shortened from `source-rust/subsystems/compiler/` (`psi/...`, `build/...`, `compiler.rs`)
and `source-rust/` (`application/...`).

## How it was made

1. **Units.** `plan.py` assigned the 3,768 files (every `.rs` under `source-rust/` and `tools/`, every
   `.omg` under `source/`) to 218 crates and 17 areas, and cut them in path order into 90 units of at most
   22,000 lines: 1,834,954 lines in all. A longer file was split by line range; a unit may hold files of
   two or more adjacent areas.
2. **Crate map.** One reader per area wrote `crate-map/<area>.md` from `crate_map_brief.md`, and
   `crate-map/README.md` joins them.
3. **First pass.** One reader per unit (Claude Sonnet, the Agent tool's `sonnet` model) read every line and
   wrote a record following `reader_brief.md`: purpose, shape, inputs and outputs, how it decides, cases by
   shape, the same job elsewhere, unused code with search counts, stated limits, fail-open places with a
   class, then coverage, five things a maintainer most needs to know, and what the reader was unsure about.
   At most 12 ran at once. All 90 wrote a record on the first attempt; none needed a restart or a message to
   continue.
4. **Second pass.** 19 first readers (units 2, 15, 17, 18, 20, 22, 27, 28, 31, 40, 42, 43, 44, 52, 57, 66,
   69, 76, 79) reported ranges they had not read or had seen only as a grep outline: 164,623 lines. Those
   were cut, within each original unit and in path order, into 36 pieces of at most 6,000 lines
   (`units/gap_001.tsv` to `units/gap_036.tsv`), and one more `sonnet` reader took each piece with the same
   brief. All 36 wrote a record and reported no unread range. In all, 126 records.
5. **Digests.** 17 writers (Claude Opus, the `opus` model), at most 6 at once, each read one area's crate
   map and records, merged them, corrected them, and opened claims in the code (`digest_brief.md`).
6. **Boundary notes.** 3 `opus` writers, after the digests, each wrote one note from the digests, records
   and code, marking every claim as opened in the code or taken from a record.
7. **This README** was written by the coordinating session (Claude Opus 5.5) from the readers' and writers'
   reports, the digest files and a script count of their entries.

Nothing was built, compiled or run.

## Known gaps

- **Records missing:** none. All 90 first-pass and 36 second-pass records exist and are not empty.
- **Ranges never read:** none reported after the second pass. Lines a reader said it read were not
  independently confirmed as read, beyond the 306 tested claims.
- **Read at lower depth, by the readers' own account:** test bodies summarised by name and subject rather
  than assertion by assertion in units 12, 46 and 90; small files compressed in unit 48; the serde snapshot
  code in `evidence_and_inspection.rs` lines 2186-6192 read "at lower attention" in unit 74.
- **Searches not run or incomplete:** unit 18 ran none of the item-7 caller counts; unit 76 and
  `unit_069_gap_033` ran no greps; unit 14 wrote its record before some greps, so the `choose_spill_victims`
  and `classify_pressure_recovery` counts were not rechecked; unit 75's `inspect` and `check` caller counts
  came from a 20-line-capped search; units 26, 46 and 48 grepped some pub functions only in groups.
- **Reported line totals that do not match the unit:** unit 13 reported 21,998 lines (unit holds 21,398),
  unit 35 25,052 (21,875), unit 65 21,832 (21,793), unit 74 21,684 (21,663); unit 81 said it read 78
  package-manager files where the unit lists 83 (digest 15 found the reading complete). Some second-pass
  readers read past their piece into the next one (gaps 16, 30, 31), so a few ranges have two records.
- **Digest reading:** the `07-psi-semantics` writer, short of room, read only part of `unit_049` (one file's
  section, its fail-open entries and its summary sections) and saw the rest as grep lines.
- **Area labels:** `units.tsv` assigns two compiler test files, `compiler/tests/final_stage_validators.rs`
  and `compiler/tests/support/schema_customer_source.rs`, to `15-package-manager`; they belong to the
  compiler (digests 06 and 15).
- **Stray directory:** the unit 67 reader made an empty directory under `source-rust/.agents` with a wrong
  relative path and removed it; it held no files and the checkout shows no other change.
- **Not in scope:** the separate `omega-rust-v2/` workspace, `bootstrap/`, tests and samples outside the read
  trees (for example `tests/`, `samples/`, `tests/bootstrap/`), non-`.rs` files under `source-rust/` and
  `tools/` other than what the readers opened while checking, and the wiki and specification.

## Layout

- `README.md`: this file.
- `plan.py`: the script that cut crates, areas and units.
- `crates.tsv`, `areas.tsv`, `units.tsv`: crate, area and unit tables for the first pass.
- `crate_map_brief.md`, `reader_brief.md`, `digest_brief.md`, `coordinator_brief.md`: the instructions each
  kind of agent followed.
- `crate-map/`: `README.md` plus one map per area.
- `units/`: `unit_001.tsv` to `unit_090.tsv` (first pass) and `gap_001.tsv` to `gap_036.tsv` (second pass).
- `second_pass_ranges.tsv`: pass number, original unit, path, first line, last line, line count.
- `records/`: 90 first-pass and 36 second-pass records.
- `digests/`: 17 area digests.
- `boundaries/`: `front_half.md`, `psi_and_native.md`, `outputs_and_drivers.md`.
