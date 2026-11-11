//! Vocal-fold paralysis: one or both folds lose their motor drive (`paralysisLeft/Right`).
mod common;
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::anatomy::human::BODY_MALE;
use chimera::control::score::Gesture;
use chimera::engine::Engine;
use chimera::voice::human_voice::{HumanVoice, SourcePatch};
use common::{load, num};

struct Take {
    rms: f64,
    f0: f64,
    aperiodicity: f64,
    min_area: f64,
}

/// A sustained /a/-ish neutral vowel from the engine (male body), measured over its last half.
fn sustain(f0: f64, patch: SourcePatch) -> Take {
    let fs = 48000.0;
    let mut e = Engine::new(fs);
    e.set_source(&SourcePatch { f0: Some(f0), ..patch });
    e.set_gate(1.0);
    let mut out = vec![0.0f32; 48000];
    let mut min_area = f64::INFINITY;
    for (k, c) in out.chunks_mut(128).enumerate() {
        e.render(c);
        if k * 128 > 24000 {
            min_area = min_area.min(e.voice().glottis.area);
        }
    }
    let x: Vec<f64> = out[24000..].iter().map(|s| *s as f64).collect();
    let rms = (x.iter().map(|s| s * s).sum::<f64>() / x.len() as f64).sqrt();
    let (mut f, mut ap, mut n) = (0.0, 0.0, 0.0);
    for w in x.chunks(2048).filter(|w| w.len() == 2048) {
        let p = yin_pitch(w, fs, YinOpts { min_f0: Some(40.0), max_f0: Some(300.0), threshold: Some(0.5) });
        f += p.f0;
        ap += p.aperiodicity;
        n += 1.0;
    }
    Take { rms, f0: f / n, aperiodicity: ap / n, min_area }
}

fn left(paralysis: f64, position: f64) -> SourcePatch {
    SourcePatch { paralysis_left: Some(paralysis), fold_position_left: Some(position), ..Default::default() }
}

fn both(paralysis: f64, position: f64) -> SourcePatch {
    SourcePatch { paralysis_right: Some(paralysis), fold_position_right: Some(position), ..left(paralysis, position) }
}

#[test]
fn healthy_folds_are_untouched_by_the_paralysis_controls() {
    // the folds move as mirror images until one is paralyzed, sample for sample; a rest position
    // means nothing to a fold that isn't paralyzed
    let a = sustain(110.0, SourcePatch::default());
    let b = sustain(110.0, SourcePatch { fold_position_left: Some(1.0), fold_detune: Some(-7.0), ..left(0.0, 1.0) });
    assert_eq!((a.rms, a.f0, a.min_area), (b.rms, b.f0, b.min_area));
}

#[test]
fn one_paralyzed_fold_leaves_a_gap_and_a_weaker_voice() {
    let healthy = sustain(110.0, SourcePatch::default());
    let para = sustain(110.0, left(1.0, 0.2));
    let paresis = sustain(110.0, left(0.5, 0.2));
    println!("healthy {:.4} paresis {:.4} paralysis {:.4} (rms)", healthy.rms, paresis.rms, para.rms);
    // the glottis never closes: the paralyzed fold rests off the midline
    assert!(para.min_area > 2.0 * healthy.min_area, "{} {}", para.min_area, healthy.min_area);
    // quieter, the more paralyzed the quieter, still voiced at the commanded pitch
    assert!(para.rms < paresis.rms && paresis.rms < 0.85 * healthy.rms);
    assert!((para.f0 - 110.0).abs() < 2.0, "{}", para.f0);
}

#[test]
fn a_paralyzed_fold_off_its_slack_pitch_beats_against_the_healthy_one() {
    // diplophonia: the paralyzed fold can't be tensed to the healthy fold's pitch, and when the
    // two are too far apart to lock, their pulse trains beat
    let locked = sustain(110.0, left(1.0, 0.2));
    let slipping = sustain(110.0, SourcePatch { fold_detune: Some(-5.0), ..left(1.0, 0.2) });
    println!("aperiodicity: locked {:.3}, slipping {:.3}", locked.aperiodicity, slipping.aperiodicity);
    assert!(locked.aperiodicity < 0.05 && slipping.aperiodicity > 0.15);
}

#[test]
fn with_both_folds_paralyzed_the_pitch_stays_put() {
    // neither fold can be tensed, so a higher commanded note leaves the voice at the slack pitch
    // (the male body's 110 Hz), and the voice is weak
    let low = sustain(110.0, both(1.0, 0.2));
    let high = sustain(180.0, both(1.0, 0.2));
    println!("bilateral paramedian: {:.1} Hz, asked for 180: {:.1} Hz", low.f0, high.f0);
    assert!((high.f0 - 110.0).abs() < 4.0 && (low.f0 - 110.0).abs() < 4.0);
    assert!(low.rms < 0.5 * sustain(110.0, SourcePatch::default()).rms);
}

#[test]
fn both_folds_paralyzed_wide_open_leave_only_breath() {
    let healthy = sustain(110.0, SourcePatch::default());
    let open = sustain(110.0, both(1.0, 1.0));
    assert_eq!(open.f0, 0.0, "still voiced");
    assert!(open.rms < 0.15 * healthy.rms, "{} {}", open.rms, healthy.rms);
}

