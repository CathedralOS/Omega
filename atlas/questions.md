# Open questions

The owner has ruled that foundation contains primitives and compilation becomes
one linear pipeline. The choices below apply that direction to code that does
not yet fit it. **Every recommendation is judgment, not an adopted ruling.**
This review changes the plan only. The six decisions in [rulings.md](rulings.md)
remain unchanged.

Source links pin `3f9ce43f0c9d179048911d18faa91ba93f60176d`.
[The plan](plans/foundation-and-backend.md) gives move dependencies and deletion
conditions; the [Omega map](subsystems/omega.md) traces the present route.

## Q-G. Realization is early provider closure and later admission work

Terminal Psi states which outside operations a program requires. Realization
chooses and checks their providers for a particular target. In the bundled
library, [Build selects][console-build] `LinuxX86Console` for `Console` on
`linux_x86_64` and binds `LinuxX86ConsoleIo::write` to system call 1. That is a
source-backed example of a binding, not a report that a sample ran.

| Crate | Rust lines | Responsibility |
| --- | ---: | --- |
| `provider-planning` | 7,231 | Chooses supply for reached requirements and obtains calling-policy plans from prepared execution inputs. |
| `provider-admission` | 26,491 | Checks selected plans, entry and installation evidence; issues admitted records and later binds measured installation facts. |
| `provider-selection` | 1,874 | Joins chosen instances, exact bodies and boundary applications to Terminal declarations. |
| `trust-model` | 708 | Reconstructs obligations from declarations, root grants and provider evidence. |
| `selected-dispatch` | 105 | Retains a realization for the matching target. Both `settle` and public `from_realization` construct `SelectedExecution`. [Source][selected] |

Counts include all tracked Rust files and tests; see [counts.json](evidence/data/counts.json).
The five crates total 36,409 lines. Their folder does not identify one temporal
stage:

- [Provider closure][closure-loop] repeatedly discovers requirements reachable
  through selected exact bodies, prepares policy inputs when that set changes,
  realizes supply and joins selected bodies. It stops when bindings stabilize
  and refuses changes to prior choices. Merely retained library code does not
  create a root.
- [Preparation][provider-inputs] still receives checked source custody and
  accepted publication, as well as Build supplies and target. Calling-policy
  execution uses a retained program and the [Build evaluator][policy-evaluator].
  A proposed source-free settlement input must supply that program and its
  authority; “Terminal Psi + target” is an incomplete account today.
- [Native admission][admission] runs during O00. Some producer work is also
  inside the [native proposal representation][proposal]. Moving the realization
  folder alone would leave that work outside the proposed stage.
- [Installation binding][bound] is later. The root pauses after O02; the
  coordinator emits handler bodies, derives their stack requirements from
  emitted objects, then binds the requirements before root O03. [Source][handlers]
  Cyclic installer/handler resource dependencies are refused.

**Options:** keep realization as an external producer; add only an early
numbered stage; or expose early closure/admission and later binding as separate
transitions, with a representation change for handler dependencies.

**Recommendation:** the third option. Give early closure an admitted input
bundle and carry its selected program and plans. Give binding the current
program's installation demands and measurements when they exist. A numbered
binding stage placed between today's O02 and O03 documents the current order
but does not eliminate child compilation while the root is paused.

**Unverified design work:** inventory numeric installation consumers in O03,
carry symbolic demands and a complete reachable-body roster far enough to
compute frame/stack facts, and bind before the first numeric encoding use. If
selection requires a measurement-dependent choice, that choice needs a later
owner. This contract must preserve existing cycle refusals. The plan must not
claim global linearity until this route is worked through.

## Q-A. Who owns the shared process runner?

`bounded-process` serves package resolvers, the native-provider runtime, CLI
and developer tools. These are host operations, not compiler stages. The
[dependency graph](evidence/data/graph.json) establishes the shared users;
the [boundary checker][boundary] restricts the runtime's permitted owners.
Putting the runner under packages would require changing that rule.

**Options:** interpreter ownership; package ownership with a runtime exception;
or a narrow shared host-support owner outside compilation.

**Recommendation:** the shared host-support owner. Move CLI-only observation
code into CLI, keep the reusable process mechanism together, and explicitly
permit that leaf for its runtime and package users. Its scope must not expand
to source semantics, Build evaluation or compiler coordination. The path name
and precise dependency rule remain a proposed design choice.

## Q-B. Can native consumers read Build and source inputs?

