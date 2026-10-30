# Corrected host execution evidence

The earlier quality/providers harnesses passed an explicit --target to
run --both. The CLI returns TargetOnly for that combination: those calls
compiled successfully but did not execute either engine. Separately invoked
native executables remain valid runtime evidence. Original argv and logs are
preserved unchanged; no old observation is relabeled.

This replay omits --target and asserts both native exit 0 and interp exit: 0.
All nine host comparisons pass: seven with the convergence compiler
5a596bcaa6fb117eefd8735c6bb545e0ebbde1cbf1b9b8c3ba79a586a6b16308
(the five original positives and two provider controls), and two provider
controls with pre-convergence producer
5f0e2fc5dde252263b64b47a6dd593fb3a41c19585f78f45ccd15998b897501e.

The retained reviewed projects and their original process-local cache bases
are reused. This is a runtime correction, not a new first-preparation timing
comparison. Source/oracle and producer provenance remain in the parent record.
The newer performance candidate is not used for any observation in this folder.
See host.json for exact argv, producer digests and transcript links; replay.py
requires the retained ignored producers and reviewed projects in this worktree.
