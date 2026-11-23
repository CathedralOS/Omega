# Record: unit 031, second pass (gap_016), area 12-native-representations

Crate: `selected-instructions`, directory `source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions/src/effects/`.
Ranges (from `units/gap_016.tsv`): `catalog/identity.rs` 1-2336, `catalog/validation.rs` 1-3601, `program.rs` 1-63.
Paths below are relative to that `effects/` directory unless they start with `source-rust/`.
Test counts: `grep -c '#\[test\]'` gives identity.rs 25, validation.rs 20, program.rs 29.

---

## 1. `catalog/identity.rs` (lines 1-2336)

Layout: production code is lines 1-779. Lines 781-2336 are `#[cfg(test)] mod tests` (read).

### Purpose
Turns a `MachineEffectCatalog` (the per-target table of what each selected instruction does to memory, traps, calls and registers) into one canonical byte string and hashes it into a `MachineEffectCatalogIdentity`. It also owns the numeric tags that name each instruction family. The same alternative and effect encoders are shared with the plan-level identity (`program/identity.rs`).

### Shape
- `trait MachineIdentityBytes { fn identity_bytes(&mut self, &[u8]) }` (`identity.rs:18`), implemented for `Vec<u8>` (:22), `sha2::Sha256` (:28) and `wire::digest_encoder::DomainDigestEncoder` (:34). It lets one encoder write to a buffer or straight into a hash.
- `machine_effect_catalog_identity(&MachineEffectCatalog) -> MachineEffectCatalogIdentity` (:40). Writes the domain string `omega.terminal-machine-effect-catalog.v43\0` (:44), then target identity bytes (:45), register-constraint catalog bytes (:46), then 38 `Option` selected-key presence flags (:47-87), five `Vec` key-list lengths (:88-100, plus the full key list for `invoke_no_normal` at :93), then `selected_keys.in_identity_order()` (:101-105), then every declaration (:106-181). Ends with `MachineEffectCatalogIdentity::from_canonical_bytes` (:182).
- Per declaration (:107-181): semantic tag (u16 LE) + optional payload byte, constraint key, then one byte each for memory (:111), trap (:137), barrier (:146), call effect (:152-173, with alignment payload), cleanup (:174), then each alternative.
- `encode_machine_alternative_identity` (:185): key, applicability (tags 0-6, :190-241), size (:242-261), latency (:262), then the encoded effects.
- `encode_machine_encoded_effects_identity` (:268): operand read/write lists, three implicit-unit lists, then memory (:277-500), stack (:501-527), trap (:528-536), control (:537-558).
- `macro_rules! effect_tag_arms` (:575-710) is one table mapping every variant name to a u16 tag. It is instantiated twice: `semantic_kind_tag` (:712, `pub(crate) const fn`) over `MachineSemanticKind`, and `alternative_family_tag` (:726, `pub const fn`) over `MachineAlternativeFamily`. `semantic_kind_payload` (:716) and `alternative_family_payload` (:730) add one payload byte for four parameterised families.
- `saturating_family_tag` (:759) and `trapping_family_tag` (:777: `122 + form.ordinal()`). `encode_len` (:740) writes a u64 LE length and calls `.expect("machine-effect catalog length fits u64")`.
- `encode_machine_alternative_key_identity` (:748): tag, optional payload, then the `variant` u32.

### Inputs and outputs
Reads only the in-memory catalog and its sub-enums (`crate::*`, `register_model::encode_constraint_key`). Produces bytes or a SHA-256 identity. It mutates nothing.

### How it decides
It makes no accept or reject decision. It keys every byte on an enum variant or numeric field. The one judgment is which selected keys are bound as presence flags and which are bound by value: 38 `Option` keys go in as `key.is_some()` only (:86). The key values appear through `in_identity_order()` (:101), so identity depends on position in that list, not on which field holds the key. The tests at :1487-1521 show that moving a key between `call_unit`, `call_scalar` and `return_aggregate` leaves the ordered list equal but changes the identity, because the list lengths at :88-100 are also written.

