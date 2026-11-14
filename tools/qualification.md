# Qualification

Sample qualification in this tree is `mbx xtask qualify`. It runs an exact
committed selection of sample programs and writes the source archive and
observations under a new output directory:

```text
mbx xtask qualify --samples samples/path,... --output <new-directory> [--target <host>]
```

The command refuses a dirty source tree. It does not approve a design, infer a
complete dependency closure, or replace review and
[landing](landing.md).
