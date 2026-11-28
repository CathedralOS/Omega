"""Collect coarse call spans from the unmodified Linux release binary with GDB."""

import os
import time

import gdb

FUNCTIONS = {
    "<compiler::Program>::prepare": "product preparation",
    "<native_compiler::NativeProgram>::emit": "native emission",
    "<source_compiler::driver::resolve_and_type::ResolveAndTypeRequest>::prepare_with_body_scope": "resolve and type",
    "source_compiler::driver::compile_time_invocations::fixed_layouts::schema::record_argument": "layout schema construction",
    "prepared_typed_trees_to_lowered_program::lower_program_with_parallel": "lower program",
    "lowered_program_to_checked_program::verdict::judge_compilation_with_activity_and_policy": "judge compilation",
}
gdb.execute("set pagination off")
gdb.execute("set print thread-events off")
gdb.execute("set debuginfod enabled off")
gdb.execute("set disable-randomization off")
gdb.execute("starti")
inferior = gdb.selected_inferior()
exe = os.path.realpath(f"/proc/{inferior.pid}/exe")
with open(f"/proc/{inferior.pid}/maps") as f:
    base = next(
        int(parts[0].split("-")[0], 16)
        for parts in map(str.split, f)
        if len(parts) >= 6 and parts[2] == "00000000" and parts[-1] == exe
    )
start = time.monotonic_ns()
spans = []
missed = []


class Return(gdb.FinishBreakpoint):
    def __init__(self, label):
        super().__init__(gdb.newest_frame(), internal=True)
        self.label = label
        self.begin = time.monotonic_ns()

    def stop(self):
        spans.append((self.label, self.begin - start, time.monotonic_ns() - start))
        return False

    def out_of_scope(self):
        missed.append(self.label)


class Entry(gdb.Breakpoint):
    def __init__(self, addr, label):
        super().__init__(f"*{addr:#x}", internal=True)
        self.label = label

    def stop(self):
        Return(self.label)
        return False


seen = set()
with open(os.environ["BENCH_NM"]) as f:
    for line in f:
        parts = line.rstrip().split(" ", 2)
        if len(parts) == 3 and parts[2] in FUNCTIONS:
            Entry(base + int(parts[0], 16), FUNCTIONS[parts[2]])
            seen.add(parts[2])
if seen != FUNCTIONS.keys():
    raise RuntimeError(f"unmapped measurement functions: {FUNCTIONS.keys() - seen}")
gdb.execute("continue")
with open(os.environ["BENCH_SPANS"], "w") as f:
    f.write("function\tstart_ns\tend_ns\n")
    f.writelines(f"{label}\t{begin}\t{end}\n" for label, begin, end in spans)
    f.writelines(f"{label}\tmissing\treturn\n" for label in missed)
