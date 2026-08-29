# Remote Package Fixture Pins

Private GitHub mirrors under the `CathedralOS` organization.

Use these exact commits for remote resolver/package-manager tests; do not use
branch names in acceptance tests.

Optional network validation, requiring private `CathedralOS` repository access:

```text
cargo test -p omega-package-manager --test remote_fixtures -- --ignored --test-threads=1
```

| Package | Repository | Exact commit |
| --- | --- | --- |
| `arithmetic-kernels` | `https://github.com/CathedralOS/arithmetic-kernels` | `dd02c8eabe81b4bbd20cc124b64183992a46fa6e` |
| `generated-table` | `https://github.com/CathedralOS/generated-table` | `4c71d3257e42bc0cc4912627469eae949cf68129` |
| `file-journal` | `https://github.com/CathedralOS/file-journal` | `6e475db09634e5733fc3efa04bb54f3f0c11aef4` |
| `process-exit` | `https://github.com/CathedralOS/process-exit` | `1fd514b1b46e85deb2cb37a117d68860371b976f` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `19406048e972378afc70e295f1fbfa8b0733c1a1` |
| `remote-journal` | `https://github.com/CathedralOS/remote-journal` | `84765f49151c73a8c957ffd737be368ac8f75145` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `60c353e7fbb5defc44bcb658e439de1db80cf6c3` |
| `opaque-carrier` | `https://github.com/CathedralOS/opaque-carrier` | `3d36453fb608bbdcd079a2e71ac2b9dd968e4049` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `49869722a9bd05a8f5c5453844f9f79b1e77c45f` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `1c2e0a3a480adc5ace7249267d3a246a435a0801` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `b11b1164cb2a919506fc0c625831fe7cb5f359b7` |
