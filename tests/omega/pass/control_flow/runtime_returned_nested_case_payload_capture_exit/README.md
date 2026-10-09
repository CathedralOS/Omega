# Returned nested case payload capture

The selected case carries two record payloads and a scalar payload into a
returning call. The consumed subject's private storage remains live until
those observations complete, including when a call produces an anonymous
dispatch subject. Each call's intervening `tick` runs exactly once; selecting
the empty case runs no tick. Both engines must return exit 0.

From this directory, with `omega` on PATH:

```sh
omega update --project . --target macos_arm64 --offline
omega run --both main.omg
```

If package preparation requests review, accept the named decisions and rerun
`omega update --resume --project . --offline`. Substitute the matching target
on the other supported hosts.
