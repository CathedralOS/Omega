# Saved IEEE Boolean observations

The saved Boolean retains the IEEE bounds selected when it was evaluated.
[Numeric values](../../../../../wiki/spec/language/numeric_values.md) requires
unqualified float-to-integer conversion to have a finite, representable truncated
operand; [fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md)
keeps observations on an exact value or surviving storage version.

The runtime witness exercises saved negation, a disjoint field write, a scalar copied
after the Boolean evaluation and surviving a later write to its former source, and an explicit state edge carrying
the Boolean with its subject. It calls each path with a finite value and NaN,
and checks the authored zero result. All four hosted ProgramEntry bindings are
present. The original nine-place IEEE conversion row remains unchanged.
