# Direct compiler lattice — active work

Last pruned: 2026-08-28.

This queue closes one artifact sequence. It is organized by producer edge and
the owner of the artifact being produced, not by historical bootstrap scripts,
validation experiments, or compiler generations.

## Fixed sequence

Let `C` be the exact production compiler source closure under `source/omega/`.
It is ordinary Omega deliberately authored with only the language surface
needed to express a robust compiler.

```text
audited Alpha VM seed
    → Alpha assembler + Alpha-written Beta cold start → bc
    → bc builds the canonical Gamma evaluator and type checker
    → Delta-to-Gamma elaboration + Gamma evaluation → delta
    → delta compiles C → omega₀
    → omega₀ compiles the same C → omega
```

`omega₀` and `omega` implement the same full Omega language from the same
source. `omega₀` may lower conservatively; `omega` may use the optimizer and
advanced backend already implemented in `C`. Difficult Omega features may be
absent from the source of `C` even though the resulting compiler accepts them.
That incidental source profile is not a language, dialect, or second compiler.

The artifact chain is the bootstrap. There is no `omega-bootstrap` compiler,
Omega subset language, checkpoint generation, DDC stage, or source directory
for either `omega₀` or `omega`.

## Repository contract

```text
source/alpha/assembler/             Alpha source-to-tape construction
source/alpha/checker/               separate derivation-checker artifact
source/beta/compiler/               bc source, artifact, cold start, admission
source/gamma/                       canonical evaluator and type checker
source/delta/compiler/              delta source, artifact, adjacent admission
source/delta/meaning/               canonical Delta-to-Gamma elaboration
source/omega/                       the one product compiler source closure C
source/omega/psi/                   target-neutral phases inside C
source/omega-rust/                  optional implementation/comparator
tests/omega/                        product-language acceptance cases
tools/lattice/                      replaceable command ordering
```

The Alpha checker is a separate binary from the Alpha VM and assembler. It is a
trust-floor service beside compiler edges, not the compiler that builds Beta
and not another rung. Gamma has no required compiler binary: `bc` builds its
Beta-written evaluator and type checker, which provide the canonical execution
route used to realize Delta.

The artifact being checked owns its validation. Do not recreate generic
`bootstrap/`, `canaries/`, `assurance/`, `refinement/`, `on-ramp/`,
`proof-kernel/`, repository-level `psi/`, or private Omega-corpus owners.
Product compiler implementation belongs to **OMEGA-PRODUCT-COMPILER-SOURCE**
in [`TASKS.md`](TASKS.md), not in this queue.

## Edge status

| Producer edge | Current state | Required result |
| --- | --- | --- |
| Alpha seed and cold start → `bc` | exact source/tape, fixed point, bounded reconstruction | reduce admission size; add the missing checker-calculus derivation |
| `bc` → Gamma evaluator/type checker | canonical Beta-written programs and bounded gates | keep compiler-sized evaluation practical |
| Gamma meaning route → `delta` | exact publication/custody machinery; canonical execution active | finalize repeated execution, realize, verify, install |
| `delta + C` → `omega₀` | source owners fixed; compiler and final `C` incomplete | accept the exact ordinary-Omega surface used by complete `C` |
| `omega₀ + C` → `omega` | model fixed | rebuild unchanged `C` and check the second edge independently |

## 1. Alpha seed and cold start → `bc`

Canonical subjects:

- `source/beta/compiler/bc.beta`: 32,605 bytes;
- `source/beta/compiler/artifacts/bc.tape`: 40,693 bytes;
- exact maximal-observation ROOT: 82,695 bytes,
  `73c5cbcba706f02a0f4fa6877ff9f1a50325ff4ef740f81a3d39a462114eec80`.

