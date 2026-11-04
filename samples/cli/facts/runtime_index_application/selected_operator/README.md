# Selected operators in runtime data indices

Each application captures `origin + 1` through the selected Arithmetic contract,
whose result equals its left operand. RetainLeft therefore preserves limits 9
and 10; builtin addition would produce 10 and 11. A payload call then overwrites
origin with 4294967295. Observation checks the selected descriptor, payload 8,
and the source write. Each twin selects payload_bad, which performs the same
write and supplies 10, violating the default domain.

All nine transport places and exact positive/reject corpus mirrors are present.
Matching-host native/Terminal qualification is required; generated foreign images
supply no host execution evidence. Full RUNTIME-VALUE-GENERICS remains open.

## Build-owned target-body migration

The `target_body` place carries no source target prefix. Its four prefixed
`probe` bodies became the requirement `SelectedProbe::probe`, implemented by
ordinary `X86...` and `Arm...` implementations and selected by explicit
target-keyed `Build::select_provider` rows (`BUILD-OWNED-TARGET-SELECTION`).
The operation, inputs, contracts and exit oracle are unchanged. The entry
wrapper and each implementation check a target tag through a second selected
requirement (17 for x86-64, 29 for AArch64), so a wrong Build row traps.
Observed with the optimized producer at `11bdd2fe41f` on macOS x86-64: the
migrated positive exits 0 under `omega run --both` (native and Terminal Psi
agree), and the twin still rejects because the `Index` literal violates its default domain. No compiler change was needed. The other hosted
targets were not executed here.
