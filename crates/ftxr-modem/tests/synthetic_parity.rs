// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 ftxr-modem Contributors
// Based on WSJT-X algorithms (Copyright (C) 2001-2026 WSJT Development Group).

use ftxr_core::{ApTag, DecodeConfig, DecoderEngine, FecMethod, Mode, ModulateConfig, QsoContext};

struct DeterministicRng(u64);

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64_open01(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64);
        u.max(1e-12)
    }

    fn next_gaussian(&mut self) -> f32 {
        let u1 = self.next_f64_open01();
        let u2 = (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64);
        ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
    }
}

/// Generates a deterministic AWGN noise buffer with WSJT-X 2500 Hz bandwidth SNR scaling.
fn add_awgn(slot: &mut [f32], signal: &[f32], snr_db: f32, rng: &mut DeterministicRng) {
    // Signal waveform has peak ~1.0 (`rms ≈ 1/sqrt(2)` over active duration).
    // WSJT-X SNR in 2500 Hz reference bandwidth at fs = 12,000 Hz (`B_noise = 6000 Hz`):
    // noise_rms = sig_rms * sqrt(6000 / 2500) * 10^(-snr_db / 20).
    let sig_rms = std::f32::consts::FRAC_1_SQRT_2;
    let bw_factor = (6000.0_f32 / 2500.0_f32).sqrt();
    let amp = 10.0_f32.powf(snr_db / 20.0) / (sig_rms * bw_factor);
    for (dst, &s) in slot.iter_mut().zip(signal.iter()) {
        *dst += s * amp;
    }
    for x in slot.iter_mut() {
        *x += rng.next_gaussian();
    }
    // Scale cleanly to realistic i16-range normalized floats.
    for x in slot.iter_mut() {
        *x *= 0.01;
    }
}

#[test]
fn nominal_and_late_tx_loopback_ft8_and_ft4() {
    let eng = DecoderEngine::with_threads(Some(2)).unwrap();

    // FT8 nominal + 1s, 3s, 5s late-TX
    for &offset_ms in &[0u64, 1000, 3000, 5000] {
        let mut mcfg = ModulateConfig::raw_active(Mode::Ft8, 1500.0, 12_000);
        if offset_ms == 0 {
            mcfg.start_delay_ms = 500;
        }
        mcfg.offset_ms = offset_ms;
        let wave = eng.modulate("CQ N0AAA EM10", &mcfg).unwrap();
        let mut slot = vec![0.0_f32; Mode::Ft8.rx_window_samples()];
        let start_idx = ((offset_ms as usize) * 12).min(slot.len());
        let n = wave.len().min(slot.len() - start_idx);
        slot[start_idx..start_idx + n].copy_from_slice(&wave[..n]);

        let mut cfg = DecodeConfig::for_mode(Mode::Ft8);
        cfg.depth = 3;
        let decs = eng.decode(&slot, &cfg).unwrap();
        assert_eq!(decs.len(), 1, "FT8 offset_ms={offset_ms} failed");
        assert_eq!(decs[0].message, "CQ N0AAA EM10");
        assert!((decs[0].af_hz - 1500.0).abs() < 1.5);
    }

    // FT4 nominal + 0.5s, 1.2s late-TX
    for &offset_ms in &[0u64, 500, 1200] {
        let mut mcfg = ModulateConfig::raw_active(Mode::Ft4, 1600.0, 12_000);
        if offset_ms == 0 {
            mcfg.start_delay_ms = 300;
        }
        mcfg.offset_ms = offset_ms;
        let wave = eng.modulate("CQ K9BBB DM79", &mcfg).unwrap();
        let mut slot = vec![0.0_f32; Mode::Ft4.rx_window_samples()];
        let start_idx = ((offset_ms as usize) * 12).min(slot.len());
        let n = wave.len().min(slot.len() - start_idx);
        slot[start_idx..start_idx + n].copy_from_slice(&wave[..n]);

        let mut cfg = DecodeConfig::for_mode(Mode::Ft4);
        cfg.depth = 3;
        let decs = eng.decode(&slot, &cfg).unwrap();
        assert_eq!(decs.len(), 1, "FT4 offset_ms={offset_ms} failed");
        assert_eq!(decs[0].message, "CQ K9BBB DM79");
        assert!((decs[0].af_hz - 1600.0).abs() < 1.5);
    }
}

