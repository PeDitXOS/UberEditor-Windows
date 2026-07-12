import { useState } from "react";

/**
 * Range + EDITABLE readout: click the number and type an exact value.
 * ↑/↓ nudge by one step (×10 with Shift), Enter commits, Esc reverts.
 * Typed values are NOT clamped to the slider's range — the engine's own
 * limits are the real ones, and a slider maximum shouldn't cap you.
 *
 * `onCommit` (optional) fires on drag release, typed commits and arrow
 * nudges — for actions that must not dispatch on every drag tick (speed).
 * With `onCommit`, `onChange` is the live draft.
 */
export function Slider({
  value,
  min,
  max,
  step,
  unit,
  disabled,
  format,
  onChange,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  disabled?: boolean;
  format?: (v: number) => string;
  onChange: (v: number) => void;
  onCommit?: (v: number) => void;
}) {
  const decimals = step < 1 ? 2 : 0;
  const shown = format ? format(value) : `${value.toFixed(decimals)}${unit ?? ""}`;
  const [draft, setDraft] = useState<string | null>(null);
  const commit = (raw: string) => {
    const n = Number(raw.replace(/[^\d.eE+-]/g, ""));
    if (Number.isFinite(n)) {
      onChange(n);
      onCommit?.(n);
    }
    setDraft(null);
  };
  return (
    <>
      <input
        type="range"
        className="h-1 min-w-0 flex-1 cursor-pointer appearance-none rounded-full bg-bg3 accent-(--color-accent) disabled:opacity-40"
        min={min}
        max={max}
        step={step}
        value={Math.min(max, Math.max(min, value))}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        onPointerUp={() => onCommit?.(value)}
        onKeyUp={(e) => {
          // keyboard drags (arrow keys on the range) also need a commit
          if (e.key.startsWith("Arrow")) onCommit?.(value);
        }}
      />
      <input
        className="focus-ring w-14 shrink-0 rounded bg-transparent text-right font-[var(--font-mono)] text-[11px] text-ink hover:bg-bg3 focus:bg-bg3 disabled:opacity-40"
        value={draft ?? shown}
        disabled={disabled}
        title="Type an exact value · ↑/↓ to nudge (Shift ×10)"
        onFocus={(e) => {
          setDraft(String(Number(value.toFixed(decimals))));
          requestAnimationFrame(() => e.target.select());
        }}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            commit((e.target as HTMLInputElement).value);
            (e.target as HTMLInputElement).blur();
          } else if (e.key === "Escape") {
            setDraft(null);
            (e.target as HTMLInputElement).blur();
          } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            const d = (e.key === "ArrowUp" ? 1 : -1) * step * (e.shiftKey ? 10 : 1);
            const next = Number((value + d).toFixed(6));
            onChange(next);
            onCommit?.(next);
            setDraft(String(Number(next.toFixed(decimals))));
          }
        }}
      />
    </>
  );
}
