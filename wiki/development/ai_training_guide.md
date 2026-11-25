# Training AI models on Omega

Models do not follow a style guide—they converge on the statistical centroid of
their training corpus and whatever the reinforcement-learning verifier rewards.
In a proof-checked systems language, naive repository scraping, synthetic math
puzzles, and `compiles && tests_pass` RL produce a predictable failure mode: a
model that writes verbose academic theorem-prover boilerplate, invents foreign
OOP/FP wrappers, and weakens contracts to silence the checker.

This guide is an engineering contract for AI researchers curating pretraining
data, designing synthetic tasks, building RL verifiers, and evaluating
checkpoints on Omega. Its goal is to keep the model's weights anchored to real,
data-oriented systems Omega.

## The five non-negotiable rules

1. **Anchor taste in the Gold Set, not adjectives.** Point data filters,
   synthetic-data prompts, and pairwise preference judges at concrete reference
   files in this repository rather than generic "clean code" rubrics.
2. **Enforce a hard corpus boundary.** Never bulk-scrape `samples/` or git
   history. Over 93% of `.omg` files in `samples/` are micro-probe fact-matrix
   fragments, and parts of `source/` still carry pre-migration import syntax or
   internal task-tracker comments.
3. **Train on systems tasks, not textbook proof toys.** Keep the task mix
   dominated by parsers, codecs, state machines, memory layouts, OS boundaries,
   and contract-preserving repairs—not Peano arithmetic or LeetCode puzzles.
4. **Gate RL rewards against contract-weakening and cheats.** A solution that
   passes by loosening `requires`, weakening a type bound, or inserting an
   `assumed` binding must receive **zero reward**.
5. **Measure taste beside `pass@1`.** Run a fixed taste diagnostic on every
   checkpoint so idiom drift, comment bloat, and contract weakening cannot rot
   silently while capability scores climb.

---

## 1. What good Omega looks like

