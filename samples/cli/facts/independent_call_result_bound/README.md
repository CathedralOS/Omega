# Independent completed-call result bounds

An independent completed call guarantees value == 7, so exact value - 7 is safe.
Every place uses that fact and operation. This row guards the contract checking
prerequisite discovered while advancing **CML4**; it does not close residual
cleanup acceptance.

Owning outcome: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
The supplier is one boundary requirement, `IndependentSource`, with five ordinary
named implementations. `WindowsSource`, `LinuxSource`, `MacosSource` and
`LinuxArmSource` are selected by Build rows for their hosted targets;
`ReservedSource` is supplied and selected by no row. Each implementation returns 7
and publishes `ensures result == 7`. The wrapper `forwarded` publishes the same
guarantee by calling the requirement, so it imports only the requirement's public
contract.

The requirement this row records: **every supplied implementation is checked
against the requirement it satisfies, and a call cycle through the providers a
Build row selects is refused.** Psi checks each named implementation under its
ordinary identity whatever the Build selects
([multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)).
A call to the requirement is a dependency on that requirement, not on every
implementation that could satisfy it
([`applied-provider-cycle-selection`](../../../../wiki/spec/language/effects.md#direct-synchronous-invocation),
settled 2026-11-12), so `ReservedSource` adds no call edge. Each invalid twin
changes only the selected `WindowsSource`'s body to call the wrapper: the
`windows_x86_64` row then realizes the cycle `forwarded` -> `IndependentSource`
-> `WindowsSource::source` -> `forwarded`. The required rejection is
``selected providers realize a cyclic synchronous `invokes` graph``. Host
implementations, consuming subtraction and caller control flow remain identical.
Ordinary guarantees are partial correctness; nontermination alone is not a reason
to withhold a separately established promise.

## Earlier twins

Before this change each twin changed the unselected `ReservedSource`'s body to call
the wrapper and pinned `cannot prove ensures contract for exit from forwarded`.
The tree-shaped checker refused those programs because `requirement_backing_machines`
made a requirement depend on every supplied satisfier. That checker was deleted in
#2823, and `applied-provider-cycle-selection` had already replaced its rule:
eligibility adds no call edge, and composition checks the selected graph. The
lowered-form checker follows the ratified rule and accepts those programs, so the
twins now place the cycle in a selected provider.

Checking for one target checks every row, so `omega --check --target
linux_x86_64` refuses these twins too, naming the `windows_x86_64` Build row.

| Place | Transport |
| --- | --- |
| [local](local/main.omg) | copied scalar |
| [field](field/main.omg) | record field |
| [parameter](parameter/main.omg) | authenticated argument requirement |
| [call result](call_result/main.omg) | direct completed call |
| [match join](match_join/main.omg) | either returned arm |
| [const name](const_name/main.omg) | the same bound named by `FLOOR` |
| [a nested machine](other_state/main.omg) | checked state arrival |
| [another machine](other_machine/main.omg) | forwarding result contract |
| [another target body](target_body/main.omg) | four Build-selected `Evaluation` implementations |

Each place has `invalid_<place>/main.omg`. The root duplicates local. Every
positive binds all four hosted ProgramEntry targets, expects zero and must run with
`mbx xtask sample-gate --filter samples/cli/facts/independent_call_result_bound`. A returned difference is tested only after the subtraction;
that observation does not supply its formation proof. Replacing the final
`difference == 0` with `difference == 1` makes every place trap on both engines.

Requirements exercised:

- Establishment: an independently justified completed-result contract supplies
  the singleton used by Exact subtraction; no caller guard reconstructs it.
- Identity and lifetime: the row exercises capture and copying of the completed
  result. Exact call identity and mutation invalidation remain implementation
  obligations; these uniform singleton results do not independently distinguish
  different invocations or later writes, so a selection mix-up between the four
  identical hosted bodies is not observable here.
- Composition: all nine transports carry the same bound into the subtraction.
- Soundness: every supplied implementation is checked against the requirement it
  satisfies, and a cycle through a selected provider is refused. Copying result
  orderings into a
  synthesized return local cannot hide a citation cycle. Exact-edge induction,
  declared carrier ranges and body inference remain separate.
- Psi 06–08: proof decisions erase without changing the copied scalar,
  branch choice or outgoing result; Terminal execution checks that value.
- Omega 00–02: checked providers and ProgramEntry settle through the ordinary
  Build companion, one selected implementation per hosted target; integer and
  record layouts retain their declared ABI.
- Omega 03–06: subtraction, calls, state arguments, record homes and joins
  preserve the value across registers, activation frames and returns.
- Omega 07–09 and emission: encoded native control and result values must agree
  with Terminal execution. An intentionally wrong final oracle must trap in both.

## Evidence and open scope

On the Windows x86-64 workstation, with a debug producer built from
`42ce2aad3206` plus this change, `omega --check --target windows_x86_64` checks
the root and all nine positives and refuses all nine twins with the fragment above.
An earlier Linux x86-64 run (`1efc2536b3bc`, opt-level 1) observed native and
interpreter exit 0 for all nine positives from the native/Psi comparison, and a trap on
both engines for all nine corrupted-oracle controls; it predates the re-expressed
twins. Windows and macOS execution of these sources is not observed; binding a
hosted target does not establish execution on that host.

## Generic evidence and normal completion

The earlier generic-evidence reproduction does not demonstrate a missing
citation dependency. Its exact selected cycle (`CircularSource::source` calling
`forwarded<CircularSource>`, which calls that same source) has no normal return.
The completion owner establishes that control fact without importing a guarantee,
so its `ensures` is vacuous under ordinary partial correctness. Withholding a
cyclic guarantee must not manufacture an exit obligation for that program.

A returning base path distinguishes that case from a recursive result whose
postcondition still needs proof:

```omega
trait Source { machine source(input: u64) -> u64 ensures result == 7; }
GoodSource satisfies Source {
    machine source(input: u64) -> u64 ensures result == 7 { 7 }
}
ReservedSource satisfies Source {
    machine source(input: u64) -> u64 ensures result == 7 {
        match input == 0 { true -> 7, false -> forwarded<ReservedSource>(input) }
    }
}
machine forwarded<S satisfies Source>(input: u64) -> u64 ensures result == 7 {
    S::source(input)
}
```

Verified optimized producer `6416b0fdfbe3cd7db319e5c4803f64d3254160cd`,
on macOS arm64 with its frozen 189-file SDK, already rejects this returning
cycle for unproved exit contracts in `ReservedSource::source` and its exact
`forwarded` specialization. Changing only the recursive branch's evidence actual
to `GoodSource` checks successfully with the same implementation declarations
still visible. These are Source observations, not a new runtime qualification or a
claim that every generic-evidence application is complete.

The incorrect no-return rejection controls and an unnecessary candidate repair
were withdrawn after that distinction was observed. No compiler behavior changes
for this correction. Diagnostics identify the unproved exits; explaining the
complete citation cycle remains separate work.
