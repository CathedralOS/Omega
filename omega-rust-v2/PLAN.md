# Omega-V2 plan

Living document. Jarod edits it freely; Claude keeps the layer statuses, spec
gaps and decision log current as work lands. Last updated 2026-11-19.

For the same content as pictures, open [ARCHITECTURE.html](ARCHITECTURE.html):
ten diagrams and no prose beyond captions.

Each statement below is marked by where it came from:

- **Decided** — Jarod said it.
- **Proposed** — Claude's suggestion, not yet confirmed. Change or strike it.
- **Open** — nobody has chosen yet.

## Why V2 exists

Omega V1 (`C:\Development\Omega`) has had per-stage front-end directories
since 2026-09-28, but the stages were never independent in practice: one
coordinator owns resolution, typing and evaluation together (see
[the loop](#why-v1-loops-back)). On 2026-11-17 `main` folded them into a
single `01_source-compilation` stage (#4030). On 2026-11-18 worker-21 began
splitting them back out into eight stages, and it is slow: its README says
only the lexer and parser compile independently, with the other six still
built together. That one agent session had run 11h24m, 3,850 steps and 589M
input tokens across 34 commits at the time of writing.

V2 takes the other route: build one layer at a time, from the spec, and do not
start the next layer until the current one is proven by a test suite fast
enough to run on every change. **Decided.**

A layer carries each decision the next layer needs in its output. When changing
a handoff, name the layer that owns the decision and the field that carries it.
Remove any consumer code that reconstructs the decision. Test the field at the
producer and its effect at the consumer. For equality, L2 supplies the operand bindings. L3 uses those bindings and any
normalized parameter bounds to select type or value equality.
**Decided.**

## What is authoritative

- **The spec is the reference.** `wiki/spec/` (104 files, copied from V1 `main`
  at `0b90bdba`) is the only source of rules, except for the departures listed
  under [Where V2 departs from the spec](#where-v2-departs-from-the-spec).
  **Decided.**
- **V2's copy of the spec is now V2's own.** Its code examples were rewritten
  on 2026-11-18 to V2's syntax: commas between arms, `::<` for generic
  arguments in expressions, the one `if` statement turned into a
  `transition`, and the stray `;` and `,` after `requires` removed. That
  first revision touched 23 files and about 60 lines, all inside code examples.
  Decision 12 later migrated reference and borrow spellings in examples and
  prose across 40 spec files. Some prose still describes earlier generic-call
  and arm syntax. **Decided** (Jarod: "v2 has to change").
- **The language guide is for humans**, not a source for V2. **Decided.**
- **`wiki/drafts` in V1 is agent working material**, not spec. **Decided.**
- **V1's `omega-rust/pipeline.md` is a proposed pipeline**, read for context
  only. V2 does not follow it. **Decided.**
- **The spec was largely agent-drafted.** Each layer treats it as a claim to
  check, and records where it is silent or contradicts itself (see
  [Spec gaps](#spec-gaps)).

## How a layer gets built

A layer is a function from one representation to the next: source bytes to
tokens, tokens to a tree, and so on. The same loop runs for every layer:

1. **Contract.** Write down the layer's input, output and every rule it
   enforces, each rule citing the spec section it comes from. A rule with no
   spec owner needs a ruling from Jarod before it is implemented. File unresolved
   language and architecture questions in Omega's root
   [OWNER_QUESTIONS.md](../OWNER_QUESTIONS.md) with the specific contract and
   example that leave the choice open. Link to the question from the affected
   layer's contract. When Jarod answers, record the ruling in the governing
   spec and remove the queue entry.
2. **Settle ownership.** Trace the decision through a concrete program. Identify
   the information it requires, the layer that owns it and what the next layer
   must receive. Implement the layer as a library depending only on earlier
   layers. Make the architectural changes before running tests.
3. **Test the handoff.** Check the producer's output and the consumer's
   decision using the [handoff review](#handoff-review).
4. **Gate.** The layer passes the checks under [What solid means](#what-solid-means).
5. **Sign-off.** Jarod reviews and marks the layer done here. Only then does
   the next layer start.

## What solid means

A layer is done when all of these are true:

- Every rule in its contract has an accept fixture and a reject fixture, and
  all pass.
- It runs over the whole spec example corpus (the 132 `omega` code blocks in
  the spec) without crashing, and every result is either accepted or a
  refusal the contract predicts.
- Its independent invariant holds (see the test system below).
- Each disputed responsibility has a regression that checks the fact handed
  forward and what the consumer does with it. The sign-off names those
  tests and any behavior the review could not verify.
- The suite's wall-clock time is measured and written in the layer table.
- Spec gaps it uncovered are logged, each with a decision.
- Jarod has signed it off.

## Test system

Built for L0 as described here; change it freely. The purpose is that
changing a layer and knowing whether it still works takes about a second,
with no compiler binary to build. Run it with `cargo test -p omega-lexer`;
rewrite expected files with `OMEGA_BLESS=1` set.

- **Where it lives:** `tools/golden` is the fixture runner and
  `tools/spec-corpus` extracts the spec's examples. Both are small Rust
  libraries used only by tests.

- **One library per layer.** Tests call the layer's function directly,
  in-process.
- **Golden fixtures.** A fixture is an input file plus the expected text dump
  of the layer's output. The dump is plain line-oriented text with source
  positions, so a failure shows as a readable diff.
- **Reject fixtures.** An input plus the expected diagnostic and its position.
  A layer must prove what it refuses, not only what it accepts.
- **Bless mode.** One command rewrites the expected files from current output;
  the resulting diff is what gets reviewed.
- **Spec corpus.** Every `omega` block in `wiki/spec` is read at test time,
  with its source file and line, so the corpus cannot drift from the spec.
  Every layer runs over it. Not done yet: examples the spec presents as
  deliberately invalid are not tagged with the layer expected to refuse them.
  The lexer did not need this, since all 132 are lexically valid.
- **An independent invariant per layer**, cheap to check and not derived from
  the layer's own logic. For the lexer: the tokens and skipped bytes, laid end
  to end, reproduce the source file exactly. For the parser: printing the tree
  and lexing the result gives the same tokens.

### Handoff review

After the responsibility moves and before sign-off, review the handoff in
parallel with three GPT-6.2 Sol reviewers. Give each the same V2 contract
and a different question. Ask for the smallest concrete case that would
expose an incorrect decision.

| Review | What to verify |
| --- | --- |
| Producer | Assert the exact fact at the public output. For positional `..`, L1 emits a final rest node with the span of those two bytes. |
| Consumer | Change one deciding input. Run the same field selection with and without the direct dependency. Keep a carrying-only control to catch a refusal that happens too early. |
| Equivalent inputs | Change something the contract says cannot affect the answer. Resolving a client against source or compiled exports, or reversing those exports, must preserve the bound identities or the specified refusal and its site. Choose an equivalence that applies to the layer under review. |

Reproduce the reviewers' cases using the V2 stage entry points. Keep a
regression for each distinct failure. For sign-off, identify the deciding
input, the producer assertion, the consumer assertion and any unresolved
language decision. [Parser handoff tests](layers/01_tokens-to-syntax-trees/tests/indexed_syntax.rs) · [Resolver handoff tests](layers/02_syntax-trees-to-symbol-resolved-trees/tests/stage.rs) · [Package authority tests](layers/03_symbol-resolved-trees-to-typed-trees/tests/calls/package_authority.rs)

## Layers

The front-end split below follows the stage names V1 has used since
2026-09-28 and worker-21 uses now. **Proposed.** Back-end layers (Terminal
Psi to native code) are not planned yet.

| Layer | Input → output | Status | Suite time |
| --- | --- | --- | --- |
| L0 Lexer | source bytes → tokens | done, signed off 2026-11-18 | 0.2 s |
| L1 Parser | tokens → syntax trees | locked, positional rest handoff signed off 2026-11-19 | 1.12 s (previous revision) |
| L2 Resolver | syntax trees → symbol-resolved trees | locked, revised handoff signed off 2026-11-19 | 0.17 s (2026-11-18 warm median, five runs) |
| L3 Typer | symbol-resolved trees → typed trees | locked, revised static integer handoff signed off 2026-11-19 | 0.46 s (previous revision, warm median, five runs of `mbx test --locked -p omega-typer --tests --quiet`) |
| L4 Preparation | typed tree → evaluated, frozen typed tree | open, preparing local signed and unsigned integer constants and closed indices, 1–128 bits | — |
| L5 Lowering | typed tree → Terminal Psi | not started | — |
| L6 Checker | Terminal Psi → checked Terminal Psi | not started | — |
| L7 Publication | checked Terminal Psi → published Psi artifact | not started | — |
| Driver | compile a stage, run it, read its plan; repeat until a plan asks for an artifact | not started; needs L5 and an interpreter | — |

The 2026-11-19 audit changed the L1–L3 handoffs after the sign-offs above.
The owner reopened L1 after the parser refused `let (first, ..) = pair;` while it accepted the same rest in a named pattern. L1 now records the final `..` and its source span. L3 uses the subject type to check the written components and their access. [Pattern specification](wiki/spec/language/patterns.md#irrefutable-data-bindings)
L1 exposes declaration names, ordered binders, the full chain of syntax parents, operators and range endpoints.
L2 binds those names to shared declaration identities and supplies the visible candidates, operator visibility and trait ancestry.
L3 interprets the binders, selects operations from normalized types, and records whether each call belongs to execution, a fact or a static argument.
Its effect checks follow the execution calls. Preparation supplies evaluated values for expressions that typing cannot close itself. The toolchain supplies standard records and machines as Omega declarations. Named calls and bracket reads use the same typed signature and precondition. The last measured front-end readiness result before this sweep was 11 of 12 examples. The conformance example
selects both codec machines across package boundaries and refuses each swapped
selection at its binding expression. The existential package example infers one
`File` witness for the stored reference and callback and rejects a callback over
`Socket`. The bundled reflection module lacks the `field` and `callable` query declarations,
so those examples stop at name lookup. With query declarations supplied, L2 binds all four
published selector operands and retains the type argument in `Buffer::<u32>::used`.
[L2 contract](layers/02_syntax-trees-to-symbol-resolved-trees/CONTRACT.md) · [L3 contract](layers/03_symbol-resolved-trees-to-typed-trees/CONTRACT.md)

### The `omega` application

**Application built through syntax parsing, 2026-11-18.** Build the executable
with `cargo build --release -p omega`, or try it from this checkout with
`cargo run -p omega -- source parse layers/01_tokens-to-syntax-trees/fixtures/accept/types.omg`.
Success is silent. A lexical or grammar refusal identifies the file, line
and byte column on stderr. Jarod's spellings in the Fable session were
`omega run main.omg` and `omega build build.omg`. The command chooses the
operation. The [build model](#v2s-build-model-a-chain-of-stages) records the
entry conventions and which choices about direct execution are still open.

**What V1 actually does.** In V1 at `0b90bdba`, the Rust package
`omega-rust/omega/application` builds the `omega` executable. It parses OS
arguments into a typed invocation. The run command turns that into a
`RunRequest`, calls `run_project`, and presents program output, diagnostics
and process status. The execution code calls `orchestration::build`. For a
project that coordinator prepares packages, executes the Build graph, compiles
the product and handles publication. For Psi execution the runtime consumes
the artifact and provisions from that compilation report. V1's usual build
spelling is `omega <root.omg>`. Its host run defaults to native execution and
also supports Psi and differential runs.
[V1 router](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/src/main.rs)
· [Run command](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/src/commands/run.rs)
· [Run compilation](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/src/execution/compilation.rs)
· [Build coordinator](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/build-system/orchestration/src/build.rs)

| Milestone | User-visible result | Acceptance test |
| --- | --- | --- |
| Syntax parsing (built) | The `application/` workspace member produces `omega source parse <file>`. The file operation reads the bytes and invokes L0 and L1 through their public APIs. | [CLI process tests](application/tests/cli.rs) cover V2 reference syntax, lexical and grammar refusals, invalid UTF-8, caller-relative paths, help, missing files and usage errors. |
| Name resolution (built) | `omega source resolve <file> [<file> ...]` treats the current directory as the package root. The library also accepts direct requester-local dependency aliases and target assembly names. | CLI process tests select a sibling declaration and identify the exact call site when its target moves into a child directory. |
| As L3 through L5 land | Extend stage compilation with typing, evaluation and lowering. Finalize closed application identities after computing their arguments. | A later stage rebinds a serialized checked identity to the intended declaration and refuses a mismatch. |
| First execution | Once L5, a Terminal Psi interpreter and its required admission checks exist, connect the run request to the driver. Start with the permitted constant first plan and the explicit Psi engine. The first direct-run fixture supplies an explicitly named file containing `machine main`; the interpreter receives the product of that invocation. | Run the binary on a file called `hello.omg`, observe exact guest output and status, and verify a compile refusal produces a diagnostic before any guest output. A sibling build-file test fixes the direct-run policy once chosen. |
| Project builds | `omega build <file>` enters `machine build`. The driver compiles and runs each requested stage with its own settled inputs and backings. A terminal artifact request compiles and publishes the product when checking and publication are available. | Exercise the four [build-model cases](#defaults-leaning-not-decided): bind an entry, select a provider, use a generated constant from authored code, and let the build itself write to Console. A cross-build returns its artifact without executing the target product on the host. |

Use V1's process-level tests as references for argument errors, raw output
and guest status, source-located refusals, selected providers and eventually
native/Psi agreement. Use V2 syntax and stage expectations for the fixtures.
[V1 argument tests](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/src/arguments/mod.rs)
· [Dependency diagnostic](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/tests/dependency_refusal_location.rs)
· [Selected provider](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/tests/build_selected_providers.rs)
· [Execution comparison](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/omega/application/tests/both_exit_status_comparison.rs)

**Before documenting bare `omega run` as available**, settle whether a
neighboring build file participates. The [open direct-run question](../OWNER_QUESTIONS.md#open-questions)
records the choice. The [execution spec](wiki/spec/build/embedding.md#standalone-execution-selection)
selects native execution when the engine is omitted. Track the build backings, grants,
stage budget and lockfile details with the existing driver questions below.

### L0 Lexer

**L0 is locked.** Implementation changes must preserve the signed lexer contract.
A change to tokenization, lexical errors, or the information handed to L1 requires
an explicit decision to reopen that contract.

- **Spec owners:** [Lexical profile](wiki/spec/language/data_and_literals.md#lexical-profile),
  [Integer radix spellings](wiki/spec/language/data_and_literals.md#integer-radix-spellings),
  [Quoted bytes](wiki/spec/language/data_and_literals.md#quoted-bytes).
- **Rules the spec states:** source is valid UTF-8 with ASCII syntax;
  identifiers match `[A-Za-z_][A-Za-z0-9_]*`; whitespace is only space, tab,
  CR and LF, and other Unicode whitespace rejects; integer literals in
  decimal, `0x`, `0b`, `0o` with digit underscores and carrier suffixes;
  quoted literals are bytes with seven escapes (`\n \r \t \0 \\ \" \xNN`), and
  codepoint escapes and raw newlines inside quotes reject.
- **Contract:** [layers/00_bytes-to-tokens/CONTRACT.md](layers/00_bytes-to-tokens/CONTRACT.md).
  17 rules, each with its spec source and the fixtures that cover it.
- **Built and signed off 2026-11-18.** Gate results:
  - 7 accept fixtures, 28 reject fixtures and 28 byte-level tests pass.
  - All 132 spec examples lex. Each one tiles its source exactly, and each
    token lexes to itself when taken alone.
  - All 19,442 `.omg` files in V1's `source`, `samples` and `tests/fixtures`
    lex with no refusals: 26 MB and 7.8 million tokens. Throughput is about
    745 MB/s on one thread and 11 GB/s on all 32, up from about 78 MB/s
    at first. On the real path the lexer is not the limit: opening the
    files is, at about 100 MB/s. See
    [layers/00_bytes-to-tokens/EXPERIMENTS.md](layers/00_bytes-to-tokens/EXPERIMENTS.md). Run with
    `cargo run --release -p omega-lexer --example lex_tree -- <directory>`.
    None of those files is lexically invalid, so this checks what V2
    accepts, not what it refuses.
  - 60,000 random short inputs never panic. Accepted ones tile and relex;
    refused ones always name a non-empty range inside the source that does
    not split a character.
  - Seven cases from V1's own lexer tests were added and agree with V1.
  - Six deliberate breakages of the scanner (nesting, CR in quotes, the
    fraction rule, the radix check, the suffix check, the line-comment end)
    were each caught by at least one test.
  - `cargo clippy -D warnings` and `cargo fmt --check` are clean.
- **What sign-off accepted:** the six gap rules G1 to G6 in the contract, on
  the basis that they contradict nothing in the spec. The largest is G1: the
  lexer has no keyword list, so `machine` and `foo` are the same kind of
  token and the parser decides. That hands the question of reserved words to
  L1.
- **V1 and the spec disagree here:** V1's lexer has a `>>` token; the spec
  says "no `>>` token is added". V2 follows the spec.
- **Where this layer departed from the loop above:** the code was written
  before the fixtures, not after, and the gap rules were implemented as
  proposals before Jarod decided them. The byte-level tests were written with
  hand-predicted results before their first run and all passed; the fixture
  dumps were generated and then read line by line against the contract.

### L1 Parser

**L1 is locked.** Jarod signed off the positional rest handoff on 2026-11-19. The parser owns grammatical structure and exposes the written
parts of each construct with their source spans. L2 resolves names and L3
interprets types and contracts. Changes to the accepted grammar or to what L1
hands downstream require an explicit decision to reopen the parser contract.

- **Spec owners:** none as a single document. The spec has no grammar; syntax
  is carried by examples spread across `wiki/spec/language/`.
- **Contract:** [layers/01_tokens-to-syntax-trees/GRAMMAR.md](layers/01_tokens-to-syntax-trees/GRAMMAR.md),
  derived from the spec's prose and all 132 examples, each form citing the
  examples that show it. The parser in `layers/01_tokens-to-syntax-trees/src` was built
  against it on 2026-11-18.
- **Gate passed and signed off 2026-11-18.**

  | Check | Result |
  | --- | --- |
  | Fixtures | 23 accepted trees and 102 exact refusals for whole files. The grammar maps each rule family to its fixtures. Another 35 cases adapted from V1's Psi suite cover malformed input, exact byte spans and V2 verdicts. |
  | Spec corpus | All 132 examples parse. 125 did on the first run; the other seven each showed a gap in the grammar, listed as F1 to F7 in GRAMMAR.md. |
  | Independent check | Trees from fixtures, spec fragments, random sources and token deletions preserve token order and bracket ownership. Printing their leaves and lexing the result reproduces the meaningful token kinds and spellings. |
  | Operator grouping | All 1,600 ordered pairs of binary operator spellings have their specified grouping or refusal. All 36 comparison pairs accept either explicit grouping. |
  | Reference syntax | 72 independently predicted fragment trees check all nine pairs of access modes in types and expressions, across four token separators. Twelve diagnostic cases cover incomplete and retired prefixes. |
  | Hostile input | 128 nested data declarations parse and 129 are refused. 17 recursive constructs nested 100,000 deep are refused on a 1 MB stack. Five ambiguous inputs, 30,000 random sources and 3,515 one-token deletions pass the robustness checks. |
  | Mutation | All 38 deliberate parser changes were caught, including removal of conformance selection rows and two changes to nested `@` after an access word. The runner requires a passing baseline and exits with failure for any missed or unapplied change. |
  | Suite time | 1.12 s median wall clock across three warm runs of `cargo test -p omega-parser --tests` on Windows. Doctests and mutation checks run separately. |
  | Speed | The first three optimizations brought parsing to 571 MB/s on one thread (6.5 ns per token, 8.4% faster than the first build) and 8.1 GB/s on 32 threads. Experiment 9 cut parsing time on individual V2 fixtures by 8.5–11.2% and combined lexer and parser time by 2.6–4.0% across three build layouts. [EXPERIMENTS.md](layers/01_tokens-to-syntax-trees/EXPERIMENTS.md) |

- **Design:** recursive descent, one function per grammar rule, with
  precedence climbing for binary operators. The tree is a flat list of
  events over the lexer's tokens, not allocated nodes. The parser never tries
  one reading and rewinds to another; each choice is made by bounded
  lookahead, because readings that contain each other make a rewinding parser
  exponential on hostile input.
- **Raised by the build, both answered 2026-11-18:** owner question 5
  (explicit comparison grouping, decision 11) and owner question 6
  (prefix `@` for references and borrows, decision 12).
- **What it found:** four places where the spec disagrees with itself, and
  seven spec examples that V1's parser, as read, would refuse. The largest:
  the spec shows an `if` statement that V1 retired, and writes `match` arms
  in a way V1 cannot parse.
- **Decisions:** all twelve grammar decisions are recorded (2026-11-18): no `if`
  statement, `match` and `transition` share one grammar, nothing after a
  `requires` before a body, no reserved words, V1's operator precedence,
  assembly is parsed, and the spec's newer forms are in. Generic arguments
  in expressions are written `::<`. Arms are
  separated by commas. Decision 11 requires explicit grouping of comparisons.
  Decision 12 assigns `@` to references and borrows.

### L2 to L4

**L2 is locked.** Jarod signed off the revised resolver handoff on 2026-11-19.
L2 publishes names, visible candidates and direct package authority. A change
that requires a new fact from L2 must identify the consumer, the missing fact
and the program that demonstrates the gap before reopening this contract.

The [L2 resolver contract](layers/02_syntax-trees-to-symbol-resolved-trees/CONTRACT.md) covers the admitted
stage, lexical and package scope, visibility, domains, operators, requirements,
generic applications and target names. The suite checks source provenance,
input-order independence and the outcome of every spec fragment.
L2 binds the visible overload candidates at a call and any explicitly named operator home. L3 infers an unqualified operator’s home from its normalized operands, checks package ownership for explicit and inferred homes, checks duplicate signatures, and selects the machine at a call. At a generic argument, L2 supplies the visible bindings and the declaration that owns the argument list. L3 selects the type or value binding after applying the enclosing and preceding arguments to the parameter’s bound. L3 and L4 establish closed application identities
after argument values are known. Jarod signed off L2 on 2026-11-18.

The revised resolver carries a public trait's direct parents and a domain's
resolved carrier through the compiled package interface. Clients can bind an
inherited requirement to its original declaration and must have authority to
name the carrier of an imported domain. L3 checks the applied requirement
signature and the qualified value type. [Resolver contract](layers/02_syntax-trees-to-symbol-resolved-trees/CONTRACT.md#handoff-to-typing-and-preparation)

The Build admits direct dependencies. L2 carries that authority forward.
When L3 selects a field or operator on a value returned through another
package, it checks the requesting package's authority to select the owner.
L3 also checks the visibility of the overload selected in a public contract.
Private ranking witnesses stay in the implementation. In a subject arm an
ordinary bare name binds the matched value. A qualified constant tests its
value. L2 publishes the binding or declaration, and L3 checks coverage.
[Package boundaries](wiki/spec/packages/boundaries.md#selection-versus-carrying-a-type)

**L3 is locked.** The first signoff was at `4baf1e5f5e9`. The computed-width case reopened the handoff because typing stopped before L4 could receive the selected call. The revised handoff passed review on 2026-11-19. L3 selects the declaration, types the operands, and identifies the source occurrence, integer carrier and any specialization arguments. L4 evaluates the admitted call. L3 uses the returned value to finish checks that depend on a closed type identity. [Typing contract](layers/03_symbol-resolved-trees-to-typed-trees/CONTRACT.md) · [Preparation contract](layers/04_typed-trees-to-prepared-trees/CONTRACT.md)

Contracts are written when each layer starts. One thing is recorded now
because it shapes what these layers' interfaces must look like.

#### Why V1 loops back

V1 runs the resolver and typer more than once per compile. The spec requires
this, for three separate reasons:

1. **Source discovery.** The full set of source files is not known up front;
   imports found while parsing admit more files. This loops lexing and
   parsing only. (Source: V1 `pipeline.md`, "Package import discovery can
   admit another source frontier". Not yet traced to a spec rule.)
2. **`build.omg` is checked and run in stages.** `build.omg` is ordinary Omega
   that the compiler must check, lower and interpret. The spec requires
   executing "only a completely checked and admitted prefix": when Build code
   registers a dependency, the rest of `build.omg` may use that dependency's
   declarations, so the rest cannot be resolved or typed until the dependency
   is prepared. The Build suspends, the continuation is checked, and it
   resumes without replay.
   ([Checked Build stages](wiki/spec/build/scoped_execution.md#checked-build-stages),
   [Admission before execution](wiki/spec/build/execution.md#admission-before-execution))
3. **Generated source.** `Build::include_source` hands the compiler new source
   bytes after the authored program is already checked. The spec makes these a
   "later resolution stratum": generated code may refer to authored
   declarations, authored code may not refer to generated ones, and final
   checking "extends the retained checkpoint" without rebuilding the authored
   front end. ([Generated-source boundary](wiki/spec/build/execution.md#generated-source-boundary))

   The timing has one visible consequence: **hand-written code cannot call
   code its own build generates.** The authored source is frozen before the
   Build runs, so the Build can inspect it; letting it also depend on the
   Build's output would be a cycle. V1 enforces this: the probe
   `samples/cli/probes/build_retry_reseal/controls/invalid_authored_selects_generated`
   expects `call 'generated_value' selects no declaration`, and the passing
   fixture `tests/fixtures/pass/build/captured_source_generated_constant`
   notes that "no authored body references the later stratum". The spec says a
   generated entry "needs a separately staged compilation" and that "an
   authored wrapper cannot call it".

Jarod, 2026-11-18: source discovery is required, and `build.omg` must be able
to augment itself by discovering files and running build-time code. V2 keeps
both needs but not V1's mechanisms for reasons 2 and 3: under the Build model
below, a later stage does what a suspended continuation or a later stratum
does in V1.

"Deferred bodies" are how V1 implements reason 2. It resolves declarations
first and leaves machine bodies pending, completing only the bodies the
current Build stage needs, because other bodies may name things that do not
exist yet.

#### V2's Build model: a chain of stages

**Decided** by Jarod, 2026-11-18, in two steps: first as two insulated stages
that talk through reflection, then generalized to any number of stages on
Zac's proposal, with refinements from Astra's review. Cracks that show up
later go to the repository's [owner-question queue](../OWNER_QUESTIONS.md).

The rule:

> **A stage starts with its inputs and its service backings settled. Anything
> it discovers or selects goes into the plan for a later compile.**

The driver never suspends a half-checked program, extends what its names
mean, and resumes it.

1. **Every program is a stage, and `build.omg` is not special.** It is an
   ordinary Omega program in its own universe: it resolves its own imports,
   may pull source files into that universe, runs build-time code and
   generates files. It is compiled and run to completion like any other
   program.
2. **A stage can use only the services it was given backings for.** The first
   stage in a chain is given none. It can still compute and return a plan.
3. **A stage's result is a plan, and a plan is inert data.** It holds
   identities, discovered and generated files, selections and requests. It
   never holds code or a live capability.
4. **A plan says how the chain continues.** It asks the driver either to
   compile and run another stage, or to compile an artifact and return it. A
   cross-build therefore never tries to run its product on the host.
5. **Stages talk through reflection, not strings.** A stage imports the
   declaration it means (`use namespace::whatever`), so the reference is
   resolved and type-checked when that stage compiles. Reflection turns it
   into a stable identity that serializes, and the plan carries that identity
   (for example requirement identity ↔ implementation identity). The next
   compile binds the identity to its own copy of the declaration. Importing a
   declaration to name it is not calling it: the spec says "selection alone
   contributes no service reach".
6. **Selecting a backing is not granting a resource.** Reflection answers
   "which `Console`?". The invocation answers "which stream may it write
   to?". The driver checks a plan's selections and requests and supplies only
   the resources the invocation permits. Without this split, a first stage
   with no services could hand its successor any host access it could name.
   A backing that touches no real resource, such as a `Console` that captures
   text in memory, is ordinary Omega all the way down and needs no grant.
7. **The product is compiled once**, from its hand-written, discovered and
   generated files together. Generated files are ordinary inputs, so
   hand-written code may call generated code.

The ordinary chain is three programs: a core-supplied first stage, the
project's `build.omg`, and the product. **A normal project still writes one
`build.omg`.** Another stage is authored only when something learned in this
one changes what the next can compile or call. The chain's start is fixed: a
project that needs a build service the default does not supply does not
replace the first stage; its `build.omg` selects that backing for a stage
after it.

What follows from it:

- **Every layer is a plain function, called once per stage.** No layer takes
  a frozen base or produces an extension, and nothing is suspended. The
  earlier `layer(frozen base, new input)` proposal is withdrawn.
- **The driver is a loop over whole straight compiles:** compile a stage, run
  it with the backings its predecessor selected and the invocation granted,
  read its plan, repeat until a plan asks for an artifact. It is built after
  L5 and a Terminal Psi interpreter exist, and it is not part of any layer.
- L2 identifies a source declaration by its package, namespace, name and
  authored declaration shape. It preserves generic argument expressions.
  After typing and evaluation, the stage can serialize a checked identity
  containing canonical argument values. For example, `Buffer<8>` and
  `Buffer<4 + 4>` share one closed identity. Reflection likewise excludes
  graph-local indices from cross-artifact identity.
- **Each compile checks every identity it is handed** lands on a declaration
  matching the one the previous stage saw. Universes can disagree, since an
  alias may denote different packages in different graphs. A mismatch is an
  error, not a silent rebind.
- **Two ways a chain can fail to end.** A cycle among known prerequisites is
  refused outright. A program that keeps producing fresh stages is bounded by
  a stage budget, and the user gets a trace of what kept running.
- **Known costs:** a stage sees its own copy of a declaration, checked for
  the host. Facts that exist only in a later universe (target layout,
  anything depending on generated code) are not available to it, and a file a
  stage imports cannot depend on files that stage generates. A stage that
  needs such a fact asks for another stage.

What the spec already has: separate universes with different dependency sets
([Two checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts)),
deterministic reflection encodings, typed product operands in `build.omg`
such as `Build::select_provider<windows_x86_64::Console, TestConsole>(builder)`,
and the rule that selection "does not construct provider rows or grant runtime
permission" ([Provider selection](wiki/spec/build/provider_selection.md#provider-selection)).
What differs is listed under [Where V2 departs from the spec](#where-v2-departs-from-the-spec).

##### How the chain starts

The first stage cannot find anything, because it has no services, not even a
filesystem. So it is the **driver**, which is toolchain code and not Omega,
that reads the command line, locates the project and its `build.omg`, and
hands those to the first stage as plain input data. The first stage is a pure
function from "what was invoked" to the first plan: next program is this
`build.omg`, with these default backings selected.

Two things follow:

- **The first stage can start life as a constant inside the driver.** A plan
  is data, so a hard-coded default plan and an Omega program that returns the
  same plan are indistinguishable to everything after them. V2 can build the
  driver with the constant and replace it with an Omega core stage later.
- **What the first stage selects** is the ordinary set of build backings:
  `Console`, reading the project's source, writing build output, and whatever
  else a normal `build.omg` expects to call.

##### Unused selections are inert

A selection for a service the next stage never reaches is not an error and
costs nothing; its backing is never set up. This has to hold, or every
`build.omg` that prints nothing would fail for being handed a `Console`. The
spec's rule is the same: "selection alone contributes no service reach". The
error is the other direction only: a stage that can reach a service call with
no usable backing is refused before it runs.

The identity check follows the same line. An identity whose declaration the
stage never loads is inert. An identity whose declaration the stage does load
must match what the previous stage saw, or the compile fails.

##### Backings do not carry over

A backing selected for one stage does nothing for any other stage. Each plan
selects backings for the next compile only, and that stage's own plan must
select again for the one after it. This follows from the rule (a stage starts
with *its* backings settled) and was confirmed in discussion relayed by Jarod,
2026-11-18. A library helper that says "the usual build backings again" is
fine; silent inheritance is not.

##### Grants only narrow

Relayed by Jarod, 2026-11-18. A grant can only be attenuated: a stage can
pass on what it was given or less, and a virtual filesystem can never be
upgraded to a real one further down the chain.

Where the first narrowing happens is not settled. Two variants were floated:

- **The command decides.** The driver reads the build or run arguments and
  starts the chain with a virtual or a real filesystem.
- **Always real at the top, then attenuate.** The chain always starts with
  the real filesystem, and the core first stage reads the arguments and
  narrows it for the next stage. This keeps the driver dumb and puts the
  policy in Omega code. It also means the first stage *holds* a grant even
  though it has no backings and calls no services: holding a resource to
  pass on is different from being able to use it.

Both obey the rule. This is the second
half of "selecting is not granting": a plan may ask for less than the stage
has, never for more than the invocation gave. The spec has the same idea
under its `staged-build-capability-attenuation` decision.

##### What gets invoked: leaning, not decided

Relayed by Jarod, 2026-11-18, marked "maybe":

- **`omega build build.omg`:** the full chain. The first stage plans for the
  given build program, and that program plans for the product.
- **`omega run main.omg`:** no build program needed. The first plan goes
  straight to the given program with default settings, so a simple program or
  script is a two-program chain. No default build file has to exist; the
  first plan just says what a trivial one would have said. Put the other way
  round: reaching `main` directly means the first stage did its own default
  build, such as selecting a default `Console`.

Commands are always `omega <subsystem> <action>` (Jarod, 2026-11-18). The
command, not the file's name, picks the mode, and there may be no blessed
file names at all: `build.omg` and `main.omg` here are only examples.

This needs no new mechanism. The first plan already depends on what was
invoked, and nothing has to be inspected before the chain starts.

**Entry names are blessed; file names are not.** `omega build <file>` targets
`machine build` in that file and `omega run <file>` targets `machine main`.
Those two names have to stay stable for one reason: the core first stage is
the only program in a chain that must name something it cannot import. It is
handed a file path at run time, so it builds the entry's identity from that
file and a fixed name, where every later stage names its next entry by
importing it. Later entries can therefore be called anything; only the first
needs a convention. Overriding the name with an argument is possible and
probably overkill.

**The first stage names the entry; it does not call it.** The user's file is
not compiled when the first stage runs. The first stage's plan says "this
file, entry `build`", and the driver compiles and runs it.

**One file with both a build entry and a main entry** also falls out, if a
plan names its next program as a set of files plus an entry point, with the
entry given as a reflection identity like every other reference between
stages. Such a file is a `build.omg` whose plan names itself, with a
different entry, as the product: the outer stage runs one entry, and that
entry recurses on the same file with the entry for the actual program.
It is compiled twice, once per stage, each in its own universe. Its one
limit: the whole file has to check in the build stage too, so `main` cannot
use a package the build discovers or code the build generates. A project that
needs either moves its build into its own file. The limit is the same loop
this model removed, showing up where it should.

Pruning by use does not lift that limit, because it can only happen after
typing: knowing what an entry reaches needs resolved and typed calls. So in
each stage the whole file is lexed, parsed, resolved and typed (L0 to L3),
and only what the stage's entry reaches goes on to lowering and beyond. The
other entry's code is checked but never lowered, run or published. The spec
has both halves: "unreachable statements are still source", and source checks
"do not force concrete layouts for unused bodies".

##### Defaults: leaning, not decided

Relayed by Jarod, 2026-11-18, as intuition and not a ruling:

- **Permissive by default:** the real filesystem and no stage budget, with
  command-line arguments to narrow either when someone cares.
- **Interpreting need not differ.** `omega run` still has to run every stage
  up to the product; only the last step changes, from emitting an artifact to
  interpreting the program.
- **One `omega.lock`** for the whole chain.

How this fits the decided model: a permissive default is a default *value* of
what the invocation grants. It changes no mechanism, and selection still does
not grant anything by itself. Two things to decide deliberately if this
becomes the ruling:

- It departs from the spec, whose default is a confined snapshot with host
  effects by explicit opt-in.
- Whether a *dependency's* `build.omg` gets the same permissive grant as the
  root project's. The spec says a root's grant is not automatically available
  to dependency builds.

Still to settle when the driver is designed: the two points above, how a
stage's discovered packages reach `omega.lock` between stages, and the exact
list of backings the first stage selects.

The first test of the model, when the driver is designed, is to rewrite four
real cases in it and see whether any needs something the model cannot give:
bind an entry, select a provider, hand-written code using a generated
constant, and a `build.omg` that itself writes to `Console`.

### L5 to L7

Spec owners are under `wiki/spec/terminal-psi/` (15 files, including a 199 KB
encoding contract). Not planned in detail.

## Spec gaps

This table records findings and their disposition. Open language and architecture
decisions belong in the [repository owner-question queue](../OWNER_QUESTIONS.md).

| # | Gap | Evidence | Affects | Decision |
| --- | --- | --- | --- | --- |
| 1 | No formal grammar. | No grammar productions anywhere in `wiki/spec`. | L1 | Grammar written in `layers/01_tokens-to-syntax-trees/GRAMMAR.md`, all twelve decisions made and signed off. |
| 11 | No limit on nesting is stated. | A parser that follows nesting without limit can be run out of stack by a small hostile file. | L1 | The parser follows 128 levels and refuses more; accepted with L1 sign-off |
| 10 | The spec's examples disagree with each other and with V1's parser. | A `requires` before a body is punctuated three ways in one file; an `if` statement appears once and is defined nowhere; `match` is shown with tuple patterns V1 does not parse. Full list in the grammar. | L1 | Decided with the grammar; V2's copy of the spec's examples is rewritten to match, its prose is not yet |
| 2 | Comment syntax is never defined. | `//` appears on 51 lines of spec examples; the only prose is that non-ASCII bytes may occur "in comments". No `/*` appears. | L0 | Accepted with L0 sign-off (G2): `//` and nesting `/* */` |
| 5 | No list of reserved words. | The spec gives only the shape of an identifier and says "no identifier is reserved for builtin types". | L0, L1 | Accepted with L0 sign-off (G1): the lexer has none; L1 decides |
| 6 | Float literal spelling is not defined. | No prose; examples use `1.0e-48` style only. | L0 | Accepted with L0 sign-off (G3): digits on both sides of the dot |
| 7 | Case of `0x`, `0b`, `0o` and the exponent `e` is not stated. | The spec spells them lowercase and never says uppercase is refused. | L0 | Accepted with L0 sign-off (G4): lowercase only |
| 8 | The punctuation vocabulary is "closed" but listed nowhere in one place. | `language/expressions.md` states the vocabulary is closed and gives only the arithmetic tiers. | L0 | Accepted with L0 sign-off (G5): V1's table without `>>`, plus V2's `@` |
| 9 | Where `_` may appear in a number is not stated. | The spec says "digit underscores" and shows `0x1_0000u32`. | L0 | Accepted with L0 sign-off (G6): anywhere after the first digit |
| 3 | Three links from the spec point at V1 files that no longer exist. | Two under `omega-rust/psi/compilation`, one ELF backend source file. | none | Ignore unless a layer needs them |
| 4 | The spec names pre-resolution evaluation and a post-typing continuation without defining their handoff. | At pinned V1 revision `0b90bdba`, [evaluate_pre_resolution](https://github.com/CathedralOS/Omega/blob/0b90bdbaaa6dbc80c3d9f4be6a1ee75ed954eebb/omega-rust/psi/compilation/pipeline/01_source-compilation/src/evaluation/source_frontier.rs#L364) returns the syntax trees it received and an authority checkpoint. | L2 to L4 | V2 resolves settled inputs and retains argument expressions. Typing and evaluation finalize a closed application's identity before persistence or reuse. |

## Open decisions

None at the plan level.

Settled 2026-11-18, recorded in the decision log: Rust on the stable
toolchain (1.100.0; V1 pins `nightly-2026-09-04`), a `layers/NN-name/` folder
per layer with its fixtures plus `tools/` for shared test tools, and a git
repository. Nothing has been committed yet.

## Where V2 departs from the spec

The spec is the reference except for the departures listed here. Each one was
decided by Jarod. V2’s spec incorporates the reference spelling in row 11.

| # | Spec rule | V2 instead | Decided |
| --- | --- | --- | --- |
| 1 | Generated source is a "later resolution stratum"; hand-written code cannot call it. ([Generated-source boundary](wiki/spec/build/execution.md#generated-source-boundary)) | Generated files are ordinary inputs to the single product compile; hand-written code may call them. | 2026-11-18 |
| 2 | Product operands in `build.omg` are resolved against the frozen, already-checked product while the Build runs. (`build/scoped_execution.md`, "Toolchain operations have designated product operands") | They are resolved in the Build's own universe, carried in the plan as reflection identities, and bound when the product compiles. | 2026-11-18 |
| 3 | The running Build may query checked product facts such as target layout. | Not available during the Build; such a need becomes a later stage. | 2026-11-18 |
| 4 | A Build suspends itself. `suspend Build::await_provider<Requirement, Implementation>(builder)` installs a provider from a build dependency and resumes the same program, and Build code is checked in stages as dependencies arrive. ([Build provider readiness](wiki/spec/build/provider_selection.md#build-provider-readiness), [Checked Build stages](wiki/spec/build/scoped_execution.md#checked-build-stages)) | No suspension. A stage's inputs and backings are settled before it starts; anything it discovers or selects is used by a later stage. | 2026-11-18 |
| 5 | A package has one Build, then its product. | A chain of any number of stages, each an ordinary Omega program. It always starts with a core-supplied stage and then the project's `build.omg`. | 2026-11-18 |
| 6 | The Build has special standing: a toolchain-supplied default environment and a `Build` context only it receives. | `build.omg` is an ordinary program. What it can do is exactly what the previous stage selected and the invocation granted. | 2026-11-18 |
| 7 | Generic arguments in expressions are written with a bare `<`, as in `apply<set_time>(x)`, in every spec example. | They are written `apply::<set_time>(x)`. A bare `<` after a name in an expression is always less-than, so the parser never has to look ahead. Declarations and types keep plain `<`. Decided by Zac and relayed by Jarod. | 2026-11-18 |
| 8 | `match` and `transition` arms are written one per line with no separator, in every spec example. | Arms are separated by commas, with an optional comma after the last. Newlines never matter. An arm has nothing to mark where the next begins, which a clause gets from the next clause word. | 2026-11-18 |
| 9 | `match` and `transition` are described separately, and V1 gives only `transition` tuple subjects. | One grammar for boTesth. `transition` is the form in tail position. | 2026-11-18 |
| 10 | Every binary operator groups to the left, comparisons included (V1's operator table, adopted as grammar decision 6). | The six comparison operators do not take a comparison as an operand without parentheses: `a < b < c`, `a == b == c` and `a < b == c` are refused. Every other operator still groups to the left. | 2026-11-18 |
| 11 | V1 uses `&` for references, borrows and bitwise AND. | V2 uses `@` for references and borrows. `&` remains bitwise AND or domain intersection and `&&` remains Boolean AND. | 2026-11-18 |

## Historical V2 owner rulings

These entries preserve earlier V2 decisions and deferred research. File new
questions in [Omega's root owner-question queue](../OWNER_QUESTIONS.md).

| # | Question | Raised | Affects | Answer |
| --- | --- | --- | --- | --- |
| 1 | May `build.omg` register a build-only package while it runs and then use it? If yes, `build.omg` itself still needs staged checking (reason 2 under [Why V1 loops back](#why-v1-loops-back)). If build-only packages must be known before it runs, the only loop left is source discovery. | 2026-11-18 | driver | Answered 2026-11-18: no. A stage may discover a package and put it in its plan; a later stage imports it. Nothing is used in the stage that registers it. |
| 2 | Where do a Build's own services come from? `build.omg` may import a file that declares or uses a service such as `Console`, but calling it needs a provider, and the decided Build model does not say who supplies one. Raised by Zac. | 2026-11-18 | driver, interpreter | Answered 2026-11-18: from the stage before it. The core-supplied first stage selects the ordinary build backings for `build.omg`, and the invocation grants the resources. See the [Build model](#v2s-build-model-a-chain-of-stages). |
| 4 | How should V2 load source files? Measured on V1's 19,442 files: reading file by file tops out near 90,000 files per second (about 125 MB/s) at 4 threads, or about 115,000 with real-time antivirus off, because each file costs 24 to 30 microseconds of operating-system work whatever its size. Reading the same bytes as one packed file runs at 6 to 9 GB/s. Getting near the drive therefore needs a packed copy of the sources plus a cheap way to see which files changed. Is that wanted, and at which stage? Details in [EXPERIMENTS.md](layers/00_bytes-to-tokens/EXPERIMENTS.md), experiment 6. | 2026-11-18 | driver, source loading | Deferred 2026-11-18, worth revisiting. Not built now: staleness detection is the hard part and nothing downstream exists to show what share of a build reading is. Revisit when a real build can be timed end to end and reading is more than about 10 to 20 percent of it, or when the full test suite's read time is felt. First thing to measure then: how cheaply the OS can report which files changed. See [What the operating system costs](#what-the-operating-system-costs). |
| 5 | Should a comparison be allowed as the operand of another comparison? Today it is, because every operator groups to the left (grammar decision 6). The cost: a generic call written the old way, `apply<set_time>(x)`, is a valid expression, `(apply < set_time) > (x)`, and gets past the parser. The typer will refuse it, but with a message about comparing a machine. Refusing chained comparisons in the parser would catch it there with a message that says to write `::<`. No spec example chains comparisons. | 2026-11-18 | L1 | Answered 2026-11-18, after it was decided, undone and decided again: no. A comparison is not the operand of another without parentheses, for all six comparison operators as one group, so `a < b < c`, `a == b == c` and `a < b == c` are refused (grammar decision 11). The reason: the grouping has to be something the writer wrote. Where the types differ the typer would refuse a chain anyway (`false < 32`), which is why keeping the chain looked harmless. Where all three are booleans nothing objects: `a == b == c` type-checks as `(a == b) == c`, which is not "all three are equal". Rust, Zig and Lean refuse chains the same way; Rocq defines a chain as both comparisons holding; Odin groups to the left. Not adopted: forcing parentheses where `&&` and `||` are mixed. Not a reason: what a V1 user might write. |
| 6 | How should a reader distinguish a nested borrow from an AND operator? | 2026-11-18 | L0, L1 | The initial answer kept prefix `&` and required spacing. Superseded by grammar decision 12: each `@` starts one reference or borrow, so `@@T` and `@@x` express two layers. |

## What the operating system costs

Measured 2026-11-18 on a Ryzen 9 7950X (16 cores, 32 threads) with source on a
Samsung 9100 PRO NVMe, Windows 11, NTFS, over V1's 19,442 Omega files (26 MB).
Everything is from a warm file cache, so the drive itself is not involved in
any of these numbers.

| What | Rate | Against the lexer on 32 threads |
| --- | --- | --- |
| Lexing, 32 threads | about 11 GB/s | |
| Reading one packed file | about 9 GB/s | about the same |
| Reading file by file, best case (4 threads, antivirus off) | about 150 MB/s | about 70 times slower |
| Reading file by file, 32 threads, antivirus on | about 100 MB/s | about 110 times slower |

Windows charges per file, not per byte:

- **About 20 microseconds to open, read and close one small file**, whatever
  its size. About 13 of that is the open.
- **Real-time antivirus adds 4 to 9 microseconds more**, about a fifth.
- **More threads stop helping at 4.** 16 and 32 threads are slower than 4.
- **Opening a directory costs the same as opening a file**, and walking the
  tree to find the files takes 330 ms, longer than reading them.
- **No route from user code is cheaper.** Opening relative to a directory
  handle and calling the kernel directly were both tried and made no
  measurable difference.

Put together: lexing the whole corpus takes about 2.4 ms and reading it file
by file takes at least 170 ms. The compiler is not what a build of many small
files waits for. Full figures and method are in
[layers/00_bytes-to-tokens/EXPERIMENTS.md](layers/00_bytes-to-tokens/EXPERIMENTS.md), experiments
4, 6 and 7. What to do about it is owner question 4.
## Decision log

| Date | Decision | By |
| --- | --- | --- |
| 2026-11-18 | V2 is a new folder built layer by layer, each tested solid before the next. | Jarod |
| 2026-11-18 | Only `wiki/spec` and `wiki/README.md` were copied from V1. | Jarod |
| 2026-11-18 | Spec is the reference; the language guide is for humans. | Jarod |
| 2026-11-18 | V1's `pipeline.md` is a proposed pipeline and is not followed. | Jarod |
| 2026-11-18 | Each layer needs a fast test system. | Jarod |
| 2026-11-18 | Source discovery and a self-augmenting `build.omg` are required. | Jarod |
| 2026-11-18 | Build model ratified: insulated stages plus reflection identities. Cracks return as owner questions in this repo. | Jarod |
| 2026-11-18 | Rust on stable, `layers/NN-name/` layout, git repository; start with the lexer. | Jarod |
| 2026-11-18 | Build model generalized to a chain of stages: every program is a stage, a stage starts with its inputs and backings settled, and selecting a backing is not granting a resource. Proposed by Zac, refined by Astra's review. | Jarod |
| 2026-11-18 | L0 Lexer signed off, with its six gap rules accepted as consistent with the spec. L1 Parser starts with its grammar. | Jarod |
| 2026-11-18 | Eight parser grammar decisions made, recorded in `layers/01_tokens-to-syntax-trees/GRAMMAR.md`: no `if` statement; `match` and `transition` are one grammar; nothing after `requires` before a body; no reserved words; V1's precedence; assembly parsed; `satisfies` expressions and `some` binders in. | Jarod |
| 2026-11-18 | Last two grammar decisions: arms are separated by commas; generic arguments in expressions are written `::<`. V2 may break compatibility with V1's source and the spec's examples freely. | Jarod, with Zac |
| 2026-11-18 | A packed source copy is deferred, not rejected: revisit when a real build shows reading is a meaningful share of it. The per-file cost Windows imposes is recorded in the plan. | Jarod |
| 2026-11-18 | Comparisons do not chain: a comparison needs parentheses to be the operand of another, for `==` and `!=` as well as the relational four. V1 is not an audience; a rule is not justified by what a V1 user might write. | Jarod, with Zac |
| 2026-11-18 | `&&` stays one token (owner question 6); the initial nested spelling was `& &T`. | Jarod |
| 2026-11-18 | Prefix `@` introduces a reference or borrow. `@@T` and `@@x` supersede the spaced ampersands from owner question 6. | Jarod |
| 2026-11-18 | L2 records the visible machines at an overloaded call. L3 selects among them using the typed operands. | Resolver implementation |
| 2026-11-18 | Bind a generic declaration in L2 and finalize its closed application identity after computing argument values, before saving or reusing it. | Jarod |
| 2026-11-18 | Reopen the L1 conformance body rule to accept the documented `Logger::write = write_log;` form. L2 binds its requirement and implementation machine. | Jarod |
| 2026-11-18 | L2 resolver signed off. L3 begins with typed operands and named machine calls. | Jarod |
| 2026-11-18 | A binding shows its numeric type in its declaration: `let count: u8 = 5;` or `let count = 5u8;`. `let count = 5; send(count)` is refused; a later use never chooses the type. | Jarod |