- [ ] Reduce the remaining admission implementation without merging distinct
  proof responsibilities. The bounded gate currently has 191 Alpha modules,
  60,458 lines, and a 1,009,325-byte Checker A source. Shape, control, data,
  memory, stack, effect, ranged-store, and meaning modules may share canonical
  decoded facts and structural indexes; they must retain separate semantic
  theorems. Owner-local cursor pooling has removed ten duplicate helper bodies
  and 2,492 source bytes across the parse-procedure and ROOT observation
  families without merging either family's semantic obligations. The frame,
  ranged-store, and stack-custody owners now reuse the already imported exact
  cell-increment primitive for 19 calls, removing three more duplicate helper
  bodies and 235 source bytes without changing the ROOT identity. Expression
  and effect census construction now also share one register-contract prefix
  accumulator for eight calls, removing two duplicate bodies and 286 source
  bytes while retaining separate arrays, terminal checks, and mutation teeth.
  Four more owner-local tails now rejoin identical cursor restoration, operand,
  target, and one-destination checks, removing 583 source bytes without merging
  their memory, effect, transition, or stack classifications.
- [x] Make repeated structural queries O(1) only where the source tables admit
  a proved canonical index. The procedure-span inventory is complete: all 53
  endpoint binders are constant-time and the remaining 44 block identities
  either return a consumed PC or retain an explicit relational boundary. The
  47 expression-census callers now use four checked boundary-prefix tables
  instead of rescanning all 1,236 primitive/push rows. Each family has an
  internal mutation tooth; primitive and push meaning remains with its existing
  owner. The 57 direct effect-census calls likewise use four checked prefixes,
  replacing 80,320 repeated local/memory/transition/event row visits per full
  traversal with one construction and constant-time queries; four independent
  teeth bind those families without moving their semantics. A literal census
  rejected generic pooling across semantic owners and retained only one
  statement-family-local label-suffix literal.
- [ ] **BLOCKED — OWNER Q18:** ratify the generic guarded
  simulation/coinduction judgment and finite certificate shape. Then reconstruct
  the exact compiler proposition below `bc` and check it with the Alpha-owned
  derivation checker. The candidate compiler may not select its proposition or
  accept its own evidence.
- [x] Keep the default edge bounded to cold construction, artifact framing,
  and exact maximal-observation reconstruction. Alternate checkers, fuzzing,
  exhaustive mutations, and developer reports remain optional. The copied
  240-file lattice corpus and its three regex-driven cross-language demo gates
  are retired; focused Omega behavior belongs in `tests/omega/`, and checker
  propositions belong in `source/alpha/checker/corpus/`.

Acceptance: changing a shared compiler macro changes `bc.beta`, one canonical
shape owner, generated identities, and directly relevant semantic obligations.
No cached viewer, receipt matrix, source-row permutation suite, or debug output
is required by the edge.

## 2. `bc` → canonical Gamma meaning

`source/gamma/interp.beta` and `source/gamma/typeck.beta` are the canonical
Gamma programs built by `bc`. Gamma supplies safe definitional evaluation; it
does not contribute a separately published native compiler between Beta and
Delta.

- [ ] Keep the exact compiler-sized evaluation bounded and practical without
  changing Alpha or Gamma meaning, hiding semantics in a runner, or weakening
  evidence joins. A 12-hour ceiling is emergency containment, not an acceptable
  normal gate duration. Profile the exact input before each optimization and
  retain byte-identical output plus focused semantic tests. A live sample of the
  active publication found 86.7% of samples in Alpha dispatch and no allocator
  or kernel hotspot. After that attempt finalizes, the next candidate is to
  cache frame-relative variable byte displacements plus the current value-column
  base, with nested non-tail restoration and tail-transfer tests. That change
  alters the canonical interpreter/tape identities and must start a new pinned
  attempt rather than invalidating the current one.
- [x] The admitted dispatch, fuel-boundary, cached-variable, and canonical-u32
  changes are reflected in the current 50,762-byte interpreter source and its
  72,810-byte tape
  (`37e5610b9bbc487e5140c5071bbf66549da200e7a1df915216658733be50fd58`).
- [x] Retain canonical evaluator input/output at the Delta producer edge and
  evaluator/type-checker source/tape identities at the `bc` → Gamma edge. The
  Delta publication evaluates an already elaborated closed Gamma program; a
  second type-checker execution there would invent another semantic stage.

## 3. Gamma meaning route → `delta`

The canonical source is `source/delta/compiler/main.alp`; the independently
declared lower-rung route is `source/delta/meaning/delta2gamma.beta` followed by
the canonical Gamma evaluator. Publication binds the exact closure and tools,
reconstructs the packed Gamma program, compares repeated assembly observations,
and validates the bounded Darwin ARM64 target dialect.

