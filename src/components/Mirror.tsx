import { useEffect, useMemo, useRef, useState } from "react";
import { useLiveFrame } from "../lib/frames";
import { mirrorColors, mirrorMotion } from "../lib/mirror";
import { useOnScreen } from "../lib/motion";
import type { Layout, LightingState } from "../lib/types";
import { Keyboard } from "./Keyboard";

/**
 * Home's keyboard: what the keys show now, as the lighting state has it (see
 * `lib/mirror.ts`). It moves only while on screen.
 */
export function Mirror({ layout, lighting, className = "home-stage" }: { layout: Layout; lighting: LightingState | null; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const onScreen = useOnScreen(ref);
  const motion = mirrorMotion(lighting);
  const frame = useLiveFrame(motion === "frames" && onScreen);
  const [t, setT] = useState(0);
  // the approximate clock of a keyboard animation restarts only when the look changes
  const look = lighting?.shown ? `${lighting.shown.request}|${lighting.shown.origin}|${lighting.shown.name}` : "";
  useEffect(() => {
    setT(0);
    if (motion !== "clock" || !onScreen) return;
    let raf = 0;
    let last = 0;
    const start = performance.now();
    const tick = (now: number) => {
      if (now - last > 40) {
        last = now;
        setT((now - start) / 1000);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [motion, onScreen, look]);
  const colors = useMemo(() => mirrorColors(lighting, layout, t, frame), [lighting, layout, t, frame]);
  return (
    <div ref={ref} className={`${className} mirror`} data-testid="mirror">
      <Keyboard layout={layout} colors={colors} legends={false} glow finish={layout.finish} />
    </div>
  );
}
