# Stable field permissions

The policy on `Pair::Restricted` asks to read `Pair::readable`. It leaves take, write and swap disabled and marks `Pair::hidden` inaccessible. The program evaluates the policy on the reflected `Pair` schema at compile time and checks both returned field descriptions. Changing the requested read to false makes the entry trap.

Run each engine from the repository root.

```sh
mbx run -p omega -- run --engine native samples/cli/probes/stable_access_plan/build.omg
mbx run -p omega -- run --engine psi samples/cli/probes/stable_access_plan/build.omg
```
