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

The requirement this row records: **a requirement's published guarantee depends
on every supplied implementation, selected or not.** Psi checks each named
implementation under its ordinary identity whatever the Build selects
([multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation)),
and [citation cycles cannot justify themselves](../../../../wiki/spec/proofs/contracts.md#citation-and-induction).
Each invalid twin changes only `ReservedSource`'s body to call the wrapper: its
conclusion would then depend on the wrapper's, which depends on the requirement's,
which holds only if `ReservedSource` proves it. The required rejection is
`cannot prove ensures contract for exit from forwarded`; `ReservedSource::source`
is refused as well. Host implementations, consuming subtraction and caller control
flow remain identical. Ordinary guarantees are partial correctness; nontermination
alone is not a reason to withhold a separately established promise.

## Baseline defect

The retired spelling carried the supplier as five target-prefixed machines, and the
family head recorded a dependency on every member (`target_family_members` in
`declaration_call_dependencies`), so the same twin was refused. Moving the supplier
to named implementations dropped that edge: a trait requirement's dependencies were
its own contract symbols only. All nine re-expressed twins were **accepted** by the
unrepaired producer. Only an implementation some Build row selects was left to
provider planning's `selected providers realize a cyclic synchronous invokes graph`
check, which never sees an unselected one. `requirement_backing_machines` in
`omega-rust/psi/compilation/checking/src/machine_calls/call_cycles.rs` now adds every
supplied satisfier (machine-level `satisfies` and named-implementation rows) to the
requirement's dependencies, and the existing independence walk does the rest.

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

Each place has `invalid/main.omg`. The root duplicates local. Every positive
binds all four hosted ProgramEntry targets, expects zero and must run with
`omega run --both`. The nine source-identical pass/fail pairs are pinned under
`tests/omega/pass/facts/independent_call_result_bound_through_<place>_exit` and
`tests/omega/fail/facts/independent_call_result_bound_through_<place>_rejected`
(nine each; the reject fixture is a byte-identical copy of `<place>/invalid/main.omg`).
A returned difference is tested only after the subtraction; that observation
does not supply its formation proof. Replacing the final `difference == 0` with
`difference == 1` makes every place trap on both engines.

The [applied requirement](applied_requirement/main.omg) states the local place on
a const-generic requirement, `IndependentSource<17u64>`. A call through it carries
a closed application with its own signature symbol, and the dependency is recorded
against the declared requirement it instantiates. Its
[twin](applied_requirement/invalid/main.omg) changes the same single line. Both are
pinned as `independent_call_result_bound_applied_requirement_{exit,rejected}`.

Requirements exercised:

- Establishment: an independently justified completed-result contract supplies
  the singleton used by Exact subtraction; no caller guard reconstructs it.
- Identity and lifetime: the row exercises capture and copying of the completed
  result. Exact call identity and mutation invalidation remain implementation
  obligations; these uniform singleton results do not independently distinguish
  different invocations or later writes, so a selection mix-up between the four
  identical hosted bodies is not observable here.
- Composition: all nine transports carry the same bound into the subtraction.
- Soundness: every supplied implementation participates in dependency checking
  through the requirement it satisfies. Copying result orderings into a
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

Fresh results are on Linux x86-64 with an optimized producer built from
`1efc2536b3bc` plus this change (opt-level 1). All nine positives report native
exit 0 and interpreter exit 0 from `omega run --both`; all nine twins reject with
the fragment above; all nine corrupted-oracle controls trap on both engines. The
baseline producer (`1efc2536b3bc`, opt-level 1) accepts all nine twins. Windows and
macOS execution of the re-expressed sources is not observed; binding a hosted
target does not establish execution on that host.

Open: the same dropped dependency in the generic-evidence production. A binder
`S satisfies IndependentSource` instantiated at the implementation that defines the
caller is still accepted, because an evidence binder stands for the implementation
a call's type argument names and its dependencies do not yet include that
implementation's row:

```omega
trait Source { machine source(input: u64) -> u64 ensures result == 7; }
GoodSource satisfies Source { machine source(input: u64) -> u64 ensures result == 7 { 7 } }
CircularSource satisfies Source { machine source(input: u64) -> u64 ensures result == 7 { forwarded<CircularSource>(input) } }
machine forwarded<S satisfies Source>(input: u64) -> u64 ensures result == 7 { S::source(input) }
```

The refusal names the machine and statement but not the cycle that withheld the
imported guarantee; naming the cycle is also open.
