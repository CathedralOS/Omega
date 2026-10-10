# carry axes

Compile-scope probe. No standard-library dependency.

- Explicit carry axes spelling: `[carry(suspension: allowed,
  cpu: any, thread: any, address: movable)]` on a case-data
  aggregate and its nested envelope.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/carry_axes/main.omg
mbx run -p omega -- update --project samples/cli/probes/carry_axes
```
