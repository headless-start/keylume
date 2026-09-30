// Live effects preview from frames the real animator renders (`previewLive`). Fetched
// once per effect and kept for reuse, but only for the most recent few dozen effects,
// and a failed fetch is forgotten so the next hover tries again.
import { api } from "./api";
import type { LiveEffect, LiveFrame } from "./types";

export const PREVIEW_SECONDS = 8;
const KEEP = 64;
const cache = new Map<string, Promise<LiveFrame[]>>();

export function previewFrames(live: LiveEffect): Promise<LiveFrame[]> {
  const key = JSON.stringify(live);
  const hit = cache.get(key);
  if (hit) {
    cache.delete(key); // most recently used goes to the back
    cache.set(key, hit);
    return hit;
  }
  const frames = api()
    .then((a) => a.previewLive(live, PREVIEW_SECONDS))
    .catch(() => {
      cache.delete(key);
      return [] as LiveFrame[];
    });
  cache.set(key, frames);
  while (cache.size > KEEP) cache.delete(cache.keys().next().value as string);
  return frames;
}

/** For tests: how many previews are kept. */
export const cachedPreviews = () => cache.size;
