# Legacy Gamma and state-machine Delta retirement

The unselected concatenative route is removed as one consumer closure, not
replaced by a new language. [D88](../architecture/bootstrap_chain/decisions.md#d88--typed-functional-gamma-is-evaluated-directly-by-beta)
selected the Beta-written functional Gamma evaluator and classified the old
compiler as evidence, not authority. The
[minimization contract](bootstrap_minimization.md) and
[cost review](bootstrap_cost_review.md#legacy-route-retirement-must-follow-its-consumers)
require preserving useful assertions and findings before coordinated retirement.
Earlier retention tables requiring a new admitted edge are superseded here as
maintenance conditions; no admitted edge or semantic contract is weakened.

The removed sources, test vectors, profiles, receipts, and tape remain recoverable
from Git revision `f8355fbad1af7672812b6eab66f8309c3c34ffee` using
`git show REVISION:PATH`. This review is not a replacement executable route.

## Exact removal boundary

| Owner at the archival revision | Removed material |
| --- | --- |
| `bootstrap/2_gamma/bootstrap/concatenative/` | Nine files: old language/profile, compiler and evaluator sources, compiler Beta receipt, Alpha tape, and READMEs. |
| `tests/gamma/` | Eighteen files: `evaluator-slice.sh`, `evaluator-reconstruction.sh`, `evaluator_reconstructor.gamma`, `compiler-fixed-point.sh`, `state-machine-customer.sh`, all of `fixtures/`, `gamma1-augmentation-experiment/`, and `gamma-to-beta-experiment/`. |
| `tests/delta/state-machine-experiment/` | Ten files: speculative compiler, seven customer fixtures, gate, and README. |
| `tools/bootstrap/gamma/artifact_env.sh` | Sole materializer for the removed Gamma compiler. |

Total: **38 files / 339,616 bytes**, including a 26,674-byte binary tape.
The seven legacy role assignments/exports and their positive hygiene requirements
are removed too. Searches of literal paths, role variables, and materializer calls
found no selected-chain consumer outside this closure. No fixture move is needed;
selected scalar and augmentation sources already have selected test owners.

The selected Beta root, Gamma evaluator source/tape/profile, functional Delta
compiler, Epsilon evaluator, Omega D, derivation checker, and Beta encoding
definitions are unchanged. Executable comparisons did not prove those edges;
deleting comparisons does not prove them either. P1 proof closure, selected
resource/refinement conformance, complete Epsilon/D, and eventual Omega self-host
remain separate unfinished obligations on [the board](../../../TASKS_BOOTSTRAP.md).

## Assertion dispositions

| Legacy assertion family | Selected coverage or reason to retire |
| --- | --- |
| Literals, calls, forward names, lexical binding, branches, signed operations, comments, I/O and bounds | [Selected Gamma behavior gate](../../../tests/gamma/evaluator-development/run.sh). Explicit comments, false branch, signed-negative comparison and signed division controls migrate into this gate. |
| Malformed source/envelope; duplicate or missing main, unresolved names/arity; arithmetic traps | Same gate, including new source-NUL and three malformed-framing cases. Whole-program static rejection supersedes reached-only legacy failures. |
| Deep tail recursion and context exhaustion | Same gate retains the 100,000-step proper-tail case and exact/adjacent selected context controls. The old ordinary-recursion capacity is not a language requirement. |
| Output before a subsequent trap | [Composed artifact gate](../../../tests/gamma/composed-artifact.sh) requires quiet atomic nonpublication. The old partial-prefix expectation contradicts the selected contract and retires. |
| Scalar recursive/surface receipts returning `0f`/`15` | [Staged Delta gate](../../../tests/delta/staged-compiler/run.sh) owns selected `scalar_recursive.delta` and `scalar_surface.delta`. Old concatenative receipt bytes/hashes retire. |
| Source augmentation | [Selected augmenter](../../../tests/gamma/self-augmentation-experiment/run.sh) retains exact 51-byte expansion and execution to 42; selected evaluator validation also runs this case. Old mutable-cell expansion syntax retires. |
| Old evaluator reconstruction and Gamma compiler fixed point | These are unselected artifact identities, not selected premises. [Functional evaluator reconstruction](../../../tests/gamma/functional-evaluator.sh) and the admitted Beta root remain. |
| Stack/cell/jump/word primitives, output-position assertions, wide-hex rejection | Retired language semantics. Selected Gamma has neither these primitives nor the old numeric-width trap; its integer parsing wraps. |
| State-machine Delta globals, fields, scopes, calls, exhaustive transitions and storage checks | This was a different speculative language. Selected functional Delta owns its actual typing/arity/scope/match checks in `staged-compiler/`; Epsilon owns mutable state/storage. The old thirteen-argument and multiword restrictions are not selected requirements. |
| Toy nested parser and postorder AST transformer | Their private input grammars, node/depth budgets, and exact error offsets retire with the experiment. Actual Delta/Epsilon parsers and recursive-data customers remain; they do not claim the same toy byte oracle. |
| Toy symbolic Alpha encoder and payload boundary | Direct Alpha encoding is not selected Delta's responsibility. The Beta encoder and Omega D's `compiler/alpha_tape.epsilon` own their respective actual contracts. Existing D buffer tests do not establish complete D opcode encoding; that obligation remains open. |
| Native/interpreted agreement, hardware/software frame thresholds, tape addresses and private size pins | Implementation-specific comparison evidence. Selected evaluator containment/resource tests cover its own layout; prototype equality and fixed capacities supply no selected proof premise. |

## Findings retained without an alternate compiler

The concatenative route cost 2,337 authored lines above the common Beta compiler
in D88's matched comparison: 753 Beta evaluator, 725 Gamma compiler, 193 Gamma1
lowerer, and 666 scalar/effect seed. The selected direct evaluator is now 1,632
Beta lines. These are historical matched components, not total repository counts.

The final state-machine Delta prototype was 815 Gamma lines / 32,916 bytes and
generated a 29,105-byte native tape. Its compact initial sample hid costs absent
from a functional compiler: a small recursive helper needed 48 lines/10 states
against nine functional lines; the recursive AST customer required 427 lines/80
states. The symbolic encoder needed 552 lines/106 states; its fixed arenas ended
at byte 118,488,640 including the 1 MiB static base. Its approximately 1 MiB output
comparison did not establish a
representative variable-length 16 MiB compiler path. Thirteen-argument call support
added 106 compiler lines and 4,001 native bytes without solving immutable recursive
`Bytes` handling. D71 through D85 retain the detailed comparison chronology.

These costs do not justify replacing selected functional Delta with the prototype
or maintaining both. Private capacities may increase when customer measurements
justify the whole-chain cost; neither old size pins nor current provisions are
language laws. A new language or replacement rung still requires an owner decision.