Omega is a proof-checked systems language whose programs are data-oriented state
machines. Before building a dataset or prompt template, align on how Omega
differs from mainstream languages (see the full table in
[language_guide.md](../language_guide/language_guide.md#if-you-know-another-language)):

- `machine` replaces functions, methods, and loops (a loop is a nested tail-recursive machine with a `terminates by` rank).
- `data` holds flat fields or sum `case`s, often carrying a `requires` invariant directly on the type.
- Zero-initialization (ZII) matters: the first `case` of a result sum is conventionally the failure case (`Invalid`, `Overflow`), so untouched storage never reads as success.
- Ownership and authority are explicit: `@T` / `@mutable T` borrows, `[linear]` values with explicit `drop(...)`, and `reaches` capability rows instead of ambient I/O.
- **Reasoning lives in the code (anchor-file source documentation), not in detached docs.** Separate external prose docs rot and decouple design reasoning from implementation. High-value Omega files put the engineering reasoning trace directly in the module's **anchor file**: stacked `mechanism` / `justification` / `provenance` header blocks (problem, constraints, memory layout, rejected dead ends), parsed lifecycle-ordered examples that the compiler still checks, `trailing_field` invariants on field declarations, `inline_why` notes only at non-obvious decision sites, `:Marker` cross-references, and honest `@Incomplete` / `@Robustness` / `@Speed` / `@Volatile` tags. Support files carry a one-sentence role header. Never confuse rich in-code reasoning traces with ceremonial Javadoc/line-narration slop.

### Gold Set (verified target distribution)

Use these maintained files as the reference distribution for style, module
organization, proof discipline, and documentation:

| Reference file | What it exemplifies |
| --- | --- |
| [`source/library/core/CompactBinary/compact_binary.omg`](../../source/library/core/CompactBinary/compact_binary.omg) | Core library codec: `terminates by remaining_steps`, nested state machines (`group`/`emit`/`last`/`more`), explicit `+Wrapping`/`-Wrapping`, reflection visitor contract (`requires machine Visit`), concise 2-line rationale comments. |
| [`samples/cli/systems/round_robin_jobs/main.omg`](../../samples/cli/systems/round_robin_jobs/main.omg) | Ownership and lifetimes in 91 lines: `[linear]` `Ticket` with explicit `drop(ticket)`, lifetime-carried borrow (`Job<'view>`), `terminates by`, zero demo boilerplate. |
| [`samples/cli/algorithms/dijkstra_binary_heap/main.omg`](../../samples/cli/algorithms/dijkstra_binary_heap/main.omg) + [`invalid_dijkstra_binary_heap/main.omg`](../../samples/cli/algorithms/invalid_dijkstra_binary_heap/main.omg) | Generic `Heap<T>` with a `machine Less` binder, flat CSR graph arrays, tight `requires` bounds, and a 36-line negative twin isolating `requires h.length >= 1`. |
| [`samples/cli/proofs/verified_sorted_insert/main.omg`](../../samples/cli/proofs/verified_sorted_insert/main.omg) + [`invalid_verified_sorted_insert/main.omg`](../../samples/cli/proofs/invalid_verified_sorted_insert/main.omg) | Pure predicate machine (`sorted`), `requires`/`ensures` interval contracts, `terminates by width -> Nat::Descending in 0..=16`, and a 63-line negative twin proving the contract rejects unsorted input. |
| [`samples/cli/scientific/dose_response_fit/domain/fit/fit.omg`](../../samples/cli/scientific/dose_response_fit/domain/fit/fit.omg) | Multi-module numerical package (`use domain\model\...`), crisp top-of-file algorithmic rationale, flat `data Moments` layout, clean state-machine loops. |
| [`samples/cli/systems/file_journal/main.omg`](../../samples/cli/systems/file_journal/main.omg) | Portable OS boundary (`FilesystemService`, explicit `reaches` capabilities) with linear `File` handles threaded through state transitions and explicitly closed. |
| [`samples/cli/collections/generic_ring_buffer/main.omg`](../../samples/cli/collections/generic_ring_buffer/main.omg) | Compact data invariant (`data Ring [copy] requires head < 8, tail < 8`), `@mutable` borrows, saturating arithmetic. *(Strip the trailing `[press Enter to close]` demo pause before training.)* |
| [`samples/cli/systems/wire_protocol/main.omg`](../../samples/cli/systems/wire_protocol/main.omg) | Wire schema with numbered fields (`#0`, `#1`), ZII-zero failure enum (`case Invalid; case Sound;`), composing with `CompactBinary`. |
| [`source/library/std/time.omg`](../../source/library/std/time.omg) *(Conditional Gold)* | Strongest API and rationale-header taste in the repo (ZII-zero `case Overflow`, normalization invariants, checked/saturating arithmetic). **Normalize before ingestion:** update its older `use *.omg` / `\|alias\|::` import syntax and strip `TASKS_TIME.md` references. |

### Anti-Pattern Set (do not imitate; penalize in curation and RL)

These real repo files and common synthetic failure modes show what poisons a
model's sense of Omega:

- **Manual loop unrolling to satisfy a checker:** [`samples/cli/simulation/game_of_life/main.omg`](../../samples/cli/simulation/game_of_life/main.omg) unrolls a 16-cell check into `at_1` … `at_15`. Exclude compiler-workaround contortions from training.
- **God-struct scratch fields:** [`samples/cli/simulation/bombe/main.omg`](../../samples/cli/simulation/bombe/main.omg) packs 55+ single-letter scratch fields (`l`, `m`, `r`, `e`, `i`, `x`, `j`, `d`, `k`, `t`, `c`, `b`, `v`) into `data Main` to work around a lowering limitation. Exclude.
- **Copy-pasted capability walls:** [`samples/gui/windowed_calculator/main.omg`](../../samples/gui/windowed_calculator/main.omg) repeats 20-line `reaches` lists across every trait method and mixes platform providers in one trait. Exclude until cleaned up.
- **Micro-probe fragments:** `samples/cli/facts/` (15,603 files) and `samples/cli/probes/` (1,391 files) are compiler test-matrix probes, not programs.
- **Academic proof-toy slop (synthetic):** Peano natural numbers, "prove `a + b == b + a`", or over-abstracted generic/monadic wrappers ported from Haskell/Lean/Rust.
- **Ceremonial doc slop (synthetic):** Javadoc-style parameter tables that restate the signature (`// @param x The x value`) or comments narrating the next line of code.
- **Contract-weakening cheats (RL):** Widening a bound, deleting a `requires`, or inserting an `assumed` binding to make the compiler accept broken code.

---

## 2. Corpus hygiene and repository manifest

Bulk-scraping this repository will contaminate the model with three separate
sources of noise: ~17,000 tiny compiler probes (~93% of `samples/`), retired
drafts/proposals/archive code, and internal development chatter (`TASKS_*.md`,
`OWNER_*.md`).

Use this manifest to separate clean training signal from quarantined material:

| Bucket | Repository paths | Policy |
| --- | --- | --- |
| **Clean Primary** | `wiki/spec/`, `wiki/language_guide/`, `source/library/core/CompactBinary/`, curated `samples/cli/{algorithms,collections,systems,text,audio,interpreters}/` | Include directly. Keep multi-file packages together in one training document/context so the model learns flat module organization, not just isolated functions. |
| **Conditional (Normalize First)** | `source/library/std/`, `source/subsystems/`, `samples/cli/scientific/` | Run the pre-ingestion filters below before including: verify current `use` syntax, strip `TASKS_*.md` / milestone references, and exclude workaround files. |
| **Eval-Only (Hold Out)** | Positive/negative twin pairs (`invalid_*` samples), sampled `samples/cli/facts/` | Hold out for verifier and soundness-boundary evaluation; never dump into pretraining bulk. |
| **Quarantine (Never Train)** | `wiki/drafts/`, `wiki/proposals/`, `_recycling/`, `samples/cli/facts/`, `samples/cli/probes/`, `TASKS*.md`, `OWNER_*.md`, git history of retired dialects | Hard-exclude from pretraining, mid-training, and SFT. |

### Automated pre-ingestion filters

Before admitting any `.omg` file (repository or synthetic) into a training split:

1. **Current dialect check:** Reject files using retired file-path imports (`use foo.omg as ...` or `|alias|::`) until migrated, or normalize them deterministically and re-verify compilation with the current compiler.
2. **Compiler verification:** Every executable training file must compile cleanly under the current Omega compiler. Never train on unexecuted pseudocode or broken snippets unless explicitly labeled as a negative example in a repair task.
3. **Strip demo & tracker noise:** Reject or strip interactive demo pauses (`[press Enter to close]`) and internal repo-process comments (`TASKS_*.md`, `D1-D7`, `rung 3`, `fire D`).
4. **Comment hygiene filter (reasoning traces vs. narration slop):** Do **not** use a naive comment-to-code line ratio—that will mistakenly strip high-value anchor-file headers (`mechanism` / `justification` / `provenance` blocks), which are some of the strongest reasoning-trace data in the corpus. Instead, filter out *ceremonial narration* (Javadoc parameter tables, comments that restate the next line of code) while preserving top-of-anchor design rationale, rejected-alternative notes, `trailing_field` invariants, `:Marker` links, and `@Incomplete` / `@Robustness` / `@Speed` / `@Volatile` honesty tags.

---

## 3. Task and synthetic-data design

If researchers generate synthetic problems using standard coding-benchmark
recipes, the dataset will collapse into 90% LeetCode and math proofs because
those are easiest to auto-grade. Actively enforce a systems-first curriculum.

### Target task mix

- **Real systems programs (~40%):** Binary codecs and wire formats, parsers and lexers, bounded ring buffers and fixed-capacity collections, state-machine protocols, memory extents/allocators, CLI utilities, and portable OS/filesystem services.
- **Contract-preserving repair & refactoring (~30%):** Given a program rejected by the lowered-form checker, fix the bug or supply the missing structural invariant *without* weakening the caller-visible `requires`/`ensures` contract.
- **Positive / negative twin authoring (~15%):** Given a data invariant or machine contract, produce both the valid program and the minimal failing twin that triggers the exact expected rejection (modeled on `dijkstra_binary_heap` + `invalid_dijkstra_binary_heap`). This teaches the model where the soundness boundary sits.
- **Multi-file module design (~15%):** Tasks that span a package boundary (`domain\model`, `domain\fit`, provider seams, explicit `reaches` capabilities) so the model learns clean decomposition instead of monolithic `main.omg` files.

### Avoiding translation slop

Translating Rust, C, or Lean corpora into Omega with a frontier model imports
foreign taste: Rust borrow-checker workarounds, C null-sentinel habits, OOP
wrapper types, and Lean tactic-style proof gymnastics. If you seed tasks from
other languages:

- Translate the **problem specification**, not the source code.
- Generate Omega solutions from scratch against the Gold Set and
  [wiki/spec/](../spec/), then filter through the verifier and taste judge.

---

## 4. Verifier and RL reward design

RL with a binary `compiles && tests_pass` reward is the fastest way to breed
slop. A policy optimizing only for compiler acceptance will discover every
shortcut that makes the checker stop complaining.

### Hard zero-reward gates

Any candidate rollout gets **reward = 0** (regardless of test output) if it
violates any of the following:

1. **Check provider contracts.** Reject repairs that assume provider
   conformance or introduce [native borrows](../spec/resources/native_borrows.md)
   to silence a proof or borrow error.

2. **No contract weakening:** On repair and implementation tasks where a
   signature or `requires`/`ensures` contract is specified, verify AST/signature
   preservation. Loosening a precondition, widening a bound, or dropping a
   termination rank is an immediate 0.
3. **Exact rejection on negative twins:** A negative twin only earns reward if
   the compiler rejects it with the *intended* diagnostic category/reason—not an
   unrelated syntax or name-resolution error.
4. **Native and Psi agreement:** For executable tasks, run both the Psi
   interpreter and the compiled native binary and compare exact exit code,
   stdout bytes, and stderr bytes (see
   [comparing native and Psi tests](../spec/build/testing.md#comparing-native-and-psi-tests)).
5. **Proof-work and resource ceilings:** Enforce the compiler's proof-work
   budget so the policy cannot brute-force proofs with bloated, unrolled code.

### Taste shaping (without breeding comment bloat)

Do **not** use generic LLM-as-judge prompts ("rate code quality, readability,
and documentation from 1–10"). Generic judges reliably reward verbose comments,
defensive boilerplate, and academic over-abstraction.

Instead, use **Gold-Set-anchored pairwise preference** (or Best-of-N filtering):

- Show the judge 2–3 Gold Set excerpts alongside 1–2 Anti-Pattern excerpts as
  explicit calibration anchors.
- Score candidates on four concrete axes:
  1. **Directness:** Flat `data` + `machine` state transitions vs. invented wrapper layers or unrolled workarounds.
  2. **Natural invariants:** Facts carried by data invariants, domains, and transition guards rather than redundant runtime checks or scratch fields.
  3. **Failure safety:** ZII-zero failure cases and explicit sum returns rather than magic sentinel integers.
  4. **In-code reasoning traces over doc slop:** Reward anchor-file headers (`mechanism` / `justification` / `provenance`), `trailing_field` invariants, and honest constraint tags where the code alone cannot show the *why*; penalize syntax narration, Javadoc parameter tables, and detached prose boilerplate.

---

## 5. Checkpoint taste evaluation

Researchers optimize what appears on their dashboard. Alongside `pass@1` and
compile rate, run a small, fixed **taste & smell suite** (~30–50 prompts held
out from training) on every checkpoint and track four regression metrics:

1. **Idiom drift rate:** On open-ended systems prompts, how often does the
   model invent foreign OOP/FP wrappers, god-struct scratch fields, or unrolled
   loops instead of idiomatic state machines?
2. **Cheat / weakening rate:** On hard checker-rejection prompts, how often
   does the model weaken `requires`, drop a bound, or reach for `assumed`?
3. **Comment slop ratio:** Fraction of comments that restate types/syntax or
   mimic Javadoc/academic prose rather than stating invariants or design intent.
4. **Dialect contamination rate:** Frequency of hallucinated syntax from Rust,
   C, Lean, or retired Omega drafts (`use *.omg`, `|alias|::`).

If `pass@1` rises while any of these four metrics regresses, treat the
checkpoint as a regression and audit the latest synthetic data batch or RL
reward weights before scaling up.
