// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 ftxr-modem Contributors
// Based on WSJT-X algorithms (Copyright (C) 2001-2026 WSJT Development Group).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use ftxr_core::ft8::decoder::{Ft8DecodeOptions, Ft8Decoder};
use ftxr_core::modulate::ft4::encode_ft4_tones_from_77bits;
use ftxr_core::modulate::ft8::encode_ft8_tones_from_77bits;
use ftxr_core::protocol::crc14::compute_crc14;
use ftxr_core::protocol::hashtable::CallsignHashTable;
use ftxr_core::protocol::ldpc::encode174_91;
use ftxr_core::protocol::pack77::pack77;
use ftxr_core::protocol::rvec::scramble_ft4;
use ftxr_core::protocol::types::{UnpackContext, UnpackMode};
use ftxr_core::protocol::unpack77::unpack77_readonly;
use ftxr_core::{DecodeConfig, Mode};
use serde::Deserialize;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("tests/fixtures directory")
}

#[derive(Deserialize)]
struct CodecVector {
    message: String,
    expected_msgsent: String,
    i3: u8,
    n3: u8,
    msg77_bits: String,
    ft4_scrambled_bits: String,
    ft8_crc14_hex: String,
    ft4_crc14_hex: String,
    ft8_codeword_bits: String,
    ft4_codeword_bits: String,
    ft8_tones: Vec<u8>,
    ft4_tones: Vec<u8>,
}

#[derive(Deserialize)]
struct CodecVectorFile {
    schema_version: u32,
    vectors: Vec<CodecVector>,
}

#[derive(Deserialize)]
struct ExpectedDecode {
    message: String,
    snr_db: i8,
    dt_sec: f32,
    af_hz: f32,
}

#[derive(Deserialize)]
struct ExpectedFixture {
    fixture_id: String,
    filename: String,
    utc: String,
    af_tol_hz: f32,
    dt_tol_sec: f32,
    snr_tol_db: i8,
    expected_decodes: Vec<ExpectedDecode>,
    #[serde(default)]
    three_stage_extra_decodes: Vec<ExpectedDecode>,
}

#[derive(Deserialize)]
struct ExpectedFile {
    schema_version: u32,
    mode: String,
    fixtures: Vec<ExpectedFixture>,
}

fn bits_to_string(bits: &[u8]) -> String {
    bits.iter()
        .map(|&b| if b == 0 { '0' } else { '1' })
        .collect()
}

fn read_wav_i16_mono(path: &Path) -> Vec<i16> {
    let bytes = std::fs::read(path).expect("read fixture wav");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let mut pos = 12usize;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let sz = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        pos += 8;
        if id == b"data" {
            data = Some(&bytes[pos..pos + sz]);
            break;
        }
        pos += sz + (sz & 1);
    }
    let raw = data.expect("data chunk");
    raw.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

#[test]
fn codec_vectors_match_bit_for_bit() {
    let fdir = fixtures_dir();
    let json_text = std::fs::read_to_string(fdir.join("codec_vectors.json")).unwrap();
    let file: CodecVectorFile = serde_json::from_str(&json_text).unwrap();
    assert_eq!(file.schema_version, 1);
    assert!(file.vectors.len() >= 50);

    let ht = CallsignHashTable::new();
    let ctx = UnpackContext::default();

    for vec in &file.vectors {
        let packed = pack77(&vec.message, None, &ctx, &ht)
            .unwrap_or_else(|e| panic!("pack77 failed for {:?}: {e}", vec.message));
        assert_eq!(packed.i3, vec.i3, "i3 mismatch for {:?}", vec.message);
        assert_eq!(packed.n3, vec.n3, "n3 mismatch for {:?}", vec.message);
        assert_eq!(
            packed.msgsent, vec.expected_msgsent,
            "msgsent mismatch for {:?}",
            vec.message
        );
        assert_eq!(
            bits_to_string(&packed.bits),
            vec.msg77_bits,
            "msg77 bits mismatch for {:?}",
            vec.message
        );

        let unpacked = unpack77_readonly(&packed.bits, UnpackMode::Rx, &ctx, &ht)
            .unwrap_or_else(|e| panic!("unpack77 failed for {:?}: {e}", vec.message));
        assert_eq!(unpacked.text, vec.expected_msgsent);

        let ft4_scr = scramble_ft4(&packed.bits);
        assert_eq!(
            bits_to_string(&ft4_scr),
            vec.ft4_scrambled_bits,
            "ft4 scrambled bits mismatch for {:?}",
            vec.message
        );

        let ft8_crc = format!("{:04x}", compute_crc14(&packed.bits));
        let ft4_crc = format!("{:04x}", compute_crc14(&ft4_scr));
        assert_eq!(ft8_crc, vec.ft8_crc14_hex);
        assert_eq!(ft4_crc, vec.ft4_crc14_hex);

        let ft8_cw = encode174_91(&packed.bits);
        let ft4_cw = encode174_91(&ft4_scr);
        assert_eq!(bits_to_string(&ft8_cw), vec.ft8_codeword_bits);
        assert_eq!(bits_to_string(&ft4_cw), vec.ft4_codeword_bits);

        let ft8_tones = encode_ft8_tones_from_77bits(&packed.bits);
        let ft4_tones = encode_ft4_tones_from_77bits(&packed.bits);
        assert_eq!(ft8_tones.as_slice(), vec.ft8_tones.as_slice());
        assert_eq!(ft4_tones.as_slice(), vec.ft4_tones.as_slice());
    }
}

