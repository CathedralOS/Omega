# Atlas unit_002: Delta implementation slice + Epsilon checking/docs

Checkout read at HEAD (later than digest commit `457adf4a11f`). Claims marked **read** unless noted **grepped** or **inferred**.

---

## `bootstrap/3_delta/implementation/checking/syntax/` (README + grammars + parse + nodes + storage)

### Purpose
Documents and implements Delta’s first source passes after lexical admission: balanced parse into retained syntax trees, grammatical-role checking with explicit worklists, and a byte ledger for syntax allocations (`README.md:1-7`, **read**). Does not resolve types or emit output (`README.md:4-6`).

### Shape
- `syntax_node` / accessors: `nodes.gamma:4-18`
- `parse_source_syntax` / `parse_syntax_nodes`: `parse.gamma:3-35`
- `validate_program_syntax` / `grammar_check_role`: `grammar.gamma:3-25`
- Worklist: `grammar_pending`, `grammar_sequence`, `grammar_run`: `work.gamma:5-32`
- Program/decl hooks: `grammar_program`, `grammar_declaration`, etc.: `declarations.gamma:1-95`
- Expressions: `grammar_expression`, `grammar_application`, etc.: `expressions.gamma:1-89`
- Patterns: `grammar_arm`, `grammar_pattern`: `patterns.gamma:1-46`
- Storage: `syntax_storage_limit`, `syntax_storage_requested`: `storage.gamma:5-16`

### Inputs and outputs
- **In:** admitted source bytes via `source_read` / `source_length` after envelope/token phases (`parse.gamma:14-20`, **inferred** from README).
- **Out:** `(pair (pair top-count spine) used)` parse carrier then unchanged program spine on grammar success (`parse.gamma:52-57`, `declarations.gamma:36-37`); failures via `compiler_source_rejection` / `compiler_failure`.

### How it decides
- Parse: byte at `start` — `(` opens frame, `)` closes or rejects at `depth 0` (`parse.gamma:21-29`).
- Grammar roles: integer `role` dispatch in `grammar_check_role` (`grammar.gamma:9-25`); expression depth vs 1024 at `grammar.gamma:12-14`.
- Atoms: `grammar_atom_role_error` uses `role` to pick `value_name_valid` / `upper_name_valid` / `field_type_shape_valid` (`work.gamma:60-69`).
- Child counts: `grammar_exact_count` / `grammar_minimum_count` return `-1` or a coordinate (`work.gamma:44-57`).
- Storage: `syntax_storage_fits` before allocating grammar batches (`declarations.gamma:44-49`, `expressions.gamma:69-76`).

### Cases by shape
- Program requires at least one `def` before success (`declarations.gamma:3-7`, README `grammar_program` table).
- `data` vs `def` via `token_data` / `token_def` on head atom (`declarations.gamma:21-28`).
- `if`/`let`/`match` exact child counts (`expressions.gamma:60-88`); applications accept variable arg lists (`expressions.gamma:40-58`, README `138-140`).
- Patterns: atom, 1-child paren, or multi-child constructor binders (`patterns.gamma:33-45`).

### Same job elsewhere (unit)
- `syntax_reverse_nodes` shared with lowering/normalization (`README.md:71-72`, `nodes.gamma:30-34`).

### Unused / shared helpers (grepped whole repo)
- `syntax_reverse_nodes`: **9** definitional/use sites under `bootstrap/3_delta/implementation/` (**grepped** `syntax_reverse_nodes` in repo).
- `grammar_child_at`: **3** lines, all in `work.gamma` (**grepped**); no outside callers — internal only, not unused.

### Stated limits
- Syntax provision 114,294,752 bytes at 40 bytes/pair (`storage.gamma:5`, README `32-69`).
- Expression depth 1,024 → resource code 8 (`grammar.gamma:12-14`, README `115-121`).
- Arity/semantic checks deferred to body checking (README `136-140`).

### Fail-open risks
- Shape probes return coordinates, not outcomes; failures use `compiler_source_rejection` when shape ≠ `-1` (`work.gamma:42-57`) — **traced**: bad shape does not fall through to role checks.
- `syntax_storage_fits` failure returns refusal before child work (`parse.gamma:23-26`) — **stated** in README allocation table.
- **none seen** for accepting invalid programs after explicit `grammar_reject` / `compiler_source_rejection`.

---

## `bootstrap/3_delta/implementation/checking/types/` (+ matches/)

