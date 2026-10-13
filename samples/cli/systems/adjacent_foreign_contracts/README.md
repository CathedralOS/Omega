# Adjacent foreign contracts

Expected exit: 0 on macOS AArch64. Calls two libc `abs` leaves and a checked
target-scoped helper immediately after their external contracts. The declarations
remain separate and produce 41, 39 and 42 respectively. The helper also retains
a semicolon-separated fact over a parameter named `macos_arm64`.

```sh
omega update --project samples/cli/systems/adjacent_foreign_contracts --target macos_arm64
omega run samples/cli/systems/adjacent_foreign_contracts/main.omg
```

Before the parser repair, package preparation consumes the second target prefix
as another proof fact and refuses at that declaration's `machine` keyword.