### Cases by shape
- The tag table is a closed list of variants (`effect_tag_arms!`, :575-710). Tags 0-269 are used with gaps. Tests pin them: `PINNED_ALTERNATIVE_FAMILY_TAGS` (:1549, 148 rows) and `PINNED_TRAPPING_FAMILY_TAGS` (:1729, 7 sample rows only).
- Saturating tags come from a hard-coded base per operation (63, 71, 79, 95, 114, :763-773) plus `carrier.ordinal()`. The I32, U64 and a few other carriers are special-cased to older tags (54, 55, 60, 61, 62, :764-768). The test at :1343 asserts 40 distinct tags.
- Atomic tags come from `form.machine_tag()` (:704). The test at :1778 comments that dense atomic tags stop at 229, float-arithmetic is 230, float-result-policy is 231, and the compare-exchange-once family uses 240 (`ONCE_FAMILY_TAG`).

### Same job elsewhere
- The declared trap byte table (:137-145) and the encoded trap byte table (:528-536) use identical numbers for 7 variants of two different enums. Both are in this file.
- `effect_tag_arms!` is applied to two enums in this file (:712, :726). Suspected duplicates outside this unit, not read: `effects/program/identity.rs` (uses the encoders here) and `effects/program/encoding.rs`, which decodes the plan with its own wire codec. `grep -c "encode_machine_alternative_identity|encode_machine_encoded_effects_identity|encode_machine_alternative_key_identity|MachineIdentityBytes"` over `source-rust/` gives 49 hits in 12 files (identity.rs 20, program/encoding.rs 3, program/identity.rs 1, lib.rs 3, effects.rs 3, catalog.rs 2, and `machine-code` files 3+3+6+1+3, `post-allocation-machine/src/identity.rs` 1). I did not read the `machine-code` or `post-allocation-machine` hits, so whether they share this encoder is suspected, not confirmed.

### Unused or one-caller code
- `machine_effect_catalog_identity`: `grep -c "machine_effect_catalog_identity\("` over `source-rust/` gives 36 hits in 4 files: identity.rs 32 (mostly tests), catalog.rs 2, `pipeline/06.../analyses/src/machine_effects.rs` 1, `register-environment/src/tests/abi_call_clobbers/provenance_effects.rs` 1. In production the call is at `catalog.rs:523`.
- `alternative_family_tag`, `semantic_kind_tag`, `saturating_family_tag`, `trapping_family_tag`: `grep -c` over `source-rust/` gives 80 hits in 7 files: register-homes `recovery/selected_instructions.rs` 8, effects.rs 2, catalog.rs 2, identity.rs 55, program/encoding.rs 4, program/identity.rs 7, lib.rs 2.
- `MachineIdentityBytes` is `pub`. I did not count its uses on their own; see the combined count above.
- No comment in the range claims "single" or "canonical".

### Stated limits
None in the range. The domain string carries a version, `v43` (:44), which is the only version marker in the file.

### Fail-open risks
- Encoded-memory tag collision. `MachineEncodedMemoryEffect::ReadTableFieldV1` writes tag 19 plus two payload fields (:278-285). `MachineEncodedMemoryEffect::CatalogFenceV1` also writes tag 19 with no payload (:440). Tag 24 is unused and tags 0-27 otherwise appear once. Both are variants of one enum in one `match`, so the byte stream is ambiguous only if a catalog hashes both shapes in a position where the next fields line up; I did not construct that case. The tests pin tags only 0-10 for memory (:1882-1953), so the 19 collision is not covered. This is a hash-binding weakness, not an accept path. Classification: open (no comment explains it).
- `encode_len` panics (`expect`, :744) rather than failing on a length above u64. Unreachable on 64-bit hosts. Classification: stated by the message only.
- Otherwise none seen.

---