### Purpose
Body type checker on retained syntax after declaration resolution; continuation-based visit/resume instead of recursive source walks (`types/README.md:3-13`, `types.gamma:1-33`).

### Shape
- Entry: `check_program_types`, `check_program_nodes`, `check_definition_type` (`types.gamma:3-33`).
- Dispatcher: `typing_frame`, `typing_visit`, `typing_resume` kinds 1–7 (`expressions.gamma:4-55`).
- `int_type` / `bytes_type` / `typing_reject` (`identities.gamma:3-8`).
- Branches, lets, calls, matches in sibling `.gamma` files.

### Inputs and outputs
- **In:** retained `program`, resolved `globals` metadata (`types.gamma:3-4`, `bindings.gamma:28-29`).
- **Out:** `compiler_complete 0` or `typing_reject` / `compiler_retained_frontend_contradiction` (`types.gamma:7-16`, README `92-130`).

### How it decides
- Definition: metadata owner/body_start must match syntax (`types.gamma:22-31`).
- Calls: head upper vs builtin vs function trie (`calls.gamma:7-36`); args left-to-right with kind-5 payload order (`calls.gamma:60-75`, README `132-143`).
- `typing_resume_argument`: requires positive actual/expected counts else internal contradiction (`calls.gamma:88-95`).
- Match: subject type > 0, owner/arity/duplicate/coverage (`matches.gamma:8-51`, `arms.gamma:22-53`, `context.gamma:4-34`).

### Cases by shape
- Unknown continuation kind at positive depth → `compiler_retained_frontend_contradiction` (`expressions.gamma:53-55`, README `108-116`).
- Argument kind-5 only with `lt 0 depth` guard (`expressions.gamma:43-46`).

### Same job elsewhere
- Continuation pattern mirrors lowering/emission/normalization frame stacks (same `(pair kind (pair payload previous))` shape) — citations within unit; **suspected** parallel to Rust visitor stacks (not opened).

### Unused / callers
- `typing_reject`, `int_type`: used throughout this unit (**read** call sites in `bindings.gamma`, `calls.gamma`, etc.).

### Stated limits
- Nested-pair argument payload retained vs product-type alternative documented (`types/README.md:132-209`).
- 65,536 active binding rows referenced via environments (`types/README.md:38-41`).

### Fail-open risks
- Nonpositive arg counts → internal failure, not type mismatch (`calls.gamma:85-95`) — **stated**; prevents silent arity/type reorder.
- `fail` on bad definition metadata (`types.gamma:32-33`) — **open** without tracing every `require_function_metadata` path in files outside this unit.

---

## `bootstrap/3_delta/implementation/emission/`

### Purpose
Serialize completed Gamma plan to receipt bytes (count then publish); validate bound support section from sealed input (`README.md:1-17`, `support.gamma:1-79`).

### Shape
- `emit_checked_program`, `emit_program`, `emit_definition` (`program.gamma:1-16`, `declarations.gamma:1-29`).
- `emission_visit` / frames 1–2 (`expressions.gamma:12-68`).
- `emit_gamma_atom_fields`, extents (`atoms.gamma:3-26`, `extents.gamma:6-36`).
- `emit_advance`, text helpers (`text.gamma:5-82`).
- `emission_unary_word` cache (`unary_words.gamma:4-31`).

### Inputs and outputs
- **In:** normalized `program`, `publish` flag, running `written` count.
- **Out:** byte count or `write` effects; coordinator adds payload limit check (`pipeline.gamma:50-61`).

### How it decides
- Atom kind 1–6 field dispatch (`atoms.gamma:13-26`).
- `emit_advance`: negative metadata propagates `-1` (`text.gamma:5-10`).
- Profile 0 vs 1 support members (`program.gamma:7-12`, `pipeline.gamma:63-75`).
- Support sums vs embedded table (`support.gamma:58-79`).

### Cases by shape
- Unary-word calls: cached prefix only for kind 7, arity 1, kind-6 head, length 1–7 (`unary_words.gamma:5-17`).

### Same job elsewhere
- Extent computation shares spelling with publication (`extents.gamma:27-36`, README `67-72`).

### Unused / callers
- `validate_support_members`: called from `pipeline.gamma:5` (**grepped** `compile_admitted_source` chain).

### Stated limits
- Payload 16,777,212 bytes (`README.md:74-77`, `pipeline.gamma:56-57`).
- Publication traversal pair bound (`README.md:212-248`).

