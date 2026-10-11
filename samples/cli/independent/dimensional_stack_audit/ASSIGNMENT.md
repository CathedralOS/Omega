NEW013: complete independent dimensional stack audit. Own source, private
algorithms, input, fixtures, frozen endpoint oracle, native/Terminal validation
and complete intended-invalid contract control. No worker dependencies.

Initial base: 8fef9837644713285aa9ed9eee42b46cd3bd1da8.
Current reproduction base: 3e977af871e2dd7278f6b59c1900aaddb6f4a968.
Branch: samples/new013; isolated worktree: Omega/.codex/worktrees/new013.
Starting checkout 03e477fa424a9e6a2db7f49e17bf896ab3f76b48 preserved.
Pending claim ticket: 31f1de197e5c4e0d8c4182ac46fbd774.
Coordination status and claim timed out; registration is unconfirmed.
No remote worker branch or shared library change.

Acceptance: exact frozen stdout and exits for 9 positive thermal/profile cases
and 11 invalid runtime inputs; --check; native and --both Terminal status;
invalid_contract rejection specifically names its requires call obligation.
Current frontier: source check passed on initial base after documented local
package acceptance. Native generation fails before execution with RuntimeSpill
Homes(NoCommonCandidate { function: 9, register: 10 }). Reproduce on current
main and diagnose the recovery boundary in selected stages 04/05. Numerical
oracle remains frozen; runtime acceptance is incomplete. Claims note also
timed out; neither registration nor note publication is confirmed. Independent
review and Library transfer are required before landing. No package denial.
