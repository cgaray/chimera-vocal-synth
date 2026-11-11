//! Two-mass vocal folds (Ishizaka & Flanagan 1972), with a left and a right fold of their own
//! (the asymmetric model of Ishizaka & Isshiki 1976 and Steinecke & Herzel 1995). Each fold is a
//! lower and an upper mass on springs, coupled to each other; the transglottal pressure pushes the
//! lower masses apart (Bernoulli: less as the flow speeds through the narrowing), the folds collide
//! at the midline, and the folds self-oscillate. Nothing sets the waveform: open and closed phases,
//! the lag between the folds, period doubling and the beating of two folds at different tensions
//! all come out of the mechanics.
//!
//! A fold's tension factor Q scales its stiffness up and its mass down, so its natural frequency
//! is proportional to Q. A healthy fold follows the commanded pitch (a slow loop corrects Q from the
//! measured period); a paralyzed fold can't be tensed and keeps its slack Q, and rests off the
//! midline. CGS units.

use crate::math::{clamp, js_max, js_min, pow};

/// lower and upper mass (g), stiffness (dyn/cm), coupling stiffness (dyn/cm), thickness (cm),
/// damping ratio open and in contact, at Q = 1 (Ishizaka & Flanagan's values)
const M: [f64; 2] = [0.125, 0.025];
const K: [f64; 2] = [80_000.0, 8_000.0];
const KC: f64 = 25_000.0;
/// √(m·k) per mass: 100 and √200 (g/s)
const SQRT_MK: [f64; 2] = [100.0, 14.142_135_623_730_951];
const D1: f64 = 0.25;
const ZETA_OPEN: [f64; 2] = [0.1, 0.6];
const ZETA_SHUT: [f64; 2] = [1.1, 1.9];
/// fold length (cm, adult male)
const LEN: f64 = 1.4;
/// rest area of the upper section with the folds lightly adducted (cm²); the lower section rests
/// wider (a convergent glottis, as before phonation), or the Bernoulli force would be exactly zero
/// at rest and the folds would never start
const A0: f64 = 0.02;
const A0_LOWER_EXTRA: f64 = 0.015;
/// pitch (Hz) of symmetric folds at Q = 1 and speaking pressure: the open-loop guess for Q;
/// the period loop removes what's left (measured, `two_mass_pitch_follows_q`)
pub const F_AT_Q1: f64 = 136.0;

#[derive(Clone, Debug)]
pub struct TwoMassFolds {
    fs: f64,
    /// displacement from rest (cm) and velocity, [fold][mass], fold 0 left, 1 right
    x: [[f64; 2]; 2],
    v: [[f64; 2]; 2],
    /// the period loop's correction to the commanded Q
    pub q_corr: f64,
    since_open: f64,
    was_open: bool,
    /// last measured period (s), 0 before the first
    pub period: f64,
    /// the two sections' areas (cm²), lower and upper (telemetry, tests)
    pub a: [f64; 2],
    /// each fold's lower-mass displacement (cm), for drawing
    pub fold_x: [f64; 2],
    /// each fold's Q, and what it was computed from (pow is too dear to run every sample)
    q: [f64; 2],
    q_from: [f64; 4],
}

/// What drives the folds this sample.
#[derive(Clone, Copy, Debug)]
pub struct FoldDrive {
    /// commanded pitch (Hz)
    pub f0: f64,
    /// transglottal pressure (dyn/cm²)
    pub dp: f64,
    /// extra static opening of the glottis (cm²): abduction, a paralyzed fold's rest gap
    pub open: [f64; 2],
    /// pressing the folds together 0..1 (glottal stop)
    pub closure: f64,
    /// paralysis per fold 0..1, and a paralyzed fold's slack pitch (Hz)
    pub paralysis: [f64; 2],
    pub rest_f0: f64,
    /// body size (1 adult male): fold length scales with it
    pub scale: f64,
}

impl TwoMassFolds {
    pub fn new(fs: f64) -> Self {
        TwoMassFolds { fs, x: [[0.0; 2]; 2], v: [[0.0; 2]; 2], q_corr: 1.0, since_open: 0.0, was_open: false, period: 0.0, a: [0.0; 2], fold_x: [0.0; 2], q: [1.0; 2], q_from: [f64::NAN; 4] }
    }

