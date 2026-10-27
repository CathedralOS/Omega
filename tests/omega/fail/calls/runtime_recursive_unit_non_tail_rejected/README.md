# Runtime recursion must be terminal

[Calls and productive loops](../../../../../wiki/spec/language/termination.md#calls-and-productive-loops)
permits runtime recursive cycles only in tail position, lowering to constant-stack
iteration. This program performs work after its recursive invocation and must
reject. The private helper publishes its arithmetic bound so the control reaches
that runtime eligibility rule rather than an unrelated formation refusal.

This was previously a positive fixture expecting caller frames to resume during
unwinding. That expectation contradicted the accepted runtime contract. Ordinary
acyclic continuation remains covered by `pass/calls/nested_machine_continuation`;
constant-stack cycles with caller-owned borrowed backing remain covered by
`pass/facts/nested_machine_invocation/ancestor_cycle`.
