# Windows file-time round-trip acceptance

The existing `pass/filesystem/windows_set_file_time_exit` now reaches its
unchanged exit 70 on native and Terminal. It writes the original eight literal
FILETIME bytes, closes/reopens the file, reads its timestamp and checks the
original Unix calibration. Five nested helper calls gained their required
`block` acknowledgements; operations, literals, Build and oracle are preserved.

[Run 41389666697](https://github.com/CathedralOS/Omega/actions/runs/41389666697)
qualified clean source `74468dd256e88cecb8fa2c9e3e85b835ea82f79f` on Windows x86_64:

- CLI opt level 1, SHA256 `1499ce051bb9732e7e9f52f58f3e19b513d3253eaab566b295c1a1f982165e26`.
- All 192 tracked project/library inputs frozen and reverified around every phase.
- Ordinary update 132.515s / ReviewRequired 3, 288 pending rows accepted normally,
  then ordinary resume 134.250s / exit 0 with its accepted lock.
- `run --both --keep` 28.828s: native 70, Terminal 70, wrapper 70.
- Native image SHA256 `0426571d3540f642e86c281e3aae12b97c9eec6780f8408066b5a3cc9a47f761`;
  Terminal artifact SHA256 `ca240e182084213ff5b6051886b426399d296403454ea4b2a979f1e9e12ffc97`.

Both products are byte-identical to the earlier 8d79903 products which reached
native 70 but failed Terminal's closed input requirement. The runtime repair
therefore changed execution of the same retained program. The missing predicate
was WriteFile's live byte-view extent bound. Both extent spellings now join the
formal place to its captured actual argument and canonical path, obtain its
existing descriptor borrow/live projection, and read the byte/element owner’s
actual length. Capacity and caller SSA IDs supply no substitute. The existing
closed comparator and false-requirement refusal are unchanged.

Windows filesystem supply admits exact DLL/target, typed semantic/storage
rosters and evaluated C word placements under the existing namespace/service
gate. The guest HANDLE table cannot select private worker handles. Synchronous
requests retain null optional pointers and same-process, noninherited duplicates.
DuplicateHandle publishes a guest identity in the same authorized mutable home;
failed close quarantines that identity. Native files remain owned by the worker
incarnation. UTF-16 paths, pointer/count words, FILETIME words and the mode-query
record use their retained storage and ABI. Guest last-error is restored/captured
immediately around every native entry; Windows maintains that state per thread
and may change it on successful calls. [Microsoft API contract](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-getlasterror).

Independent source reviews of the exact 8d79903 adapter and 74468dd shared-view
predicate path found no blocking issue. This acceptance executes the selected
shared-view guard and file-time path; other boundary access modes and the broader
Windows filesystem are not newly qualified.

Artifact `13117775514` retains original producer/profile, raw phase streams,
accepted review/lock, input and product manifests. Small reports/effects were
retained separately; the byte-identical 6.2MB products were reused with SHA256
and original ZIP CRC verification. No compiler binary or full ZIP was downloaded.
Earlier failures remain separate: bb7 lacked five acknowledgements; b24 passed
native 70 but lacked filesystem supply; 8d passed admission/native 70 and exposed
the unsupported closed extent. No SDK/Linux replays or broad suite were run.
Only this set-file-time board item is subtracted; breadth, metadata, portable
Windows work and occurrence-specific release proofs remain open.
