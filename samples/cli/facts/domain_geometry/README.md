# Domain geometry

An admitted no-wrap domain predicate proves `base + requested` fits `addr`
when `requested < length`. This is a numeric prerequisite of Cathedral's
backed allocator, not proof of storage custody or allocator completion.

Acceptance command: `omega run samples/cli/facts/domain_geometry/main.omg`.
The anchor and all nine places return native exit 0 and Terminal exit 0: local,
field, parameter, call result, match join, const name, another state, another
machine, and target body (see [Verification](#verification) for the recorded
targets). Each weakened-domain twin rejects the same exact address addition. The
constant place retains its original `[copy]` carrier, predicate, bound,
initializer and operation.

Qualified constants now check their completed initializer through the ordinary
typed result judgment before publishing qualification. Substitution retains the
exact selected declaration's proved type. This admits the existing mathematical
`no_wrap` definition without assigning a guessed width to `addr::Bound` or
allowing a qualification to prove itself. False predicates and forged issuer
qualifications still reject, including unused constants.

The source corpus mirrors all nine positive programs and their rejection twins
under `tests/omega/{pass,fail}/facts/domain_geometry_through_*`. Additional constant
controls cover selected module identity, qualified aliases and payloadless cases.
Foreign target bodies are source-checked. The recorded native rows and the
host-bound runs below cover `macos_arm64` and `macos_x86_64`; no runtime
observation covers `linux_x86_64`, `linux_arm64` or `windows_x86_64` yet.

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

## Verification

Recorded native rows: `tests/omega/corpus_native_macos_arm64.txt` carries
`pass/facts/domain_geometry_through_*_exit built ... exit:0` for all nine places,
which is the authored result on a host that executes `macos_arm64`.

Verified on Intel macOS (`macos_x86_64`) with a compiler built from
`6e8f4d98f4ad813beba1d27d890b395df6adbede`, `opt_level` 1, binary digest
`bdecc2229b060553`: with this host's `macos_x86_64::ProgramEntry` added to a
scratch copy, bare `omega run --both <place>/main.omg` reports `native exit 0`
and `interp exit: 0` for the anchor and all nine places, and `omega --check
<place>/invalid/main.omg` rejects each of the nine weakened-domain twins with
`exact arithmetic in machine `cut` state `entry` terminal expression may
overflow `addr``. `target_body` additionally carries an identical
`macos_x86_64 machine cut(..)` body in that scratch copy: as authored, its four
bodies are inert siblings on this host and the refusal names them (`cut`, which
is declared only for targets `linux_arm64`, `linux_x86_64`, `macos_arm64`,
`windows_x86_64`).

The scratch copy exists only because the authored places bind the four hosted
targets: an unmodified `omega run --both` selects `macos_x86_64` and refuses
`selected target macos_x86_64 has no bound required root slot` before execution.
Nothing is recorded in `corpus_native_macos_x86_64.txt` for that reason, and
`corpus_gate.py --native` on this host realizes these fixtures for their first
declared target (`macos_arm64`) without executing it, so the authored `exit: 0`
oracle is enforced there rather than blessed.

A supplied `--target` performs compile-publication only. `omega run --both
--target <name>` neither interprets nor executes — `ExecutionOutcome::TargetOnly`
returns before both — so its exit 0 is not an engine observation and must not be
read as one; `build/swarm/probe_fact.sh` reports such runs as `AGREES` and is
therefore not a runtime check. Real observations need the bare form on a copy
that binds the host target.

The nine places were red before `2b96fbda3a1`: the captured computed transition
subject `address == 30` kept the `Unit` inference sentinel, and the unit-plan
builder refused `` `main` has no executable body: this compiler does not lower a
statement of it yet (statement sequence: local data); in state `main::entry`,
statement 2 ``. That commit types a Boolean-valued operator's captured temp from
the operator itself, which is the owning layer; the comparison's result carrier
is not inferred from an operand.

This closes this numeric fact row only. [BUMP-ALLOCATOR-CANARY](../../../../TASKS.md) still requires the
real backed allocator, custody and conservation, exclusive strategy borrowing,
coexisting allocations, failure preservation, cleanup and return, reset only
after every allocation returns, and lifecycle/authority rejection controls.
Within this fact folder, `domain_geometry_conserved_result_exit` and
`domain_geometry_projected_conserved_result_exit` run with native exit 0 and
Terminal exit 0, while `owned_qualified_reference_carrier_exit`, its `field`,
`returned` and `selected` siblings fail at terminal-artifact production with
`Lowering(Unsupported("scalar qualification has no primitive carrier"))`; that
owned-qualification lowering gap is the next remaining requirement of this
capability.

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

