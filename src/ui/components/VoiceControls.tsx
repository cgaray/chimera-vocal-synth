import { useStore } from "../store.ts";
import { EncoderPages, type EncoderPage } from "./Encoder.tsx";
import { Chip } from "./Screen.tsx";
import type { VoiceSource } from "../../wasm/engine.ts";

const NOTE = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"];
export const noteName = (f: number) => {
  const m = Math.round(69 + 12 * Math.log2(f / 440));
  return `${NOTE[((m % 12) + 12) % 12]}${Math.floor(m / 12) - 1}`;
};

const pct = (v: number) => ({ value: String(Math.round(v * 100)), unit: "%" });
/** a paralyzed fold's rest position, named as a laryngoscopist would */
const foldPosition = (v: number) => ({ value: String(Math.round(v * 100)), unit: v < 0.08 ? "median" : v < 0.33 ? "paramedian" : v < 0.7 ? "intermediate" : "lateral" });

type FoldSettings = Pick<VoiceSource, "paralysisLeft" | "paralysisRight" | "foldPositionLeft" | "foldPositionRight">;
/** common vocal-fold paralyses: the folds' paralysis and rest positions (the slack pitch is left alone) */
const FOLD_PRESETS: { name: string; title: string; folds: FoldSettings }[] = [
  { name: "Healthy", title: "Both folds move", folds: { paralysisLeft: 0, paralysisRight: 0, foldPositionLeft: 0.2, foldPositionRight: 0.2 } },
  { name: "Left paralyzed", title: "Left fold paralyzed, paramedian: breathy and weak", folds: { paralysisLeft: 1, paralysisRight: 0, foldPositionLeft: 0.2, foldPositionRight: 0.2 } },
  { name: "Right paralyzed", title: "Right fold paralyzed, paramedian: breathy and weak", folds: { paralysisLeft: 0, paralysisRight: 1, foldPositionLeft: 0.2, foldPositionRight: 0.2 } },
  { name: "Both paralyzed", title: "Both folds paralyzed, paramedian: weak, and stuck at one pitch", folds: { paralysisLeft: 1, paralysisRight: 1, foldPositionLeft: 0.2, foldPositionRight: 0.2 } },
  { name: "Both open", title: "Both folds paralyzed, lateral: no voice, only breath", folds: { paralysisLeft: 1, paralysisRight: 1, foldPositionLeft: 1, foldPositionRight: 1 } },
];
const FOLD_KEYS = ["paralysisLeft", "paralysisRight", "foldPositionLeft", "foldPositionRight"] as const;

/**
 * The vocal-fold paralysis presets, for the vocal folds' screen header: one chip that steps
 * through them (it names the one playing, or "Custom" after the Folds page's dials moved).
 */
export function FoldPresetChip() {
  const s = useStore((st) => st.source);
  const setSource = useStore((st) => st.setSource);
  // a fold that isn't paralyzed has no rest position to match
  const matches = (f: FoldSettings) => FOLD_KEYS.every((k) => Math.abs(s[k] - f[k]) < 1e-3 || (k === "foldPositionLeft" && f.paralysisLeft === 0 && s.paralysisLeft === 0) || (k === "foldPositionRight" && f.paralysisRight === 0 && s.paralysisRight === 0));
  const i = FOLD_PRESETS.findIndex((p) => matches(p.folds));
  const next = FOLD_PRESETS[(i + 1) % FOLD_PRESETS.length];
  return (
    <Chip toggle={false} on={i !== 0} tone={i !== 0 ? "amber" : undefined} title={`${i >= 0 ? FOLD_PRESETS[i].title : "Custom (Voice → Folds)"}. Next: ${next.name}`} onClick={() => setSource(next.folds)}>
      {i >= 0 ? FOLD_PRESETS[i].name : "Custom"}
    </Chip>
  );
}
const cm = (v: number) => ({ value: v.toFixed(1), unit: "cm" });
const signedCm = (v: number) => ({ value: `${v >= 0 ? "+" : ""}${v.toFixed(1)}`, unit: "cm" });

/**
 * Play mode's controls: the voice and the articulators without a handle, four at a time
 * (OP-1 style). Values live in the store as before.
 */