## 2. `catalog/validation.rs` (lines 1-3601)

Layout: lines 1-9 imports. Lines 11-2053 are four `#[cfg(test)]` modules (`atomic_tests` :11-286, `float_result_policy_tests` :287-452, `tests` :453-2053). All production code is lines 2055-3601 (about 1,550 lines). I read all of it.

### Purpose
Checks each `MachineEffectDeclaration` in a machine-effect catalog against the register-constraint row it is attached to and the target architecture. A declaration that claims the wrong barrier, memory effect, trap, call effect, operand custody or control shape for its instruction family is refused. This is the admission gate for catalogs built by the ISA crates.

### Shape
- `pub(super) fn validate_declaration(architecture, &RegisterInstructionConstraint, &MachineEffectDeclaration) -> Result<(), MachineEffectCatalogValidationError>` (:2173-2499).
- `fn validate_encoded_effects(architecture, constraint, declaration, applicability, encoded) -> Result<(), ()>` (:2501-3482), called once per alternative (:2470).
- `fn validate_applicability(constraint, applicability) -> Result<(), ()>` (:3484-3601).
- Helpers: `has_policy_trap` (:2066), `touches_external_state` (:2081), `positional_roster` (:2114), `encoded_scalar_constraint` (:2138), `implicit_roster_matches` (:2145), `call_roster_matches` (:2154).
- Error variants used: `BarrierMismatch`, `InvalidEncodedEffects`, `EmptyAlternatives`, `NonCanonicalAlternatives`, `AlternativeFamilyMismatch`, `InvalidAlternativeApplicability`, `InvalidSizeKnowledge` (all `MachineEffectCatalogValidationError`, defined in `catalog.rs`, not in my range).

### Inputs and outputs
Reads the constraint row (operands with access, class, fixed view, tie, early-clobber; implicit uses, defs and clobbers) and the declaration. Produces `Ok(())` or one error naming the semantic. Mutates nothing.

### How it decides
Everything is keyed on `declaration.semantic` (an enum), operand position numbers, and the target architecture, never on a string or display name.
- Barrier (:2179-2218): expected barrier is computed from the semantic family (external effect, control flow, call, none) and compared by equality.
- Special preconditions for `PortWrite` (x86 only, no operands, no implicit uses or defs, :2224), `AtomicFence` and `MemoryFence` (no operands, no implicit uses, defs or clobbers, and every alternative equal to a fixed effects value, :2234-2259).
- Memory (:2275-2334) and trap (:2335-2394) are computed per semantic and compared to the declaration. Cleanup must be `NoneV1` (:2395). Call effect (:2400-2449) must be the exact variant for the semantic, with a power-of-two stack alignment for call-like ones; foreign calls on a `SystemCall` constraint family must use `SupervisorNormalReturnV1` (:2436).
- Alternatives: non-empty (:2450), strictly sorted by key (:2455), each key's family equals the semantic's family (:2462-2466), applicability is checked (:2467), encoded effects are checked (:2470), size is rejected if 0 or if `maximum < minimum` (:2478-2496).
- Encoded effects, per alternative:
  - Operand-lists must be strictly increasing (:2604-2621). Every listed read must be a `Use`/`UseDef` operand and every write a `Def`/`UseDef` operand (:2622-2639). The write list must equal exactly the `Def`/`UseDef` operands (:2640-2653). The read list must equal exactly the `Use`/`UseDef` operands (:2654-2680). Exceptions: return semantics expect no reads (:2666), and the all-aliased subtract applicability drops the left and right operand from the expected reads (:2669-2675).
  - Implicit defs must equal the constraint's; clobbers must equal the constraint's (or be a superset for `OptimizationSelected`, :2681-2690). Implicit uses must equal the constraint's, except register-indirect returns, which may list a non-empty subset (:2691-2701).
  - Family-specific blocks: copy and zero bytes (:2705-2730), hosted exit, read and write byte (:2731-2767), `InvokeNoNormal`, `CallAggregate`, `CallScalar`, `CallUnit`, `NormalizedForeignCall` (:2768-2868). Each requires specific operand roster, fixed views and control shape.
  - Control-versus-trap consistency (:2869-2877), expected barrier derived from the encoded control (:2878-2919), branch, jump and return control shapes (:2920-2940), floating-control save and restore (:2941-2951).
  - The final `match (encoded.memory, encoded.stack, encoded.trap)` (:2952-3480) is an allow-list of 30 or so tuples, each guarded by the semantic and the declared memory. Anything not matched is `Err(())` (:3479).