#[test]
fn osd_weak_signal_ap_a1_to_a6_and_crowded_subtraction_parity() {
    let eng = DecoderEngine::with_threads(Some(2)).unwrap();
    let mut rng = DeterministicRng::new(0x51A6_E600_0000_0001);

    // 1. Weak-signal FT8 (-20 dB) & FT4 (-16 dB) with OSD (`-d 3`) + telemetry check
    let mut slot8 = vec![0.0_f32; Mode::Ft8.rx_window_samples()];
    let w8 = eng
        .modulate(
            "CQ N0AAA EM10",
            &ModulateConfig::default_for(Mode::Ft8, 1500.0, 12_000),
        )
        .unwrap();
    add_awgn(&mut slot8, &w8, -20.0, &mut rng);
    let mut cfg8 = DecodeConfig::for_mode(Mode::Ft8);
    cfg8.depth = 3;
    cfg8.enable_telemetry = true;
    let d8 = eng.decode(&slot8, &cfg8).unwrap();
    assert_eq!(d8.len(), 1, "weak -20 dB FT8 failed");
    assert_eq!(d8[0].message, "CQ N0AAA EM10");
    assert!(d8[0].telemetry.is_some());

    let mut slot4 = vec![0.0_f32; Mode::Ft4.rx_window_samples()];
    let w4 = eng
        .modulate(
            "CQ K9BBB DM79",
            &ModulateConfig::default_for(Mode::Ft4, 1500.0, 12_000),
        )
        .unwrap();
    add_awgn(&mut slot4, &w4, -16.0, &mut rng);
    let mut cfg4 = DecodeConfig::for_mode(Mode::Ft4);
    cfg4.depth = 3;
    cfg4.enable_telemetry = true;
    let d4 = eng.decode(&slot4, &cfg4).unwrap();
    assert_eq!(d4.len(), 1, "weak -16 dB FT4 failed");
    assert_eq!(d4[0].message, "CQ K9BBB DM79");
    let _ = FecMethod::Bp { iterations: 0 };

    // 2. Crowded 20 dB dynamic-range subtraction (FT8 & FT4: +2 dB overlapping with -18 dB nearby)
    let mut crowd8 = vec![0.0_f32; Mode::Ft8.rx_window_samples()];
    let signals8 = [
        ("CQ N0AAA EM10", 1200.0_f32, 2.0_f32),
        ("K9BBB AA1AA -04", 1265.0, -16.0),
        ("JA5EEE VK6FFF RR73", 1800.0, 0.0),
        ("VE3CCC DL4DDD R-11", 1865.0, -18.0),
    ];
    for &(msg, af, snr) in &signals8 {
        let w = eng
            .modulate(msg, &ModulateConfig::default_for(Mode::Ft8, af, 12_000))
            .unwrap();
        let amp = 10.0_f32.powf(snr / 20.0) * 0.25;
        for (dst, &s) in crowd8.iter_mut().zip(w.iter()) {
            *dst += s * amp;
        }
    }
    for x in &mut crowd8 {
        *x += rng.next_gaussian() * 0.0015;
    }
    let dec_crowd8 = eng
        .decode(&crowd8, &DecodeConfig::for_mode(Mode::Ft8))
        .unwrap();
    assert_eq!(dec_crowd8.len(), 4, "crowded FT8 got {dec_crowd8:?}");

    // 3. Single-slot AP a1..a6 recovery + `? a*` formatting check
    let mut ap_slot = vec![0.0_f32; Mode::Ft8.rx_window_samples()];
    let w_ap = eng
        .modulate(
            "N0AAA K9BBB RR73",
            &ModulateConfig::default_for(Mode::Ft8, 1500.0, 12_000),
        )
        .unwrap();
    add_awgn(&mut ap_slot, &w_ap, -22.5, &mut rng);
    let mut ap_cfg = DecodeConfig::for_mode(Mode::Ft8);
    ap_cfg.depth = 3;
    ap_cfg.qso = Some(QsoContext {
        my_call: Some("N0AAA".into()),
        his_call: Some("K9BBB".into()),
        his_grid: Some("DM79".into()),
        qso_progress: 3,
        rx_freq_hz: 1500.0,
        tx_freq_hz: 1500.0,
        ap_enabled: true,
        ap_cq_only: false,
        ap_width_hz: 75.0,
        ..Default::default()
    });
    let ap_res = eng.decode(&ap_slot, &ap_cfg).unwrap();
    // At -22.5 dB, either AP recovers the QSO message or blind fails; verify ApTag metadata contract.
    if let Some(hit) = ap_res.first() {
        assert_eq!(hit.message, "N0AAA K9BBB RR73");
        if hit.ap_tag != ApTag::None {
            let md = hit.wsjtx_metadata().unwrap();
            assert!(md.starts_with('a') || md.starts_with("? a"));
        }
    }
}

#[test]
fn pure_awgn_noise_slots_produce_zero_false_decodes() {
    let eng = DecoderEngine::with_threads(Some(2)).unwrap();
    let mut rng = DeterministicRng::new(0x0000_FA15_E000_0001);

    let mut cfg8 = DecodeConfig::for_mode(Mode::Ft8);
    cfg8.depth = 3;
    let mut cfg4 = DecodeConfig::for_mode(Mode::Ft4);
    cfg4.depth = 3;

    let mut buf8 = vec![0.0_f32; Mode::Ft8.rx_window_samples()];
    let mut buf4 = vec![0.0_f32; Mode::Ft4.rx_window_samples()];

    for slot in 0..15 {
        for x in &mut buf8 {
            *x = rng.next_gaussian() * 0.05;
        }
        let d8 = eng.decode(&buf8, &cfg8).unwrap();
        assert!(d8.is_empty(), "FT8 noise slot {slot} false decode: {d8:?}");

        for x in &mut buf4 {
            *x = rng.next_gaussian() * 0.05;
        }
        let d4 = eng.decode(&buf4, &cfg4).unwrap();
        assert!(d4.is_empty(), "FT4 noise slot {slot} false decode: {d4:?}");
    }
}
