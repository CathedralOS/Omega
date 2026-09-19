# Package Review

This branch owns review material after successful checked compilation.
[`evidence/`](evidence/README.md) records deterministic compiler-issued facts.
The manager consumes evidence and owns review policy and package acceptance.

Under the ratified install/update model, compiler-derived reachability, unsafe
API, and assumption rows inform review and the lock's accepted baseline and
decisions. The project trusts whoever lands the lock. These rows do not certify
lock acceptance. Native promotion/replay stays with the compiler handoff;
install/update does not need an additional promotion layer. Actual compiler
proof/reach and native artifact checks remain.

Return to the [package subsystem map](../README.md).
