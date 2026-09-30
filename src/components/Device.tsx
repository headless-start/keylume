import { api } from "../lib/api";
import { attempt } from "../lib/store";
import type { Status } from "../lib/types";
import { Icon } from "./Icon";

/** "Firmware 3.04" from the version word the keyboard reports (0x0304). */
export const firmwareLabel = (fw: number | null | undefined) => (fw == null ? "" : `Firmware ${fw >> 8}.${(fw & 0xff).toString(16).padStart(2, "0")}`);

/** Connection state in a few words, for the top bar and Home. */
export function deviceState(status: Status | null): { tone: "ok" | "warn" | "off"; text: string } {
  if (status?.paused) return { tone: "warn", text: "Paused" };
  if (!status?.connected) return { tone: "off", text: "Not connected" };
  if (status.device?.simulated) return { tone: "ok", text: "Simulated" };
  return { tone: "ok", text: "Connected" };
}

export function LightsButton() {
  return (
    <button className="btn icon-btn" onClick={() => attempt("Lights", async () => (await api()).toggleLights())} title="Lights on/off (Ctrl+Alt+L)" aria-label="Lights on/off">
      <Icon name="power" size={17} />
    </button>
  );
}
