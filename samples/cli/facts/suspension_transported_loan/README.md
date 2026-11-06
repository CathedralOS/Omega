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
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

A compatible record view keeps the original backing's suspension restrictions.
The viewed type's default permissions cannot replace that backing's carry floor.

Copies of stored references retain their original data and access when the
source slot changes, including changes made while evaluating later arguments.
Shared descendants keep the complete originating call input borrowed until they
finish. A field view returned by a helper carries the same requirements through
source-slot replacement and later argument writes, for every possible supplier.

## References installed by helpers

A helper can leave an output pointing at its original cell or install a
reference to another cell. Saving that reference must keep the chosen cell
usable after a call that may pause, even when the output variable changes later.
The [conditional helper fixture](../../../../tests/omega/pass/borrows/supplied_output_conditional_helper_keeps_reference_exit/main.omg)
reads 11 when the helper keeps the reference and 73 when it replaces it.
Its [policy twin](../../../../tests/omega/fail/borrows/supplied_output_conditional_helper_keeps_reference_forbidden/main.omg)
forbids both backings from surviving the pause, so either choice retains the
same suspension obligation.

The [ordinary helper fixture](../../../../tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg)
also keeps the helper's original input borrowed until the saved reference's
last use. The [shared containing-record fixture](../../../../tests/omega/pass/borrows/supplied_output_shared_stored_input_ceiling_exit/main.omg)
reads a stored reference without acquiring mutable access to its cell. Their
Build companions explicitly select `Immediate` for `Scheduler` on each hosted
target; a requirement alone does not select an implementation.

Checking records the completed output's actual suppliers and their full loan
obligations. Copies keep that association, and lowering transports each
conditional request onto its exact emitted call. A selected helper must prove
the request for every normal return. Complete source duties, current predicate
truth, access and lifetime each retain their own proof requirements.

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
