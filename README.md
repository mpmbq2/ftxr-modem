# `ftxr-modem` — Pure-Rust FT8 & FT4 Modem Engine

`ftxr-modem` is a standalone, pure-Rust **FT8 and FT4** digital-mode modem engine (77-bit protocol codec, continuous-phase GFSK TX modulator, and weak-signal RX decoder) based on the classic single-threaded algorithms of WSJT-X (`lib/ft8/`, `lib/ft4/`, and `lib/77bit/`).

It compiles to a single self-contained native binary with **zero C, C++, Fortran, FFTW, Qt, or Hamlib runtime dependencies** and operates both as a standalone CLI tool (`decode` / `encode`) and as a persistent, multiplexed warm stdio IPC worker (`--worker`).

## License & Attribution

- **License:** GNU General Public License v3.0 only (`GPL-3.0-only`). See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
- **Upstream Attribution:** Based on FT8, FT4, and 77-bit algorithms from WSJT-X (Copyright (C) 2001–2026 Joe Taylor, K1JT, and the WSJT Development Group).
- **Trademark Notice:** `WSJT`, `WSJT-X`, `MAP65`, and `QMAP` are trademarks of the WSJT Development Team. `ftxr-modem` is an independent project based on WSJT-X algorithms and is neither affiliated with nor endorsed by the WSJT Development Team.

## Installation

### Prebuilt Release Binaries (Linux & macOS, `x86_64` & `aarch64`)

Download the release tarball and `SHA256SUMS` from [GitHub Releases](https://github.com/mpmbq2/ftxr-modem/releases):

```bash
TAG="v0.1.0"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m | sed 's/amd64/x86_64/; s/arm64/aarch64/')"
TARBALL="ftxr-modem-${TAG}-${OS}-${ARCH}.tar.gz"

curl -fsSLO "https://github.com/mpmbq2/ftxr-modem/releases/download/${TAG}/${TARBALL}"
curl -fsSLO "https://github.com/mpmbq2/ftxr-modem/releases/download/${TAG}/SHA256SUMS"

# Verify checksum (Linux: sha256sum; macOS: shasum -a 256)
grep "  ${TARBALL}\$" SHA256SUMS | sha256sum -c -
tar -xzf "${TARBALL}"
./ftxr-modem --version
```

### From Source via Cargo (Rust >= 1.85)

```bash
cargo install --locked --git https://github.com/mpmbq2/ftxr-modem --tag v0.1.0 ftxr-modem
```

Or build from a local checkout:

```bash
cargo build --release --locked --bin ftxr-modem
```

## Standalone CLI Usage

### Decode `.wav` or Raw 12 kHz PCM

```bash
# Decode FT8 WAV in jt9-compatible text format
ftxr-modem decode --mode ft8 --depth 3 tests/fixtures/210703_133430.wav

# Decode FT4 WAV with structured JSON + telemetry (Costas drift & QSB envelope)
ftxr-modem decode --mode ft4 --depth 3 --format json --telemetry tests/fixtures/000000_000002.wav

# Stream JSON lines from raw headerless 12 kHz i16le PCM on stdin
cat slot_12k.raw | ftxr-modem decode --mode ft8 --raw-pcm i16le --format jsonl -
```

### Encode FT8 / FT4 Waveforms

```bash
# Generate a standard 12 kHz WAV (500 ms nominal FT8 start delay, padded to 15 s)
ftxr-modem encode --mode ft8 --af 1500 --sr 12000 --full-slot -o tx_ft8.wav "CQ N0AAA EM10"

# Generate 48 kHz raw active FT4 f32le PCM with a 300 ms UTC late-TX offset
ftxr-modem encode --mode ft4 --af 1400 --sr 48000 --start-delay-ms 0 --offset-ms 300 --raw-pcm -o tx_ft4.f32 "CQ N0AAA EM10"
```

### License & Version Probes

```bash
ftxr-modem --version
ftxr-modem --version --json
ftxr-modem --license
```

## Persistent Warm IPC Worker (`--worker`)

Launch `ftxr-modem --worker` for low-latency persistent integration over `stdin`/`stdout` using the length-prefixed binary `FTXR` v1 framing protocol:

- Pre-warms all 12 FT8/FT4 FFT plans and scratch pools before emitting `ready`
- Streams `decode_event` messages immediately as candidates complete (QSO-frequency first)
- Runs `encode_tx` on an isolated OS thread so TX modulation never blocks on an active Pass 3 decode
- Honors cooperative mid-pass `cancel(job_id)` and per-slot `timeout_ms` deadlines in `< 50 ms`
- Writes all diagnostics strictly to `stderr` (`stdout` carries exclusively `FTXR` frames)

See [`docs/ipc-protocol.md`](docs/ipc-protocol.md) for the complete v1 wire specification.

## Workspace Crates

- [`crates/ftxr-core`](crates/ftxr-core) — Pure-Rust GPLv3 FT8/FT4 DSP, 77-bit codec, FEC, modulator, and decoder library crate.
- [`crates/ftxr-modem`](crates/ftxr-modem) — Pure-Rust GPLv3 standalone CLI and warm stdio IPC v1 worker binary (`ftxr-modem`).

## Verification & Compliance

```bash
cargo test --workspace --locked
./scripts/verify-compliance.sh
cargo deny check
```
