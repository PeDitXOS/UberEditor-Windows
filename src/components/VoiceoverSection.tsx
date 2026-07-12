import { useEffect, useMemo, useState } from "react";

import type { TtsCatalog, TtsEngineInfo, TtsVoice } from "../engine/types";
import { engine, useStore } from "../state/store";
import { Slider } from "./Slider";

/** One catalog fetch per app run (`say -v ?` is a subprocess). */
let catalogPromise: Promise<TtsCatalog> | null = null;
function fetchCatalog(): Promise<TtsCatalog> {
  catalogPromise ??= engine.listTtsVoices();
  return catalogPromise;
}

/** The form survives remounts (selecting a clip and coming back must not
 *  lose a half-written script). */
const persisted = {
  text: "",
  engineId: "",
  voiceId: "",
  rate: null as number | null,
  insert: true,
};

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center gap-2 py-1">
      <span className="w-20 shrink-0 text-[11px] text-ink-dim" title={hint}>
        {label}
      </span>
      {children}
    </label>
  );
}

const selectCls =
  "focus-ring min-w-0 flex-1 cursor-pointer rounded-md border border-line bg-bg1 px-2 py-1 text-[12px] text-ink";

/** Voices grouped by language, Spanish first (the app's home turf). */
function voiceGroups(eng: TtsEngineInfo | undefined): [string, TtsVoice[]][] {
  if (!eng) return [];
  const groups = new Map<string, TtsVoice[]>();
  for (const v of eng.voices) {
    if (!groups.has(v.lang)) groups.set(v.lang, []);
    groups.get(v.lang)!.push(v);
  }
  return [...groups.entries()].sort(([a], [b]) => {
    const es = (l: string) => (l.toLowerCase().startsWith("es") ? 0 : 1);
    return es(a) - es(b) || a.localeCompare(b);
  });
}

/**
 * Voiceover from text, inline in the Inspector's no-selection state: write
 * (or AI-write) a script, pick an engine and a voice, and the audio lands in
 * Media (optionally as a clip at the playhead). Engines are modular:
 * built-ins (macOS `say`, the toolkit's Kokoro) plus any JSON manifest in
 * the tts_engines folder.
 */
