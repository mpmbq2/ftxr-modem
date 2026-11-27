// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 ftxr-modem Contributors
// Based on WSJT-X algorithms (Copyright (C) 2001-2026 WSJT Development Group).

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAGIC: &[u8; 4] = b"FTXR";
const FLAG_HAS_BINARY: u16 = 0x0001;
const FLAG_BINARY_F32LE: u16 = 0x0002;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("tests/fixtures directory")
}

fn write_frame<W: Write>(
    w: &mut W,
    flags: u16,
    json: &[u8],
    payload: &[u8],
) -> std::io::Result<()> {
    let eff_flags = flags
        | if payload.is_empty() {
            0
        } else {
            FLAG_HAS_BINARY
        };
    let mut hdr = [0u8; 16];
    hdr[0..4].copy_from_slice(MAGIC);
    hdr[4..6].copy_from_slice(&1u16.to_le_bytes());
    hdr[6..8].copy_from_slice(&eff_flags.to_le_bytes());
    hdr[8..12].copy_from_slice(&(json.len() as u32).to_le_bytes());
    hdr[12..16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    w.write_all(&hdr)?;
    w.write_all(json)?;
    w.write_all(payload)?;
    w.flush()
}

fn read_frame<R: Read>(r: &mut R) -> std::io::Result<(serde_json::Value, Vec<u8>)> {
    let mut hdr = [0u8; 16];
    r.read_exact(&mut hdr)?;
    assert_eq!(&hdr[0..4], MAGIC, "stdout had non-FTXR frame header");
    let ver = u16::from_le_bytes([hdr[4], hdr[5]]);
    assert_eq!(ver, 1);
    let json_len = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as usize;
    let payload_len = u32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
    let mut json = vec![0u8; json_len];
    r.read_exact(&mut json)?;
    let mut payload = vec![0u8; payload_len];
    r.read_exact(&mut payload)?;
    let val: serde_json::Value = serde_json::from_slice(&json).expect("valid JSON frame body");
    Ok((val, payload))
}

fn read_wav_i16_bytes(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    let mut pos = 12usize;
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
            return bytes[pos..pos + sz].to_vec();
        }
        pos += sz + (sz & 1);
    }
    panic!("missing data chunk in {}", path.display());
}

