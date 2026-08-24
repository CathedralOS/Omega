# Remote Package Fixture Pins

Initial private GitHub mirrors under the `CathedralOS` organization.

Use these exact commits for remote resolver/package-manager tests; do not use
branch names in acceptance tests.

Optional network validation, requiring private `CathedralOS` repository access:

```text
cargo test -p omega-packages --test remote_fixtures -- --ignored --test-threads=1
```

| Package | Repository | Initial commit |
| --- | --- | --- |
| `arithmetic-kernels` | `https://github.com/CathedralOS/arithmetic-kernels` | `2ddfebda4d0436a09b41ef986ed8faa8fd88f890` |
| `generated-table` | `https://github.com/CathedralOS/generated-table` | `eafbcc49a270298a5cc1e965ab58f719be1aabaf` |
| `file-journal` | `https://github.com/CathedralOS/file-journal` | `433d155612edb0f43f5a82cfcf7e69dfaab71cf3` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `643262287a7b313fee47bb984c6ee77bb6984868` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `49eef21c21b6e83ee37376c9ea47aaca7aff619b` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `8b2fd2d280089a7c9320b63854df7bd57f69c0ed` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `1f9a63402e10527eddaae0fd8b5b8f4b023bf0d7` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `e90ca5c236ae6c8b20038454a01e8ca194dfdb5b` |
