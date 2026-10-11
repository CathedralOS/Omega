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

The native corpus route executes this program. Public `omega update` currently
rejects package review with an unresolved nominal/source-backed toolchain type
owner for the layout-wrapped signature; `omega run` therefore cannot yet obtain
the accepted entry binding. Do not bypass that review to claim CLI acceptance.