Current source preparation has substantial inputs: `ComputedBuildConfig`
[retains accepted Build source and executed selections][budget-custody], and
[provider-policy preparation][provider-inputs] receives checked source custody.
By contrast, the native request is assembled with [completed scalar values and
selected inputs][native-request], and the [native facade][compile] consumes a
retained Terminal artifact. These are distinct boundaries.

**Options:** permit whole `build-requests` and `source-files` packages on the
Terminal/native side; or split and carry only the semantic facts and provenance
those consumers need.

**Recommendation:** split and carry. A composition mode, target layout fact or
consumed-source identity may belong in a leaf vocabulary, while captured files,
resolvers and executed Build job state stay with their source-side owner. Copy
or share immutable records by their meaning; do not retain a whole earlier
program merely because a later operation needs one field. The exact field
inventory and source-free calling-policy input are still required work.

## Q-C. Who sequences source preparation and Build-time evaluation?

`source-compiler` already [depends on `build-time-evaluation`][source-deps].
The evaluator has production [P04 preparation calls][evaluation-jobs] and
[retained product preparation][evaluation-retained]. Its manifest also names
P02 and P03, but observed P02/P03 lowering calls are test-only; production
[stores a P03 `SeededTypingBase`][seeded-type]. A manifest edge or a type import
is not evidence of a production stage invocation.

**Options:** recognize multiple source sequencers, or separate source
coordination from execution and give continuation one owner.

**Recommendation:** one source sequencer prepares checked evaluation entries,
runs an artifact-only execution service, then resumes with the results. Move
production stage calls out of the evaluator before attempting a dependency
inversion. Simply making the evaluator call `source-compiler` would form a
cycle. The source-preparation job graph and interpreter execution are different
responsibilities; their interfaces need to expose that distinction.

## Q-D. May later forms name earlier types?

A leaf identity definition can be shared without retaining the old program.
The current native vocabulary already uses that distinction. Conversely,
[`EmissionFacts`][emission-facts] retains the optimized abstract program and
passes it around O03–O09 to emission. A chain of re-exports would not change
that access.

**Options:** require every definition to be re-exported by its predecessor; or
allow explicit vocabulary leaves while each stage reads only its declared
current program and carried facts.

**Recommendation:** allow leaves and forbid hidden earlier-program access.
Document which fields each producer establishes and which later consumer needs
them. Sharing unchanged structural declarations may be appropriate; retaining
an earlier executable graph to recover missing facts needs repair. Do not infer
a data-flow defect merely from a foundation-owned type: the declaration-selection
ledger is already [stored in typed trees][composition].

## Q-F. What do layout and frame insertion need from encoders?

O07 encodes instructions; O08/O09 select branch forms and resolve displacement,
and frame application can stretch branches again. O08 also [re-encodes private
callback materializations][callback-encoder] to compare their canonical bytes.
Moving every ISA function into O07 would leave later stages calling a previous
stage.

**Options:** carry encoding alternatives and fixup descriptions; retain a small
shared encoding library as an explicit exception; or combine layout and frame
work with encoding.

**Recommendation:** first prove a carried encoding contract. It needs byte
alternatives, signed ranges, alignment, target-relative bases and bitfield
geometry, as well as callback/image-address fixups and frame-induced changes.
Later stages can then choose and patch without rediscovering target encoding
rules. Preserve the independent verifier's ability to reject changed bytes.

**Unverified:** whether this complete descriptor set can replace every current
post-O07 ISA call without changing refusal behavior. The present reason for
separate frame application—relaxing exact body displacements before adding
frames—must survive. Moving encoders before that proof creates a new forbidden
edge rather than completing the migration.


[admission]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization.rs#L44
[bound]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/provider-admission/src/installation_binding.rs#L60
[boundary]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/tools/xtask/src/boundary.rs#L1
[budget-custody]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/products/src/evaluated_build.rs#L133
[callback-encoder]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/ordinary.rs#L676
[closure-loop]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L260
[compile]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/compile.rs#L14
[composition]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1457
[console-build]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/source/library/std/build.omg#L677
[emission-facts]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L261
[evaluation-jobs]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L633
[evaluation-retained]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/product/retained_provider_selections.rs#L120
[handlers]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L78
[native-request]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/build-system/orchestration/src/compile/native_completion.rs#L100
[policy-evaluator]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/build-system/build-settings/src/calling_policy_execution.rs#L45
[proposal]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/native_proposal.rs#L240
[provider-inputs]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14
[seeded-type]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_frontier.rs#L303
[selected]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/selected-dispatch/src/selected_execution.rs#L14
[source-deps]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/Cargo.toml#L16
