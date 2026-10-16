# Directed radius bound

The planar radius `sqrt(x² + y²)` that orbital integration uses, evaluated
with directed square root. A 3-4-5 hypotenuse is exactly 5 in every rounding
mode, whether the squared length arrives through a local, a field, a
parameter, or a call result. `sqrt(2)` is not exact: toward zero and toward
negative select the lower neighbor, toward positive selects the upper
neighbor, and nearest square root matches toward positive for binary64 and
toward zero for binary32. A negative operand produces a NaN.

On agreement the program prints `RADIUS` and exits 0.

`invalid/` calls the nonnegative radius contract with `-1.0` and must be
rejected because that precondition is disproved.

```sh
omega run samples/cli/arithmetic/directed_radius_bound/main.omg < /dev/null
```
