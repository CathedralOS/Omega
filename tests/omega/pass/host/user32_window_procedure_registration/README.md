# Windows registered callback lifecycle

This program exercises Windows registration through an ordinary owned-result
protocol, including deterministic rejection and reuse of the returned capacity.

The authored adapter lays out the 64-byte WNDCLASSEX prefix, reserves its private
procedure slot at byte 8, and composes the full borrowed 13-byte class name and
last icon field into the 80-byte record. Win64 enters four callback formals.
`WM_NCCREATE` stores and checks per-window GWLP_USERDATA marker 77 before
returning 1. After creation the caller verifies 77, changes it to 88, and checks
that dispatched message 32769 returns 88. MSG is borrowed as all 48 bytes. A
negative cbWndExtra forces native rejection; retry uses a distinct class name.

One linear `Registration` occurrence retains an acquired current-executable
loader pin and one core `CountedQuantity<RegistrationSlot>` capacity unit.
The unit follows `Held` in both vacant and installed states; emitted thunks
introduce no capacity. The scalar occurrence marker checks runtime payload
transport; the compiler's owned claim records establish custody correspondence. Rejected registration returns it unchanged. Successful teardown
returns the same vacant capacity; it is registered again before final release.
`DestroyWindow`, an independent false `IsWindow`, and successful
`UnregisterClassA` establish the single-threaded adapter's drain before releasing
the pin. Only a successful native release sets the local completion flag needed
by cleanup; the acquired handle remains unchanged until the owned claim is
consumed. A failed unregister retains the installed occurrence; a failed
`FreeLibrary` retains vacant capacity. The process executable itself remains
resident; this is release of an acquired loader reference, not native unloading.

The core Content unit is this program's local budget for one live registration,
not an assertion about the OS registry's or loader's total capacity. Its routed
origin authorizes that local budget; neither the scalar marker nor equal handle
bits establish an external resource claim. The exact selected adapter and actual
GetModuleHandleExA success supply the observed loader-reference correspondence.
RegisterClassExA success supplies the foreign registration root, and its failure
supplies none. The checked owned claim conserves the local occurrence through
those runtime outcomes; it does not derive foreign success from an annotation.

Microsoft documents NULL as the calling executable and flags 0 as an incremented
loader reference requiring FreeLibrary:
https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandleexa
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-unregisterclassa

The Windows corpus run exits 70 after the complete lifecycle. The separate
portable `registration_capacity_case_round_trip` fixture exits 70 in native
execution and Terminal interpretation. `callback_private_deep_forwarded_record`
retains the same lifecycle through four inline wrappers, a call result, a local
and a generic helper parameter. Invalid capacity/qualification controls remain
under `tests/omega/fail/ownership`.

Expected Windows exit is 70. Other hosts have no runtime binding.
