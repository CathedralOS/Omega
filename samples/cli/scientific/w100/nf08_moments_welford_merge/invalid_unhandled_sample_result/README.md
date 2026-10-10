# NF08 invalid unhandled sample result

Run `omega --check main.omg` from this directory.
Intended rejection: MomentValue cannot be assigned to f64. A singleton sample
variance returns InsufficientSample; callers must match the result.
Rejection is valid only if checking reaches this type mismatch. Cache or
package-admission errors do not qualify the intended rejection.

NF01 must provide metadata declaring the proposed `numerics` package alias.
