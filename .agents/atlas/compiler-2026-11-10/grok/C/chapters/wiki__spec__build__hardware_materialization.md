# Symbolic hardware materialization

Source: `wiki/spec/build/hardware_materialization.md` lines 1–69 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| HARDWARE_MATERIALIZATION1 | "Runtime-known addresses remain data requiring separate [range authority](../resources/extents.md)." | Symbolic hardware materialization | Runtime-known addresses must stay data, not source-level addresses, and must carry separate range authority as extents specify | not stated | 2 |
| HARDWARE_MATERIALIZATION2 | "Toolchain-known code identities do not become source-visible numeric addresses." | Symbolic hardware materialization | Toolchain-known code identities must not surface in source as numeric addresses | not stated | 9 |
| HARDWARE_MATERIALIZATION3 | "A closed symbolic vocabulary distinguishes sealed data symbols and entry-stub identities." | Symbolic hardware materialization | Materialization must use a closed symbolic vocabulary that separates sealed data symbols from entry-stub identities | not stated | 9 |
| HARDWARE_MATERIALIZATION4 | "A normalized layout places those sources, including split fragments, without user address arithmetic." | Symbolic hardware materialization | Normalized layout must place sealed sources and split fragments without relying on user-written address arithmetic | not stated | 9 |
| HARDWARE_MATERIALIZATION5 | "Each source declares its consumption phase." | Resolution and phases | Every materialization source must declare which consumption phase applies | not stated | 9 |
| HARDWARE_MATERIALIZATION6 | "Resolve at the last legal phase: fixed-image layout, native relocation, loader relocation, or a generated runtime writer." | Resolution and phases | Each source must be resolved at the latest legal phase among fixed-image layout, native relocation, loader relocation, or generated runtime writer | not stated | 9 |
| HARDWARE_MATERIALIZATION7 | "A field consumed by the loader before Omega executes must fit the format's native relocation vocabulary;" | Resolution and phases | Loader-consumed fields required before Omega runs must fit the target format's native relocation vocabulary | not stated | 9 |
| HARDWARE_MATERIALIZATION8 | "a post-handoff table may use a writer." | Resolution and phases | Post-handoff tables may be resolved via a generated writer when that is a legal phase | not stated | 9 |
| HARDWARE_MATERIALIZATION9 | "The materialization plan binds range, alignment, consumption phase, machine regime, and scoped artifact-installation authority." | Resolution and phases | Materialization plans must bind range, alignment, consumption phase, machine regime, and scoped artifact-installation authority | not stated | 9 |
| HARDWARE_MATERIALIZATION10 | "Policy and layout alignment must agree;" | Resolution and phases | Policy alignment requirements and layout alignment must match | not stated | 9 |
| HARDWARE_MATERIALIZATION11 | "concrete-site validation checks the entire occupied range before consumption." | Resolution and phases | Concrete-site validation must check the full occupied range before consumption | not stated | 9 |
| HARDWARE_MATERIALIZATION12 | "A valid layout alone establishes none of those authorities." | Resolution and phases | Valid layout alone must not establish range, alignment, phase, regime, or installation authority | not stated | 9 |
| HARDWARE_MATERIALIZATION13 | "A checked writer derives from the normalized layout and sealed sources." | Generated writers and publication | Checked writers must be derived only from normalized layout and sealed sources | not stated | 9 |
| HARDWARE_MATERIALIZATION14 | "It receives one exact mapped, pinned, writable unpublished placement and a resolver restricted to the admitted artifact." | Generated writers and publication | Writer must get exactly one mapped, pinned, writable unpublished placement and a resolver limited to the admitted artifact | not stated | 2 |
| HARDWARE_MATERIALIZATION15 | "It validates placement and resolves each sealed target once before writing fragments directly to the destination." | Generated writers and publication | Writer must validate placement, resolve each sealed target once, then write fragments directly to the destination | at the operation | 9 |
| HARDWARE_MATERIALIZATION16 | "No numeric entry address or arbitrary-offset writer is exposed." | Generated writers and publication | Source and APIs must not expose numeric entry addresses or arbitrary-offset writers | not stated | 9 |
| HARDWARE_MATERIALIZATION17 | "Target lowering retains the complete plan, placement, source identities, final-content validation, and derived footprint;" | Generated writers and publication | Target lowering must retain full plan, placement, source identities, final-content validation, and derived footprint | not stated | 9 |
| HARDWARE_MATERIALIZATION18 | "compact report/cache keys are not authority." | Generated writers and publication | Compact report or cache keys must not be treated as authority for materialization decisions | not stated | SHARED |
| HARDWARE_MATERIALIZATION19 | "Failure establishes no consumer value." | Generated writers and publication | Failed materialization or publication must not establish any consumer value | not stated | 2 |
| HARDWARE_MATERIALIZATION20 | "Partial bytes cannot become hardware-visible merely by occupying storage." | Generated writers and publication | Partial writes must not count as hardware-visible publication solely from occupying storage | not stated | 9 |
| HARDWARE_MATERIALIZATION21 | "Publication is atomic;" | Generated writers and publication | Publication must be atomic | at the operation | 9 |
| HARDWARE_MATERIALIZATION22 | "restoring the original destination bytes transactionally is not required." | Generated writers and publication | Compiler or runtime need not require transactional restore of pre-publication destination bytes | not stated | 2 |
| HARDWARE_MATERIALIZATION23 | "Consumer validation compares the complete current bytes with the exact writer output before observation; a compact digest alone is insufficient for that join." | Generated writers and publication | Consumer validation must byte-compare full current content to exact writer output before observation; digest alone must not suffice | at the operation | 8 |
| HARDWARE_MATERIALIZATION24 | "The consumer package defines its semantic validator and resulting established value." | Generated writers and publication | Semantic validator and established value are defined by the consumer package, not assumed universal | not stated | 9 |
| HARDWARE_MATERIALIZATION25 | "For an IDT, the OS checks admitted roots, selectors, gate kinds, privilege, IST choices, reserved bits, and canonical base/limit." | Generated writers and publication | IDT admissibility checks listed gate fields are OS responsibility, not automatic from layout | not stated | SHARED |
| HARDWARE_MATERIALIZATION26 | "Layout validity is not hardware-table admissibility, and these policies are not compiler-owned types." | Generated writers and publication | Layout validity must not be equated with hardware-table admissibility; OS table policies are not compiler-owned types | not stated | 9 |
| HARDWARE_MATERIALIZATION27 | "Materialization and installation have separate authorities and receipts." | Generated writers and publication | Materialization authority and installation authority must stay separate, with separate receipts | not stated | 9 |
| HARDWARE_MATERIALIZATION28 | "The writer can write the destination and use its sealed resolver, but cannot publish a table." | Generated writers and publication | Writers may write destination and use sealed resolver but must not publish a table | not stated | 9 |
| HARDWARE_MATERIALIZATION29 | "The installer requires the consumer-established value and independent publication authority; it cannot manufacture the bytes." | Generated writers and publication | Installer must require consumer-established value and separate publication authority and must not synthesize table bytes | not stated | 9 |
| HARDWARE_MATERIALIZATION30 | "It records [external roots](external_roots.md) before the checked publication instruction makes them reachable." | Generated writers and publication | External roots must be recorded before checked publication makes them reachable | at the operation | 8 |
| HARDWARE_MATERIALIZATION31 | "Shared establishment/content/linearity infrastructure does not imply a universal hardware-table/executable typestate algebra." | Generated writers and publication | Generic establishment/content/linearity must not be treated as one universal hardware-table or executable typestate algebra | not stated | 2 |
| HARDWARE_MATERIALIZATION32 | "A post-firmware writer's software-fault-free certificate combines mapped/pinned/ writable destination and stack facts, WCSU provisioning, checked offsets and fragment tiling, admitted CPU support, bounded work, and no suspension, blocking, allocation, dynamic dispatch, or unsupported instruction path." | Bootstrap safety | Post-firmware writer certificate must include listed destination, stack, WCSU, offset/tiling, CPU, bounded-work, and forbidden-path premises | not stated | 10 |
| HARDWARE_MATERIALIZATION33 | "Under those premises deterministic software faults are excluded." | Bootstrap safety | When certificate premises hold, deterministic software faults must be ruled out for the certified writer | not stated | 10 |
| HARDWARE_MATERIALIZATION34 | "NMI, machine check, and physical failure remain explicit boot-envelope assumptions, not proved absence." | Bootstrap safety | NMI, machine check, and physical failure must remain explicit boot assumptions; compiler must not claim their absence is proved | not stated | SHARED |
| HARDWARE_MATERIALIZATION35 | "[Checked assembly](../language/assembly.md) owns source syntax, exact block assertions/clobbers, instruction requirements, regime changes and control flow." | Checked instructions | Checked assembly chapter owns syntax, block assertions/clobbers, instruction requirements, regime changes, and control flow for these sites | not stated | 10 |
| HARDWARE_MATERIALIZATION36 | "Entry/exit operations such as `iretq` and `sysret` are deriver-only, not ways for source to manufacture control exits." | Checked instructions | `iretq` and `sysret` must be deriver-only; source must not use them to manufacture control exits | not stated | 10 |
| HARDWARE_MATERIALIZATION37 | "Provider-only checked `lidt` requires consumer CPU/table publication authority and accounts for its descriptor read, scratch clobber, state change, and reach." | Checked instructions | Provider-only `lidt` must require consumer CPU/table publication authority and must account for descriptor read, scratch clobber, state change, and reach | at a call | 10 |
| HARDWARE_MATERIALIZATION38 | "The compiler owns that instruction contract, not the OS's IDT lifecycle." | Checked instructions | `lidt` instruction contract is compiler-owned; OS IDT lifecycle is not the compiler contract | not stated | 10 |
| HARDWARE_MATERIALIZATION39 | "[Machine-state evidence](machine_state_evidence.md) checks the final artifact;" | Checked instructions | Final artifact must be checked via machine-state evidence per linked spec | not stated | 8 |
| HARDWARE_MATERIALIZATION40 | "prebuilt foreign code uses provider admission." | Checked instructions | Prebuilt foreign code must enter via provider admission, not bypass it | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Runtime-known addresses remain data requiring separate range authority." | Addresses known only at runtime stay ordinary data values in the language; extent/range authority is a separate proof obligation. | "Remain data" only forbids promoting them to static addresses; runtime address types might still exist with bundled authority. |
| "Resolve at the last legal phase: fixed-image layout, native relocation, loader relocation, or a generated runtime writer." | The compiler must pick the latest phase allowed for each source among the four listed kinds. | "Last legal" is per-target policy; multiple sources may resolve at different phases in one plan. |
| "a post-handoff table may use a writer." | Post-handoff tables are permitted to defer resolution to a generated runtime writer. | Any table after handoff may use a writer; earlier phases might also use writers when handoff has not occurred. |
| "Policy and layout alignment must agree;" | Policy-stated alignment and layout-computed alignment must match or compilation rejects. | Agreement is required only at concrete-site validation time, not when the plan is first formed. |
| "concrete-site validation checks the entire occupied range before consumption." | Validation runs on the full byte range the layout occupies before any consumer reads it. | "Before consumption" means before Omega execution only; loader consumption might use a different check. |
| "A valid layout alone establishes none of those authorities." | No authority in the materialization plan may be inferred solely from layout validity. | Layout validity is necessary but authorities must come from separate evidence; whether layout can contribute partial facts is not stated. |
| "It receives one exact mapped, pinned, writable unpublished placement" | Writer gets a single uniquely determined unpublished mapping with pinned writable access. | "One" forbids multiple concurrent writer placements; whether remap/retry after failure is allowed is not stated. |
| "resolves each sealed target once before writing fragments directly to the destination." | Each sealed symbol is resolved at most one time per writer run before writes. | Resolution may happen once per fragment tile; "each sealed target" might allow re-resolution across tiles if targets differ. |
| "Failure establishes no consumer value." | Any failed write or validation leaves consumers without an established value. | Failure before publication only; partial validation success might still block observation without establishing value. |
| "Publication is atomic;" | Hardware-visible publication happens in one step with no observable intermediate table. | Atomicity is logical; concurrent observers might still see stale bytes if they bypass the join rules. |
| "restoring the original destination bytes transactionally is not required." | Implementations need not roll back destination bytes on failed publication. | Rollback is optional; whether partial publication must still be prevented is separate from transactional restore. |
| "a compact digest alone is insufficient for that join." | Join before observation must use full byte equality, not digest-only proof. | Digest may be used for caching if full bytes are also available elsewhere; "insufficient" applies only to the observation join. |
| "The consumer package defines its semantic validator and resulting established value." | Consumer packages supply validators; the compiler does not fix one global established-value type. | Compiler still type-checks a known interface; "defines" might mean runtime package code only. |
| "Layout validity is not hardware-table admissibility, and these policies are not compiler-owned types." | Compiler layout checks do not subsume OS hardware-table policies. | Compiler may still emit diagnostics referencing OS rules; they are not modeled as Omega types. |
| "The writer can write the destination and use its sealed resolver, but cannot publish a table." | Writing bytes and publishing hardware visibility are distinct capabilities on the writer. | "Publish a table" might mean only CPU table-load instructions; writer might still trigger publication through installer-only paths. |
| "It cannot manufacture the bytes." | Installer must receive bytes from materialization path; it cannot synthesize table content. | Installer cannot create bytes from authority alone; whether it may copy from another established source is not stated. |
| "Shared establishment/content/linearity infrastructure does not imply a universal hardware-table/executable typestate algebra." | Generic linearity/establishment types do not automatically model all hardware tables. | A project may define local typestate; "universal" forbids one built-in algebra for all targets. |
| "Under those premises deterministic software faults are excluded." | Certificate is conditional: if premises hold, deterministic software faults cannot occur in the writer. | "Excluded" is a proof obligation the compiler must discharge when certifying, not a runtime guarantee without certificate. |
| "Entry/exit operations such as `iretq` and `sysret` are deriver-only, not ways for source to manufacture control exits." | Source programs cannot emit these; only the deriver may use them. | Source may appear in checked assembly blocks under provider rules; "deriver-only" still allows provider assembly, not arbitrary source. |
| "Provider-only checked `lidt` requires consumer CPU/table publication authority" | `lidt` is only available on provider paths and needs consumer-side publication authority. | Publication authority is checked at call site; whether the provider or consumer names the authority is not stated here. |
| "prebuilt foreign code uses provider admission." | Foreign binaries enter only through provider admission, not ordinary materialization writers. | Admission is required for evidence; materialization rules might still apply to linked fragments separately. |

