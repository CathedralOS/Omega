NEW013: complete independent dimensional stack audit. Own source, private
algorithms, input, fixtures, frozen endpoint oracle, native/Terminal validation
and complete intended-invalid contract control. No worker dependencies.

Initial base: 8fef9837644713285aa9ed9eee42b46cd3bd1da8.
Failure reproduction base: 3e977af871e2dd7278f6b59c1900aaddb6f4a968.
Compiler validation base: 71d6144cc88ba852d1dbd17327295d2eb1f0ffee.
Branch: samples/new013; isolated worktree: Omega/.codex/worktrees/new013.
Starting checkout 03e477fa424a9e6a2db7f49e17bf896ab3f76b48 preserved.
Pending claim ticket: 31f1de197e5c4e0d8c4182ac46fbd774.
Coordination status and claim timed out; registration is unconfirmed.
No remote worker branch or shared library change.

Acceptance: all 20 frozen native stdout/exits match exactly: 9 valid thermal /
profile cases and 11 invalid runtime inputs. Terminal termination agrees for
all 20 through --both; CLI Terminal stdout comparison is not exposed and is
not claimed. Source check passes. The separate complete invalid_contract
project rejects specifically nonnegative_tolerance(-1)'s requires value >= 0,
with no unrelated package, syntax, bounds or incomplete-proof rejection.
The retained Linux x86-64 ELF independently passes all 20 cases unchanged.
Numerical absolute and relative tolerances remain zero; expectations were
frozen before the first Omega invocation and were never revised from output.

The complete unchanged application on 3e977af8 reproduced RuntimeSpill
Homes(NoCommonCandidate { function: 9, register: 10 }) before execution.
Its write_integer block parameter crosses hosted output before a fixed
remainder use. Stable main includes 239252d7 admitted interval recovery;
validation uses that existing stable facility. Unlanded compiler experiments
were preserved separately and removed from this candidate. No compiler guard
or custody policy was weakened. Default synthetic-root custody failed;
approved per-command escalated compiler runs retained UID1000/rootUID0.
No package acceptance was denied or retried after denial.

Current frontier: the full application is accepted on Linux x86-64. The clean
candidate, Git bundle/patch, source hashes, raw outputs and retained ELF are
prepared for independent review through Library transfer. Independent review
and main-only queue publication remain; neither is asserted complete. Claims
registration remains unconfirmed after bounded coordination timeouts. No
macOS ARM64 or Windows x86-64 executor was available for runtime validation.