- Applicability (:3501-3600): checks that the referenced operands exist, are distinct, have the right access and share a register class. `OptimizationSelected` is accepted only when `optimization.execution_phase() == PostAllocationMachine` (:3595-3599).

### Cases by shape
- Fixed operand layouts, all by position: copy bytes `positional_roster(constraint, 3, 5)` (:3051), zero bytes `(2, 3)` (:3066), scan byte sequence `(2, 5)` (:3107), compare byte sequences `(4, 8)` (:3124), encoded scalar load and store `encoded_scalar_constraint` with six operands (:2138-2143, :3080, :3093). Packed loads and stores require exactly 3 operands (:2579). Hosted exit and write byte require exactly 1 operand (:3009, :3034). Hosted read byte requires none (:3022).
- Temporary stack: exactly 8 bytes for system-register read and write, exactly 16 bytes for directed-rounding float arithmetic, and only on x86-64 (:2219-2223, :2953-2982).
- Return stack pop and call return-address push must have equal stack-pointer view and equal non-zero byte count (:3181-3218).
- `CallAggregate` allows at most 2 result operands (:2796).
- Load widths are an explicit table: LoadPacked3/5/6/7 give 3/5/6/7, Load8/16/32/64 give 1/2/4/8 (:3366-3376). Frame store, `Store64`, is 8 bytes only (:3379). Floating-control save and restore accept 4 or 8 bytes (:2984-2998).

### Same job elsewhere
- `ResultAliasesOperands` (:3540-3560) and `ResultDistinctFromOperands` (:3561-3581) run the same condition. Neither branch tests aliasing versus distinctness; only the identity encoding (identity.rs:207-226) and the subtract-reads exception (:2669-2675) tell them apart.
- Operand-list rosters for calls are checked three ways: `call_roster_matches` (:2154), the generic write/read-list checks (:2640-2680) and per-arm `positional_roster` calls. The generic checks already force reads and writes to equal the constraint's access lists, so the per-arm checks repeat part of that (e.g. :3049-3050 and :3051).
- Suspected duplicate, not read: the same ISA checks may be restated by the ISA crates' conformance tests (`isa-conformance` appears in the crate map as a dependent).

### Unused or one-caller code
- `validate_declaration`: `grep` over `source-rust/` excluding validation.rs gives 1 call, `catalog.rs:521`, plus the `use` at `catalog.rs:463`. Production callers are therefore 1. The same function is called heavily by the tests in this file.
- `validate_isa_machine_effect_catalog` and `validate_machine_effect_catalog` (named in the file header, defined in `catalog.rs:1639`, `effects.rs:20`, `lib.rs:128`) are not defined in my range. The header comment of validation.rs names the former as living here; it does not.
- The comment in `catalog.rs:1587` says separately supplied catalogs enter through `validate_isa_machine_effect_catalog`; I saw that line only in a grep, not as part of this unit.

### Stated limits
None in production code. The file has no TODO or "unsupported" comments.

