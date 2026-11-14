# Transported loans across suspension

Parent: `STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../../TASKS.md).
Requirement: checked per-value loan carry evidence for suspension production,
for descriptors transported into a record (the remaining lineages stay on the
board).

Fact: A loan transported into a record before a possible suspension keeps its original backing, so the record reads 73 after the call.

Each place forms a loan of a `Cell` holding 73, carries that descriptor
through the place, copies it into `Message { left: 5, body, right: 9 }`, and
reads the record after `suspend Scheduler::park()`. The record's descriptor is
a transfer: it is in neither checked resource roster. The per-value join first
joins the transfer's own backing, then accepts it only when every supplier is
owned by the same invocation, read through the transfer's source owner and
grants at least the transferred access; each supplier is then retained and
checked in turn. The parameter place is coverage: an incoming descriptor
carries its own entry permission and already ran on the base.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [source](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

The [stored-reference recast production](recast/README.md) carries the same fact
through all nine places after forming a compatible record view. It preserves
the original backing's carry floor rather than acquiring the target type's
default permissions.

The [copied-source production](copied_source/README.md) first selects and copies
stored references, then forms the view. It retains the original data and access
when the source slot changes, including later argument effects, and keeps
shared descendants and the complete originating call input borrowed until they
finish. Its nine places and fact-sensitive twins are pinned alongside the
inherited-policy, access and restoration controls.

The [helper-returned projection production](projected_call_source/README.md)
keeps a saved field view usable after a possible suspension, including selected
sources, source-slot replacement and later argument writes. Its controls retain
the complete original call input and every possible supplier.

Twins change only the backing's declaration to
`carry(suspension: forbidden, ...)`. No twin reads the backing after the call;
it is live only because the transported descriptor retains it, so its
suspension floor applies and each twin rejects naming the backing. A join that
stopped retaining the transported descriptor's backing would accept the twins.
They do not distinguish the supplier walk from the transfer's own backing
join, which name the same storage here.

Runtime falsifiers: `read` returns 0 unless both canaries survive beside the
descriptor, and `main` traps unless the read observes 73. The match join runs
both arms (73 and 11). The target-body place selects `X86Transport` or
`ArmTransport` through Build; each checks its own tag.

Run each positive with `omega run --both <place>/main.omg` and each twin with
`omega --check <place>/invalid/main.omg`. Pinned fixtures are
`tests/omega/pass/facts/suspension_transported_loan_through_<place>_exit` and
`tests/omega/fail/facts/suspension_transported_loan_through_<place>_rejected`.

## Requirements

- Establishment: the loan is formed by `&cell` (or `&cells[SELECTED]`, or an
  incoming parameter) before the crossing; the copy into `Message` only
  transports it.
- Identity and lifetime: the record's descriptor names its exact supplying
  loan occurrence. The match join runs each arm with its own backing.
- Composition: calls (`pass`, nested and other machines, the selected body)
  keep the supplier's loan; the consuming read uses the transported descriptor.
- Soundness: the backing's carry floor still applies through the transport.
- Terminal Psi and native: every positive runs with `omega run --both` and
  both engines exit 0.

## Evidence

Base `origin/main` 3db1f7baade (opt-level 1, linux x86-64): eight positives
refuse with "call to `park` may suspend while `message` remains live ...
missing retained-loan obligation: the loan lifetime resource is unavailable".
The parameter place already runs (native 0, interpreter 0) because an
incoming descriptor carries its own entry permission.

Open: Terminal suspension loan rows still label an accepted transfer's
resource `Unresolved` beside its exact supplier edges. Execution of the original row on
`linux_arm64`, `macos_arm64` and `windows_x86_64` is not yet qualified; see each
production's evidence for its own host observations. The scheduler completes inline; this does not demonstrate actual
parking.
