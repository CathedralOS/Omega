# Checked dependency reuse

Owner: `CHECKED-PACKAGE-REUSE`.

`dependency` publishes three bodies and no result guarantees. `project`
returns a reference to the caller's cell. `count` returns its argument, so
`observed - 19` is proved from that arrival. `equal` is an observation-free
denotation used by `require_equal`. The caller accepts only the live view 19
and the difference 0. The other arm traps. `Expected exit: 0`.

Those queries read the dependency through one declaration, state, and
expression correspondence. A second consumer reuses that frozen body instead
of judging it from source again. A different typed program does not take the
first consumer's whole-program judgment.

```sh
omega run --both samples/cli/facts/checked_dependency_reuse/main.omg
```
