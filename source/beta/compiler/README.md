# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `beta_compiler.alpha` is the canonical immediate-predecessor source;
- `bc.beta` is a self-hosted comparison implementation, not the canonical
  immediate-predecessor source;
- `artifacts/` contains the current platform-independent tape;
- `cold-start/` owns direct construction and focused compiler tests;
- `validation/` contains only machinery that targets the canonical source or
  its emitted tape;
- `artifact_env.sh` installs the admitted tape into the selected Alpha seed.

Construction, testing, and evidence generation do not grant authority by
themselves. One Alpha source directly produces one exact Beta compiler tape.
The validation directory belongs here because the artifact
being admitted owns its validation. Alternate checkers and stress corpora are
useful tests, but acceptance must ultimately terminate in the independently
rooted checker under `source/alpha/checker/`.
