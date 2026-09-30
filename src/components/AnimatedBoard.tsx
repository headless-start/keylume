import { useEffect, useRef, useState } from "react";
import { player } from "../lib/animate";
import { stillColors } from "../lib/library";
import { previewFrames } from "../lib/liveCache";
import { useOnScreen } from "../lib/motion";
import type { Hex, Layout, LiveFrame, Profile } from "../lib/types";
import { Keyboard } from "./Keyboard";

/** What plays before anything is on the keys: a slow rainbow wave. */
export const SHOWCASE: Profile = {
  id: "showcase", name: "", category: "", tags: [], description: "", source: "builtin",
  lighting: { kind: "effect", effect: { mode: "wave", speed: 1, brightness: 4, direction: 0, rainbow: true, color: "#ffffff" } },
};

/**
 * A keyboard playing a (possibly unsaved) profile on screen, so you see what it does
 * before it goes to the keyboard. Live effects are rendered by the real animator
 * (re-fetched shortly after their settings stop changing). It pauses while off screen or
 * hidden.
 */
export function AnimatedBoard({ profile, layout, legends = false, spill = false, className = "stage" }: {
  profile: Profile | null; layout: Layout; legends?: boolean; spill?: boolean; className?: string;
}) {
  const [colors, setColors] = useState<Record<string, Hex>>(() => (profile ? stillColors(profile, layout) : {}));
  const [frames, setFrames] = useState<LiveFrame[] | undefined>();
  const ref = useRef<HTMLDivElement>(null);
  const onScreen = useOnScreen(ref);
  const live = profile?.lighting.kind === "live" ? profile.lighting.live : null;
  const liveKey = live ? JSON.stringify(live) : "";
  // restart only when what plays changes, not when an equal profile object comes in
  const key = profile ? JSON.stringify(profile.lighting) : "";

  useEffect(() => {
    setFrames(undefined);
    if (!live) return;
    let dead = false;
    const t = setTimeout(() => previewFrames(live).then((f) => !dead && setFrames(f)), 250);
    return () => { dead = true; clearTimeout(t); };
  }, [liveKey]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!profile) { setColors({}); return; }
    const play = player(profile, layout, frames);
    if (!play || !onScreen) { setColors(stillColors(profile, layout)); return; }
    let raf = 0, last = 0;
    const start = performance.now();
    const tick = (now: number) => {
      if (now - last > 40) { last = now; setColors(play((now - start) / 1000)); }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [key, layout, frames, onScreen]); // eslint-disable-line react-hooks/exhaustive-deps

  return <div ref={ref} className={className}><Keyboard layout={layout} colors={colors} legends={legends} glow spill={spill} /></div>;
}