### Fail-open risks
- Count/publication mismatch → `fail` not silent publish (`pipeline.gamma:58-61`) — **traced**.
- Negative `emit_advance` → internal contradiction before publish (`README.md:80-87`) — **stated**.

---

## `bootstrap/3_delta/implementation/lowering/` (+ matches/)

### Purpose
Translate type-checked retained Delta into expanded Gamma plan; explicit continuations; optional `$dp` projection helper for wide patterns (`README.md:1-8`, `program.gamma:3-50`).

### Shape
- `lower_program`, `lowering_visit` / `lowering_resume` kinds 1–6 (`program.gamma`, `expressions.gamma:27-48`).
- Checked arithmetic guards (`arithmetic.gamma:4-35`).
- Constructors/products (`constructors.gamma:1-20`).
- Match context/atoms (`matches.gamma`, `arms.gamma`, `bindings.gamma`).

### Inputs and outputs
- **In:** `program`, `globals`; locals as name trie of binding atoms.
- **Out:** `compiler_complete` wrapping `gamma_program` (`program.gamma:8-10`).

### How it decides
- Application head: upper → constructor; else builtin kind / operators / `gamma_function` (`calls.gamma:28-52`).
- Wide patterns: `lowering_inline_projection_width` 256 (`bindings.gamma:9-28`).
- Data declarations may install `$dp` once (`program.gamma:13-23`).

### Cases by shape
- Nullary constructor: tag only or pair with empty product (`constructors.gamma:5-8`).

### Same job elsewhere
- Match lowering parallels type-check match flow (shared subject/context ideas) — both in unit (**read**).

### Stated limits
- Pre-normalization heights may exceed 255 (`README.md:165-171`).

### Fail-open risks
- `lowering_local` → `fail` if binding missing (`bindings.gamma:10-11`) — assumes checker established bindings; **open** if metadata wrong.

---

## `bootstrap/3_delta/implementation/normalization/` (+ capture/)

### Purpose
Split over-255-height Gamma bodies via helper extraction + free-binding capture before emission (`README.md:1-19`, `program.gamma:3-24`).

### Shape
- `normalize_program`, `normalization_visit` / `normalization_resume` (`program.gamma`, `expressions.gamma:7-46`).
- `normalization_extract` / `normalization_helper_done` (`helpers.gamma:5-27`).
- Capture: `capture_visit`, merge sort unions (`capture.gamma`, `capture/sets.gamma`, `capture/result.gamma`).

### Inputs and outputs
- **In:** lowered `gamma_program`.
- **Out:** new program with appended helper definitions (`program.gamma:12-16`).

### How it decides
- Reuse node if `budget >= gamma_height` else descend or extract at `budget == 1` (`expressions.gamma:9-22`).
- Capture ordering: generated before source bindings (`capture/bindings.gamma:4-18`).

### Cases by shape
- `$h` (marker 104) call heads: reuse argument list without re-walk (`capture/calls.gamma:8-14`).

### Same job elsewhere
- Frame kinds 1–3 parallel emission/lowering argument/let walks (**read** within unit).

### Stated limits
- `normalization_height_limit` 255 (`program.gamma:7-8`).
- Capture pair accounting closed form (`README.md:136-217`).

### Fail-open risks
- **none seen** for skipping extraction when height exceeds budget — over-height at budget 1 always extracts (`expressions.gamma:10-11`, **read**).

---

## `bootstrap/3_delta/implementation/pipeline.gamma`

### Purpose
Orchestrates compile: support validate → envelope/tokens → parse → grammar → census → resolve → typecheck → profile → lower → normalize → emit (`pipeline.gamma:4-75`).

### Shape
- `compile_admitted_source`, `prepare_admitted_source`, `publish_checked_program` (`pipeline.gamma:4-61`).

### Inputs and outputs
- **In:** `profile` Int; sealed input globals (**inferred** from support/README).
- **Out:** compiler outcomes; successful publish returns `compiler_complete 10` (`pipeline.gamma:60`).

### How it decides
- Each phase checks `(first outcome) 0` before next (`pipeline.gamma:5-47`).
- Publication only if preflight count matches and ≤ 16777212 (`pipeline.gamma:53-61`).

### Same job elsewhere
- **suspected:** `omega-rust` product pipeline entry (digest area_01 `orchestration` / `compiler` crates) — not opened.

### Fail-open risks
- `emit_advance` negative → metadata contradiction, not publish (`pipeline.gamma:54-55`) — **traced**.

---

## `bootstrap/3_delta/implementation/representation/`

