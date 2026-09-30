// The newest live frame the keyboard got, for Home's mirror. Kept out of the app store:
// it changes about 25 times a second and only the mirror draws it. While something
// subscribes and the page is visible, the backend is asked every second to keep sending
// (it stops by itself a few seconds after the window stops asking).
import { useSyncExternalStore } from "react";
import { api } from "./api";
import type { LiveFrameEvent } from "./types";

let latest: LiveFrameEvent | null = null;
const listeners = new Set<() => void>();
let stop: (() => void) | undefined;

/** Keep `f` unless it's older than what's here (an older request, or an earlier frame). */
export function setLatestFrame(f: LiveFrameEvent | null) {
  if (!f || (latest && (f.request < latest.request || (f.request === latest.request && f.seq <= latest.seq)))) return;
  latest = f;
  listeners.forEach((l) => l());
}

function start(): () => void {
  let dead = false;
  let off: (() => void) | undefined;
  const renew = () => {
    if (document.visibilityState !== "hidden") api().then((a) => a.watchLighting()).catch(() => {});
  };
  api().then((a) => {
    if (dead) return;
    off = a.onLightingFrame(setLatestFrame);
    renew();
  });
  const timer = setInterval(renew, 1000);
  document.addEventListener("visibilitychange", renew);
  return () => {
    dead = true;
    off?.();
    clearInterval(timer);
    document.removeEventListener("visibilitychange", renew);
  };
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  if (listeners.size === 1) stop = start();
  return () => {
    listeners.delete(cb);
    if (!listeners.size) {
      stop?.();
      stop = undefined;
    }
  };
}

const none = () => () => {};

/** The newest live frame, while `enabled` (otherwise nothing is fetched and this is null). */
export function useLiveFrame(enabled: boolean): LiveFrameEvent | null {
  return useSyncExternalStore(enabled ? subscribe : none, () => (enabled ? latest : null));
}
