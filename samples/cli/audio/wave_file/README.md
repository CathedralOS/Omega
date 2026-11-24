# Two-frame WAV file

This application uses the same `WavOutput` provider as the tone samples to
write `two_frames.wav` in its working directory. The two mono PCM samples
are 0 and 255 at 8000 Hz. The file has a 44-byte RIFF/WAVE header followed
by those two samples. The entry traps if the writer reports a short write.
The build selects the hosted filesystem providers for Linux x86-64 and ARM64,
macOS ARM64, and Windows x86-64 and ARM64.

Run `mbx xtask sample-gate --filter samples/cli/audio/wave_file` to execute
the application on native and Psi. Its declared file oracle compares each
engine's bytes to `expected/two_frames.wav`, generated independently with
Python's standard `wave` encoder. A change to either PCM byte should fail
the corresponding file expectation even if both engines agree.
