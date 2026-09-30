import { useEffect, useState } from "react";
import { BLUE_SWATCHES, isHex, normalizeHex, OTHER_SWATCHES } from "../lib/color";
import type { Hex } from "../lib/types";

export function ColorField({ value, onChange, label, swatches = true }: { value: Hex; onChange: (c: Hex) => void; label?: string; swatches?: boolean }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  return (
    <div className="colorfield">
      {label && <span className="lbl">{label}</span>}
      <div className="row">
        <input type="color" value={value} onChange={(e) => onChange(e.target.value)} aria-label={label ?? "colour"} />
        <input
          className="hex"
          value={text}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          onBlur={() => (isHex(text) ? onChange(normalizeHex(text)) : setText(value))}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
      </div>
      {swatches && (
        <div className="swatches">
          {[...BLUE_SWATCHES, ...OTHER_SWATCHES].map((c) => (
            <button key={c} className={`sw ${c === value ? "on" : ""}`} style={{ background: c }} title={c} onClick={() => onChange(c)} aria-label={`use ${c}`} />
          ))}
        </div>
      )}
    </div>
  );
}

export function Slider({ label, value, min, max, step = 1, onChange, format }: {
  label: string; value: number; min: number; max: number; step?: number; onChange: (v: number) => void; format?: (v: number) => string;
}) {
  return (
    <label className="slider">
      <span className="lbl">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
      <span className="val">{format ? format(value) : value}</span>
    </label>
  );
}

export function Segmented<T extends string | number>({ options, value, onChange, label, className }: {
  options: { value: T; label: string }[]; value: T; onChange: (v: T) => void; label?: string; className?: string;
}) {
  return (
    <div className="segmented-wrap">
      {label && <span className="lbl">{label}</span>}
      <div className={`segmented ${className ?? ""}`} role="radiogroup" aria-label={label}>
        {options.map((o) => (
          <button key={String(o.value)} role="radio" aria-checked={o.value === value} className={o.value === value ? "on" : ""} onClick={() => onChange(o.value)}>
            {o.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function Toggle({ label, checked, onChange, hint }: { label: string; checked: boolean; onChange: (v: boolean) => void; hint?: string }) {
  return (
    <label className="toggle">
      <span>
        <span className="lbl">{label}</span>
        {hint && <span className="hint">{hint}</span>}
      </span>
      <input type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="track" aria-hidden><span className="thumb" /></span>
    </label>
  );
}

export function Busy({ text = "Working…" }: { text?: string }) {
  return <div className="busy"><span className="spinner" aria-hidden /> {text}</div>;
}

export function Empty({ title, children }: { title: string; children?: React.ReactNode }) {
  return <div className="empty"><h3>{title}</h3>{children}</div>;
}

/** Name + save, at the bottom of every builder's panel. */
export function SaveRow({ name, setName, label, onSave, placeholder = "Name (optional)" }: {
  name: string; setName: (n: string) => void; label: string; onSave: () => void; placeholder?: string;
}) {
  return (
    <div className="save-row">
      <input placeholder={placeholder} value={name} onChange={(e) => setName(e.target.value)} aria-label="Profile name" />
      <button className="btn primary" onClick={onSave}>{label}</button>
    </div>
  );
}

/** A settings row: what it is on the left (with an optional short hint), the control on the right. */
export function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="row-setting">
      <span className="row-text"><span className="lbl">{label}</span>{hint && <span className="hint">{hint}</span>}</span>
      <span className="row-control">{children}</span>
    </div>
  );
}

/** The top of every page past Home: its title, then what switches its view (tabs), and its
 * own actions on the right. */
export function PageHead({ title, children, actions }: { title: string; children?: React.ReactNode; actions?: React.ReactNode }) {
  return (
    <div className="page-head">
      <h1>{title}</h1>
      {children}
      {actions && <div className="page-actions">{actions}</div>}
    </div>
  );
}

/** A titled group of settings rows. */
export function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="group" aria-label={title}>
      <h3>{title}</h3>
      <div className="group-card">{children}</div>
    </section>
  );
}
