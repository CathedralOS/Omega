# Quantizer reconstruction bound

Run `omega run main.omg` on Linux x86-64. Expected exit: 0.

`bounded_sensor_compression` rounds each sample onto the grid
`step = 2 * tolerance + 1` and certifies that the reconstruction stays within
`tolerance`. Independent intervals cannot see that
`(dividend / divisor) * divisor` shares its dividend: the product only lies
in `dividend - (divisor - 1) ..= dividend` when the dividend is nonnegative
and the divisor is at least one.

This program states that window for a local step, a swapped multiply, a
constant divisor, a call result, and an unwritten field. Exit 0 means those
contracts were proved and the five concrete reconstructions are 5, 5, 5, 0,
and 5. The
`controls/too_tight` source still has to be rejected: one extra unit of
required accuracy is not entailed. A negative dividend does not get this
window.