export function VoiceControls() {
  const s = useStore((st) => st.source);
  const setSource = useStore((st) => st.setSource);
  const a = useStore((st) => st.artic);
  const setArtic = useStore((st) => st.setArtic);
  const pages: EncoderPage[] = [
    {
      name: "Voice",
      encoders: [
        { label: "Pitch", value: s.f0, min: 40, max: 1100, log: true, onChange: (v) => setSource({ f0: v }), format: (v) => ({ value: v.toFixed(0), unit: `Hz ${noteName(v)}` }) },
        { label: "Breath", value: s.pressure, min: 2, max: 22, onChange: (v) => setSource({ pressure: v }), format: (v) => ({ value: v.toFixed(1), unit: "cmH₂O" }) },
        { label: "Pressed", value: s.tension, min: 0, max: 1, onChange: (v) => setSource({ tension: v }), format: pct },
        { label: "Vibrato", value: s.vibratoDepth, min: 0, max: 120, onChange: (v) => setSource({ vibratoDepth: v }), format: (v) => ({ value: v.toFixed(0), unit: "cents" }) },
      ],
    },
    {
      name: "Tone",
      encoders: [
        { label: "Bright", value: s.brightness, min: 0, max: 1, onChange: (v) => setSource({ brightness: v }), format: pct },
        { label: "Breathy", value: s.breathiness, min: 0, max: 1, onChange: (v) => setSource({ breathiness: v }), format: pct },
        { label: "Rough", value: s.roughness, min: 0, max: 1, onChange: (v) => setSource({ roughness: v }), format: pct },
        { label: "Fry", value: s.fry, min: 0, max: 1, onChange: (v) => setSource({ fry: v }), format: pct },
      ],
    },
    {
      name: "Pitch",
      encoders: [
        { label: "Glide", value: s.glide, min: 0.005, max: 0.6, log: true, onChange: (v) => setSource({ glide: v }), format: (v) => ({ value: (v * 1000).toFixed(0), unit: "ms" }) },
        // where a paralyzed fold's own pitch sits: off the voice's pitch, the two folds beat (diplophonia)
        { label: "Slack fold", value: s.foldDetune, min: -12, max: 12, onChange: (v) => setSource({ foldDetune: v }), format: (v) => ({ value: `${v >= 0.05 ? "+" : ""}${v.toFixed(1)}`, unit: "st" }) },
      ],
    },
    {
      name: "Folds",
      encoders: [
        { label: "L paralysis", value: s.paralysisLeft, min: 0, max: 1, onChange: (v) => setSource({ paralysisLeft: v }), format: pct },
        { label: "L rests", value: s.foldPositionLeft, min: 0, max: 1, onChange: (v) => setSource({ foldPositionLeft: v }), format: foldPosition },
        { label: "R paralysis", value: s.paralysisRight, min: 0, max: 1, onChange: (v) => setSource({ paralysisRight: v }), format: pct },
        { label: "R rests", value: s.foldPositionRight, min: 0, max: 1, onChange: (v) => setSource({ foldPositionRight: v }), format: foldPosition },
      ],
    },
    {
      name: "Mouth",
      encoders: [
        { label: "Rounding", value: 1 - a.lipWidth, min: 0, max: 1, onChange: (v) => setArtic({ lipWidth: 1 - v }), format: pct },
        { label: "Lips open", value: a.lipAp, min: -0.3, max: 2.2, onChange: (v) => setArtic({ lipAp: v }), format: cm },
        { label: "Protrude", value: a.lipProt, min: -0.1, max: 1.4, onChange: (v) => setArtic({ lipProt: v }), format: cm },
        { label: "Jaw", value: a.jaw, min: 0, max: 2.4, onChange: (v) => setArtic({ jaw: v }), format: cm },
      ],
    },
    {
      name: "Tongue",
      encoders: [
        { label: "Root", value: a.root, min: -0.9, max: 0.9, onChange: (v) => setArtic({ root: v }), format: signedCm },
        { label: "Tip curl", value: a.ttCurl, min: 0, max: 1, onChange: (v) => setArtic({ ttCurl: v }), format: pct },
      ],
    },
    {
      name: "Throat",
      encoders: [
        { label: "Soft palate", value: a.velum, min: -0.25, max: 1.2, onChange: (v) => setArtic({ velum: v }), format: (v) => (v <= 0 ? { value: "Shut", unit: "" } : { value: v.toFixed(2), unit: "cm²" }) },
        { label: "Larynx", value: a.larynx, min: -1.5, max: 1.5, onChange: (v) => setArtic({ larynx: v }), format: signedCm },
        { label: "Twang", value: a.epilarynx, min: 0, max: 1, onChange: (v) => setArtic({ epilarynx: v }), format: pct },
        { label: "False folds", value: a.ventricular, min: 0, max: 1, onChange: (v) => setArtic({ ventricular: v }), format: pct },
      ],
    },
  ];
  return <EncoderPages label="Voice" pages={pages} />;
}

/** Hold breath and Whisper, for the voice screen's header. */
export function VoiceToggles() {
  const voiced = useStore((st) => st.source.voiced);
  const setSource = useStore((st) => st.setSource);
  const hold = useStore((st) => st.hold);
  const setHold = useStore((st) => st.setHold);
  return (
    <>
      <Chip on={hold} onClick={() => setHold(!hold)}>
        Hold breath
      </Chip>
      <Chip on={!voiced} onClick={() => setSource({ voiced: !voiced })}>
        Whisper
      </Chip>
    </>
  );
}
