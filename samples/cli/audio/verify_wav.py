"""Verify a WAV file written by the cli/audio samples.

Usage:
    python verify_wav.py <file.wav> [--blocks N]

Prints channel count, sample width, frame rate, and frame count, then a
zero-crossing frequency estimate: the count of transitions across the
unsigned midpoint 128, halved and divided by the duration in seconds.
With `--blocks N` the frames are split into N-sample blocks and one
estimate is printed per block (for scale_melody, --blocks 2000 gives one
line per note).
"""

import sys
import wave


def estimate(frames, rate):
    crossings = 0
    previous = frames[0]
    for sample in frames[1:]:
        if (previous < 128) != (sample < 128):
            crossings += 1
        previous = sample
    seconds = len(frames) / rate
    return crossings / 2 / seconds


def main():
    args = [arg for arg in sys.argv[1:] if not arg.startswith("--")]
    blocks = None
    for arg in sys.argv[1:]:
        if arg.startswith("--blocks="):
            blocks = int(arg.split("=", 1)[1])
    if len(args) != 1:
        print(__doc__)
        return 2
    path = args[0]
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
