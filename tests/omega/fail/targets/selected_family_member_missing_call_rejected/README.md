# Missing body inside the selected family member

From the repository root, with the development compiler on PATH:

```text
omega --check --target macos_arm64 tests/omega/fail/targets/selected_family_member_missing_call_rejected/main.omg
omega run --target macos_arm64 tests/omega/fail/targets/selected_family_member_missing_call_rejected/main.omg
omega run --target windows_x86_64 tests/omega/fail/targets/selected_family_member_missing_call_rejected/main.omg
```

The macOS check rejects with `bridge` calling the UEFI-only `Firmware::read`;
native production reports the same authored reason. The Windows command
produces its image without running it: its selected relay does not reach the
bridge. Every family body remains source-checked in both invocations. These
separate invocations do not establish one-artifact multi-target realization.