export function VoiceoverSection() {
  const generateSpeech = useStore((s) => s.generateSpeech);
  const ttsProgress = useStore((s) => s.ttsProgress);

  const [catalog, setCatalog] = useState<TtsCatalog | null>(null);
  const [text, setText] = useState(persisted.text);
  const [engineId, setEngineId] = useState(persisted.engineId);
  const [voiceId, setVoiceId] = useState(persisted.voiceId);
  const [rate, setRate] = useState<number | null>(persisted.rate);
  const [insert, setInsert] = useState(persisted.insert);

  useEffect(() => {
    let alive = true;
    void fetchCatalog()
      .then((c) => {
        if (!alive) return;
        setCatalog(c);
        if (!persisted.engineId) {
          const usable = c.engines.find((e) => e.available) ?? c.engines[0];
          if (usable) selectEngine(usable);
        }
      })
      .catch((e) => engine.uiLog("error", `tts catalog: ${e}`));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // keep the module copy in sync so a remount restores the form
  useEffect(() => {
    Object.assign(persisted, { text, engineId, voiceId, rate, insert });
  }, [text, engineId, voiceId, rate, insert]);

  const current = useMemo(
    () => catalog?.engines.find((e) => e.id === engineId),
    [catalog, engineId],
  );

  /** Engine change resets voice (Spanish first) and rate to its defaults. */
  const selectEngine = (eng: TtsEngineInfo) => {
    setEngineId(eng.id);
    const groups = voiceGroups(eng);
    setVoiceId(groups[0]?.[1][0]?.id ?? "");
    setRate(eng.rate ? eng.rate.default : null);
  };

  const busy =
    (ttsProgress && ttsProgress.stage !== "done" && ttsProgress.stage !== "error") || false;
  const canGenerate = text.trim().length > 0 && !!current?.available && !busy;

  return (
    <div className="mt-3 rounded-lg border border-line bg-bg2 p-3">
      <h3
        className="panel-eyebrow mb-2"
        title={
          catalog?.engines_dir
            ? `Engines are modular: drop a JSON manifest in ${catalog.engines_dir} to add your own`
            : undefined
        }
      >
        AI · Voiceover
      </h3>
      <textarea
        className="focus-ring h-24 w-full resize-y rounded-md border border-line bg-bg1 px-2 py-1.5 text-[12px] leading-relaxed text-ink placeholder:text-ink-faint"
        value={text}
        placeholder="What should the voice say?"
        onChange={(e) => setText(e.target.value)}
      />
      <Row label="Engine">
        <select
          className={selectCls}
          value={engineId}
          title={current?.detail}
          onChange={(e) => {
            const eng = catalog?.engines.find((x) => x.id === e.target.value);
            if (eng) selectEngine(eng);
          }}
        >
          {(catalog?.engines ?? []).map((e) => (
            <option key={e.id} value={e.id} disabled={!e.available}>
              {e.name}
              {e.available ? "" : " — unavailable"}
            </option>
          ))}
        </select>
      </Row>
      {(catalog?.engines ?? [])
        .filter((e) => !e.available)
        .map((e) => (
          <p key={e.id} className="text-[10px] leading-snug text-ink-faint">
            {e.name} unavailable: {e.detail}
          </p>
        ))}
      <Row label="Voice">
        <select className={selectCls} value={voiceId} onChange={(e) => setVoiceId(e.target.value)}>
          {voiceGroups(current).map(([lang, voices]) => (
            <optgroup key={lang} label={lang}>
              {voices.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </optgroup>
          ))}
          {current && current.voices.length === 0 && <option value="">(default voice)</option>}
        </select>
      </Row>
      {current?.rate && rate !== null && (
        <Row label={current.rate.label}>
          <Slider
            value={rate}
            min={current.rate.min}
            max={current.rate.max}
            step={current.rate.step}
            onChange={(v) => setRate(v)}
          />
        </Row>
      )}
      <label className="flex cursor-pointer items-center gap-2 py-1 text-[11px] text-ink-dim">
        <input
          type="checkbox"
          className="accent-(--color-accent)"
          checked={insert}
          onChange={(e) => setInsert(e.target.checked)}
        />
        Insert the clip at the playhead
      </label>

      {ttsProgress && (
        <div className="mt-1.5 rounded-md border border-line bg-bg1 p-2">
          <div className="flex items-center gap-2 text-[11px] text-ink">
            <span className="flex-1">{ttsProgress.message}</span>
            <span className="font-[var(--font-mono)] text-ink-faint">
              {Math.round(ttsProgress.progress * 100)}%
            </span>
          </div>
          <div className="mt-1 h-1 overflow-hidden rounded-full bg-bg3">
            <div
              className={`h-full rounded-full ${
                ttsProgress.stage === "error" ? "bg-danger" : "bg-accent"
              }`}
              style={{ width: `${Math.round(ttsProgress.progress * 100)}%` }}
            />
          </div>
        </div>
      )}

      <button
        className="focus-ring mt-2 w-full rounded-md bg-(--color-accent) px-3 py-1.5 text-[12px] font-semibold text-black hover:brightness-110 disabled:opacity-40"
        disabled={!canGenerate}
        onClick={() => void generateSpeech(text, engineId, voiceId || null, rate, insert)}
        title={
          !text.trim()
            ? "Write the script first"
            : !current?.available
              ? "The selected engine is unavailable"
              : "Synthesize in the background; the audio lands in Media"
        }
      >
        {busy ? "Generating…" : "🎙 Generate voiceover"}
      </button>
    </div>
  );
}