### Fail-open risks
- **Encoded trap is not tied to the declared trap on rows that fall into the last generic arm** (:3473-3478). That arm accepts `(NoneV1 memory, UnchangedV1 stack, NeverV1 or MayArchitecturalFaultV1 trap)` whenever `declaration.memory == NoneV1`. The declared trap is checked against the per-semantic expected trap (:2390), and encoded trap is checked only against the policy-trap rule (:2869-2877). So a `PortWrite`, `ProcessorHalt`, `ControlRegisterRead/Write`, `PortRegisterRead/Write`, `ModelSpecificRead/Write`, `InterruptControl`, `TaskRegisterRead`, `SegmentSelectorRead`, or a non-x86 `SystemRegisterRead/Write` declaration (expected declared trap `MayArchitecturalFaultV1`, :2339-2361) passes with `encoded.trap = NeverV1`. The reverse mismatch, declared `NeverV1` but encoded `MayArchitecturalFaultV1`, is relied on by the test at :873-947 (jump rows). Classification: traced inside this file for the validator; I did not trace whether any ISA catalog builder produces such rows.
- `AtLeastOneOperandDoesNotAliasView` ignores `excluded_view` (:3582-3594); only operand existence, distinctness, read access and class are checked. A bogus view id passes. Classification: open.
- `OptimizationSelected` applicability checks the phase only (:3595-3599), not that the encoded effects belong to the claimed optimization. Classification: open.
- The `callback_writes` and `table_call` flags (:2260-2274) are computed from the alternatives themselves and then used to pick the expected declared memory and trap (:2276-2277, :2336-2337). A foreign-call declaration therefore gets its expected memory from what its own alternatives say, and is then bound by the tuple arms at :3126-3167. Not an accept hole by itself; recorded because the expectation is self-derived. Classification: traced.
- `Store64` frame-store arm (:3379-3383) does not constrain the `stack_pointer` view or operand counts beyond the generic custody checks. Classification: open.
- `_ => Err(())` at :3479 and `(_, call NoneV1) => {}` at :2443 are closed defaults on the final match. The `(_, NoneV1)` call arm accepts any semantic with no call effect, which is correct for non-call semantics. A call semantic with `NoneV1` is caught earlier at :2412-2429, :2430-2442, :2401-2411.

---

## 3. `program.rs` (lines 1-63 of 1466)

I also read lines 64-1466 (not in my range, no notes beyond what is below).

### Purpose
Header and type definitions for the pre-allocation machine-effect plan: for each selected function, block and instruction, the machine effects the later register-allocation stage must respect. The plan carries the exact target, register-constraint, and machine-effect catalog identities it was built against.

### Shape (range 1-63)
- Header comment (:1-6) names `PreAllocationMachineEffectPlan`, `FunctionMachineEffects`, `BlockMachineEffects`, `InstructionMachineEffects`.
- `pub mod encoding; pub mod identity;` (:8-9) and an inline `mod instructions` (:10-52) holding:
  - `FunctionMachineEffects { machine: MachineId, blocks }` (:25), with an `instructions()` flat iterator (:19).
  - `BlockMachineEffects { block: SelectedBlockId, instructions }` (:31).
  - `InstructionMachineEffects { instruction, kind: SelectedInstructionKind, constraint: RegisterConstraintKey, unit_uses, unit_defs, unit_clobbers, memory, trap, barrier, call, cleanup, provenance, alternatives: Vec<MachineAlternative> }` (:37-51).
- Re-exports (:54-56) and imports (:58-62). Line 64, outside the range, defines `PreAllocationMachineEffectIdentity` with `sha256_identity!`.
- Outside the range, `PreAllocationMachineEffectPlan` is at :79-89 (fields: identity, selected plan identity, optimization unit identity, work schedule, target, register environment, register constraints, machine-effect catalog identity, functions) with `encode`/`decode` at :92-98 that delegate to `encoding::*`.

### Inputs and outputs
Plain data records with `Debug, Clone, PartialEq, Eq`; no logic in the range beyond the iterator.

### How it decides
No decisions in the range. The tests (outside the range) show decode rejects a plan whose recomputed identity differs (`InvalidIdentity`), whose version is not 51 (`UnsupportedVersion`, test at :206-253), or whose work settlements name unclaimed sites (`InvalidField`, :1103-1132).

