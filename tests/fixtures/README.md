# `ftxr-modem` Regression Fixtures

All files in this directory are self-contained regression fixtures enabling `cargo test --workspace --locked` to validate `ftxr-core` and `ftxr-modem` with zero external WSJT-X, Fortran, FFTW, or `digi-dx` dependencies.

| File | Mode | Size / Samples | SHA-256 | Provenance & License |
| :--- | :--- | :--- | :--- | :--- |
| `210703_133430.wav` | FT8 | 360,246 B / 180,000 | `9feb99c275770a6618538026da7decc6b09eb6cf63121e5168fa86dcdf00c2f5` | Off-air HF FT8 recording from WSJT-X `samples/FT8/210703_133430.wav` (`GPLv3`) |
| `000000_000002.wav` | FT4 | 145,196 B / 72,576 | `d9e91fa04ba138a7b9f41b4103823c77ca1c3a9775101f6b14d60935bcd3813b` | Multi-signal FT4 recording from WSJT-X `samples/FT4/000000_000002.wav` (`GPLv3`) |
| `191111_110200.wav` | FT8 | 360,202 B / 180,000 | `bfd0bd61ac22b6d19a861039023fdf8e17397ae084277e63174b3ab3773c4022` | Off-air HF FT8 recording originally from `kgoba/ft8_lib` (`MIT License`, Copyright (c) Karlis Goba) |
| `codec_vectors.json` | FT8/FT4 | JSON vectors | — | Golden 77-bit pack/unpack, CRC-14, LDPC(174,91), and channel-tone test vectors across `i3`/`n3` message types |
| `expected_ft8.json` | FT8 | JSON expectations | — | Golden FT8 decode sets & AF/DT/SNR tolerances for `210703_133430.wav` and `191111_110200.wav` |
| `expected_ft4.json` | FT4 | JSON expectations | — | Golden FT4 decode set & AF/DT/SNR tolerances for `000000_000002.wav` |
