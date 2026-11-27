# Write-only field carrier

Expected exit: 0. An owned record carries an exclusive write-only reference to
an established cell. A consuming helper replaces its value with 61 while the
caller retains the untouched spare field at 17. Carrying the descriptor grants
no observation of the old referent.

After ordinary package review, run:

```sh
omega run samples/cli/collections/write_only_field_carrier/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/write_only_field_carrier` from the repository root.