## Terms used without definition in this chapter

- runtime-known addresses
- range authority
- toolchain-known code identities
- source-visible numeric addresses
- closed symbolic vocabulary
- sealed data symbols
- entry-stub identities
- normalized layout
- split fragments
- user address arithmetic
- consumption phase
- last legal phase
- fixed-image layout
- native relocation
- loader relocation
- generated runtime writer
- format's native relocation vocabulary
- post-handoff table
- materialization plan
- machine regime
- scoped artifact-installation authority
- policy and layout alignment
- concrete-site validation
- occupied range
- checked writer
- mapped / pinned / writable
- unpublished placement
- admitted artifact
- sealed resolver
- sealed target
- numeric entry address
- arbitrary-offset writer
- target lowering
- source identities
- final-content validation
- derived footprint
- compact report/cache keys
- consumer value
- hardware-visible
- publication
- consumer validation
- compact digest
- consumer package
- semantic validator
- established value
- IDT
- admitted roots / selectors / gate kinds / privilege / IST choices / reserved bits
- canonical base/limit
- hardware-table admissibility
- compiler-owned types
- installation
- authorities and receipts
- installer
- publication authority
- external roots
- checked publication instruction
- establishment/content/linearity infrastructure
- hardware-table/executable typestate algebra
- post-firmware writer
- software-fault-free certificate
- WCSU provisioning
- fragment tiling
- admitted CPU support
- bounded work
- suspension / blocking / allocation / dynamic dispatch
- unsupported instruction path
- deterministic software faults
- boot-envelope assumptions
- NMI / machine check / physical failure
- checked assembly
- exact block assertions/clobbers
- regime changes
- deriver-only
- `iretq` / `sysret`
- provider-only
- `lidt`
- consumer CPU/table publication authority
- descriptor read / scratch clobber / state change / reach
- OS's IDT lifecycle
- machine-state evidence
- final artifact
- prebuilt foreign code
- provider admission

CHAPTER COMPLETE