#[test]
fn cli_decodes_golden_ft8_and_ft4_wav_fixtures_within_bounds() {
    let bin = env!("CARGO_BIN_EXE_ftxr-modem");
    let fdir = fixtures_dir();

    // 1. FT8 fixtures via CLI (`--format json`)
    let ft8_exp: ExpectedFile =
        serde_json::from_str(&std::fs::read_to_string(fdir.join("expected_ft8.json")).unwrap())
            .unwrap();
    assert_eq!(ft8_exp.schema_version, 1);
    assert_eq!(ft8_exp.mode, "ft8");

    for fx in &ft8_exp.fixtures {
        let wav_path = fdir.join(&fx.filename);
        let out = Command::new(bin)
            .args([
                "decode",
                "--mode",
                "ft8",
                "--depth",
                "3",
                "--format",
                "json",
                wav_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "ft8 decode {} failed: {}",
            fx.fixture_id,
            String::from_utf8_lossy(&out.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(doc["utc"], fx.utc);
        let decodes = doc["decodes"].as_array().unwrap();
        let mut by_msg: HashMap<String, &serde_json::Value> = HashMap::new();
        for d in decodes {
            by_msg.insert(d["message"].as_str().unwrap().to_string(), d);
        }

        for exp in &fx.expected_decodes {
            let got = by_msg.get(&exp.message).unwrap_or_else(|| {
                panic!(
                    "{} missing expected FT8 decode {:?}",
                    fx.fixture_id, exp.message
                )
            });
            let got_af = got["af_hz"].as_f64().unwrap() as f32;
            let got_dt = got["dt_sec"].as_f64().unwrap() as f32;
            let got_snr = got["snr_db"].as_i64().unwrap() as i8;
            assert!(
                (got_af - exp.af_hz).abs() <= fx.af_tol_hz,
                "{} {:?}: af {got_af} vs {}",
                fx.fixture_id,
                exp.message,
                exp.af_hz
            );
            assert!(
                (got_dt - exp.dt_sec).abs() <= fx.dt_tol_sec,
                "{} {:?}: dt {got_dt} vs {}",
                fx.fixture_id,
                exp.message,
                exp.dt_sec
            );
            assert!(
                (got_snr - exp.snr_db).abs() <= fx.snr_tol_db,
                "{} {:?}: snr {got_snr} vs {}",
                fx.fixture_id,
                exp.message,
                exp.snr_db
            );
        }

        // Also verify 3-stage WSJT-X parity when extra decodes are listed.
        if !fx.three_stage_extra_decodes.is_empty() {
            let pcm = read_wav_i16_mono(&wav_path);
            let mut dec = Ft8Decoder::new();
            let ht = CallsignHashTable::new();
            let cancel = AtomicBool::new(false);
            let cfg = DecodeConfig::for_mode(Mode::Ft8);
            let stage3 = dec
                .decode_i16(
                    &pcm,
                    &cfg,
                    &ht,
                    &cancel,
                    Ft8DecodeOptions::wsjtx_three_stage(),
                    |_| {},
                )
                .unwrap();
            let msgs: HashMap<&str, _> = stage3.iter().map(|d| (d.message.as_str(), d)).collect();
            for exp in fx
                .expected_decodes
                .iter()
                .chain(fx.three_stage_extra_decodes.iter())
            {
                assert!(
                    msgs.contains_key(exp.message.as_str()),
                    "{} 3-stage missing {:?}",
                    fx.fixture_id,
                    exp.message
                );
            }
        }
    }

    // 2. FT4 fixture via CLI (`--format json`)
    let ft4_exp: ExpectedFile =
        serde_json::from_str(&std::fs::read_to_string(fdir.join("expected_ft4.json")).unwrap())
            .unwrap();
    assert_eq!(ft4_exp.schema_version, 1);
    assert_eq!(ft4_exp.mode, "ft4");

    for fx in &ft4_exp.fixtures {
        let wav_path = fdir.join(&fx.filename);
        let out = Command::new(bin)
            .args([
                "decode",
                "--mode",
                "ft4",
                "--depth",
                "3",
                "--format",
                "json",
                wav_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(doc["utc"], fx.utc);
        let decodes = doc["decodes"].as_array().unwrap();
        let mut by_msg: HashMap<String, &serde_json::Value> = HashMap::new();
        for d in decodes {
            by_msg.insert(d["message"].as_str().unwrap().to_string(), d);
        }

        for exp in &fx.expected_decodes {
            let got = by_msg.get(&exp.message).unwrap_or_else(|| {
                panic!(
                    "{} missing expected FT4 decode {:?}",
                    fx.fixture_id, exp.message
                )
            });
            let got_af = got["af_hz"].as_f64().unwrap() as f32;
            let got_dt = got["dt_sec"].as_f64().unwrap() as f32;
            let got_snr = got["snr_db"].as_i64().unwrap() as i8;
            assert!((got_af - exp.af_hz).abs() <= fx.af_tol_hz);
            assert!((got_dt - exp.dt_sec).abs() <= fx.dt_tol_sec);
            assert!((got_snr - exp.snr_db).abs() <= fx.snr_tol_db);
        }
    }
}

#[test]
fn cli_formats_jt9_markers_stdin_raw_pcm_and_legal_subcommand() {
    let bin = env!("CARGO_BIN_EXE_ftxr-modem");
    let fdir = fixtures_dir();

    // `license` subcommand matches `--license`
    let lic_flag = Command::new(bin).arg("--license").output().unwrap();
    let lic_cmd = Command::new(bin).arg("license").output().unwrap();
    assert!(lic_flag.status.success());
    assert!(lic_cmd.status.success());
    assert_eq!(lic_flag.stdout, lic_cmd.stdout);
    let lic_str = String::from_utf8(lic_cmd.stdout).unwrap();
    assert!(lic_str.contains("GPL-3.0-only"));
    assert!(lic_str.contains("neither affiliated with nor endorsed by"));

    // FT8 uses '~', FT4 uses '+'
    let ft8_jt9 = Command::new(bin)
        .args([
            "decode",
            "--mode",
            "ft8",
            "--depth",
            "3",
            "--format",
            "jt9",
            fdir.join("191111_110200.wav").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(ft8_jt9.status.success());
    let ft8_txt = String::from_utf8(ft8_jt9.stdout).unwrap();
    assert!(ft8_txt.contains("110200") && ft8_txt.contains(" ~  CQ TA6CQ KN70"));
    assert!(ft8_txt.contains("<DecodeFinished>"));

    let ft4_jt9 = Command::new(bin)
        .args([
            "decode",
            "--mode",
            "ft4",
            "--depth",
            "3",
            "--format",
            "jt9",
            fdir.join("000000_000002.wav").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(ft4_jt9.status.success());
    let ft4_txt = String::from_utf8(ft4_jt9.stdout).unwrap();
    assert!(ft4_txt.contains("000002") && ft4_txt.contains(" +  KB0VHA KA1YQC R 539 MA"));

    // Raw i16le PCM over stdin (`-`)
    let pcm_i16 = read_wav_i16_mono(&fdir.join("191111_110200.wav"));
    let mut raw_bytes = Vec::with_capacity(pcm_i16.len() * 2);
    for s in pcm_i16 {
        raw_bytes.extend_from_slice(&s.to_le_bytes());
    }
    let mut child = Command::new(bin)
        .args([
            "decode",
            "--mode",
            "ft8",
            "--depth",
            "3",
            "--raw-pcm",
            "i16le",
            "--utc",
            "110200",
            "--format",
            "jsonl",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(&raw_bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let jsonl = String::from_utf8(out.stdout).unwrap();
    assert!(jsonl.contains("CQ TA6CQ KN70"));
    assert!(jsonl.contains("decode_finished"));
}
