# Resultless Windows callback invocation

On Windows x64, prepare the bundled dependency through the ordinary package
review workflow, then run:

```text
omega run tests/omega/pass/calls/callback_private_materialization/main.omg
```

The genuine [LineDDA](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-linedda)
export from gdi32.dll invokes a
[VOID(int, int, LPARAM) callback](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nc-wingdi-lineddaproc).
It visits the horizontal segment from (2, 4) toward (5, 4), excluding the
endpoint. Exit 0 requires the callback's ordinary helper to have marked the
caller-owned context observed and stored the last point (4, 4). Exit 71 names
a failed registrar or missing/wrong callback observation.

The native-only callback occupies argument five at stack offset 32 after
Microsoft shadow space; its context occupies argument six at offset 40.
The inbound callback has a stricter R10/XMM5 clobber promise, so matching
argument geometry does not allow the entry to alias its ordinary body.
The callback is synchronous and call-scoped; it creates no durable registration
or future-root permission. No synthetic DLL or nonexistent export supplies it.

Cross-target PE generation and independent artifact audits are useful checks,
but neither establishes the promised Windows invocation. The Windows-specific
exit oracle remains required on a matching host.
