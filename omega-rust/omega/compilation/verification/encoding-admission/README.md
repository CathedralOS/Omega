# Encoding admission

Checks current machine encodings, layouts and relaxation records without
importing an executable compiler stage. Instruction emission and layout changes
stay with native production. This package owns independent byte/branch checks,
frame equations and the contract projections compared by explicit replay.

The producer shares frame arithmetic, contract projections and byte-span
geometry with replay. Producer and validator reconstruct block order separately.
Ordinary instruction checks decode bytes independently; foreign call checks keep
the existing shared template recipes. Relaxation checks retain the existing
root, budget, trace-content and output-identity checks; these are not a new
independent implementation of the optimizer's sweep.

A record or its content identity is not source or installation authority.
Current-form checks compare the completed encoding with its carried contracts.
Replay additionally receives the exact physical originals requested at compile
time and refuses a missing or mismatched original. Tests that build candidates
stay beside the producers and call these checks directly, so this package has
no production or test dependency on a producer stage.
