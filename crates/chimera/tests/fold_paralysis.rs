//! Vocal-fold paralysis: one or both folds lose their motor drive (`paralysisLeft/Right`).
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::engine::Engine;
use chimera::voice::human_voice::SourcePatch;

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
