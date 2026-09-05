# Remote Package Fixture Pins

Private GitHub mirrors under the `CathedralOS` organization.

Use these exact commits for remote resolver/package-manager tests; do not use
branch names in acceptance tests.

Optional network validation, requiring private `CathedralOS` repository access:

```text
mbx test -p package-manager --test remote_fixtures -- --ignored --test-threads=1
```

| Package | Repository | Exact commit |
| --- | --- | --- |
| `arithmetic-kernels` | `https://github.com/CathedralOS/arithmetic-kernels` | `5f511ca12ef9ee5f075b9dff09fed574f80d8e89` |
| `generated-table` | `https://github.com/CathedralOS/generated-table` | `2d92ebb735a6e4f9072db21e5677b0d6bf80bd4e` |
| `file-journal` | `https://github.com/CathedralOS/file-journal` | `deea7b84b13cf6e1b67868f314d0e5dbdb9ff20e` |
| `process-exit` | `https://github.com/CathedralOS/process-exit` | `066585cfd952b6251780bdfdbdf17ca0e970b07e` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `d0fe2b00c2485700ace9242114bfa8c8e4a6c526` |
| `remote-journal` | `https://github.com/CathedralOS/remote-journal` | `11a2c6e3825a4a9221fe536164417846f88cd63c` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `9f274c21386ea3cd7d7cce5b8d20bcb935f06f58` |
| `opaque-carrier` | `https://github.com/CathedralOS/opaque-carrier` | `4db835ee361b24281ce8be139c5247e6256400a8` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `bd0d679da697af139bf3f9dd43d0d84935c7705b` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `7ee14134ff4756f58f9f1386258066ff794fff5b` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `5d72ed263b69248b3be021f32c71754f27d5f293` |
