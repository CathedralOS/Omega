# Constant calls must be safe before execution

A helper used to calculate a range limit must not run into a trap before the
compiler rejects it. These records show that the repaired compiler admits safe
calls and refuses unsafe calls first.

All nine guarded examples run with native and Terminal exit zero on macOS ARM64.
Their nine twins reject the Trap route through limit before execution. The
ordinary runtime control still traps, and the layout customers retain exits zero
and 70. Five source admission tests and eight unchanged saved-program tests pass.
The focused corpus qualifies 32 fixtures: 16 checks and 16 intended rejects,
including actual execution and inspection of the saved calling policy.

The initial 30-fixture attempt includes one stale accepted compiler lock; that
setup refusal receives no semantic credit. Normal update and native/Terminal
execution of that recursion control pass, followed by three fresh corpus checks.
The other 29 completed observations retain their original records. The initial
wrapper also misclassified the correct CLI exit 70; its correction uses the
completed engine observations without repeating them.

The manifest binds the source bundle and archive. receipt-files.json binds every
archived file. Producer receipts give actual revisions, patches, optimized
profiles and binary digests. Intermediate failures remain unchanged and labeled.
The +wt oracle patch, exact fixture inputs and accepted-lock replay inputs are
included; a clean-source stamp is not substituted for a dirty observation.

The bundle exports the measured source under refs/corpus-observations. Restore
its listed published prerequisites before fetching that bundle. Raw data is kept
in this separate evidence commit; the feature PR carries only a readable summary.