### Purpose
Gamma plan ADT: definitions, expression nodes with cached height/extent/unary prefix (`representation/README.md:1-64`, `gamma/expressions.gamma:4-33`).

### Shape
- `gamma_program`, `gamma_definition` accessors (`gamma.gamma:3-20`).
- `gamma_node` packs extent + unary_word (`expressions.gamma:4-13`).
- Names/primitives (`names.gamma`, `primitives.gamma`).

### Inputs and outputs
- Constructed only from lowering; consumed by normalization and emission.

### Fail-open risks
- **none seen** at representation layer (no acceptance judgments).

---

## `bootstrap/3_delta/implementation/implementation.gamma.sources`

### Purpose
Manifest listing SHA-256 and paths for `GammaSourceClosureV1` compiler members (`implementation.gamma.sources:1-66`, **read**).

### Shape
Ordered `member <hash> <len> <digest> <path>` rows.

---

## `bootstrap/3_delta/support/`

### Purpose
Authoritative Gamma text copied into generated programs: byte rope, conformance I/O, `main` adapter (`README.md:6-10`, `support.gamma.sources:1-4`).

### Shape
- `$dbe`…`$dbgi` (`bytes.gamma:13-53`).
- `$pbg`…`$pbw` (`conformance.gamma:9-39`).
- `main` input/output limits (`adapter.gamma:6-13`).

### How it decides
- Trap via `/ 1 0` on contract violation (`bytes.gamma:17-18`, `conformance.gamma:10-11`).

### Stated limits
- Input/output 4194304 in adapter (`adapter.gamma:7-12`).

---

## `bootstrap/4_epsilon/` documentation (FEATURE_LEDGER, README, LANGUAGE, EVALUATOR_*)

### Purpose
Normative Epsilon v1 contract, feature rationale, evaluator entry/profile, rung orientation (`LANGUAGE.md:1-7`, `README.md:1-8`, `FEATURE_LEDGER.md:1-9`).

### Shape
- Judgments `CheckEpsilon` / `RunEpsilon` (`LANGUAGE.md:22-33`).
- Closed rejection/trap tables (`LANGUAGE.md:684-758`).
- EREQ envelope and artifact table (`EVALUATOR_ENTRY.md:21-29`, `53-66`).

### How it decides
- Phase-ordered diagnostics (`LANGUAGE.md:698-716`).
- Premise DAG for body/control (`LANGUAGE.md:539-604`).
- Resource classes (`LANGUAGE.md:760-805`).

### Stated limits
- No small semantic caps unless reported as `Incomplete` (`LANGUAGE.md:802-805`).
- Evaluator profile counters (`EVALUATOR_PROFILE.md:89-100`).

### Fail-open risks
- Contract requires fail-closed on budget exhaustion (`LANGUAGE.md:776-779`) — specification, not code in this unit.

---

## `bootstrap/4_epsilon/checking/` (selected `.delta` modules in unit)

### `check.delta`
- Chains parse→catalog→local resolve→finish body check (`check.delta:6-43`).
- **Out:** `EpsilonLocalResolutionOutcome`.

### `calls/arguments.delta`
- Arity/type join for calls; state vs machine applications (`arguments.delta:71-247`).
- **Decides** on `epsilon_argument_types_join` and `EpsilonArgumentsCompatible` vs mismatch/incomplete.

### `calls/constructors.delta`, `machines.delta`
- Qualified/unqualified/receiver call paths; constructor vs field vs machine (`constructors.delta:11-108`, `machines.delta:1-207`).

### `calls/state_targets.delta`
- Transition continuations: state/machine namespace join, `InvalidControlTarget` (`state_targets.delta:5-199`).

### `catalog/declarations.delta`, `type_relations.delta`
- Build resolution catalog; unknown qualified machine owners; `epsilon_types_equal` structural (`declarations.delta:38-376`, `type_relations.delta:8-184`).

### `control/*.delta`
- Block exits vs return types (`block_exits.delta:5-185`).
- Machine/state local resolution (`blocks.delta:1-128`).
- Suppressed continuation checking after never (`continuations.delta:4-308`).
- Transition coverage / exhaustiveness (`coverage.delta:1-146`, `patterns.delta:1-214`).

### Inputs and outputs (common)
- **In:** `EpsilonSourceView`, accumulated `EpsilonLocalResolutionScan` syntax + ledgers.
- **Out:** updated scan or final `EpsilonLocallyResolvedProgram` (`blocks.delta:85-128`).

