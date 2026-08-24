# Remote Package Fixture Pins

Private GitHub mirrors under the `CathedralOS` organization.

Use these exact commits for remote resolver/package-manager tests; do not use
branch names in acceptance tests.

Optional network validation, requiring private `CathedralOS` repository access:

```text
cargo test -p omega-packages --test remote_fixtures -- --ignored --test-threads=1
```

| Package | Repository | Exact commit |
| --- | --- | --- |
| `arithmetic-kernels` | `https://github.com/CathedralOS/arithmetic-kernels` | `dd02c8eabe81b4bbd20cc124b64183992a46fa6e` |
| `generated-table` | `https://github.com/CathedralOS/generated-table` | `30bc421c91af2286639908e719351df563f3fd99` |
| `file-journal` | `https://github.com/CathedralOS/file-journal` | `1d30b1e3d94a5bd112a214d81741a762292a5e88` |
| `network-overreach` | `https://github.com/CathedralOS/network-overreach` | `19406048e972378afc70e295f1fbfa8b0733c1a1` |
| `remote-journal` | `https://github.com/CathedralOS/remote-journal` | `3aacc4dc185bc44bc56fd1750b9f3622bf932546` |
| `axiom-ledger` | `https://github.com/CathedralOS/axiom-ledger` | `5f0f2ea1b7a576a43563f1c58e4597ffb1f51778` |
| `provider-switchboard` | `https://github.com/CathedralOS/provider-switchboard` | `49869722a9bd05a8f5c5453844f9f79b1e77c45f` |
| `capability-vault` | `https://github.com/CathedralOS/capability-vault` | `1c2e0a3a480adc5ace7249267d3a246a435a0801` |
| `graph-workbench` | `https://github.com/CathedralOS/graph-workbench` | `b11b1164cb2a919506fc0c625831fe7cb5f359b7` |
