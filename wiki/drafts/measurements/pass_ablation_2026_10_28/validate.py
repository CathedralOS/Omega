"""Run the affected fact row and its reject twins, with raw observations."""

import json
from pathlib import Path
import re
import sys

import run as experiment


def main():
    omega = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    producer = experiment.benchmark.require_optimized(omega)
    (output / "producer.json").write_text(json.dumps(producer, indent=2), encoding="utf-8")
    fact = experiment.REPOSITORY / "samples/cli/facts/dyn_dispatch_selector_payload_bound"
    places = ["local", "field", "parameter", "call_result", "match_join", "const_name",
              "other_state", "other_machine", "target_body"]
    observations = []
    for place in ["root"] + places:
        project = fact if place == "root" else fact / place
        for invalid in ([False] if place == "root" else [False, True]):
            source = project / ("invalid/main.omg" if invalid else "main.omg")
            if place != "root":
                pin = experiment.REPOSITORY / "tests/omega" / ("fail" if invalid else "pass") / "facts"
                pin /= f"dyn_dispatch_selector_payload_bound_through_{place}_{'rejected' if invalid else 'exit'}"
                assert source.read_bytes() == (pin / "main.omg").read_bytes(), pin
                normalize = lambda text: re.sub(r'Build::application\(builder, "[^"]*"\);', '', text)
                assert normalize(source.with_name("build.omg").read_text()) == normalize((pin / "build.omg").read_text()), pin
            directory = output / (place + ("-reject" if invalid else "-positive"))
            directory.mkdir()
            arguments = [str(omega)] + (["--check"] if invalid else ["run", "--both"])
            # An explicit --target makes `run` compile-only, even for this host.
            if invalid:
                arguments += ["--target", "windows_x86_64"]
            arguments.append(str(source))
            code, wall, transcript = experiment.capture(arguments, directory)
            passed = (code != 0 and "exact arithmetic" in transcript and "may overflow `u64`" in transcript) if invalid else (
                code == 0 and "interp exit: 0" in transcript and "native exit 0" in transcript)
            observations.append(dict(place=place, invalid=invalid, command=arguments, exit=code,
                                     main_sha256=experiment.benchmark.sha256_file(source),
                                     build_sha256=experiment.benchmark.sha256_file(source.with_name("build.omg")),
                                     passed=passed, output=transcript))
            (output / "results.json").write_text(json.dumps(observations, indent=2), encoding="utf-8")
            print(f"{place} {'reject' if invalid else 'both'}: {'PASS' if passed else 'FAIL'}", flush=True)
    assert all(row["passed"] for row in observations), "affected row failed; see results.json"


if __name__ == "__main__":
    main()
