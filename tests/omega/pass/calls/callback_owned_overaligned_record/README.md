# Owned over-aligned callback storage

After ordinary package preparation, run on macOS AArch64:

```text
omega run tests/omega/pass/calls/callback_owned_overaligned_record/main.omg
```

The authored layout gives three u64 members their C offsets 0/8/16,
32-byte alignment and a padded 32-byte extent. Genuine system libffi invokes
the owned by-value callback and returns its complete record through X8.
An ordinary helper permutes all three members. Exit 0 requires 43/17/91
and unchanged 17/91/43 input. Over-aligned storage is required in the caller,
callback snapshot, ordinary body and result area, not only in metadata.

An independent Clang C-record/libffi probe confirmed this exact 32/32 ABI
and permutation on this host before the Omega witness was authored.

From this fixture directory, the optional independent ABI check is:

```text
xcrun clang -O1 abi_probe.c -lffi -o /tmp/omega-alignment-probe
/tmp/omega-alignment-probe
```

Package preparation must review the layout-wrapped public signature with the
authored application's managed source owner. The generated type is public only
when both its policy and schema are public; generation cannot hide a public
carrier or launder a private input.
