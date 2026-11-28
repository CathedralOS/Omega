# Atomic Cell

The probe stores 10, adds 5, and checks that the fetch returned 10. A decisive
compare-exchange then replaces 15 with 99. Its `Exchanged` arm checks the
updated word. Its `Mismatched` arm receives the observed word and takes the
failure path.

```sh
mbx run -p omega -- --check samples/cli/probes/atomic_cell/main.omg
```
