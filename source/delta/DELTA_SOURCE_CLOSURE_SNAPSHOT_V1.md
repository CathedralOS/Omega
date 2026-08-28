# Delta source-closure snapshot V1

This format commits to the exact canonical Delta compiler source independently
of its checkout path. It does not define Delta v1, grant compiler authority, or
describe the later Omega source closure `C`.

## Semantic record

The canonical snapshot retains:

- a stable source identity and role set;
- exact byte length and SHA-256;
- source dependency edges;
- generated-input recipes, if any;
- selected build-host and final-target profiles;
- exact compiler-tool and output-artifact identities;
- content-set and closure commitments.

Filesystem paths live only in a separate location sidecar. Renaming a checkout,
changing the working directory, or using an equivalent symlink must not alter
the semantic snapshot. Changing source bytes must reject.

[`source_closure_snapshot_v1.py`](source_closure_snapshot_v1.py) verifies the
record and its mutation controls. [`source-closure-snapshot-v1.sh`](source-closure-snapshot-v1.sh)
is a replaceable convenience runner for those checks; it is not a compiler
stage.

The direct `Delta C → omega₀` edge will carry its own exact package-resolved
source closure and source-to-artifact refinement. It must not be represented as
a private intermediate bridge action graph.
