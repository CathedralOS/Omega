# Distinct Builds in one package batch

`BUILD-EVALUATES-ONCE` acceptance: two target consumers share each exact Build,
while the root and its ordinary dependency remain different semantic inputs.
The root selects the helper's scalar provider and consumes its result, 42.
The original program must return normally (exit 0) in native and Terminal
execution; it traps if the provider result changes.

From before preparation through both native products, the public package API
must perform two Build executions, three settled checks (the two Builds and
one Product), one Product source completion and one Terminal production.
Both targets retain their own outcome and native artifact while sharing the
same Terminal storage. These are Linux x86_64 measurements; cross-target image
production is not execution on another host.

```sh
mbx run -p package-manager --example package_batch -- tests/omega/pass/build/package_batch_distinct_builds/main.omg 1 1 2 3
omega run --both tests/omega/pass/build/package_batch_distinct_builds/main.omg
```

The companion equal-source and malformed-unrequested-entry measurements are
explained in the compiler driver README. This is a measured workflow control,
not a new language transport mechanism or nine-place fact row. The broader
Build target-row capability remains open.
