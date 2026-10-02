# Named reference arithmetic guarantees

`Countdown::arm(&mut timer, 4)` stores 5 through a named reference and
publishes both live-field guarantees. `tick` consumes those guarantees and
returns 4. The program exits 0 after observing 4 and traps otherwise.

From the repository root, using the locally built compiler on Linux x86-64:

```sh
omega update --project tests/omega/pass/contracts/ensures_named_parameter_arithmetic_exit --target linux_x86_64
omega run --both tests/omega/pass/contracts/ensures_named_parameter_arithmetic_exit/main.omg
```

The package has no external dependencies or authority choices. Package update
prepares its local acceptance baseline. Native and interpreted exits must both
be 0. The negative fixture `fail/contracts/ensures_named_parameter_arithmetic_wrong`
overwrites the field with zero and must fail the postcondition check.
