# `ftxr-modem`

Standalone pure-Rust FT8 and FT4 modem CLI and warm stdio IPC v1 worker binary (`GPL-3.0-only`), powered by [`ftxr-core`](../ftxr-core).

Supports:
- `ftxr-modem decode` (`.wav` and headerless 12 kHz raw PCM, `jt9` / `jsonl` / `json` outputs)
- `ftxr-modem encode` (`.wav` and raw PCM output at configurable sample rates and offsets)
- `ftxr-modem --worker` (multiplexed binary-framed stdio IPC v1 daemon)
- `ftxr-modem --version` / `--license` legal notice inspection

Derived from WSJT-X classic FT8/FT4 routines (Copyright (C) 2001–2026 Joe Taylor, K1JT, and the WSJT Development Group). Licensed under `GPL-3.0-only` — see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
