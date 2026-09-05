# Delta resource-boundary gate

Run `sh tests/delta/resource-boundary/run.sh` from the repository root. The gate
materializes and pins the complete canonical compiler, then compiles three full
authored Delta sources through `DCREQ` profile 1 and the selected Gamma evaluator.
The host neither parses declarations nor injects counters or compiler rows.

The function-row controls use D30's selected limit of 32,768:

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 32,768 distinct functions | 720,923 | Reject 11, source coordinate 38 |
| Same prefix plus new `f32768` | 720,945 | Incomplete 4, source coordinate 720,928, limit 32,768, requested 32,769 |
| Same prefix plus duplicate `f00000` | 720,945 | Reject 8, source coordinate 720,928 |

Each source begins with `(data Flag (Off) (On))`, which must consume no function
rows. Its first function is `(def f00000 () Missing 0)`, followed by 32,767
distinct ordinary definitions. At the exact boundary, the complete global
census must finish and declaration resolution must report `Missing` at byte 38.
An adjacent new name instead refuses its function-row allocation before that
later phase. A duplicate is not a new row and retains the earlier-phase
duplicate diagnosis. All three inputs are below the separate 4-MiB source limit.
Fixture sizes, SHA256 identities, and full 40-byte output frames are pinned.

Each evaluation uses the existing full-customer diagnostic allowance of 300
seconds. The gate prints elapsed time for each exact observation and reports a
raw evaluator failure or timeout without relabeling it as compiler
`Incomplete`. A selected evaluator heap or stack failure does not pass, and the
boundary is not reduced to accommodate it.

These controls establish the function-row boundary only. They do not establish
all D30 capacities, acceptance or emission of every 32,768-function program, or
closure of the Delta edge. Other frontend and request behavior remains in the
[frontend](../frontend-boundary/README.md) and
[request](../request-boundary/README.md) gates.