- [ ] Complete exact attempt
  `cfcaaee8786d3f12b8102140546b7520a3dd661170d50b2187a0858557cd2322`.
  Until both executions finish, do not change any retained source, translator,
  evaluator, compiler, publication tool, or closure-manifest input named by the
  attempt. A bounded smoke execution is not a substitute.
- [ ] When both executions agree, finalize the assembly-publication receipt,
  replay exact realization with the pinned compiler/linker/SDK inputs, verify
  executable identity, and generate the terminal artifact-custody receipt.
- [ ] Install only the admitted result under
  `source/delta/compiler/artifacts/darwin-arm64-v1/`. Retain the unsigned
  `delta-compiler`, assembly-publication receipt, realization observation,
  artifact-custody receipt, one canonical raw execution, and a non-authoritative
  installation manifest. Reconstruct tapes, packed input, decoded assembly,
  ordinal wrappers, and empty diagnostics temporarily. Keep install/verify
  commands under adjacent `validation/`; create no generic evidence archive.
  The atomic six-file installer, reconstruction verifier, and fail-closed
  artifact loader are implemented and tested; the canonical installation stays
  absent until the active exact attempt finishes and its custody receipt passes.
  The initial realization is now an explicit-input, no-discovery command that
  binds stable assembly/toolchain snapshots, exact command order, empty process
  streams, and the existing observation verifier before exclusively publishing
  its four-file staging result; no hand-assembled clang invocation is required.
- [x] Realization replay, strict target validation, source closure custody, and
  reconstruction-bearing receipt machinery are implemented under
  `source/delta/compiler/validation/`.
- [ ] **BLOCKED — OWNER Q16:** ratify the independent Delta v1 language,
  resource, exhaustion, and observation semantics, then bind that subject into
  lower-rooted source-to-artifact refinement. The translator and corpus cannot
  select their own contract. Execution, realization, frontend, and performance
  work remain unblocked.

## 4. `delta + C` → `omega₀`

Delta is an independent robust compiler-host language. It need not be valid
Omega. The Delta compiler needs to accept only the compositional ordinary-Omega
forms actually used by complete `C`; accepted forms retain ordinary Omega
meaning and unsupported forms reject deterministically.

- [ ] Consume the deterministic transitive compiler manifest published by
  **OMEGA-PRODUCT-COMPILER-SOURCE**. Maintain no bootstrap-private source list,
  AST profile, feature list, or checkpoint tree.
- [ ] **BLOCKED — OWNER Q8:** settle requested-target versus source-selected
  target semantics, finalize the durable product build entry, and bind the
  package-resolved manifest for `C`.
- [ ] Derive the exact ordinary-Omega surface used by the resolved closure and
  implement it in Delta with checked semantics, conservative lowering, target
  realization, explicit resource ceilings, and deterministic rejection outside
  that surface.
- [ ] Keep generated/compile-time source, package acceptance, build inputs,
  imported tools, target selection, and emitted-artifact custody explicit. Omit
  interpreters, REPLs, viewers, proof explorers, debuggers, and other tools not
  imported by the compiler executable.
- [ ] Run `delta C → omega₀`, reconstruct and check the exact source/artifact
  edge, retain target dependencies/admissions, and run the compiler acceptance
  suite with `omega₀`.

Acceptance: the first Omega build is one direct Delta invocation over the
product-owned closure. No shell/Python translation, private IR generation, or
second source tree participates.

## 5. `omega₀ + C` → `omega`

- [ ] Run `omega₀ C → omega` without modifying, regenerating, translating, or
  selectively replacing any part of `C`.
- [ ] Reconstruct and check the second source/artifact edge independently.
- [ ] Demonstrate that conservative and production lowering implement the same
  pinned source meaning.
- [x] Treat binary equality and Rust agreement as reproducibility or diagnostic
  evidence only. Correctness comes from checked edges and explicit admissions.

## Tooling and external dependencies

Every required producer, checker, and gate remains directly invocable.
`tools/lattice/` may order exact commands and print failures; it may not parse,
resolve, lower, discover source, manufacture evidence, or make trust decisions.

The first authoritative product build also requires the package/security owner
to publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows are review data rather than acceptance
authority. Track product compiler work in [`TASKS.md`](TASKS.md), package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md), and
language-design blockers in [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).
