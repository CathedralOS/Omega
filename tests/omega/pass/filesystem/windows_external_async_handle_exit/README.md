# External HANDLE transfer lifetime

The Windows native acceptance is exit 70: an asynchronous handle opened through
an independent `CreateFileW` binding refuses reads and writes, including empty
requests, without changing the caller's canary. A synchronous handle and its
duplicate then read consecutive bytes; closing the original leaves the duplicate
usable at the shared cursor, and cleanup succeeds.

This exercises the missing runtime distinction in
`windows_raw_argument_failures_exit`, whose asynchronous control only tests the
raw provider's own open-time flag rejection. It does not establish a general
arbitrary-HANDLE release classification or completion for other NT information
classes. `FILESYSTEM-PLATFORM-SERVICES` in `TASKS.md` retains the Windows runtime
requirement. Cross-publication alone is not that acceptance.
