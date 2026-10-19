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

- **Establishment:** `region(base, length, endpoint)` establishes `Region in Bounded` (`no_wrap(region.base, region.length)`) from the checked precondition `embed(base) + embed(length) <= embed(endpoint)`, and `const NAMED: Region in Bounded` establishes it via the declaration-site typed result judgment. Each `<place>/invalid/main.omg` twin weakens only `domain Region::Bounded` to `embed(region.base) <= addr::Bound`, leaving `base + requested` unproved at the exact addition site, with zero post-hoc caller equality guards.
- **Identity and lifetime:** `Region` carries exact `base: addr` and `length: u64` fields; owned moves, record-field projections (`Envelope.region`), and `[copy]` constant substitution retain the `Bounded` domain membership on the exact value identity.
- **Composition:** `cut` and `forward` substitute the actual `Region in Bounded` across parameters, `keep(whole)` result postconditions, `match` arm roster intersections, successor state transitions (`finish`), and all four target-qualified `cut` bodies to discharge `base + requested` directly from `requested < length`.
- **Soundness boundaries:** `no_wrap` is a mathematical `let` definition (`core::Strict<0>`) erased before runtime; domain membership proves numeric non-overflow without manufacturing storage custody or `Granted` authority.
- **Terminal Psi and carrier lowering:** `Region` lowers as a two-field structural aggregate (`addr`, `u64`), and the proved `base + requested` addition lowers to target-neutral exact address arithmetic in Terminal Psi.
- **Native realization, frame/layout encoding, and image emission:** Every positive place and root `main.omg` binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and executes with `omega run --both` exit `0` agreement on both `linux_x86_64` and `windows_x86_64`.

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

