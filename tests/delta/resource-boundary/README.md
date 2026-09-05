# Delta resource-boundary gate

Run `sh tests/delta/resource-boundary/run.sh` from the repository root. The gate
materializes and pins the complete canonical compiler, then compiles eight full
authored Delta sources through `DCREQ` profile 1 and the selected Gamma evaluator.
The host neither parses declarations nor injects counters or compiler rows.

[`function_rows.py`](function_rows.py) retains the three function-row controls
at D30's selected limit of 32,768:

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

[`constructor_rows.py`](constructor_rows.py) supplies five constructor-row
controls at D30's selected limit of 65,536:

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 65,536 distinct constructors in one data declaration | 720,953 | Reject 11, source coordinate 18 |
| Same constructor prefix plus new `C65536` | 720,964 | Incomplete 3, source coordinate 720,915, limit 65,536, requested 65,537 |
| Same constructor prefix plus duplicate `C00000` | 720,964 | Reject 7, source coordinate 720,915 |
| 32,768 constructors in `T`, then 32,769 in `U` | 720,974 | Incomplete 3, source coordinate 720,925, limit 65,536, requested 65,537 |
| Full `T`, then another `T` with new `C65536` | 720,971 | Reject 6, source coordinate 720,920 |

Every constructor source starts with `C00000 Missing` and ends with an ordinary
`main : Bytes -> Bytes`. At the exact boundary, the whole census completes and
declaration resolution rejects the unknown payload type at byte 18. The fresh
adjacent constructor instead refuses before that later phase; the duplicate
retains its identity diagnosis without requesting a row. Splitting the same
constructors between two data owners proves the counter is global, not per
type. The last control proves a duplicate type rejects before provisioning its
fresh constructor, even when the constructor table is full. All five cases are
below the separate source-byte provision. Their length, digest, source
coordinate, status, and complete 40-byte frame expectations are fixed in the
fixture owner; no expected diagnosis is obtained from compiler output.

Each evaluation uses the existing full-customer diagnostic allowance of 300
seconds. The gate prints elapsed time for each exact observation and reports a
raw evaluator failure or timeout without relabeling it as compiler
`Incomplete`. A selected evaluator heap or stack failure does not pass, and the
boundary is not reduced to accommodate it.

These controls test only the function- and constructor-row boundaries. They do
not establish all D30 capacities, acceptance or emission of every in-bound program, or
closure of the Delta edge. Other frontend and request behavior remains in the
[frontend](../frontend-boundary/README.md) and
[request](../request-boundary/README.md) gates.