### Cases by shape
None in the range.

### Same job elsewhere
`InstructionMachineEffects` copies `memory`, `trap`, `barrier`, `call`, `cleanup` per instruction, the same five fields the catalog holds per declaration (`catalog/identity.rs:106-181`). Whether the plan fields must equal the catalog's is checked in `pipeline/06.../analyses/src/machine_effects.rs`, which I did not read (suspected).

### Unused or one-caller code
`pre_allocation_machine_effect_identity`: `grep -c` over `source-rust/` gives 48 hits in 6 files (program.rs 32 in tests, program/identity.rs 9, program/encoding.rs 3, analyses/machine_effects.rs 2, effects.rs 1, lib.rs 1). `PreAllocationMachineEffectPlan` outside program.rs: 42 hits in 10 files, including stages 07 and 08 (`pipeline/07.../allocation/output.rs` 2, `pipeline/08.../machine/mod.rs` 5, `optimization.rs` 3, `plan.rs` 2) and `register-homes/src/lib.rs` 2.

### Stated limits
None in the range.

### Fail-open risks
None seen in the range.

---

## Coverage

- `catalog/identity.rs` 1-2336: read in full (2,336 lines).
- `catalog/validation.rs` 1-3601: read in full (3,601 lines).
- `program.rs` 1-63: read in full (63 lines). I also read 64-1466 (1,403 lines), outside the unit.
- Skimmed: none. Not read: none.
- Total in-range lines read in full: 6,000.

## Five things a maintainer most needs to know about this unit

1. Only `validate_declaration` in `validation.rs` is production code that other files call (one call site, `catalog.rs:521`). About 1,550 of its 3,601 lines are production; the other 2,050 are four test modules placed before the code (lines 11-2053).
2. The identity bytes are version-pinned by a domain string (`...catalog.v43`, `identity.rs:44`) and by many pinned-byte tests. Adding a variant means choosing a tag in the shared `effect_tag_arms!` table (:575) that serves both `MachineSemanticKind` and `MachineAlternativeFamily`, and the pinned table (:1549) must be extended by hand.
3. The validator is an allow-list: the last tuple `match` (`validation.rs:2952-3480`) accepts only listed (memory, stack, trap) shapes, each guarded by semantic and declared memory. A new instruction family with its own memory effect needs a new arm there plus entries in the expected-memory (:2275) and expected-trap (:2335) tables.
4. Encoded-trap checking is weak on the generic last arm (`validation.rs:3473-3478`): a no-memory row declared "may fault" can be encoded as "never traps" and still validate. Separately, `ReadTableFieldV1` and `CatalogFenceV1` both hash with memory tag 19 (`identity.rs:282`, `:440`).
5. `validate_applicability` treats `ResultAliasesOperands` and `ResultDistinctFromOperands` identically (`validation.rs:3540-3581`) and ignores `excluded_view` (:3582-3594), so those applicability kinds are only distinguished in identity bytes and by the subtract special case at :2669-2675.

## What I am unsure about

- Whether any ISA catalog builder (`isa-x86_64`, `isa-aarch64`) produces the encoded-trap-Never rows that the generic arm would accept; I did not read them.
- Whether the tag-19 collision can ever occur in one catalog (whether any ISA uses both a table-call alternative and a catalog fence with adjacent identical bytes); I did not construct it.
- Whether the 148-row pinned tag table covers every `MachineAlternativeFamily` variant except the five parameterised ones; the test at :1704-1727 asserts coverage only for variants reachable from `MachineSemanticKind::ALL`.
- The `machine-code` and `post-allocation-machine` hits for `encode_machine_alternative_identity` and friends were counted but not read, so whether they share the encoder or copy it is unknown.
- `validate_isa_machine_effect_catalog` is named in the validation.rs header but defined in `catalog.rs`; I did not read its body.
