# IEEE total-order satisfiers

This program checks signed zero, infinities and NaN payload ordering through
`F32::TotalOrder` and `F64::TotalOrder`. Its authored success result is exit 70.

`Order<T>` takes an ordinary type argument. The machines explicitly satisfy
`Order<f32>::before` and `Order<f64>::before`; neither infers a trait subject
nor declares a whole-trait conformance. The `F32`/`F64` namespaces hold no
runtime operands.

The bit patterns are immutable invocation-local variables. Their recast views
observe the original storage, not copied reference results. Observing a local
view and passing a local by reference demand the same addressable home.

After ordinary package review:

```sh
omega run --both tests/omega/pass/float/runtime_total_order_satisfiers_exit/main.omg
```

The corpus harness supplies its reviewed fixture policy:

```sh
python3 tools/corpus_gate.py --both --filter float/runtime_total_order_satisfiers_exit
```

Companion controls cover scalar parameter views, Boolean and constrained-float
views, conflicting loans, width mismatch, and a machine claiming the wrong
`Order<T>` application. None of the total-order machinery changes arithmetic
`<`, which remains false on NaNs.
