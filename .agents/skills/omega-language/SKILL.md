---
name: omega-language
description: Omega programming language reference for writing machines, states, and transitions. Use when writing or debugging .omg code — syntax essentials, compiler invocation, where the authoritative docs live, and a field-tested trap catalog for the gaps the spec doesn't warn about.
---

# Omega Language

Omega is a proof-carrying systems language whose programs are data-oriented state
machines. It is not Rust with different syntax — the abstractions you reach for
in other languages (`if`, loops, functions, growable buffers, runtime dispatch)
are deliberately absent, and the ones present (states, transitions, domains,
proof obligations, boundary providers) want a different shape of thought. The
authoritative source is this repository; this
skill routes to it rather than duplicating it.

## Thinking in Omega

Read this before writing any `.omg` code. These are the defaults an Omega
program is built from — fighting them is what produces the compiler fights.

- **States are the control flow.** A machine is named states connected by
  transitions; there is no `if` and no loop keyword. A conditional is a
  transition whose guard picks the next state; a loop is a transition back to
  an earlier state (a jump, not a call). A `match` expression only picks a
  value by comparing plain values; branching on case or record shapes is
  `transition` dispatch. Sketch the state graph first — if the states have
  good names, the program is already mostly written.
- **Fields are the memory.** No heap and no locals that outlive a state —
  everything persistent is a `data` field with a declared type and usually a
  declared bound. The field layout *is* the program's data model. Design it
  before writing states; a missing field is a missing idea.
- **Write for the prover, not against it.** Bounds, domains, and
  single-assignment fields exist so the compiler can prove the program. Declare
  tight bounds (the real worst case, not capacity), prefer fields assigned
  exactly once, guard on literals before fields. When the prover refuses, the
  productive fix is restructuring — split a state, narrow a bound, hoist a
  computed index into a field — not decorating the expression it rejected.
- **Budgets, not growth.** Fixed arrays everywhere; decide widths up front.
  Text is a fixed `[u8; N]` byte block padded with spaces, written whole. A
  quoted literal is a byte view, not text; an encoding such as Utf8 is an
  explicit domain.
- **Boundaries are the only I/O.** Console, files, sockets, even process exit —
  anything outside the machine is a `boundary trait` satisfied by exactly one
  provider per target, selected in `build.omg`. You never call the OS
  directly; you call a trait, and the build wires the provider. Printing and
  exiting are separate powers: `Console` does not include `ProcessExit`.
- **`--check` is the floor, not the finish.** Real miscompiles have passed it
  cleanly. Build, run, look at actual bytes; choose test values that could only
  be right if the logic genuinely ran.

## Ground truth ordering

Two different questions, two different authorities:

1. **What the language is: the wiki.** `wiki/language_guide/` and `wiki/spec/`
   decide syntax, meaning, and every claim you make about Omega. Look the form
   up there before writing or teaching it (the owner's rule, 2026-09-28).
2. **What works today: the current compiler.** `--check` and the built binary
   tell you whether the reference compiler implements a wiki form yet. Write
   the wiki form it accepts. When the two disagree, don't invent a middle
   spelling; pick another wiki form or say it isn't implemented. Pull Omega
   `main` and rebuild `target/debug/omega.exe` first (`mbx build -p omega`,
   about 3 minutes); a stale binary gives stale answers.
3. **Implementation evidence, not authority: `samples/`, `tests/`,
   `source/library/`.** These lag the wiki. Measured 2026-09-28 at main
   `86353504ea`: std, tests and samples still spell ranges `i32 [0..=255]`
   (344 uses) and exit through `Console::exit_process` (720 files), and the
   wiki rejects both. Never copy a spelling from them without checking the
   wiki.
4. **`wiki/proposals/`, `wiki/drafts/`, `OWNER_QUESTIONS.md`** — candidate changes
   and open decisions, not current rules.

## Invoking the compiler

The compiler package is `omega` (binary `omega`); there is no `omega-cli`.
A built binary usually exists at `target/debug/omega.exe` in the repo.
Run it through `mbx` from the repository root (AGENTS.md makes `mbx` mandatory):

```bash
mbx run -p omega -- --check path/to/main.omg
mbx run -p omega -- --target windows_x64 path/to/main.omg
```

Or invoke the built binary directly: `target/debug/omega.exe --check main.omg`.
`--help` prints the usage banner (verified 2026-09-28).
`--target` places `build/` next to the **source file's** directory, not the
invoking CWD. A compiled program's own relative native paths resolve against its
running process CWD — set it explicitly when harnessing the exe.

`--check` passing is not proof of correct output — several known miscompiles pass
it cleanly. For anything producing text, build, run, and inspect actual bytes
(`| cat -A` marks control characters and line ends). See the trap catalog.

## What good Omega looks like

Taste, in one paragraph: states are small and single-purpose — a state that
does two things is two states. Machines stay boring: they sequence states and
hold fields, they don't carry cleverness. The state graph should read like the
problem's shape — when a reader can trace your machine and recognize the
domain's workflow, it's idiomatic. Match the idiom of `samples/` before
inventing your own; when a needed idiom doesn't exist yet, add the smallest one
that works and note it.

## Syntax essentials

- `let` requires a type: `let v: u32 = expr;`. Parenthesize transition return
  values: `transition { _ -> (a + b) }`.
- Exact arithmetic is a proof obligation: plain `u32 + u32` is refused unless
  the operands are provably in range. There is **no `T [lo..=hi]` type
  suffix** (guide ch. 7). State bounds as clauses: `requires x in 0..=1000` on
  a parameter, `where` on a data declaration (`data Player where health in
  0..=100 { health: i32; }`), a case-local `where` on a case (`case Hit(value:
  i32) where value in 0..=255;`), `ensures` on a result, or a dominating
  guard. Otherwise widen with `as` or opt into a named arithmetic policy
  (`u32 in Wrapping`, `Saturating`, `Trapping`). Don't use Wrapping for a loop
  counter just to silence the prover: the spec says Wrapping loop updates need
  an independent no-wrap proof before the loop's descent can be trusted.
- Owned (affine) values move at most once; the guide spells the transfer with
  `move` (`self.inventory = move replacement;`, `done(move value)`). `[copy]`
  types copy freely. Using an owned value a second time fails "affine value
  `x` was already transferred or consumed; it cannot be moved here" (measured
  2026-09-28).
- Native interop exists: `satisfies Trait::method via
  Binding::DllImport("Kernel32.dll", "Symbol")` — real WinSock/file calls
  verified end-to-end, with sharp edges; read the trap catalog first.
- Numeric output is manual: no printf/itoa. `n / 10`, `n % 10`, `+ 48`, emit
  via `narrow_u32_to_u8_trapping` from
  `omega::language::core::numeric_conversion` into a space-padded buffer.

## The self-free surface — wiki forms, verified 2026-09-28

