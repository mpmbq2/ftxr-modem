# Changelog

All notable changes to `ftxr-modem` are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-26

### Added
- **Pure-Rust FT8 & FT4 core engine (`ftxr-core`):**
  - Complete 77-bit WSJT-X message packing/unpacking (`pack77`/`unpack77`), thread-safe session-scoped `CallsignHashTable`, CRC-14 (`0x2757`), LDPC(174,91) encoder, and FT4 `rvec` scrambler.
  - Continuous-phase GFSK TX modulators for FT8 (`79` symbols, `BT = 2.0`, nominal `500 ms` delay) and FT4 (`105` symbols, `BT = 1.0`, nominal `300 ms` delay), supporting arbitrary sample rates and UTC-aligned late-TX offsets with click-free fade-in.
  - Classic single-threaded-parity FT8 (`-8`) and FT4 (`-5`) RX pipelines with warm `FftPlanCache`, RAII `PassScratchPool`/`CandidateScratchPool`, multi-symbol coherent LLRs, hybrid Belief Propagation + GF(2) Ordered Statistics Decoding (`osd174_91`), single-slot AP hypotheses (`a1`–`a6` + `? a*` quality tagging), multi-pass subtraction, QSO-first priority batching, deterministic parallel candidate decoding via `rayon`, cooperative mid-pass cancellation (`AtomicBool`), and opt-in `DecodeTelemetry` (FEC path, Costas frequency drift, and 8-bucket QSB envelope).
- **Standalone CLI & Warm Stdio IPC v1 Worker (`ftxr-modem`):**
  - `ftxr-modem decode` supporting 16-bit PCM / 32-bit float WAV and headerless raw PCM (`i16le`/`f32le`) with `jt9`-compatible text, streaming `jsonl`, and `json` outputs.
  - `ftxr-modem encode` supporting WAV and raw PCM output with configurable sample rate, start delay, late-TX offset, and slot padding.
  - `ftxr-modem --worker` persistent warm stdio IPC v1 daemon (`docs/ipc-protocol.md`) with 16-byte `FTXR` binary framing, non-blocking isolated TX fast-path (`encode_tx`), streaming `decode_event` emission, cooperative `<50 ms` `cancel`, deadline watchdog, and clean session management.
  - `--version`, `--version --json`, `--license`, and `license` commands with embedded GPLv3 and WSJT-X attribution notices.
- **Compliance, CI & Release Automation:**
  - Canonical FSF `GPL-3.0-only` `LICENSE`, upstream WSJT-X `NOTICE`, `deny.toml`, `scripts/verify-compliance.sh`, self-contained regression tests, and GitHub Actions CI + 4-target cross-platform release packaging (`linux-x86_64`, `linux-aarch64`, `darwin-x86_64`, `darwin-aarch64`, source archive, and `SHA256SUMS`).
