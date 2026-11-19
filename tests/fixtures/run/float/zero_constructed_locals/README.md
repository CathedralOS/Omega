# Zero-constructed float locals

Declarations without an initializer create positive zero for `f32` and `f64`,
including fields in records, when the type permits zero. The compiler previously
refused these valid programs. This covers the float-local requirement of
**LOWERED-FORM-CHECKER** in [TASKS.md](../../../../../TASKS.md).
[Default domains and zero initialization](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
distinguish zero construction from assuming that arbitrary uninitialized
storage contains a value.

[main.omg](main.omg) declares plain and record `f32`/`f64` locals without
initializers. Parameterized helpers distinguish positive zero from negative
zero, nonzero values and NaNs. The record's nonzero neighbor canaries expose
incorrect field reads or stores. The Build binds all four hosted entries.

The [negative twin](../../../fail/float/zero_constructed_locals/main.omg)
changes only `narrow >= 0.0f32` to `narrow > 0.0f32`. Zero storage cannot
establish that default domain before observation. Both probes declare
`requires true`, so private inference cannot narrow the input contract.
The callers and runtime oracle remain identical.