fn physical(p: SourcePatch) -> SourcePatch {
    SourcePatch { physical_folds: Some(true), ..p }
}

#[test]
fn physical_folds_hold_the_commanded_pitch() {
    // two-mass folds self-oscillate at whatever their tension gives; the period loop tunes it
    for f0 in [90.0, 110.0, 150.0, 220.0] {
        let t = sustain(f0, physical(SourcePatch::default()));
        println!("two-mass at {f0} Hz: {:.1} Hz, rms {:.4}, aperiodicity {:.3}", t.f0, t.rms, t.aperiodicity);
        assert!((t.f0 / f0 - 1.0).abs() < 0.01, "asked {f0}, got {}", t.f0);
        assert!(t.aperiodicity < 0.05);
    }
}

#[test]
fn physical_folds_paralysis_emerges_from_the_mechanics() {
    // one slack fold drags the voice's pitch down and the glottis no longer closes
    let healthy = sustain(160.0, physical(SourcePatch::default()));
    let one = sustain(160.0, physical(left(1.0, 0.2)));
    println!("at 160 Hz: healthy {:.1} Hz, left paralyzed {:.1} Hz (min area {:.4} vs {:.4})", healthy.f0, one.f0, one.min_area, healthy.min_area);
    assert!(one.f0 < 0.85 * healthy.f0 && one.min_area > 2.0 * healthy.min_area);
    // both slack: the pitch stays near the body's speaking pitch (110 Hz) whatever is asked
    let both_high = sustain(180.0, physical(both(1.0, 0.2)));
    assert!((both_high.f0 - 115.0).abs() < 10.0, "{}", both_high.f0);
    // a very slack fold: the folds fall into period doubling (Steinecke & Herzel's subharmonics)
    let slack = sustain(110.0, physical(SourcePatch { fold_detune: Some(-12.0), ..left(1.0, 0.2) }));
    println!("one fold an octave slack: {:.1} Hz, aperiodicity {:.3}", slack.f0, slack.aperiodicity);
    assert!(slack.f0 < 70.0 && slack.aperiodicity > 0.15);
    // both wide open: breath only
    let open = sustain(110.0, physical(both(1.0, 1.0)));
    assert_eq!(open.f0, 0.0);
}

#[test]
fn physical_folds_hold_at_every_device_rate() {
    let rms = |fs: f64| {
        let mut e = Engine::new(fs);
        e.set_source(&physical(SourcePatch::default()));
        e.set_gate(1.0);
        let mut out = vec![0.0f32; 2 * fs as usize];
        for c in out.chunks_mut(128) {
            e.render(c);
        }
        let tail = &out[fs as usize..];
        assert!(tail.iter().all(|x| x.is_finite()), "{fs} Hz went NaN");
        (tail.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    let reference = rms(48000.0);
    for fs in [8000.0, 16000.0, 22050.0, 44100.0, 96000.0, 192000.0] {
        let r = rms(fs);
        println!("{fs} Hz: rms {r:.4} (48 kHz {reference:.4})");
        assert!(r > 0.5 * reference && r < 2.0 * reference, "{fs} Hz: {r} vs {reference}");
    }
}

/// The golden speech scenario rendered with the given source settings: 10 ms RMS envelope.
fn speech_envelope(patch: SourcePatch) -> Vec<f64> {
    let scenarios = load("voice");
    let s = scenarios.as_array().unwrap().iter().find(|s| s["name"] == "speech-male").unwrap();
    let mut v = HumanVoice::new(48000.0, BODY_MALE, 2);
    v.set_source(&patch);
    let n = s["samples"].as_u64().unwrap() as usize;
    let mut out = vec![0.0f32; n];
    let mut i = 0;
    let mut ops: Vec<_> = s["ops"].as_array().unwrap().iter().collect();
    ops.sort_by_key(|o| o["at"].as_u64().unwrap());
    for o in ops {
        let at = o["at"].as_u64().unwrap() as usize;
        v.render(&mut out[i..at]);
        i = at;
        match o["op"].as_str().unwrap() {
            "gate" => v.set_gate(num(&o["level"])),
            "schedule" => {
                let mut gs: Vec<Gesture> = serde_json::from_value(o["gestures"].clone()).unwrap();
                let off = v.clock + num(&o["offset"]);
                for g in gs.iter_mut() {
                    g.t0 += off;
                    g.t1 += off;
                }
                v.schedule(&gs);
            }
            op => panic!("unexpected op {op}"),
        }
    }
    v.render(&mut out[i..]);
    assert!(out.iter().all(|x| x.is_finite()));
    out.chunks(480).map(|c| (c.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt()).collect()
}

#[test]
fn physical_folds_speak_with_the_same_voicing() {
    // voiceless consonants (abduction) and glottal stops (pressing) start and stop the two-mass
    // folds through the same controls as the parametric pulse: the loudness envelope follows it
    let a = speech_envelope(SourcePatch::default());
    let b = speech_envelope(physical(SourcePatch::default()));
    let (ma, mb) = (a.iter().sum::<f64>() / a.len() as f64, b.iter().sum::<f64>() / b.len() as f64);
    let cov: f64 = a.iter().zip(&b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
    let r = cov / (va * vb).sqrt();
    println!("speech envelope correlation, parametric vs two-mass: {r:.3}; mean rms {ma:.4} vs {mb:.4}");
    assert!(r > 0.8, "{r}");
}
