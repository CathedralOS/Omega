"""Check and execute whole-member D customers through the canonical edge.

Each subject is framed as one EREQ v1 envelope and evaluated by the canonical
evaluator receipt reconstructed through the selected lower chain: the bound
Gamma evaluator runs the bound Delta compiler over the packed Epsilon
evaluator closure plus the bound canonical entry. The host frames bytes and
compares observations; only the canonical edge checks and executes Epsilon.

Observations are canonical: tag 00 + little-endian i32 exit + exact stdout,
tag 01 + trap kind + stdout prefix, tag 02 + reject reason + coordinate, or
an EEOUT refusal frame / lower-chain Incomplete status. The six whole-member
customer identities are records of the same bound D members pinned by
tests/epsilon/interpreted-omega-experiment/run.sh; the complete-closure
customer is the bound packed D closure plus this gate's own composition main.
"""

import hashlib
import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path

EREQ = b"EEREQ\x01\x00\x00"


def run_evaluator(evaluator, program, sealed_input, timeout, label):
    started = time.monotonic()
    print(f"{label}: started, {timeout}s watchdog", flush=True)
    process = subprocess.Popen(
        [str(evaluator)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, errors = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input,
            timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{label}: timed out; no language judgment")
    elapsed = time.monotonic() - started
    if errors:
        raise SystemExit(
            f"{label}: unexpected stderr after {elapsed:.3f}s: {errors!r}")
    print(f"{label}: status {process.returncode}, {len(output)} bytes out, "
          f"{elapsed:.3f}s", flush=True)
    return process.returncode, output, elapsed


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if (len(source), actual) != (length, digest):
        raise SystemExit(f"{label}: identity changed to {len(source)} bytes, {actual}")


def main():
    directory = Path(sys.argv[1]).resolve()
    gate = Path(__file__).resolve().parent
    selected = sys.argv[2:]  # optional exact customer names

    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon evaluator closure", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    entry_source = (Path(os.environ["OMEGA_REPO_ROOT"]) /
                    "tests/epsilon/evaluator-entry/evaluator_entry.delta").read_bytes()
    require_identity("canonical entry", entry_source, 10950,
                     "52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3")

    subject = epsilon + entry_source
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    status, receipt, _ = run_evaluator(
        directory / "evaluator.exe",
        (directory / "delta_compiler.gamma").read_bytes(), request,
        900, "canonical receipt reconstruction")
    if status != 0:
        raise SystemExit(f"canonical receipt reconstruction status {status}")
    require_identity("canonical evaluator receipt", receipt, 729060,
                     "bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368")

    closure_digest = hashlib.sha256(epsilon).digest()

    def ereq(source, stdin=b""):
        return (EREQ + struct.pack("<III", 1, len(source), len(stdin))
                + closure_digest + source + stdin)

    members_dir = Path(os.environ["OMEGA_PATH_OMEGA_COMPILER"])
    customers_dir = gate.parent / "interpreted-omega-experiment/customers"

    # Whole, unchanged D members. These sizes and digests are records of the
    # same bound members pinned by omega_compiler.epsilon.sources and by the
    # interpreted-omega-experiment gate, not independent identities.
    REPRESENTATIONS = (members_dir / "representations.epsilon", 31993,
                       "9b29b3e4c5f410ca65b158f98bd099734e3c57e177ac2f8c89c92a88ccc715ec")
    REQUEST_UTF8 = (members_dir / "request_and_utf8.epsilon", 23678,
                    "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5")
    LEXICAL = (members_dir / "lexical_classification.epsilon", 2520,
               "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f")
    LEXER = (members_dir / "lexer.epsilon", 44649,
             "e16e7a42ee0848ff56b06dff4e9900569ae57724a281c0d3bada847717412ba6")
    ALPHA_TAPE = (members_dir / "alpha_tape.epsilon", 30832,
                  "26e943b2386e1f27761951af92d163cdaa54f32bdd990aa4694b91f10cb095a3")

    alpha_program = bytes.fromhex(
        "01 ff ffffffffffffffff" "01 fe 0100000000000000" "01 fd 0000000000000000"
        "11 00" "10 00 ff 4a00000000000000" "13 4c00000000000000" "03 fd fe"
        "0e 00 1e00000000000000" "0c 1e00000000000000" "00 fd" "12 00" "14")

    cases = []

    def add_customer(name, members, expected_size, expected_digest, stdin,
                     expected_observation):
        source = b""
        for path, size, digest in members:
            data = path.read_bytes()
            require_identity(f"{name} member {path.name}", data, size, digest)
            source += data
        require_identity(f"{name} packed", source, expected_size, expected_digest)
        cases.append((name, ereq(source, stdin), expected_observation))

    add_customer("Omega D lexical helpers",
                 (REPRESENTATIONS, LEXICAL,
                  (customers_dir / "omega_lexical/main.epsilon", 1486,
                   "6ce07453269102f7f468241d1a066a21cbe08c2a0652bb460c9c34d2f6ef11b2")),
                 35999, "61f5051d6effd0af72a353c1273a3691adce0d625494ca6e9850691c0d6668e5",
                 b"", b"\x00\x00\x00\x00\x00A")
    add_customer("Omega D Alpha tape buffers",
                 (REPRESENTATIONS, ALPHA_TAPE,
                  (customers_dir / "omega_alpha_tape/main.epsilon", 7274,
                   "a186166b32d38cfafdc02fc47f1ad46f2af7d18ed15b5bb0f18b0caa525bb35d")),
                 70099, "5f9de2eb32ceff006b9a494c54d8f447a259e32d244e3ee5218b9d2aee27be0c",
                 b"", b"\x00\x00\x00\x00\x00ABCDEFGH"
                 b"\x0c\x09\x00\x00\x00\x00\x00\x00\x00\x14" + alpha_program)
    add_customer("Omega D request and UTF-8",
                 (REPRESENTATIONS, REQUEST_UTF8,
                  (customers_dir / "omega_request/main.epsilon", 9510,
                   "0f838c478d6497cb4ca92d35f2aeed167b61eca4ab245847fa04da0fc2f7b0b7")),
                 65181, "f1ff455b84aeb07822da3d703941167e53d57805c5cc14b7288e3eb895433d72",
                 b"", b"\x00\x00\x00\x00\x00A\n")
    add_customer("Omega D request invocation fields",
                 (REPRESENTATIONS, REQUEST_UTF8,
                  (customers_dir / "omega_request_invocation/main.epsilon", 5930,
                   "b02e8f6f973a55ead1131a28cb939af0d6c31501d38bd7b15cba2cff4e6710bd")),
                 61601, "3445b9f234f35d79cd47f4359c4c1da2bc0f59032d1321dae692fa913cfafd17",
                 b"", b"\x00\x00\x00\x00\x00A\n")
    add_customer("Omega D numeric-base sums",
                 (REPRESENTATIONS, LEXICAL,
                  (customers_dir / "omega_numeric_base/main.epsilon", 1479,
                   "abf50c23d589624d59b7b3603918d5ab76e6a6192594c50534fbee6cdf334386")),
                 35992, "94b755b96b11707ea24e2027e421d486cd2e47dfb57afd545bd1c72a185a536b",
                 b"", b"\x00\x00\x00\x00\x00A")
    add_customer("Omega D lexer",
                 (REPRESENTATIONS, REQUEST_UTF8, LEXICAL, LEXER,
                  (customers_dir / "omega_lexer/main.epsilon", 6771,
                   "e4a262f1b011402970f958afbc6c950882bb75906fc7244b3ea19c8d489a0e06")),
                 109611, "274db84d81e4e5c37a0abae4e033f54a4c474cb4e123541cdcd7dca76306b5b9",
                 b"", b"\x00\x00\x00\x00\x00A")

    # The complete-D customer: the whole bound 525,334-byte closure plus this
    # gate's own composition main, which feeds the sealed Omega source to D's
    # actual OmegaScalarCompiler::compile and publishes the emitted tape.
    # Checking covers every declared member; execution crosses parser,
    # scalar compilation, and Alpha tape emission. Expected observation is
    # pinned in expected.hex.
    packed = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("packed Omega D closure", packed, 525334,
                     "b507fb785ea450409f3cd1c34f3c451656a124d23ecd912f9e8f8e6e45f41c64")
    composition_main = (gate / "main.epsilon").read_bytes()
    require_identity("composition main", composition_main, 1759,
                     "4fb023e60c166d5700fddc343a8ee8f3242d3c2915e7a7556ec36bef19aded9b")
    omega_source = (gate / "program.omg").read_bytes()
    require_identity("Omega source", omega_source, 33,
                     "7142aacc6080f4e3c41078e712ed5f4a9947c005c5d29d19b6dedcca9aa106d6")
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    whole_source = packed + composition_main
    cases.append(("Omega D complete closure", ereq(whole_source, omega_source),
                  expected))

    # The check-only counterpart: the same whole closure with a main that
    # exits immediately. Its allocation isolates the checking phase so the
    # complete-closure run above can be read as checking + execution; see the
    # README allocation table.
    check_main = (gate / "check_only_main.epsilon").read_bytes()
    require_identity("check-only main", check_main, 245,
                     "6ba150eb76208d88a764e4a90a6bdaf97d63a170ad26fd56d5016affadda0bbb")
    cases.append(("Omega D complete closure check only",
                  ereq(packed + check_main), b"\x00\x00\x00\x00\x00"))

    watchdog = int(os.environ.get("OMEGA_DCOMP_OBSERVATION_SECONDS", "14400"))
    if watchdog <= 0:
        raise SystemExit("OMEGA_DCOMP_OBSERVATION_SECONDS must be positive")

    if selected:
        known = {name for name, _, _ in cases}
        unknown = [name for name in selected if name not in known]
        if unknown:
            raise SystemExit(f"unknown exact D customers: {unknown}")
        cases = [case for case in cases if case[0] in set(selected)]

    for name, request, expected_observation in cases:
        status, observation, _ = run_evaluator(
            directory / "evaluator.exe", receipt, request, watchdog, name)
        if (status, observation) != (0, expected_observation):
            raise SystemExit(
                f"{name}: expected status 0 and {expected_observation[:60].hex()}, "
                f"received status {status} and {observation[:60].hex()}")
        print(f"{name}: canonical observation exact", flush=True)

    print(f"Epsilon D composition: {len(cases)} canonical observations pass",
          flush=True)


if __name__ == "__main__":
    main()
