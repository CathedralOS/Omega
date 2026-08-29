# Beta compiler

This directory owns the compiler artifact required by the Beta rung:

- `cold-start/bc-alpha.alpha` is the current Alpha-written candidate that must
  be promoted to the canonical `beta_compiler.alpha`;
- `bc.beta` is a self-hosted comparison implementation, not the canonical
  immediate-predecessor source;
- `artifacts/` contains the current platform-independent tape;
- `cold-start/` constructs that tape through the historical fixed point;
- `validation/` contains proof machinery that must be adapted to the promoted
  Alpha source or deleted;
- `artifact_env.sh` installs the admitted tape into the selected Alpha seed.

Construction, testing, and evidence generation do not grant authority by
themselves. The target state has one Alpha source directly producing one exact
Beta compiler tape. The validation directory belongs here because the artifact
being admitted owns its validation. Alternate checkers and stress corpora are
useful tests, but acceptance must ultimately terminate in the independently
rooted checker under `source/alpha/checker/`.
