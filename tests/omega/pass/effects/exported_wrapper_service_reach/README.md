# Exported wrapper service reach

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/exported_wrapper_service_reach/main.omg
```

`Worker::read` declares its direct `Readable` boundary use and a conservative
`Queryable` contribution. The exported `forward` wrapper inherits both without
repeating `reaches`. Its independent `invokes Readable` contract remains required.
Package review and Terminal replay must preserve the derived service row.
