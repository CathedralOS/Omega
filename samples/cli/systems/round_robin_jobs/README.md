# Round-robin jobs

A two-slot dispatcher rotates owned jobs on ranked internal state backedges.
Each job carries a linear ticket, an affine label and a reference to its
assigned worker. The dispatcher hands the current job through an ordinary
call result before arriving at the next iteration. Completion consumes both
tickets, then checks each selected component and its worker association before
returning a checksum. A separate state cycle writes through rotating projected
worker references, which the caller observes after return.

Run `omega run samples/cli/systems/round_robin_jobs/main.omg`. Expected exit: 0.
Zero and even rounds select checksum 20; odd rounds select checksum 30. The
worker write cycle leaves last-round values 1 and 2. Incorrect contents,
association, arrival custody or reference bindings take the Trap branch.

Regression fixtures retain separate owner/component oracles, a transfer through
an extra named local, and invalid duplicated references, restored consumed
claims and unconsumed conditional-exit claims. Portable Terminal installations
exercise the target-neutral custody route; they do not establish native
execution on another host.
