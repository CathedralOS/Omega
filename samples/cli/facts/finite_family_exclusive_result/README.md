# finite family exclusive result

Owning outcome: **FINITE-GENERIC-DISPATCH**.

A finite-family result retains its exclusive input counter through storage and forwarding, and consuming that result increments the counter once.

The source uses independent static machines under [machine declarations](../../../../wiki/spec/language/machines.md), public input-relative origins under [dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families), and [returned carrier lifetimes](../../../../wiki/spec/language/lifetimes.md#returned-views).

All nine places carry the backing counter into the same selected-result operation: local, field, parameter, call result, match join, const name, a nested machine, another machine and another target body. The const place materializes the named initial array in owned runtime storage. Each place selects both rows and providers, stores and forwards each returned owner through a state transition, and checks that two consumptions increment the same surviving counter exactly twice. The data declaration selects the ordinary `drop_token(value: &mut Token)` cleanup hook. `finish` reads the marker and returns; cleanup of its remaining ticket increments the borrowed counter. There is no explicit increment or disposal call in `finish`.

The preserved baseline compiler accepts update of the valid frozen control, then refuses lowering with `captured reference leaves have no resources`. The fix rejoins the exact existing checked loan by its formation expression, state, statement and result-relative leaf before enclosing destination prefixes. It neither turns public regions into permission suppliers nor changes exclusive access admission. Unknown replacements, mismatching leaves, unresolved selectors and incomplete alternatives retain conservative refusal.

Each place has a twin replacing only the provider's backing with an invocation-local array; the twin must reject with `does not outlive the call`. All sources and oracles are mirrored under `tests/omega/{pass,fail}/facts/finite_family_exclusive_result`.

Run normal `omega update --project <place> --target windows_x86_64`, then `omega run --both <place>/main.omg`. All positive projects bind the four hosted targets and declare exit 0. The local resumption measures Windows x86-64; earlier Linux observations keep their original producer identity and do not establish execution of this repair on Linux, macOS or arm64.

The resumed fixed baseline `7df8b4d92070d83d357849fb2eb77e05c61f36a4` already runs the public local control in both engines, but the retained target-operation audit rejects every place. The repaired call transaction captures result metadata before consuming inputs and retains actual normal-call consumption as ancestry history after returned and stored outputs are installed. Shared copies use the ordinary observed-copy operation, while exclusive transport preserves the original occurrence, backing, access and parents. Original borrowed entry anchors remain available to restoration checks; only their disposal lies outside the local activation. Return-origin validation and the default production pipeline remain unchanged.

Focused producer receipts, complete runtime observations, refusal controls and review are retained in the local claim's ignored `build/slot9-resume-evidence/`. Those earlier observations establish explicit once-only consumption. The current automatic-cleanup programs have their own evidence below.

## Build-owned target-body migration

The `target_body` place carries no source target prefix. Its four prefixed
`helper` bodies became the requirement `SelectedHelper::helper`, implemented by
ordinary `X86...` and `Arm...` implementations and selected by explicit
target-keyed `Build::select_provider` rows (`BUILD-OWNED-TARGET-SELECTION`).
The operation, inputs, contracts and exit oracle are unchanged. The entry
wrapper and each implementation check a target tag through a second selected
requirement (17 for x86-64, 29 for AArch64), so a wrong Build row traps.
Observed with the optimized producer at `11bdd2fe41f` on macOS x86-64: the
migrated positive exits 0 under `omega run --both` (native and Terminal Psi
agree), and the twin still rejects for returning a view of a local that does not outlive the call. No compiler change was needed. The other hosted
targets were not executed here.

## Automatic cleanup qualification

The current programs use the same counter, widths 16/32, implementation choices,
transport and exit-zero checks. Their data owner now selects a nominal cleanup
hook, and returning from `finish` disposes the remaining ticket. The hook must
run exactly once per returned token. Each twin still changes only the backing
to an invocation-local array and must reject its escape.

A focused frozen precursor with the same nominal hook checks with optimized
compiler `538cc2df6785`, but native lowering refuses the hook's read of its
stored pointer: `descriptor storage access conflicts with a retained loan`.
The original lifetime twin still rejects. Its immutable inputs and raw results
are retained in the owning qualification record. At committed inputs `91805a009c1d`, the ordinary CLI already runs all nine new
programs with native/Terminal exits 0, and all nine twins reject. The corpus
checks all nine positives and twins correctly, but its independent target audit
refuses all nine native cases for that same descriptor-storage reason. This is
a false rejection in the audit. No successful corpus execution or repaired
compiler result is inferred. Earlier explicit-settler observations do not
establish automatic cleanup.
