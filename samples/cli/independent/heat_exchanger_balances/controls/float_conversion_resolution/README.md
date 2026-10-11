This complete reduced source is a valid diagnostic control, separate from the
intended-invalid bounds source. On base b4008bb3c7894aebad47eaccf8af12bb4a8006b1,
using compiler SHA-256 ec44090c516c5d371dc74028234ff2fe20b8ea279e33fe461dcd51388e327a07:

```
omega update --project samples/cli/independent/heat_exchanger_balances/controls/float_conversion_resolution --target linux_x86_64
```

exits 1 with 240 diagnostics beginning with `machine satisfaction target
I8::from_f32 does not resolve to an exact trait requirement or top-level boundary
requirement`. A direct-root float import reaches a pending package review, while
this domain-module import fails. No acceptance was attempted on this control.
The candidate repair retains a visible named overload family until the existing
typed matcher selects its exact operand/result policy signature. The general
language fixture is `pass/float/module_imported_policy_conversion_family_exit`.
