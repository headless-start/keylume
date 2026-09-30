import { useEffect } from "react";
import { deviceState, LightsButton } from "../components/Device";
import { Icon } from "../components/Icon";
import { Mirror } from "../components/Mirror";
import { api } from "../lib/api";
import { mirrorNote } from "../lib/mirror";
import { attempt, useApp } from "../lib/store";

/** The front page: the keyboard as it is now, its name, and the way in. Click the keyboard
 * (or Customise) to change what it shows. */
export function HomePage() {
  const { layout, lighting, status, go, setLibrary, refreshLighting } = useApp();
  // a fresh look whenever Home opens (and when the window comes back)
  useEffect(() => {
    refreshLighting();
    const back = () => document.visibilityState === "visible" && refreshLighting();
    document.addEventListener("visibilitychange", back);
    return () => document.removeEventListener("visibilitychange", back);
  }, [refreshLighting]);
  if (!layout) return null;
  const state = deviceState(status);
  const shown = lighting?.shown;
  const reachable = lighting?.phase !== "disconnected" && lighting?.phase !== "paused";
  const customise = () => { if (shown?.profileId) setLibrary({ selected: shown.profileId }); go("library"); };
  // why a keyboard that's plugged in isn't used (refused by the identity check, didn't answer)
  const why = !status?.connected && status?.error && !/unplugged|disconnected/i.test(status.error) ? status.error : null;
  const request = lighting?.request;
  const note = mirrorNote(lighting);
  // the look on the keys, by name (only what the keyboard accepted, as the mirror)
  const look = reachable && shown ? (shown.origin === "off" || shown.brightness === 0 ? "Lights off" : shown.name) : null;

  return (
    <section className="page home">
      <button className="home-board" onClick={customise} aria-label="Customise the keyboard" aria-describedby={note ? "home-note" : undefined}>
        <Mirror layout={layout} lighting={lighting} />
      </button>
      {note && <p id="home-note" className="sr-only">{note}</p>}

      <div className="home-info">
        <h1>{layout.name}</h1>
        {look && <p className="home-look"><span className="sr-only">On the keys: </span>{look}</p>}
        <p className="device-status"><span className={`dot ${state.tone}`} />{status?.busy ? `Busy: ${status.busy}…` : state.text}</p>
        {!status?.connected && !status?.paused && <p className="home-hint-line">Plug in your keyboard with its USB cable; Keylume finds it by itself.</p>}
        {why && <p className="home-error" role="alert">{why}</p>}
        {(lighting?.phase === "requested" || lighting?.phase === "pending") && request && <p className="home-pending" role="status">Applying {request.name}…</p>}
        {lighting?.phase === "failed" && request && <p className="home-error" role="alert">Couldn't apply {request.name}: {lighting.error}</p>}
        <div className="home-actions">
          {shown?.running && reachable && (
            <button className="btn" onClick={() => attempt("Stop", async () => (await api()).stopLive())}><Icon name="stop" size={14} />Stop</button>
          )}
          <LightsButton />
          <button className="btn primary" onClick={customise}>Customise</button>
        </div>
      </div>
    </section>
  );
}
