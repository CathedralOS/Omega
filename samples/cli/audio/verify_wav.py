"""Verify a WAV file written by the cli/audio samples.

Usage:
    python verify_wav.py <file.wav> [--blocks N] [--expect SAMPLE]

Prints channel count, sample width, frame rate, and frame count, then a
zero-crossing frequency estimate: the count of transitions across the
unsigned midpoint 128, halved and divided by the duration in seconds.
With `--blocks N` the frames are split into N-sample blocks and one
estimate is printed per block (for scale_melody, --blocks 2000 gives one
line per note).
With `--expect square_tone`, `sine_tone`, or `scale_melody`, also require
the canonical header and every authored PCM byte. A mismatch exits nonzero.
"""

import argparse
from pathlib import Path
import struct
import sys
import wave


def estimate(frames, rate):
    if not frames:
        return 0.0
    crossings = 0
    previous = frames[0]
    for sample in frames[1:]:
        if (previous < 128) != (sample < 128):
            crossings += 1
        previous = sample
    seconds = len(frames) / rate
    return crossings / 2 / seconds


def expected_image(sample):
    if sample == "square_tone":
        frames = bytes(192 if index % 18 < 9 else 64 for index in range(8000))
    elif sample == "sine_tone":
        cycle = (128, 148, 167, 185, 199, 211, 220, 225,
                 228, 225, 220, 211, 199, 185, 167, 148,
                 128, 108, 89, 71, 57, 45, 36, 31,
                 28, 31, 36, 45, 57, 71, 89, 108)
        frames = bytes(cycle[((index + 1) * 115343 // 65536) % 32]
                       for index in range(8000))
    else:
        periods = (31, 27, 24, 23, 20, 18, 16, 15)
        frames = bytes(192 if index % period < period // 2 else 64
                       for period in periods for index in range(2000))
    header = struct.pack("<4sI4s4sIHHIIHH4sI", b"RIFF", 36 + len(frames),
                         b"WAVE", b"fmt ", 16, 1, 1, 8000, 8000, 1, 8,
                         b"data", len(frames))
    return header + frames


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path")
    parser.add_argument("--blocks", type=int)
    parser.add_argument("--expect", choices=("square_tone", "sine_tone", "scale_melody"))
    arguments = parser.parse_args()
    path, blocks = arguments.path, arguments.blocks
    if blocks is not None and blocks <= 0:
        parser.error("--blocks must be positive")
    if arguments.expect:
        expected = expected_image(arguments.expect)
        actual = Path(path).read_bytes()
        if actual != expected:
            print(f"{arguments.expect}: WAV differs from the authored header/PCM "
                  f"({len(actual)} bytes, expected {len(expected)})", file=sys.stderr)
            return 1
        print(f"{arguments.expect}: exact header and PCM match ({len(actual)} bytes)")
    with wave.open(path) as wav:
        channels = wav.getnchannels()
        width = wav.getsampwidth()
        rate = wav.getframerate()
        count = wav.getnframes()
        frames = wav.readframes(count)
    print(f"channels={channels} sampwidth={width} rate={rate} frames={count}")
    if blocks is None:
        print(f"estimate_hz={estimate(frames, rate):.1f}")
    else:
        for block in range(count // blocks):
            piece = frames[block * blocks : (block + 1) * blocks]
            print(f"block {block}: estimate_hz={estimate(piece, rate):.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
