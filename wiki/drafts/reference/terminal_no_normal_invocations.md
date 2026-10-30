# Ordinary invocations without normal completion

An ordinary call whose selected checked body has no normal completion still
performs ordinary invocation entry. `InvokeNoNormal` retains the exact callee,
scalar and erased actuals, structural arguments, input claim transfers,
requirement obligations and crash continuations. Its edge has an invocation
identity distinct from operation identities and external completion identities.
The canonical reader reconstructs the selected callee's control graph; a
returning, absent or ambiguous callee cannot justify this terminator.

The caller remains suspended in its activation. This edge introduces no result,
normal successor, normal cleanup, restored-loan receipt or abandonment
permission. Callee crashes follow the retained crash routes. Absence of a normal
return does not prove termination or authorize disposal of the caller's remaining
resources. The callee's declared result and physical calling convention remain
intact, including hidden result storage where its ABI requires it; the caller
publishes no semantic result from that storage. An interpreter observing an
unexpected normal return refuses the malformed execution.

This is an implementation note for the reference producer. The accepted
[call and outcome contract](../../spec/terminal-psi/calls_and_outcomes.md)
governs ordinary invocation, absence of a crash result, caller suspension and
retained custody. This representation preserves those rules; it adds no source
completion promise or authority.

The owning representation is
[Terminal control flow](../../../omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module/control_flow/termination.rs).
Its canonical wire marker is documented in
[encoding](../../spec/terminal-psi/encoding.md).
