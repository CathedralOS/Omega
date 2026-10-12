"""Validate frozen direct-DFT oracles against actual output from both engines."""
import argparse
import json
import math
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def oracle(case):
    values = list(map(int, case["input"].split()))
    sampling, gain, offset, size, overlap, window, length = values[:7]
    samples = [value * gain / 1000000 + offset / 1000 for value in values[7:]]
    weights = [1.0 if window == 0 else (1 - math.cos(2 * math.pi * index / size)) / 2 for index in range(size)]
    energy = sum(weight ** 2 for weight in weights)
    spectra, powers = [], []
    for start in range(0, length - size + 1, size - overlap):
        weighted = [samples[start + index] * weights[index] for index in range(size)]
        spectrum = []
        for frequency in range(size // 2 + 1):
            # O(N^2) direct complex DFT, independent of the program's radix-2 FFT.
            transformed = sum(value * complex(math.cos(-2 * math.pi * frequency * index / size), math.sin(-2 * math.pi * frequency * index / size)) for index, value in enumerate(weighted))
            spectrum.append(abs(transformed) ** 2 * (1 if frequency in (0, size // 2) else 2) / (sampling * energy))
        power = sum(value ** 2 for value in weighted) / energy
        assert math.isclose(sum(spectrum) * sampling / size, power, rel_tol=1e-10, abs_tol=1e-12), "Parseval"
        spectra.append(spectrum)
        powers.append(power)
    density = [sum(spectrum[index] for spectrum in spectra) / len(spectra) for index in range(size // 2 + 1)]
    assert all(math.isclose(actual, pinned, rel_tol=1e-12, abs_tol=1e-14) for actual, pinned in zip(density, case["density"])), "pinned oracle changed"
    return density, sum(powers) / len(powers), sampling / size


def check(case, directory):
    density, power, spacing = oracle(case)
    for engine in ("native", "terminal"):
        rows = (directory / f"{engine}.stdout").read_text().splitlines()
        assert rows[0] == f'segments={case["segments"]}', rows
        frequencies = [int(row.split("=")[1]) / 1000000 for row in rows[1::2]]
        measured = [int(row.split("=")[1]) / 1000000000 for row in rows[2::2]]
        assert len(measured) == len(density)
        assert all(abs(actual - expected) <= 1e-6 for actual, expected in zip(frequencies, case["frequency"]))
        assert all(abs(actual - expected) <= 2e-9 + abs(expected) * 1e-10 for actual, expected in zip(measured, density)), (engine, measured, density)
        assert abs(sum(measured) * spacing - power) <= (len(density) * spacing * 2e-9 + abs(power) * 1e-10), "physical normalization"
        status = (directory / f"{engine}.status").read_text()
        assert status == "0" if engine == "native" else "return_code: 0" in status, status
        print(f'{case["name"]} {engine}: segments={case["segments"]} integrated_power={sum(measured)*spacing:.9g} expected={power:.9g} bins={len(measured)}')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runner", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    arguments = parser.parse_args()
    arguments.evidence.mkdir(parents=True, exist_ok=True)
    cases = json.loads((ROOT / "fixtures.json").read_text())
    inputs = []
    for case in cases:
        directory = arguments.evidence / case["name"]
        directory.mkdir(exist_ok=True)
        input_path = directory / "input.txt"
        input_path.write_text(case["input"])
        inputs.append(input_path)
    valid = "128 1000 0 8 0 0 8 " + "0 " * 8
    invalid = ["", valid.replace("128 ", "0 ", 1), valid.replace("128 ", "-1 ", 1), valid.replace(" 8 0 0 8 ", " 7 0 0 8 "), valid.replace(" 8 0 0 8 ", " 8 8 0 8 "), valid.replace(" 8 0 0 8 ", " 8 -1 0 8 "), valid.replace(" 8 0 0 8 ", " 8 0 2 8 "), valid.replace(" 8 0 0 8 ", " 8 0 0 7 "), valid.replace(" 8 0 0 8 ", " 8 0 0 65 "), valid.replace("1000", "0", 1), valid.replace("1000", "10001", 1), valid.replace("128", "1000001", 1), valid.replace(" 8 0 0 8 ", " 8 0 0 0 "), valid + "1", valid[:-2], valid + "?", valid.replace("128", "nan", 1), valid.replace("128", "inf", 1), valid.replace("128", "128.0", 1), valid.replace("128", "00000128", 1), valid + " " * 4096]
    for index, text in enumerate(invalid):
        directory = arguments.evidence / f"invalid{index}"
        directory.mkdir(exist_ok=True)
        input_path = directory / "input.txt"
        input_path.write_text(text)
        inputs.append(input_path)
    subprocess.run([arguments.runner, str(ROOT / "main.omg"), str(arguments.evidence), *map(str, inputs)], check=True)
    for case in cases:
        check(case, arguments.evidence / case["name"])
    for index in range(len(invalid)):
        directory = arguments.evidence / f"invalid{index}"
        for engine in ("native", "terminal"):
            assert (directory / f"{engine}.stdout").read_text() == "ERROR input or options\n"
            status = (directory / f"{engine}.status").read_text()
            assert status == "10" if engine == "native" else "return_code: 10" in status, status
    print(f"{len(invalid)} invalid controls rejected numerically observed in both engines")

if __name__ == "__main__":
    main()
