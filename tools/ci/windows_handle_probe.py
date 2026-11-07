"""Observe six native handle cases; this is not compiler acceptance evidence.

The acquisition arguments match the unchanged canonicalization customer's seed
and attributes opens. No content transfer is attempted. A successful NUL open
falsifies character-device exclusion by flags alone. An unavailable CONOUT$ on
the hosted runner says nothing about exclusion on a console-attached process.
"""

import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import platform
import tempfile


def main():
    if os.name != "nt":
        raise SystemExit("This native observation requires Windows")
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    native = ctypes.WinDLL("ntdll", use_last_error=True)

    def bind(library, name, arguments, result):
        function = getattr(library, name)
        function.argtypes = arguments
        function.restype = result
        return function

    handle = wintypes.HANDLE
    u32 = wintypes.DWORD
    create = bind(kernel, "CreateFileW", [wintypes.LPCWSTR, u32, u32,
                  ctypes.c_void_p, u32, u32, handle], handle)
    duplicate = bind(kernel, "DuplicateHandle", [handle, handle, handle,
                     ctypes.POINTER(handle), u32, wintypes.BOOL, u32], wintypes.BOOL)
    current = bind(kernel, "GetCurrentProcess", [], handle)
    close = bind(kernel, "CloseHandle", [handle], wintypes.BOOL)
    kind = bind(kernel, "GetFileType", [handle], u32)
    console_mode = bind(kernel, "GetConsoleMode", [handle, ctypes.POINTER(u32)], wintypes.BOOL)
    allocate_console = bind(kernel, "AllocConsole", [], wintypes.BOOL)
    allocate_memory = bind(kernel, "VirtualAlloc", [ctypes.c_void_p, ctypes.c_size_t,
                           u32, u32], ctypes.c_void_p)

    class IoStatus(ctypes.Structure):
        _fields_ = [("status_or_pointer", ctypes.c_void_p),
                    ("information", ctypes.c_size_t)]

    query_mode = bind(native, "NtQueryInformationFile", [handle,
                      ctypes.POINTER(IoStatus), ctypes.c_void_p, u32, u32], ctypes.c_long)

    def observed_call(function, *arguments):
        ctypes.set_last_error(0)
        result = function(*arguments)
        return result, ctypes.get_last_error()

    def handle_text(value):
        return f"0x{(value or 0):016x}"

    def close_observation(value):
        result, error = observed_call(close, value)
        return {"success": bool(result), "last_error": error}

    def observe(path, object_name, protocol_name, arguments):
        acquired, error = observed_call(create, str(path), *arguments)
        row = {"object": object_name, "path": str(path), "protocol": protocol_name,
               "arguments": dict(zip(("desired_access", "share_mode", "security_attributes",
                                      "creation_disposition", "flags", "template"), arguments)),
               "acquisition": {"handle": handle_text(acquired), "last_error": error}}
        if acquired in (None, ctypes.c_void_p(-1).value):
            row["acquisition"]["success"] = False
            return row
        row["acquisition"]["success"] = True
        alias = handle()
        process = current()
        result, error = observed_call(duplicate, process, acquired, process,
                                      ctypes.byref(alias), 0, False, 2)
        row["duplicate"] = {"success": bool(result), "handle": handle_text(alias.value),
                            "last_error": error, "source_process": handle_text(process),
                            "target_process": handle_text(process), "desired_access": 0,
                            "inherit": False, "options": 2}
        if result:
            value, error = observed_call(kind, alias)
            row["file_type"] = {"value": value, "last_error": error}
            mode = u32()
            result, error = observed_call(console_mode, alias, ctypes.byref(mode))
            row["console_mode"] = {"success": bool(result), "last_error": error,
                                   "value": mode.value if result else None}
            # Allocate at most six pages, deliberately without VirtualFree.
            # Native storage therefore survives Python frames and interpreter
            # teardown until the OS destroys this process, even after PENDING.
            # Never read unfinished outputs or wait on an unrelated file event.
            memory, error = observed_call(allocate_memory, None, 4096, 0x3000, 4)
            if memory:
                status = IoStatus.from_address(memory)
                file_mode = u32.from_address(memory + ctypes.sizeof(IoStatus))
                code = query_mode(alias, ctypes.byref(status), ctypes.byref(file_mode), 4, 16)
                pending = (code & 0xFFFFFFFF) == 0x103
                row["file_mode"] = {"ntstatus": f"0x{code & 0xFFFFFFFF:08x}",
                                    "pending": pending, "class": 16, "length": 4,
                                    "value": file_mode.value if code == 0 else None,
                                    "information": status.information if not pending else None}
            else:
                row["file_mode"] = {"allocation_failed": True, "last_error": error}
            row["duplicate_close"] = close_observation(alias)
        row["original_close"] = close_observation(acquired)
        return row

    allocated, error = observed_call(allocate_console)
    system = Path(os.environ["SystemRoot"]) / "System32"
    report = {"source": os.environ.get("GITHUB_SHA"), "platform": platform.platform(),
              "pointer_bits": ctypes.sizeof(ctypes.c_void_p) * 8,
              "no_compiler_acceptance": True,
              "environment_limit": "CONOUT$ failure on a headless runner does not establish console exclusion",
              "alloc_console": {"success": bool(allocated), "last_error": error},
              "native_libraries": {name: {"path": str(system / name),
                  "sha256": hashlib.sha256((system / name).read_bytes()).hexdigest()}
                  for name in ("kernel32.dll", "ntdll.dll")}, "cases": []}
    try:
        with tempfile.TemporaryDirectory(prefix="omega-handle-probe-") as directory:
            regular = Path(directory) / "regular.txt"
            regular.write_bytes(b"canon")
            for protocol, arguments in (("seed", (1073741824, 7, None, 2, 1048704, None)),
                                        ("attributes", (128, 7, None, 3, 34603008, None))):
                for name, path in (("regular_file", regular), ("null_device", "NUL"),
                                   ("console_output", "CONOUT$")):
                    report["cases"].append(observe(path, name, protocol, arguments))
    except Exception as error:
        report["setup_exception"] = repr(error)
    failures = []
    if len(report["cases"]) != 6:
        failures.append("expected exactly six observation rows")
    regular_rows = [row for row in report["cases"] if row["object"] == "regular_file"]
    if len(regular_rows) != 2:
        failures.append("expected both ordinary-file setup controls")
    for row in regular_rows:
        if not (row["acquisition"]["success"]
                and row.get("duplicate", {}).get("success")
                and row.get("file_type", {}).get("value") == 1):
            failures.append(f"ordinary-file {row['protocol']} acquisition/duplication/disk control failed")
    if "setup_exception" in report:
        failures.append("native observation setup raised an exception")
    report["setup_control_failures"] = failures
    output = Path("build/windows-handle-probe.json")
    output.parent.mkdir(exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(output.read_text(encoding="utf-8"), end="")
    if failures:
        raise SystemExit("Native observation setup controls failed; actual JSON was retained")


if __name__ == "__main__":
    main()
