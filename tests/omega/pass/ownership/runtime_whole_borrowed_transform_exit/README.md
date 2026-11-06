# Transform and restore a whole borrowed referent

```sh
omega run --both tests/omega/pass/ownership/runtime_whole_borrowed_transform_exit/main.omg
```

Expected native and Terminal exit: 42. The helper consumes the whole linear
referent, then returns a transformed value to its original storage. Both a fresh
value and an identity result restore the root. The caller observes each update
and explicitly consumes the final value once.

At `6b1c338126fa` on Linux, the [original one-worker corpus setting](https://github.com/CathedralOS/Omega/actions/runs/42038889485)
checks this unchanged program in 99.374 seconds and produces native/Terminal 42
in 100.610 seconds, within the original 120-second limit. Its separate ordinary
package update, review and execution also produces 42 in both engines in
[run 42026360839](https://github.com/CathedralOS/Omega/actions/runs/42026360839).
The earlier corpus attempt used two workers and timed out; that failed result
remains recorded. No compiler change or performance ratio is inferred from
changing the observer back to the original worker setting. The other hosted
targets retain their own qualification records.