### How it decides (cross-cutting)
- Smallest-offset final candidate merge (`type_relations.delta:155-184`).
- Callable identity before arity (`LANGUAGE.md:549-558`, mirrored in `arguments.delta:176-200`).
- Expression use kind (`EpsilonOrdinaryExpressionUse` vs transition) gates machine admission (`machines.delta:138-147`).

### Cases by shape
- Grouped call heads peeled via `epsilon_call_head_core` (`constructors.delta:113-129`).
- Sum transitions need coverage or final wildcard (`coverage.delta:118-146`).

### Stated limits
- README lists implemented vs remaining evaluator work (`README.md:40-82`).

### Fail-open risks
- Scan continues after parent cannot form (“premise DAG”) per LANGUAGE — implementations should still visit children; **inferred** from comments, not fully traced every path in this unit.

---

## Does the same job as Rust code

| This unit (bootstrap) | Suspected Rust analogue | Status |
| --- | --- | --- |
| Delta `parse.gamma` + syntax grammars | Target-neutral parse / retain AST in `omega-rust/psi/` (digest area_03/psi — not opened) | **suspected** |
| Delta `checking/types/*` body checker | Semantic/type checking on checked PSI | **suspected** |
| Delta `lowering/*` + `representation/gamma/*` | Lowering to Terminal Psi / internal IR | **suspected** |
| Delta `normalization/*` | Outline / height splitting before codegen (no exact single crate named in digests) | **suspected** |
| Delta `emission/*` | CLI/native emission orchestration in `omega-rust/omega/` (digest area_01) | **suspected** |
| Delta `pipeline.gamma` | `orchestration` + `compiler` pipeline coordination (digest area_01) | **suspected** |
| Epsilon `checking/*.delta` | Staged local-resolution / fact accumulation toward full compiler checking | **suspected**; no `omega-rust` crate opened |
| Epsilon `LANGUAGE.md` rules | Language spec / `wiki/spec` contracts | **different job** (spec, not implementation) |

Digests searched: `area_01.md` (orchestration/compiler/cli); no digest entry for `bootstrap/3_delta` (**grepped** `.agents/atlas/compiler-2026-11-10/digests`).

---

## Coverage

**Read in full (line counts per unit TSV):**

All 80 rows in `grok/E/inputs/unit_002.tsv`: syntax (203+25+95+89+46+77+34+58+16), types (33+228+78+26+95+55+8+51+29+53+34), emission (278+26+29+68+36+16+98+82+31), `implementation.gamma.sources` (66), lowering (183+35+28+58+20+48+43+58+50+50), normalization (418+23+7+32+27+27+11+46+56+46+27+24), pipeline (75), representation (64+20+33+14+21), support (66+13+53+39+4), epsilon docs (250+216+64+857+318), epsilon checking deltas (247+164+206+199+376+184+43+184+128+308+146+214) — **read** via sequential Read tool calls.

**Skimmed:** none.

**Not read:** none.

---

## Five things a maintainer most needs to know

1. Delta compile order is fixed in `pipeline.gamma`; emission never runs before `normalize_program` on success paths (`pipeline.gamma:8-10`, `50-51`).
2. Syntax grammar and body typing both use explicit continuation stacks (kinds enumerated in `expressions.gamma` files) rather than recursive source descent.
3. Gamma plan nodes carry precomputed serialization metadata at construction (`representation/gamma/expressions.gamma:24-27`); emission is structure-directed only.
4. Normalization extraction preserves evaluation position and uses capture merge/sort with logarithmic merge passes (`normalization/README.md:45-48`, `capture/result.gamma:17-26`).
5. Epsilon checking modules in this unit implement catalog + scan-based local resolution toward `LANGUAGE.md` section 8 rules; evaluator entry (`EVALUATOR_ENTRY.md`) is a separate canonical boundary from the diagnostic driver (`EVALUATOR_PROFILE.md:10-18`).

---

## What I am unsure about

- Exact locations of `collect_global_identities`, `resolve_collected_declarations`, and `validate_application_profile` (called from `pipeline.gamma` but not in this unit) — **not read**.
- Whether every `fail` in lowering/normalization is unreachable given checker invariants — **UNDECIDED** without reading boundary/assert helpers outside unit.
- Full execution semantics of Epsilon scan types (`EpsilonLocalResolutionScanSyntax` fields) — only partial context from files read; remainder lives in other closure members per `README.md:159-161`.

---

UNIT COMPLETE

