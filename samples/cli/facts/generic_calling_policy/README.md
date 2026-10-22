# Generic calling-policy capture

Owning task: **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** in
[TASKS.md](../../../../TASKS.md). The requirement is exact generic named-policy
selection, inline member closure and ordinary target-specific helper composition.

**Fact: GenericPolicy<Box<u64>, 8> places all eight bytes of boundary result 42 before the same value is observed.**

The selected implementation captures the nested type `Box<u64>` and const
`PolicyWidth = 8` in its inline `plan`. That plan calls the ordinary
`value_plan` target family. Each body supplies its matching convention, result
register, initial regime and shadow space to `scalar_plan`. The boundary call
`LocalEntry::value()` returns 42; each place observes that same value and exits
0. The observation checks runtime behavior. It does not establish the static
placement fact: compatible complete byte coverage is checked on the evaluated
plan before realization. Each twin changes only `PolicyWidth` from 8 to 4 and
must refuse because the u64 result's other four bytes have no placement.

## Canonical transport row

Every positive below, including the root, declares `Expected exit: 0` and binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64` and
`linux_arm64`. The nine place sources are pinned as
`tests/omega/pass/facts/generic_calling_policy_through_<place>_exit` with the
Width-4 rejects as
`tests/omega/fail/facts/generic_calling_policy_through_<place>_rejected`.

| Place | Positive | Width-4 reject twin |
| --- | --- | --- |
| Root | [main](main.omg) | Existing [inline const control](invalid_inline_const_exit/main.omg) |
| Local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Another state | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Another target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Run each positive with `omega run --both <place>/main.omg`; check each twin with
`omega --check <place>/invalid/main.omg`. The twin's intended diagnostic is
`calling policy accepted an invalid plan: value 0 placement does not cover every source byte`.
Four entry bindings establish authored target coverage, not native execution on
unavailable hosts. Target compile observations and matching-host execution are
recorded separately.

## Governing contracts

- [Named conformances](../../../../wiki/spec/language/conformances.md#declaration-and-selection): independent explicit implementation telescopes and inline members, with exact package-scoped identities.
- [Calling-plan authorship](../../../../wiki/spec/build/calling_plans.md#authorship-and-evaluation): deterministic admitted plan evaluation; an ordinary helper may construct the plan, and each closed policy application has its own contract.
- [Plan validation](../../../../wiki/spec/build/calling_plans.md#validation): every result byte is placed once and compatibly; registers, regime, shadow space and target applicability remain compiler obligations.
- [Target families](../../../../wiki/spec/language/machines.md#target-scoped-families): every body is checked and published; realization selects the matching body without narrowing source checking.
- [Loan resources](../../../../wiki/spec/terminal-psi/loans.md): shape or logical facts cannot invent reference custody or extend access.

## Requirements and evidence

1. **Establishment.** The checked inline member retains the exact nested type and
   Width-8 application. Evaluation constructs a plan whose result placement
   covers the declared u64 shape; normal plan validation establishes that fact.
   The Width-4 controls break this obligation directly, before the runtime
   oracle. Policy selection supplies no authority for an opaque provider.
2. **Identity and lifetime.** The same selected implementation application owns
   the inline capture and boundary contract. Runtime scalar copies, fields and
   state/parameter transfers retain the returned carrier. The rejection branch
   borrows immutable diagnostic text. Calls and named-state returns preserve
   its literal-issued image backing and substitute every invocation-relative
   lifetime debt. Unknown origins and whole moved subtrees still require their
   complete captured source relations.
3. **Composition.** The selected inline plan composes through an ordinary target
   family and a scalar-plan helper. Boundary result 42 then crosses each row
   place before the same runtime observation. Twins retain that source shape
   and consuming operation, changing only the policy input width.
4. **Soundness boundaries.** Exact family membership is checked before consuming
   an Omega-selected body. Admission still checks all reached bodies for
   termination, linear carriers, service reach, suspension/blocking and authored
   declaration authority. Selection does not turn an external or deferred body
   into executable code. Width-4 result placement must fail independently.
5. **Psi 05–07.** Terminal Psi retains the target-neutral boundary demand and
   checked runtime bodies. Evaluated target policy data remains a separate
   realization input. All closed runtime family bodies still undergo ordinary
   lowering; build-time use does not hide their carrier/custody gaps. `--both`
   is the execution oracle for the resulting Terminal program.
6. **Omega 00–02.** The named application is joined to its exact inline plan;
   Omega chooses target bodies and passes exact family/body handles to shared
   target-neutral admission and evaluation. System V AMD64, Microsoft x64 and
   AAPCS64 policy data supplies compatible u64 result placements. Build entry
   and provider rows remain independent checked inputs.
7. **Omega 03–06.** Realization must preserve the boundary result through each
   place's ordinary calls, record storage, state transitions and register/frame
   homes. This row exercises that path; it does not independently qualify every
   frame-size, register-pressure or instruction family.
8. **Omega 07–09 and emission.** Native execution must observe 42 and agree with
   Terminal execution at exit 0. The native row tests final emitted behavior;
   it does not claim unmeasured instruction encodings or cross-host execution.

## Baselines and current frontier

Original Linux-only baseline: producer `591336bdc9e0b38268cbfa63b17aabb92a5d2389`,
Linux x86-64, verified opt-level 1. The original root runs with native exit 0
and interpreter exit 0 after its ordinary per-checkout std entry review. The
original Width-4 control rejects at byte-coverage validation. Initial missing
entry acceptance is retained separately as setup evidence.

The new frozen local source initially refused before execution: the generic
inline plan reached `value_plan`, whose family head was treated as a bodyless
build-time requirement. Exact selected-body admission/execution now reaches
normal program-entry preparation without that refusal. Native production next
exposed incomplete captured reference origins while lowering a target helper
returning `BoundaryPlanResult`. Exact literal-issued image backing now survives
ordinary call and named-state result substitution without becoming a caller
parameter or manufacturing a loan. The unchanged root then ran with native exit
0 and Terminal interpreter exit 0. Temporary tracing was removed. The root and
all nine canonical positives then passed public `omega run --both` at exit 0;
the nine Width-4 controls rejected at byte-coverage validation. The mirrored
corpus checked every positive and rejected every twin, and all nine new plus
seven retained policy fixtures executed with native/Terminal agreement.
The unranked selected-helper control still rejected for missing ordinary checked
termination. A retained oracle-corruption probe returned 43 against the 42
observer and trapped in both engines; changing that observer to 43 produced
exit 0 in both, independently checking the numerical oracle.

The implementation producer was `54e5330034903b591c3b6c6085c9c1d935e1e5b3`,
verified opt-level 1, binary SHA-256
`9724da4cf6830aae9511c4c7468227d0efe3d969c485eb5277507764df3db6ad`.
Root compilation additionally emitted matching PE32+ x86-64, Mach-O ARM64 and
ELF AArch64 images. Those are compile/emission observations, not foreign-host
runtime evidence. Structured qualification and the claim record retain the
original and integrated producer/input identities separately. Integration,
exact-candidate independent review and authorized main publication remain
required before calling this row complete.

The earlier Linux-specific probes retain additional evidence for two distinct
closed applications, nested const data arguments, type bounds, literal/named
const identity and private constant projection. Their historical observations
are not new four-host or canonical-row execution evidence.

Remaining parent acceptance includes product-producer parity, open const
expressions and package projection, mixed implementation telescopes, other
policy/member kinds, reference-row source migration and complete named member
contract preservation. This row alone does not close the parent task.

Pinned for this host at base `c1c6bc8cb8ec` (fixture rename only; compiler
unchanged): the corpus recorded 9/9 positives `checked` with
`check-target:macos_arm64` and 9/9 Width-4 twins `rejected` with the
byte-coverage diagnostic in `corpus_outcomes.txt`, 9/9 native
`built ... exit:0` rows in `corpus_native_macos_arm64.txt` and 9/9
`exit:0 interpreter-exit:0 interpreter:agrees` rows in
`corpus_both_macos_arm64.txt`. The rename guard kept all 36 rows byte-identical
apart from their key text, and a post-rename diff run reported 0 structural
moves with 0 twins rejecting without their expected fragment.
