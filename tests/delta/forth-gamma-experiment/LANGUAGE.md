# Experimental Forth-Gamma profile

This test-owned language extends the retained concatenative Gamma profile only
to test whether interpretation can make a Forth-like rung competitive with
selected functional Gamma.

It retains hexadecimal literals, words, explicit `jump` and `branch`, stack
operators, checked cells, sealed input, output, and arithmetic. It adds two
forms:

```text
value NAME
... VALUE ...
... word -- to VALUE ...
... text "printable text possibly spanning lines" ...
```

`value NAME` declares one unique zero-initialized 64-bit value. Evaluating its
name pushes its current word. `to NAME` pops and stores one word. Values and
words share a namespace; neither may collide with builtins. `main` must remain a
word.

`text` consumes one following quoted token and appends its contents directly to
output. Spaces, semicolons, comment markers, and source newlines inside the
quotes are data. `\n` is the only escape. Quoted text is an output form, not a
stack value, heap string, input value, or general-purpose byte sequence.

The interpreter still validates declaration shape and reached operations only.
It does not infer or check stack effects, and it does not reject unknown names
or underflow in unreachable word bodies. Closing that gap would require an
additional static contract and is part of the experiment verdict, not silently
assumed functionality.
