# `ftxr-core`

Pure-Rust **FT8 and FT4** modem core library (`GPL-3.0-only`) providing:

- Thread-safe, session-scoped 77-bit WSJT-X message packing/unpacking (`pack77`/`unpack77`) and `CallsignHashTable`
- CRC-14 (`0x2757`), LDPC(174,91) encoding, and FT4 `rvec` scrambling
- Continuous-phase GFSK TX modulation (`BT = 2.0` FT8, `BT = 1.0` FT4) with configurable start delay and UTC-aligned late-TX offsets
- Classic WSJT-X-parity FT8 and FT4 RX pipelines with warm `FftPlanCache`, RAII scratch pools, multi-symbol coherent LLRs, Belief Propagation + GF(2) OSD (`osd174_91`), single-slot AP (`a1`–`a6`), `rayon` parallel candidate decoding with deterministic subtraction ordering, cooperative cancellation, and optional `DecodeTelemetry`

Derived from WSJT-X classic FT8/FT4 routines (Copyright (C) 2001–2026 Joe Taylor, K1JT, and the WSJT Development Group). Licensed under `GPL-3.0-only` — see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
