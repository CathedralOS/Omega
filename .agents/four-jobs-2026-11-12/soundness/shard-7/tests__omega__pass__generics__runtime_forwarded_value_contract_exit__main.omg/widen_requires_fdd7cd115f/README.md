# Forwarded runtime value contract

`forward<K>` proves `bounded<K>`'s requirement from its own `K <= 10`
contract. The argument remains a runtime value binder, not a static type
selection. The entrypoint traps unless the nested call returns 3.

From the repository root:

```sh
omega update --project tests/omega/pass/generics/runtime_forwarded_value_contract_exit --target macos_arm64
omega run --both tests/omega/pass/generics/runtime_forwarded_value_contract_exit/main.omg
```

Use the host's target for package preparation. The build binds all four hosted
targets; native and checked-interpreter execution both exit 0 on macOS ARM64.
