# Domain geometry

An admitted no-wrap domain predicate proves `base + requested` fits `addr`
when `requested < length`. This is a numeric prerequisite of Cathedral's
backed allocator, not proof of storage custody or allocator completion.

Acceptance command: `omega run samples/cli/facts/domain_geometry/main.omg`.
The anchor and all nine places return native exit 0 and Terminal exit 0 on
Linux x86-64: local, field, parameter, call result, match join, const name,
another state, another machine, and target body. Each weakened-domain twin
rejects the same exact address addition. The constant place retains its original
`[copy]` carrier, predicate, bound, initializer and operation.

Qualified constants now check their completed initializer through the ordinary
typed result judgment before publishing qualification. Substitution retains the
exact selected declaration's proved type. This admits the existing mathematical
`no_wrap` definition without assigning a guessed width to `addr::Bound` or
allowing a qualification to prove itself. False predicates and forged issuer
qualifications still reject, including unused constants.

The source corpus mirrors all nine positive programs and their rejection twins
under `tests/omega/{pass,fail}/facts/no_wrap_domain_through_*`. Additional constant
controls cover selected module identity, qualified aliases and payloadless cases.
Foreign target bodies are source-checked; runtime observations here cover Linux.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root (anchor) | [source](main.omg) | - |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) |
| other state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) |

This closes this numeric fact row only. [BUMP-ALLOCATOR-CANARY](../../../../TASKS.md) still requires the
real backed allocator, custody and conservation, exclusive strategy borrowing,
coexisting allocations, failure preservation, cleanup and return, reset only
after every allocation returns, and lifecycle/authority rejection controls.

## Requirements and Evidence

Owning task: **BUMP-ALLOCATOR-CANARY** ([TASKS.md](../../../../TASKS.md), P1). This row discharges the numeric address-geometry prerequisite (`base + requested` in `addr` proved from `Region in Bounded` and `requested < length`) across all nine transport places.

- **Establishment:** `region(base, length, endpoint)` establishes `Region in Bounded` (`no_wrap(region.base, region.length)`) from the checked precondition `embed(base) + embed(length) <= embed(endpoint)`, and `const NAMED: Region in Bounded` establishes it via the declaration-site typed result judgment; layout geometry queries and domain bounds establish exact compile-time and static byte extent and alignment relationships (`base + requested <= addr::Bound`) directly at definition and entry sites without post-hoc caller equality guards.
- **Identity and lifetime:** `Region` carries exact `base: addr` and `length: u64` fields; layout geometry and domain facts remain tied to the exact type/carrier and scalar subjects across owned moves, record-field projections (`Envelope.region`), and `[copy]` constant substitution without losing `Bounded` domain membership across all 9 places.
- **Composition:** `cut` and `forward` compose and substitute the actual `Region in Bounded` through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact address addition (`whole.base + requested`) proved directly from `requested < whole.length`; `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the geometry/domain bound (`embed(region.base) <= addr::Bound` instead of `embed(base) + embed(length) <= addr::Bound`), leaving addition unproved without any caller guard difference.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed when the geometry/domain bound is weakened or exceeded, rejecting at compile time with `exact arithmetic in machine ... may overflow \`addr\``; `no_wrap` is a mathematical `let` definition (`core::Strict<0>`) erased before runtime; proof preserves mathematical integrity while preventing out-of-bounds address synthesis without manufacturing storage custody or `Granted` authority.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** `Region` lowers as a two-field structural aggregate (`addr`, `u64`), layout geometry queries and domain bounds (`no_wrap`, `Strict<0>`) lower into canonical target-neutral `PSITERM\0` with mathematical and proof-only bindings erased while preserving runtime carrier representations and exact address arithmetic operations, executing identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), target layout plans (`size_of`/`align_of`, field/bit offsets for `Region.base` and `Region.length`, `IntegerAt` stored widths) evaluate and validate without reopening source, and `CallPlan`/`StatePlan` lower aggregates and scalar carriers across target registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64`/`arm64`) materializing layout constants and exact address addition (`base + requested`), assigns register homes and disjoint stack-frame activation storage, composes bounded activation claims, and inserts machine entry prologues and epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and branches relax with offset reflow after prologue/epilogue insertion, transitive machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission executes and agrees with Terminal Psi (`omega run --both` exit 0) across all hosted targets, including `windows_x86_64`, `linux_x86_64`, and `macos_arm64`.

The match repair retains predicate-qualified ownership sources and intersects
the actual incoming membership rosters at the join. Distinct-source and nested
local joins, three distinct incoming owners, and shared-reference joins also run
in both engines. A last unqualified arm, reuse of a moved source, shared-to-mutable
borrow escalation, and routed-authority forgery reject. Wrapping captures and
mutated captures cannot supply the mathematical fact. The original match source
was not rewritten to obtain this result.

The real `Extent` split now passes address formation but still rejects child
`Granted` construction and untransferred parent custody. It remains the customer
after this numeric repair; neither domain geometry nor this sample creates a
storage grant.

