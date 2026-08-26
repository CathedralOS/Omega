#!/usr/bin/env python3
import sys
from omgrfn17_ckir import decode, selected, static_view_bytes
from omgrfn17_frame import require, split
from omgrfn17_owner import run
from omgrfn17_source import parse_selected_source

def check():
    frame = split(sys.stdin.buffer.read())
    source = parse_selected_source(frame.omgcomp)
    module = decode(frame.ckir)
    counts = selected(module)
    require(len(source.guards) == counts[23] == counts[24] == counts[25],
            "source-site to synthetic-family cardinality")
    static = static_view_bytes(module)
    require(static == source.literal,
            "optional authored byte literal to optional StaticByteView bytes")

if __name__ == "__main__": run("R4-lowering", check)
