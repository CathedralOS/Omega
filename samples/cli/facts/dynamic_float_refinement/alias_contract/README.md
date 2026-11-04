# A named public floating-point interval

This is another production of the [dynamic return guarantee](../README.md),
under `BUILD-OWNED-TARGET-SELECTION` in [TASKS.md](../../../../../TASKS.md).

The public requirement spells its inclusive interval as
`public_interval(result, 0.0, 0.5)`. Quarter proves the exact result `0.25`.
Every place calls through an actual `dyn Fraction`, forwards the `f64` result
to a consumer that requires the public interval, and traps unless it observes
`0.25`. Forwarding helpers declare that same public result guarantee.

The nine folders match the parent row. Each `invalid/main.omg` changes only
Quarter's own guarantee and return to `0.75`; it must reject for failing the
unchanged named public interval. The match place runs both choices and checks
their distinct route markers. The target place calls an ordinary
`FloatInvocation` requirement, with `SelectedFloatInvocation` selected by four
explicit Build rows. The public interval remains on both declarations.

The pins are
`tests/omega/pass/facts/dynamic_float_alias_through_<place>_exit` and
`tests/omega/fail/facts/dynamic_float_alias_through_<place>_outside_interval`.
Sample and pin Main/Build files are identical. Each positive and [the root](main.omg)
binds all four hosted targets and declares exit zero.

Construction must prove the implementation's exact guarantee, refinement must
prove the named public interval, and the completed call must retain that interval
for its caller. The runtime comparison cannot establish the precondition after
the fact. Selector identity, checking of unselected implementations, portable
Terminal Psi, provider selection, calling plans and actual native/Terminal
agreement follow the [parent requirements](../README.md).

All nine positives have observed native/Terminal exit-zero agreement on all
four hosted targets, and their nine twins reach the intended refusal. Linux
x86-64 and macOS ARM64 keep their qualification at `51ca56236d0f`; Windows
x86-64 and Linux ARM64 use published source `9566f10e0d29`. Each positive's
constrained consumer accepts the public named interval. The
[parent evidence](../README.md) identifies those runs and the separately
observed root programs, with their actual source revisions.

The retained macOS pilot reports contain the completed outcomes even though
the outer workflow reached its one-hour limit and was canceled. The
[parent evidence](../README.md) keeps those two facts distinct. Broader dynamic
dispatch and target-selection acceptance remain open.
