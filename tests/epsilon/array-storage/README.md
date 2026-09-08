# Epsilon array-storage scalability gate

Run the durable gate from the repository root with:

```sh
sh tests/epsilon/array-storage/run.sh
```

The gate materializes the role-manifest Epsilon and Delta source closures,
rebuilds both the ordinary execution receipt and a private storage-invariant
receipt through the selected Delta compiler, then executes one ordinary 654-byte
Epsilon fixture and one private control. The fixture owns
`Main.values: [i32; 2048]`, fills every index through a tail-state loop, reads
and verifies every index through a second tail-state loop, and writes `A` only
after all checks complete. The expected diagnostic observation is
`000000000041`, with outer status zero and empty stderr. The private control
expects 40 bytes of `01` and exercises malformed/sparse runtime storage paths.

This is private scalability evidence for sparse array-child replacement. It is
not a new Epsilon operation, a final storage representation, or a language
resource/profile limit. The fixture is intentionally independent of any
implementation-specific helper or synthetic invariant call.

The private control is deliberately separate from authored-language admission:
it validates only the visited indexed-child paths and does not claim behavior
for untouched siblings or a public Epsilon operation. The default gate covers
both controls in one invocation.

The fixed source identity is 654 bytes,
`365534cde66cb10a61eee9b52956a4f69d9fd69edacc82d7798319ef36756467`.

The current receipt identity is 712070 bytes,
`c657884fde474a63c8ffafd628959c8c47e6a7440172d3a4d449991d1dab148f`.
The private invariant driver is 8736 bytes,
`9d658973aba81c16b1da2e723ca1fe598132804fe9f0344ab859ce5cd70fe341`, and its
compiled receipt is 714142 bytes,
`ee06d8ef25d58983dd409e377e4ae18094a72f0990ed7d317c406d68de9491f9`.
The same V4 evaluator and unchanged fixture measured 570.692 seconds with the
prior 702903-byte receipt
(`3bc739535e467378a4c82d03358bcbe2ae91ba7447139c20ef202d1313585f92`)
and 59.236 seconds after the array-storage change, before the current call-head
checking change. Those are historical host measurements, not
execution bounds. `OMEGA_EPSILON_ARRAY_SECONDS` controls the fixture watchdog
(default 1200 seconds); `OMEGA_EPSILON_RECEIPT_SECONDS` controls receipt
reconstruction (default 400 seconds). A timeout is not a language judgment.

Python 3 and a POSIX shell are prerequisites. On macOS use the command above;
on Windows run the same command in Git Bash with `python3` on its PATH. Both
routes call the same Python protocol implementation and the existing
platform-selecting seed materializer. In either shell, a larger observation
allowance can be selected explicitly:

```sh
OMEGA_EPSILON_ARRAY_SECONDS=2400 sh tests/epsilon/array-storage/run.sh
```

The gate has been exercised on macOS arm64. Windows runtime validation remains
outstanding; the documented Git Bash route has not been executed here.