    /// seconds since the glottis last opened
    pub fn since_open(&self) -> f64 {
        self.since_open
    }

    /// One sample (semi-implicit Euler, run at the airway's oversampled rate). Returns the glottal
    /// area (cm²): the narrower of the two sections, or 0 when the folds touch.
    pub fn step(&mut self, d: &FoldDrive) -> f64 {
        let len = LEN * d.scale;
        let q_cmd = js_max(0.1, d.f0 / F_AT_Q1) * self.q_corr;
        let q_rest = js_max(0.1, d.rest_f0 / F_AT_Q1);
        let from = [q_cmd, q_rest, d.paralysis[0], d.paralysis[1]];
        if from != self.q_from {
            self.q_from = from;
            for f in 0..2 {
                let pr = d.paralysis[f];
                self.q[f] = if pr > 0.0 { pow(q_cmd, 1.0 - pr) * pow(q_rest, pr) } else { q_cmd };
            }
        }
        let q = self.q;
        // rest areas: adduction leaves a small gap, pressing closes it (negative: pushed together)
        let rest = A0 * (1.0 - 2.0 * d.closure);
        let a1 = rest + A0_LOWER_EXTRA + d.open[0] + d.open[1] + len * (self.x[0][0] + self.x[1][0]);
        let a2 = rest + d.open[0] + d.open[1] + len * (self.x[0][1] + self.x[1][1]);
        self.a = [a1, a2];
        // pressure on the lower masses: the full transglottal pressure while the upper section is
        // shut, less the Bernoulli drop as air speeds through the narrowest section while it is open
        let p1 = if a1 <= 0.0 {
            0.0
        } else {
            let amin = js_min(a1, a2);
            if amin <= 0.0 { d.dp } else { d.dp * (1.0 - (amin / a1) * (amin / a1)) }
        };
        let dt = 1.0 / self.fs;
        for f in 0..2 {
            let qf = q[f];
            let (m1, m2) = (M[0] / qf, M[1] / qf);
            let (k1, k2, kc) = (K[0] * qf, K[1] * qf, KC * qf);
            let shut = [a1 <= 0.0, a2 <= 0.0];
            let mut acc = [0.0; 2];
            for i in 0..2 {
                let (m, k) = if i == 0 { (m1, k1) } else { (m2, k2) };
                let a = if i == 0 { a1 } else { a2 };
                let zeta = if shut[i] { ZETA_SHUT[i] } else { ZETA_OPEN[i] };
                // m·k doesn't depend on Q, so neither does the damping
                let r = 2.0 * zeta * SQRT_MK[i];
                // in contact, the tissue pushes back three times as stiffly; each fold takes half
                let contact = if shut[i] { -3.0 * k * a / (2.0 * len) } else { 0.0 };
                let xi = self.x[f][i];
                let xj = self.x[f][1 - i];
                let drive = if i == 0 { len * D1 * p1 } else { 0.0 };
                acc[i] = (drive - r * self.v[f][i] - k * xi + contact - kc * (xi - xj)) / m;
            }
            for i in 0..2 {
                self.v[f][i] += acc[i] * dt;
                self.x[f][i] += self.v[f][i] * dt;
                // keep a runaway (absurd pressure, a sample rate too slow) from going NaN
                self.x[f][i] = clamp(self.x[f][i], -0.3, 0.6);
                self.v[f][i] = clamp(self.v[f][i], -2000.0, 2000.0);
            }
        }
        self.fold_x = [self.x[0][0], self.x[1][0]];
        let area = js_max(0.0, js_min(a1, a2));

        // the period loop: each opening, nudge Q so the measured pitch meets the commanded one.
        // Slow and bounded, so a paralyzed fold's beating doesn't throw it around.
        self.since_open += dt;
        let open = area > 0.002;
        if open && !self.was_open && self.since_open > 0.4 / js_max(30.0, d.f0) {
            if self.since_open < 0.05 {
                self.period = self.since_open;
                let ratio = clamp(d.f0 * self.period, 0.8, 1.25);
                self.q_corr = clamp(self.q_corr * pow(ratio, 0.25), 0.6, 1.6);
            }
            self.since_open = 0.0;
        }
        self.was_open = open;
        area
    }
}
