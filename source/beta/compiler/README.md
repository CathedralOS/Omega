# Beta compiler

This directory owns the compiler artifact admitted as the Beta rung:

- `bc.beta` is the compiler source;
- `artifacts/` contains the platform-independent admitted tape;
- `cold-start/` constructs that tape from Alpha;
- `validation/` reconstructs the exact source-to-artifact obligation;
- `artifact_env.sh` installs the admitted tape into the selected Alpha seed.

Construction, testing, and evidence generation do not grant authority by
themselves. The validation directory belongs here because the artifact being
admitted owns its validation. Alternate checkers and stress corpora are useful
tests, but acceptance must ultimately terminate in the independently rooted
checker under `source/alpha/checker/`.