`self` is optional, not load-bearing (guide ch. 3: "A free entry machine has no
implicit state"). Everything below follows the wiki and `--check`-verifies
against the reference compiler at Omega main `86353504ea`:

```omega
use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine start() reaches Console + ProcessExit {
    Console::write_line("Hello, Omega.");
    ProcessExit::exit_process(0);
}
```

- **Free-standing machines** take only parameters — no receiver, no `data`
  block required: `machine choose(flag: bool) -> u64 { ... }`.
- **Boundary calls can be static**: `Console::write_line(...)` names the
  trait's contract directly inside a `reaches` scope; no `Binding<Console>`
  field needed. (Receiver-style `self.console.write_line` with a
  `console: Binding<Console>` field is the guide's hello world and still works.)
- **Reach is a `+`-joined set of services**, and every direct boundary call
  needs its service listed: `reaches Console + ProcessExit`.
- **Exit is `ProcessExit`, not Console.** Core owns
  `ProcessExit::exit_process(return_code: i32)` (spec `process_exit.md`:
  "Console must not inherit ProcessExit wholesale"). Std's
  `Console::exit_process` is a retained convenience that still compiles; don't
  teach or write it.
- **States re-declare their parameters** and capture nothing from the
  enclosing machine: `state wipe(line: &mut [u8; 28], k: u64)`. A guard's
  facts carry into the state it lands in, so after `transition k < 28u64 {
  true -> wipe(line, k) ... }` the body can index `line[k]` and jump back
  with `scan(line, k + 1u64)` without Wrapping.
- **Case payload bounds use a case-local `where`**:
  `case Hit(value: i32) where value in 0..=255;`. Building a `Hit` must prove
  the bound; matching one supplies it.
- **Dispatch on a case**: subject-form transition with payload binding,
  `transition reply { Reply::Hit { value } -> keep(value) Reply::Miss -> miss() }`
  (must cover every case), or a guard list comparing payload-free cases
  (`phase == Phase::Red -> ...`). `match` is value-position only and compares
  plain values (guide ch. 6).
- **Boundary traits reach their own service automatically** (guide ch. 19).
  Don't write `reaches Console` on members of `boundary trait Console`; add
  `reaches` there only for other services. A provider fills a member with
  `machine FakeClock::now() -> u64 satisfies Clock::now { ... }`.
- **`build.omg` is one machine**: `machine build(builder: &mut Build)` with
  exactly one of `builder.application("name")`, `builder.package("name")`,
  or a workspace member set (mixed or duplicate kinds are rejected),
  `builder.depend(Source::Path { location: ... })`, and one
  `builder.roots.bind(<target>::ProgramEntry, start)` per target — a free
  entry binds by bare name. Providers auto-select; `select_provider` is only
  needed to break ties.

## Field notes — read before writing these shapes

The trap catalog (`references/traps.md`) holds measured findings from real
programs, each dated and evidenced. Consult it **before** writing:

| Writing... | Read section |
|---|---|
| Any console/text output | Text output; literal-assignment traps |
| A `DllImport` boundary, sockets, file I/O | Native calls and boundary traits |
| A tagged union / enum with payload fields | Enum construction (landed since; re-verify) |
| Dispatch on a case value | Dispatch on case values (`transition`, not `match`) |
| Anything with a bound, or an older entry's fix | The "Retired spelling" note at the top |
| Loops with running offsets, buffer assembly | Proof-obligation shapes; indexing traps |
| A machine taking arguments beside `self` | Machine-call semantics (landed since; re-verify) |

The traps were measured against the 2026-08 compiler. The compiler has moved;
re-verify any trap before designing around it, and check whether an upstream
issue exists (e.g. CathedralOS/Omega#3 for the import-catalog case bug).

## Where things live

- **Language guide** (`wiki/language_guide/`): read the chapter for the topic,
  not the whole guide (~13k lines).
- **Spec** (`wiki/spec/`): subject-organized contracts — `language/` for source
  semantics (state contracts, domains, machines, ownership, termination...),
  `layouts/`, `proofs/`, `terminal-psi/`, plus publication policy.
- **Library** (`source/library/`): `core/` (array, slice, vec, option, int, nat,
  arena, ptr, numeric_conversion, real/rat/cauchy math, layout, assembly, task),
  `std/` (console, filesystem, math, time, units, wire, calling — plus
  `targets/<target>/` per-platform provider impls), `alloc/`,
  `blocking-executor/`, `topology/`.
- **Samples** (`samples/`): `cli/{basics,arithmetic,algorithms,collections,text,
  systems,proofs,games,interpreters,device,probes,rendering,simulation,apps}`,
  `gui/`, `stdin_*`, `uefi/`. `samples/cli/basics/multiplication_table/main.omg`
  is the canonical fixed-buffer text-output shape.
- **Repo workflow**: `AGENTS.md` (mbx commands, validation scope, baseline
  gates, Psi/Omega ownership firewall — read it before touching compiler code).

## Guide chapter map

| Topic | Chapter |
|---|---|
| data, values, literals | `chapter_1_data_values_literals.md` |
| ownership, borrowing, moves | `chapter_2_ownership_borrowing_moves.md` |
| machines | `chapter_3_machines.md` |
| states, transitions | `chapter_4_states_transitions.md` |
| expressions, evaluation | `chapter_5_expressions_evaluation.md` |
| pattern matching, dispatch | `chapter_6_pattern_matching_dispatch.md` |
| contracts, flow facts | `chapter_7_types_constraints_invariants.md` |
| domains | `chapter_8_domains.md` |
| proof obligations | `chapter_9_proof_obligations.md` |
| compile-time proofs | `chapter_10_compile_time_proofs.md` |
| invariant windows | `chapter_11_invariant_windows.md` |
| dependent types | `chapter_12_dependent_types.md` |
| generics | `chapter_13_generics.md` |
| traits, erased dispatch | `chapter_14_traits.md` |
| modules, imports, visibility | `chapter_15_modules_imports_visibility.md` |
| errors, traps, failure | `chapter_16_errors_traps_failure.md` |
| drops, cleanup | `chapter_17_drops_and_cleanup.md` |
| concurrency | `chapter_18_concurrency.md` |
| capabilities, reach, boundaries | `chapter_19_capabilities_effects_boundaries.md` |
| memory layout, ABI | `chapter_20_memory_layout_abi.md` |
| semantic reflection | `chapter_21_reflection.md` |
| inline assembly | `chapter_22_inline_assembly.md` |
| testing | `chapter_23_testing.md` |
| embedding, scripting | `chapter_24_embedding.md` |
| source packages | `packages.md` |
