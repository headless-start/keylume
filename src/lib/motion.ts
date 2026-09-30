// When previews may run: whether the page is visible, and whether an element is on screen.
// (Lighting previews play whatever the system's reduce-motion setting says: they're what you
// point at or open to see a design. That setting calms the interface itself, in the CSS.)
import { useEffect, useState, useSyncExternalStore, type RefObject } from "react";

const pageVisible = () => typeof document === "undefined" || document.visibilityState !== "hidden";

export function usePageVisible(): boolean {
  return useSyncExternalStore(
    (cb) => {
      document.addEventListener("visibilitychange", cb);
      return () => document.removeEventListener("visibilitychange", cb);
    },
    pageVisible,
  );
}

/** Is the element on screen, in a visible page? (Unknown counts as yes.) */
export function useOnScreen(ref: RefObject<Element | null>): boolean {
  const [onScreen, setOnScreen] = useState(true);
  const visible = usePageVisible();
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => setOnScreen(entries.some((e) => e.isIntersecting)));
    io.observe(el);
    return () => io.disconnect();
  }, [ref]);
  return onScreen && visible;
}
