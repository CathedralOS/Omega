# Laminate response validation

The independent Decimal tensor-rotation and LDL reference was frozen before
Omega execution. Its `data/expected.json` SHA-256 is
`9b7c5c43f6ba24db04517f11a0341d7f4ad28880a515043d63ed70bfd23648eb`.
`reference.py` verifies that identity before comparing output. Numerical
tolerance remains 0.000002 absolute and zero relative; output labels, value
counts and exit statuses must match. The 16 inputs remain the authored `CASES`
in `validate.py`, including singular and invalid models, invalid wire records,
EOF and CRLF. The validators never regenerate the oracle or edit package review.

The application and controls were recovered from the original authoring and
correction commands. Their source and fixture hashes were compared with the
retained final candidate inventory before restoration. Historical accepted
locks were not copied. A new consumer needs its own ordinary package review;
an archived lock or a successful historical execution supplies no admission.

From the repository root, with the recorded compiler:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
APP=samples/cli/independent/laminate_stiffness_response
CLI=/absolute/path/to/qualified/omega
"$CLI" update --project "$APP" --target linux_x86_64 --offline
# Review any reported decisions through the normal package workflow first.
python3 -B "$APP/validate.py" --omega "$CLI" --route check
"$CLI" --offline --target linux_x86_64 --build-dir "$APP/build/native" "$APP/main.omg"
python3 -B "$APP/validate.py" --omega "$CLI" --route native --executable "$APP/build/native/omega-program"
```

The public SDK sponsor prepares the same accepted origin, compiles normally,
and executes the native product's retained Terminal artifact with its exact
runtime provisions. It supplies no replacement provider proposal. The linker
selects libraries through the verified CLI unit's dependency fingerprints and
records source, binary and dependency hashes:

```sh
python3 -B tools/link_terminal_sponsor.py --omega "$CLI" --source "$APP/validation/terminal_sdk.rs" --output "$APP/build/terminal-sdk"
python3 -B "$APP/validate.py" --omega "$CLI" --route terminal --terminal-sponsor "$APP/build/terminal-sdk"
```

Every case receives a fresh finite input stream and isolated containing domain.
The verifier requires all 16 ordered captures, empty Terminal stderr, correct
exit statuses, and every numerical output against the frozen reference.
`--route both` remains a host termination comparison; it does not capture
Terminal stdout. Embedded numerical guards remain additional checks.

The separate complete bounds control must reject index 4 in an array of length
4 with the specific diagnostic in `controls/bounds/expected.txt`:

```sh
python3 -B "$APP/validate.py" --omega "$CLI" --route invalid
```

Package or infrastructure failures cannot satisfy that control. Compiler
regressions also exercise three runtime selectors through a borrowed record,
a raw array parameter, a local array with fields between indices, and both
stored and directly projected call-returned collections. Distinct nonzero
values expose incorrect bases or selector order. Missing selector bounds and
shared-to-mutable collection loans must still be refused.
Reports and captures live under ignored `build/validation/`. Record the source
revision, compiler revision/hash, target, commands and actual remaining failures
with each qualification; a passing older producer does not qualify a repair.