#[test]
fn worker_ipc_streaming_qso_first_concurrent_tx_cancel_and_error_recovery() {
    let bin = env!("CARGO_BIN_EXE_ftxr-modem");
    let fdir = fixtures_dir();
    let crowded_ft8 = read_wav_i16_bytes(&fdir.join("210703_133430.wav"));

    let mut child = Command::new(bin)
        .args(["--worker", "--threads", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut c_in = BufWriter::new(child.stdin.take().unwrap());
    let mut c_out = BufReader::new(child.stdout.take().unwrap());

    // 1. Handshake
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"hello","id":"h1","protocol_version":1,"client":"integration-test"}"#,
        &[],
    )
    .unwrap();
    let (ready, _) = read_frame(&mut c_out).unwrap();
    assert_eq!(ready["op"], "ready");
    assert_eq!(ready["protocol_version"], 1);
    assert_eq!(ready["threads"], 2);

    // 2. Streaming multi-signal decode on 210703_133430 with QSO-first focus at 1648 Hz (`K1JT EA3AGB -15`)
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"decode_slot","id":"stream1","slot_id":"210703_133430","mode":"ft8","depth":3,"rx_freq_hz":1648.0,"freq_tol_hz":20.0,"qso_first":true,"sample_rate":12000,"sample_format":"i16le"}"#,
        &crowded_ft8,
    )
    .unwrap();

    let mut events = Vec::new();
    let first_event_at;
    let done_at;
    let mut maybe_first = None;
    loop {
        let (v, _) = read_frame(&mut c_out).unwrap();
        match v["op"].as_str().unwrap() {
            "decode_event" => {
                if maybe_first.is_none() {
                    maybe_first = Some(Instant::now());
                }
                events.push(v["decode"].clone());
            }
            "decode_done" => {
                done_at = Instant::now();
                first_event_at = maybe_first.expect("at least one decode_event");
                assert_eq!(v["status"], "completed");
                assert_eq!(v["decoded_count"], events.len());
                break;
            }
            other => panic!("unexpected op {other}"),
        }
    }
    assert!(events.len() >= 19, "got {} decodes", events.len());
    assert!(first_event_at <= done_at);
    // QSO-first focus at 1648 Hz should emit the 1648 Hz signal first.
    assert_eq!(events[0]["message"], "K1JT EA3AGB -15");

    // 3. Non-fatal error recovery (bad sample_rate, bad sample_format, unknown op)
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"decode_slot","id":"bad_sr","mode":"ft8","sample_rate":48000}"#,
        &[],
    )
    .unwrap();
    let (e1, _) = read_frame(&mut c_out).unwrap();
    assert_eq!(e1["op"], "error");
    assert_eq!(e1["code"], "invalid_sample_rate");
    assert_eq!(e1["fatal"], false);

    write_frame(
        &mut c_in,
        0,
        br#"{"op":"encode_tx","id":"bad_fmt","mode":"ft8","message":"CQ N0AAA EM10","af_hz":1500.0,"sample_format":"u8"}"#,
        &[],
    )
    .unwrap();
    let (e2, _) = read_frame(&mut c_out).unwrap();
    assert_eq!(e2["op"], "error");
    assert_eq!(e2["code"], "invalid_sample_format");
    assert_eq!(e2["fatal"], false);

    // 4. Concurrent encode_tx (12 kHz late-TX) + mid-pass cancellation during active Pass 3 decode
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"decode_slot","id":"cslot","mode":"ft8","depth":3,"sample_rate":12000,"sample_format":"i16le"}"#,
        &crowded_ft8,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(10));

    let t_tx = Instant::now();
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"encode_tx","id":"tx12","mode":"ft8","message":"CQ N0AAA EM10","af_hz":1500.0,"sample_rate":12000,"sample_format":"i16le","start_delay_ms":0,"offset_ms":2000}"#,
        &[],
    )
    .unwrap();

    loop {
        let (v, payload) = read_frame(&mut c_out).unwrap();
        match v["op"].as_str().unwrap() {
            "encode_result" => {
                assert_eq!(v["id"], "tx12");
                assert_eq!(v["sample_rate"], 12_000);
                assert_eq!(payload.len(), (151_680 - 24_000) * 2);
                let tx_limit = if cfg!(debug_assertions) { 1000 } else { 100 };
                assert!(
                    t_tx.elapsed().as_millis() < tx_limit,
                    "concurrent encode_tx took {} ms (limit {tx_limit} ms)",
                    t_tx.elapsed().as_millis()
                );
                break;
            }
            "decode_event" => {}
            "decode_done" => panic!("active decode finished before concurrent encode_tx completed"),
            other => panic!("unexpected op {other}"),
        }
    }

    let t_cancel = Instant::now();
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"cancel","id":"cid","target_id":"cslot"}"#,
        &[],
    )
    .unwrap();

    let mut saw_ack = false;
    let mut saw_done = false;
    while !(saw_ack && saw_done) {
        let (v, _) = read_frame(&mut c_out).unwrap();
        match v["op"].as_str().unwrap() {
            "cancel_ack" => {
                assert_eq!(v["cancelled"], true);
                saw_ack = true;
            }
            "decode_done" => {
                assert_eq!(v["id"], "cslot");
                assert_eq!(v["status"], "cancelled");
                let cancel_limit = if cfg!(debug_assertions) { 2000 } else { 100 };
                assert!(
                    t_cancel.elapsed().as_millis() < cancel_limit,
                    "cancel took {} ms (limit {cancel_limit} ms)",
                    t_cancel.elapsed().as_millis()
                );
                saw_done = true;
            }
            "decode_event" => {}
            other => panic!("unexpected op {other}"),
        }
    }

    // 5. Verify 48 kHz late-TX f32le sizing and 12 kHz f32le encode -> decode_slot roundtrip
    write_frame(
        &mut c_in,
        0,
        br#"{"op":"encode_tx","id":"tx48","mode":"ft8","message":"CQ N0AAA EM10","af_hz":1500.0,"sample_rate":48000,"sample_format":"f32le","start_delay_ms":0,"offset_ms":2000}"#,
        &[],
    )
    .unwrap();
    let (tx48, tx48_pcm) = read_frame(&mut c_out).unwrap();
    assert_eq!(tx48["op"], "encode_result");
    assert_eq!(tx48["sample_rate"], 48_000);
    assert_eq!(tx48_pcm.len(), (606_720 - 96_000) * 4);

    write_frame(
        &mut c_in,
        0,
        br#"{"op":"encode_tx","id":"tx_rt","mode":"ft8","message":"K9BBB N0AAA -12","af_hz":1420.0,"sample_rate":12000,"sample_format":"f32le","pad_to_slot":true}"#,
        &[],
    )
    .unwrap();
    let (tx_rt, tx_pcm) = read_frame(&mut c_out).unwrap();
    assert_eq!(tx_rt["op"], "encode_result");
    assert_eq!(tx_rt["sample_count"], 180_000);

    write_frame(
        &mut c_in,
        FLAG_BINARY_F32LE,
        br#"{"op":"decode_slot","id":"rx_rt","mode":"ft8","depth":2,"sample_rate":12000,"sample_format":"f32le","enable_telemetry":true}"#,
        &tx_pcm,
    )
    .unwrap();
    let mut decoded_rt = false;
    loop {
        let (v, _) = read_frame(&mut c_out).unwrap();
        match v["op"].as_str().unwrap() {
            "decode_event" => {
                if v["decode"]["message"] == "K9BBB N0AAA -12" {
                    assert!(v["decode"]["telemetry"].is_object());
                    decoded_rt = true;
                }
            }
            "decode_done" => break,
            other => panic!("unexpected op {other}"),
        }
    }
    assert!(decoded_rt);

    // 6. Clean shutdown & EOF exit 0
    write_frame(&mut c_in, 0, br#"{"op":"shutdown","id":"bye"}"#, &[]).unwrap();
    let (bye, _) = read_frame(&mut c_out).unwrap();
    assert_eq!(bye["op"], "ok");
    drop(c_in);
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn worker_ipc_exits_3_on_frame_desync_and_0_on_clean_eof() {
    let bin = env!("CARGO_BIN_EXE_ftxr-modem");

    // Clean immediate EOF exits 0
    let mut eof_child = Command::new(bin)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(eof_child.stdin.take());
    assert_eq!(eof_child.wait().unwrap().code(), Some(0));

    // Invalid magic frame desync exits 3
    let mut desync_child = Command::new(bin)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut sin = desync_child.stdin.take().unwrap();
        sin.write_all(b"BAD!000000000000").unwrap();
        sin.flush().unwrap();
    }
    assert_eq!(desync_child.wait().unwrap().code(), Some(3));
}
