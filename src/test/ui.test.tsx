import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { App, ConflictBanner } from "../App";
import { Keyboard } from "../components/Keyboard";
import { Mirror } from "../components/Mirror";
import { Thumb } from "../components/Thumb";
import { api, setApi, type KeylumeApi } from "../lib/api";
import { createMockApiFrom, type MockApi } from "../lib/mock";
import { effectFrame, liveFrameColors, player } from "../lib/animate";
import { ALL_FEATURES, canShow } from "../lib/features";
import { previewColors, SECTIONS, stillColors } from "../lib/library";
import { brightness } from "../lib/preview";
import { spellFrames } from "../lib/spell";
import { LIBRARY_START, useApp } from "../lib/store";
import type { LightingState, Profile } from "../lib/types";
import { unpackLibrary } from "../lib/wire";

// The real generated library + layout, served to the UI through the mock backend.
const fixture = unpackLibrary(JSON.parse(readFileSync("public/mock/library.json", "utf8")));
const SPELL = { words: [{ text: "Keeps", color: "#00c8ff" }, { text: "Glows", color: "#ffb000" }], background: "#020a3a" };

let base: KeylumeApi;
beforeAll(() => {
  base = createMockApiFrom(fixture, { latency: 0 });
  setApi(base);
});
beforeEach(() => useApp.setState({ library: LIBRARY_START })); // the Library remembers its view between visits

describe("Keyboard", () => {
  it("renders every key and reports clicks", () => {
    const clicked: string[] = [];
    const { container } = render(<Keyboard layout={fixture.layout} colors={{}} onKeyDown={(id) => clicked.push(id)} />);
    expect(container.querySelectorAll("[data-key]")).toHaveLength(68);
    fireEvent.pointerDown(container.querySelector('[data-key="esc"]')!);
    expect(clicked).toEqual(["esc"]);
  });
});

describe("spell (mirror of the Rust generator)", () => {
  it("spells two words letter by letter and ends with both", () => {
    const f = spellFrames(fixture.layout, SPELL.words, SPELL.background);
    const bg = SPELL.background;
    expect(f).toHaveLength(11); // K E E P S + G L O W S + final
    expect(Object.values(f[0]).filter((c) => c !== bg)).toHaveLength(1);
    expect(f[2].e).toBe("#ffffff"); // the second E flashes
    const last = f[f.length - 1];
    expect([last.k, last.g, last.s, last.q]).toEqual(["#00c8ff", "#ffb000", "#ffffff", bg]); // shared S glows white
  });
});

describe("library thumbnails", () => {
  it("every profile draws a visible, representative picture", () => {
    for (const p of fixture.profiles) {
      const c = previewColors(p, fixture.layout);
      expect(Object.keys(c)).toHaveLength(68);
      if (p.id !== "fx-off") expect(brightness(c), p.id).toBeGreaterThan(0.02);
    }
    const byId = (id: string) => fixture.profiles.find((p: { id: string }) => p.id === id);
    const distinct = (id: string) => new Set(Object.values(previewColors(byId(id), fixture.layout))).size;
    // stills show only what the keyboard can: the firmware animates each key...
    for (const id of ["fx-wave-cyan", "fx-starlight-red", "ghost-rainbow"]) expect(distinct(id), id).toBeGreaterThan(3);
    // ...a whole-board live effect (and the spectrum cycle) is one colour at a time...
    for (const id of ["fx-spectrum", "live-acid-trip", "live-ocean-flow", "flag-belgium-live"]) expect(distinct(id), id).toBe(1);
    // ...and the music modes draw bars, lit and unlit
    expect(distinct("live-plasma-bars")).toBeGreaterThan(1);
  });

  it("thumbnails show a design at its brightness; editors get its full colours", () => {
    const p: Profile = { id: "d", name: "Dim", category: "Mine", tags: [], description: "", source: "user", lighting: { kind: "perKey", brightness: 2, keys: { esc: "#ff8000" } } };
    expect(previewColors(p, fixture.layout).esc).toBe("#ff8000");
    const still = stillColors(p, fixture.layout).esc;
    expect(still).not.toBe("#ff8000");
    expect(parseInt(still.slice(1, 3), 16)).toBeLessThan(255);
  });
});

describe("hover previews", () => {
  it("every keyboard animation moves (except the still ones)", () => {
    const still = new Set(["off", "static", "user-picture", "screen-sync"]);
    for (const m of ["off", "static", "breathing", "spectrum", "wave", "ripple", "raindrop", "snake", "reactive", "converge", "sine-wave",
      "kaleidoscope", "line-wave", "user-picture", "laser", "circle-wave", "dazzle", "rain-down", "meteor", "reactive-off", "music-bars",
      "screen-sync", "music-pulse"] as const) {
      const e = { mode: m, speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#0060ff" };
      const frames = [0, 0.7, 1.9, 3.1].map((t) => JSON.stringify(effectFrame(e, fixture.layout, t)));
      expect(Object.keys(effectFrame(e, fixture.layout, 0))).toHaveLength(68);
      expect(new Set(frames).size > 1, m).toBe(!still.has(m));
    }
  });

  it("plays spells letter by letter and maps live frames onto the keys", () => {
    const byId = (id: string) => fixture.profiles.find((p: { id: string }) => p.id === id);
    const spellProfile: Profile = { id: "s", name: "s", category: "Mine", tags: [], description: "", source: "user", lighting: { kind: "spell", brightness: 4, ...SPELL } };
    const spell = player(spellProfile, fixture.layout)!;
    expect(spell(0)).not.toEqual(spell(3));
    expect(player(byId("deep-ocean-flame"), fixture.layout)).toBeNull(); // per-key: nothing to play
    const bars = liveFrameColors({ levels: Array(32).fill(6) }, { kind: "spectrum", color: "#ff0000", rainbow: false, mirror: false }, fixture.layout);
    expect(bars.q).not.toBe(bars.lctrl === "#000000" ? "x" : "#000000");
    const flat = liveFrameColors({ color: "#123456" }, { kind: "heartbeat", color: "#123456", bpm: 60 }, fixture.layout);
    expect(new Set(Object.values(flat))).toEqual(new Set(["#123456"]));
  });
});

describe("ConflictBanner", () => {
  it("explains the pause and offers to close the other app", () => {
    render(<ConflictBanner reason="EPOMAKER Driver is running" />);
    expect(screen.getByRole("alert")).toHaveTextContent("EPOMAKER Driver is running.");
    expect(screen.getByRole("alert")).toHaveTextContent("resumes by itself");
    expect(screen.getByRole("button", { name: "Close it" })).toBeInTheDocument();
  });
});

const rail = () => screen.getByRole("navigation", { name: "Places" });
/** From Home, open the keyboard's pages (Customise), then a place from the rail (Settings is
 * at its foot). */
async function go(where: string) {
  if (!screen.queryByRole("navigation", { name: "Places" })) {
    fireEvent.click(await screen.findByRole("button", { name: "Customise" }));
  }
  fireEvent.click(within(rail()).getByRole("button", { name: where }));
}
const card = (name: string) => screen.queryAllByTestId("design-card").find((c) => c.querySelector(".name")?.textContent === name);
const tile = (name: string) => screen.queryAllByTestId("collection-tile").find((c) => c.querySelector("b")?.textContent === name);
/** A collection tile, once the Library's Discover view shows it. */
const findTile = async (name: string) => {
  let found: HTMLElement | undefined;
  await waitFor(() => { found = tile(name); expect(found).toBeDefined(); });
  return found!;
};
const shownCount = () => Number(document.querySelector(".results-head .count")!.textContent!.split(" ")[0].replace(/,/g, ""));
const sections = () => screen.getByRole("tablist", { name: "Sections" });

describe("App", () => {
  beforeEach(() => useApp.setState({ page: "home" }));

  it("opens on Home: the keyboard, its name and whether it's connected, and Customise", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { level: 1, name: fixture.layout.name })).toBeInTheDocument();
    expect(screen.getByText("Simulated")).toBeInTheDocument();
    expect(screen.getByTestId("mirror")).toBeInTheDocument();
    // nothing else: no bar across the top (the window's title says Keylume), no firmware, no favourites
    expect(screen.queryByRole("banner")).toBeNull();
    expect(screen.queryByText(/firmware/i)).toBeNull();
    expect(screen.queryByRole("region", { name: "Favourites" })).toBeNull();
    // and nothing else to press: no gear in the corner, no hint popping up over the keyboard
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
    expect(document.querySelector(".home-hint")).toBeNull();
    // no rail here: the keyboard itself (or Customise) opens its pages
    expect(screen.queryByRole("navigation", { name: "Places" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Customise" }));
    // one way around: every place in the rail, Settings at its foot, nothing across the top
    expect(within(rail()).getAllByRole("button").map((b) => b.textContent)).toEqual(["Library", "Create", "Side light", "Keys", "Settings"]);
    expect(screen.queryByRole("tablist", { name: "Lighting" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("button", { name: "Customise" })).toBeInTheDocument();
  });

  it("opens on the library: favourites, then every collection as a tile", async () => {
    render(<App />);
    await go("Library");
    expect(await screen.findByRole("region", { name: "Favourites" })).toBeInTheDocument();
    const present = SECTIONS.filter((s) => useApp.getState().profiles.some((p: Profile) => p.section === s));
    // packs have no tab of their own: the ones that come with Keylume sit under Themes
    expect(within(sections()).getAllByRole("tab").map((t) => t.textContent)).toEqual(["Discover", "Favourites", "Mine", ...present]);
    expect(within(sections()).queryByRole("tab", { name: "Packs" })).toBeNull();
    expect(within(screen.getByRole("region", { name: "Themes" })).getAllByTestId("collection-tile").some((t) => t.textContent?.includes("Solar Terms"))).toBe(true);
    for (const s of present) expect(screen.getByRole("region", { name: s })).toBeInTheDocument();
    expect(tile("CS2")).toBeDefined();
    expect(tile("Heroes")).toBeDefined();
    // the side panel starts on what's on the keyboard, or a welcome
    expect(screen.getByRole("complementary", { name: "Design" })).toBeInTheDocument();
  });

  it("opens a collection from its tile, applies a design, then another of its looks", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    fireEvent.click(tile("Blue")!);
    expect(await screen.findByRole("heading", { name: "Blue" })).toBeInTheDocument();
    await waitFor(() => expect(card("Deep Ocean")).toBeDefined());
    await act(async () => { fireEvent.click(card("Deep Ocean")!.querySelector(".card-main")!); });
    await waitFor(() => expect(useApp.getState().lighting?.shown?.profileId).toMatch(/^deep-ocean-/));
    expect(await screen.findByText("On keyboard")).toBeInTheDocument();
    const panel = screen.getByRole("complementary", { name: "Design" });
    expect(within(panel).getByRole("heading", { name: "Deep Ocean" })).toBeInTheDocument();
    await act(async () => { fireEvent.click(within(panel).getByRole("button", { name: "Flame" })); });
    await waitFor(() => expect(useApp.getState().lighting?.shown?.profileId).toBe("deep-ocean-flame"));
    // back to Discover
    fireEvent.click(screen.getByRole("button", { name: /Colours$/ }));
    expect(await screen.findByRole("region", { name: "Colours" })).toBeInTheDocument();
  });

  it("finds designs by word, by colour and by kind, and surprises you", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    fireEvent.change(screen.getByLabelText("Search profiles"), { target: { value: "deep ocean flame" } });
    await waitFor(() => expect(shownCount()).toBeGreaterThan(0));
    expect(shownCount()).toBeLessThan(10);
    fireEvent.change(screen.getByLabelText("Search profiles"), { target: { value: "" } });
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Colour" })).getByRole("radio", { name: "Purple" }));
    expect(await screen.findByRole("heading", { name: "Purple designs" })).toBeInTheDocument();
    const purple = shownCount();
    expect(purple).toBeGreaterThan(20);
    fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "live" } });
    await waitFor(() => expect(shownCount()).toBeLessThan(purple));
    const before = useApp.getState().current;
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Surprise me" })); });
    await waitFor(() => expect(useApp.getState().current).not.toBe(before));
  });

  it("creates profiles of every kind from one page", async () => {
    render(<App />);
    await go("Create");
    expect(await screen.findByRole("tab", { name: /Paint keys/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("button", { name: "Show on keyboard" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: /Animated/ }));
    expect(screen.getByRole("listbox", { name: "Effect" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: /^.?Live$/ }));
    // every live effect type renders its settings without crashing, grouped in one picker
    const picker = screen.getByLabelText("Live effect type") as HTMLSelectElement;
    expect(within(picker).getAllByRole("group").map((g) => g.getAttribute("label"))).toEqual(["Calm", "Party", "Music", "Bars", "Useful", "Games"]);
    for (const o of [...picker.options]) {
      fireEvent.change(picker, { target: { value: o.value } });
      expect(picker).toHaveValue(o.value);
    }
    fireEvent.change(picker, { target: { value: "morse" } });
    expect(screen.getByLabelText("Morse message")).toHaveValue("GG");
    fireEvent.change(picker, { target: { value: "siren" } });
    expect(screen.getByText("Colour A")).toBeInTheDocument();
    fireEvent.change(picker, { target: { value: "bars" } });
    expect(screen.getByLabelText("Bar pattern")).toHaveValue("plasma");

    fireEvent.click(screen.getByRole("tab", { name: /Spell/ }));
    fireEvent.change(screen.getByLabelText("Word 1"), { target: { value: "GG" } });
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save & spell it" })); });
    await waitFor(() => expect(useApp.getState().current).toMatch(/^user-spell-gg/));
  });

  it("edits a copy of an animated profile in the right tab", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    fireEvent.change(screen.getByLabelText("Search profiles"), { target: { value: "rainbow ghost" } });
    await waitFor(() => expect(card("Rainbow Ghost")).toBeDefined());
    await act(async () => { fireEvent.click(card("Rainbow Ghost")!.querySelector(".card-main")!); });
    fireEvent.click(within(screen.getByRole("complementary", { name: "Design" })).getByRole("button", { name: "Edit a copy" }));
    expect(await screen.findByRole("tab", { name: /Animated/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByLabelText("Profile name")).toHaveValue("Rainbow Ghost (mine)");
  });

  it("has no game mode and no link to any game: CS2 is designs only", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    expect(screen.queryByRole("button", { name: /mode/ })).toBeNull();
    expect(useApp.getState().profiles.some((p) => p.id === "live-cs2-live")).toBe(false);
    expect(useApp.getState().profiles.find((p) => p.id === "cs-binds")?.name).toBe("CS2 Binds");
    await go("Create");
    fireEvent.click(screen.getByRole("tab", { name: /^.?Live$/ }));
    expect(within(screen.getByLabelText("Live effect type")).queryByRole("option", { name: "CS2 Live" })).toBeNull();
    await go("Settings");
    await screen.findByRole("heading", { level: 1, name: "Settings" });
    expect(screen.queryByText("Game mode")).toBeNull();
    expect(screen.queryByText("Counter-Strike")).toBeNull();
  });

  it("browses sections: CS2 opens Games, themes in order, ghosts in Effects, flags by region", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    const open = (section: string) => fireEvent.click(within(sections()).getByRole("tab", { name: section }));
    const tiles = () => screen.getAllByTestId("collection-tile").map((t) => t.querySelector("b")!.textContent);

    open("Games");
    expect(tiles().slice(0, 2)).toEqual(["CS2", "CS2 Skins"]);
    fireEvent.click(tile("CS2")!);
    await waitFor(() => expect(card("CS2 Binds")).toBeDefined());
    fireEvent.click(screen.getByRole("button", { name: /Games$/ }));
    fireEvent.click(tile("CS2 Skins")!);
    await waitFor(() => expect(card("Asiimov")).toBeDefined());
    fireEvent.click(screen.getByRole("button", { name: /Games$/ }));
    // a game's designs open on its default keys
    fireEvent.click(tile("Dota 2")!);
    await waitFor(() => expect(card("Jungle Dawn")).toBeDefined());
    await act(async () => { fireEvent.click(card("Jungle Dawn")!.querySelector(".card-main")!); });
    await waitFor(() => expect(fixture.profiles.find((p: Profile) => p.id === useApp.getState().current)?.name).toBe("Jungle Dawn · Game Keys"));

    open("Flags");
    fireEvent.click(tile("Europe")!);
    await waitFor(() => expect(card("France")).toBeDefined());
    open("Comics");
    expect(tiles()[0]).toBe("Heroes");
    open("Themes");
    expect(tiles().slice(0, 8)).toEqual(["Myths", "Nature", "Space", "Festivals", "Abstract", "Art", "Food", "Cities"]);
    open("Effects");
    fireEvent.click(tile("Ghost")!);
    await waitFor(() => expect(card("Rainbow Laser Ghost")).toBeDefined());
    expect(card("Royal Laser Ghost")).toBeDefined();
    expect(card("Gold Ghost")).toBeDefined();
  });

  it("moves between cards with the arrow keys and keeps the view between visits", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    fireEvent.click(within(sections()).getByRole("tab", { name: "Effects" }));
    fireEvent.click(tile("Ghost")!);
    await waitFor(() => expect(card("Gold Ghost")).toBeDefined());
    const search = screen.getByLabelText("Search profiles");
    const cards = () => screen.getAllByTestId("design-card").map((c) => c.querySelector(".card-main"));
    const key = (k: string) => fireEvent.keyDown(document.activeElement!, { key: k });
    search.focus();
    key("ArrowDown");
    expect(document.activeElement).toBe(cards()[0]);
    key("ArrowRight");
    expect(document.activeElement).toBe(cards()[1]);
    key("End");
    expect(document.activeElement).toBe(cards().at(-1));
    key("Home");
    key("ArrowUp"); // from the top row back to the search
    expect(document.activeElement).toBe(search);

    const scroller = () => document.querySelector(".lib-main") as HTMLElement;
    scroller().scrollTop = 420;
    fireEvent.scroll(scroller());
    await go("Settings");
    await screen.findByRole("heading", { level: 1, name: "Settings" });
    await go("Library");
    expect(await screen.findByRole("heading", { name: "Ghost" })).toBeInTheDocument();
    expect(scroller().scrollTop).toBe(420);
  });

  it("controls the side light", async () => {
    render(<App />);
    await go("Side light");
    expect(await screen.findByRole("switch", { name: /Match my lighting/ })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Rainbow Wave" }));
    await waitFor(() => expect(useApp.getState().settings?.sideFollow).toBe(false));
    expect(useApp.getState().settings?.sideCustom).toMatchObject({ mode: "wave", rainbow: true });
    expect(screen.getByRole("radiogroup", { name: "Direction" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch", { name: /Match my lighting/ }));
    await waitFor(() => expect(useApp.getState().settings?.sideFollow).toBe(true));
  });

  it("adds profiles from a dropped file and shows them under Mine", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    const pack = JSON.stringify({ profiles: [{ name: "Dropped Sunset", lighting: { kind: "perKey", keys: { esc: "#ff7a00" } } }] });
    const drop = (f: File) => ({ dataTransfer: { types: ["Files"], files: [f] } });
    const file = new File([pack], "sunset.keylume.json", { type: "application/json" });
    fireEvent.dragOver(window, drop(file));
    expect(await screen.findByText("Drop to add to your library")).toBeInTheDocument();
    await act(async () => { fireEvent.drop(window, drop(file)); });
    await waitFor(() => expect(card("Dropped Sunset")).toBeDefined());
    expect(within(sections()).getByRole("tab", { name: "Mine" })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByText("Drop to add to your library")).toBeNull();

    await act(async () => { fireEvent.drop(window, drop(new File(["hello"], "notes.txt"))); });
    expect(await screen.findByText("notes.txt: not a Keylume profile file")).toBeInTheDocument();
  });

  it("keeps settings short: true colours on by default", async () => {
    render(<App />);
    await go("Settings");
    expect(await screen.findByRole("switch", { name: /True colours/ })).toBeChecked();
    fireEvent.click(screen.getByRole("switch", { name: /True colours/ }));
    await waitFor(() => expect(useApp.getState().settings?.trueColors).toBe(false));
    fireEvent.click(screen.getByRole("switch", { name: /True colours/ }));
    // no folders, storage details or licence talk: just which Keylume this is
    expect(screen.queryByText(/localStorage|your data|open source|licen[cs]e/i)).toBeNull();
    expect(screen.getByText(/^Keylume \d/)).toBeInTheDocument();
    // one page for the keyboard and the app: the keyboard's own settings first, each said once
    const groups = screen.getAllByRole("region").map((r) => r.getAttribute("aria-label")).filter((l) => l !== "Unsaved keyboard settings");
    expect(groups).toEqual(["Keyboard", "Keys", "Keylume", "Backup"]);
    expect(screen.getAllByText("Polling rate")).toHaveLength(1);
    // the keyboard's settings are written only when asked: Save shows once something changed
    expect(screen.queryByRole("button", { name: "Save to keyboard" })).toBeNull();
    fireEvent.click(await screen.findByRole("switch", { name: /Mac layout/ }));
    expect(screen.getByRole("button", { name: "Save to keyboard" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.queryByRole("button", { name: "Save to keyboard" })).toBeNull();
    const layer = await screen.findByText("Layer for library designs");
    fireEvent.click(within(layer.closest(".row-setting") as HTMLElement).getByRole("radio", { name: "2" }));
    await waitFor(() => expect(useApp.getState().settings?.liveLayer).toBe(1));
    fireEvent.click(within(layer.closest(".row-setting") as HTMLElement).getByRole("radio", { name: "3" }));
  });

  it("remaps a key in the panel beside the keyboard, then writes it", async () => {
    render(<App />);
    await go("Keys");
    const board = await screen.findByRole("group", { name: `${fixture.layout.name} keyboard` });
    expect(screen.getByText("Pick a key")).toBeInTheDocument();
    const write = screen.getByRole("button", { name: "Write to keyboard" });
    expect(write).toBeDisabled();
    fireEvent.pointerDown(board.querySelector('[data-key="caps"]')!);
    expect(await screen.findByRole("heading", { level: 2, name: "Caps" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: "Media" }));
    fireEvent.click(screen.getByRole("button", { name: "Mute" }));
    expect(screen.getByText("1 unsaved change")).toBeInTheDocument();
    await act(async () => { fireEvent.click(write); });
    await waitFor(() => expect(screen.getByText("No unsaved changes")).toBeInTheDocument());
  });

  it("says so when the keyboard doesn't keep the Fn layer, and never shows it as in use", async () => {
    render(<App />);
    window.confirm = () => true;
    const a = await api();
    const spy = vi.spyOn(a, "setKeymap").mockRejectedValue("The keyboard didn't keep the change to 5: it still has what it had there");
    await go("Settings");
    const row = (await screen.findByText("Fn layer")).closest(".row-setting") as HTMLElement;
    await act(async () => { fireEvent.click(within(row).getByRole("button", { name: "Use it" })); });
    expect(await screen.findByText(/didn't keep the change to 5/)).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Use it" })).toBeInTheDocument();
    expect(within(row).queryByText("In use")).toBeNull();
    spy.mockRestore();
  });

  it("puts the standard Fn layer on the keyboard: Fn + number keys give F1 … F12", async () => {
    render(<App />);
    window.confirm = () => true;
    await go("Settings");
    // the keyboard came with media on the number row: Settings offers the standard layer
    const row = (await screen.findByText("Fn layer")).closest(".row-setting") as HTMLElement;
    expect(row).toHaveTextContent("Fn + 1 … = for F1 … F12");
    await act(async () => { fireEvent.click(within(row).getByRole("button", { name: "Use it" })); });
    expect(await within(row).findByText("In use")).toBeInTheDocument();
    const fn = await (await api()).getKeymap("fn", 0);
    expect(fn["1"]).toEqual({ kind: "key", code: 0x3a, modifier: 0, code2: 0 });
    expect(fn["equal"]).toEqual({ kind: "key", code: 0x45, modifier: 0, code2: 0 });
    expect(fn["x"]).toEqual({ kind: "consumer", usage: 0xcd });
    // Keys shows it, and Reset on the Fn layer puts back that standard layer (not nothing)
    await go("Keys");
    fireEvent.click(await screen.findByRole("radio", { name: "Fn layer" }));
    const board = await screen.findByRole("group", { name: `${fixture.layout.name} keyboard` });
    await waitFor(() => expect(board.querySelector('[data-key="1"]')).toHaveTextContent("F1"));
    fireEvent.click(screen.getByRole("button", { name: "Reset layer" }));
    expect(screen.getByText("No unsaved changes")).toBeInTheDocument();
    fireEvent.pointerDown(board.querySelector('[data-key="2"]')!);
    fireEvent.click(await screen.findByRole("button", { name: "Reset this key" }));
    expect(screen.getByText("No unsaved changes")).toBeInTheDocument();
  });

  it("reaches every page without crashing", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    await go("Create");
    expect(await screen.findByRole("tablist", { name: "What to create" })).toBeInTheDocument();
    await go("Side light");
    expect(await screen.findByRole("heading", { level: 1, name: "Side light" })).toBeInTheDocument();
    // every page past Home names itself in a head row (not only for screen readers)
    expect(screen.getByRole("heading", { level: 1, name: "Side light" })).not.toHaveClass("sr-only");
    await go("Keys");
    fireEvent.click(within(await screen.findByRole("tablist", { name: "Keys" })).getByRole("tab", { name: "Macros" }));
    expect(await screen.findByRole("radiogroup", { name: "Macro slots" })).toBeInTheDocument();
    await go("Settings");
    expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    await go("Library");
    expect(await screen.findByRole("region", { name: "Favourites" })).toBeInTheDocument();
  });
});

// ---- what the keyboard really shows ------------------------------------------------------

/** A fresh simulated keyboard that takes a moment to answer, so "applying" can be seen. */
function slowKeyboard(): MockApi {
  const m = createMockApiFrom(fixture, { latency: 15 });
  setApi(m);
  useApp.setState({ lighting: null, current: null, page: "home", status: null });
  return m;
}
/** Pick the i-th favourite (as the Library or the tray would), and give its name. */
const pick = (i: number) => {
  const id = useApp.getState().settings!.favorites[i];
  void useApp.getState().apply(id);
  return fixture.profiles.find((p: Profile) => p.id === id)!.name;
};
/** What the keyboard is known to show. */
const onKeys = () => useApp.getState().lighting?.shown?.name;

/** Back to the shared keyboard (and forget the other one's lighting, which counts differently). */
const shared = () => { setApi(base); useApp.setState({ lighting: null, page: "home" }); };

describe("Home mirrors the keyboard", () => {
  afterEach(shared);

  it("claims a look only once the keyboard took it, and says when it didn't", async () => {
    const m = slowKeyboard();
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: fixture.layout.name });
    const first = pick(0);
    expect(await screen.findByText(`Applying ${first}…`)).toBeInTheDocument();
    expect(onKeys()).not.toBe(first);
    await waitFor(() => expect(onKeys()).toBe(first));
    expect(screen.queryByText(/^Applying/)).toBeNull();
    // Home names the look on the keys, once the keyboard took it
    await waitFor(() => expect(document.querySelector(".home-look")).toHaveTextContent(`On the keys: ${first}`));

    // a write that fails is reported, and the old look is still the one claimed
    m.sim.failNext("device not functioning");
    const second = pick(1);
    expect(await screen.findByText(`Couldn't apply ${second}: device not functioning`)).toBeInTheDocument();
    expect(onKeys()).not.toBe(second);

    // unplugged: nothing is claimed; plugged back in: what the keyboard has is read back
    act(() => m.sim.setConnected(false));
    expect(await screen.findByText(/Plug in your keyboard/)).toBeInTheDocument();
    expect(screen.getByText("Not connected")).toBeInTheDocument();
    expect(onKeys()).toBeUndefined();
    expect(document.querySelector(".home-look")).toBeNull();
    act(() => useApp.setState({ status: { ...useApp.getState().status!, error: "a device with the keyboard's USB ids is connected, but Keylume won't write to it: test" } }));
    expect(await screen.findByText(/won't write to it: test/)).toBeInTheDocument(); // a refusal says why
    act(() => m.sim.setConnected(true));
    expect(await screen.findByText("Simulated")).toBeInTheDocument();
    await waitFor(() => expect(onKeys()).toBeDefined());

    // paused while the vendor's app runs
    act(() => m.sim.setPaused("EPOMAKER Driver is running"));
    expect(await screen.findByText("Paused")).toBeInTheDocument();
    expect(screen.getAllByRole("alert").some((a) => a.textContent?.includes("EPOMAKER Driver is running"))).toBe(true);
    act(() => m.sim.setPaused(null));
  });

  it("ends on the last of several quick picks", async () => {
    slowKeyboard();
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: fixture.layout.name });
    const picks = [pick(0), pick(1), pick(4)];
    await waitFor(() => expect(onKeys()).toBe(picks[2]));
    await waitFor(() => expect(useApp.getState().lighting?.phase).toBe("applied"));
    expect(onKeys()).toBe(picks[2]);
  });

  it("stops a live effect on its last frame, and turns the lights off", async () => {
    slowKeyboard();
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: fixture.layout.name });
    const live = pick(2); // Ocean Flow, a live effect
    await waitFor(() => expect(useApp.getState().lighting?.shown?.running).toBe(true));
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    await waitFor(() => expect(useApp.getState().lighting?.shown?.running).toBe(false));
    expect(onKeys()).toBe(live);
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Lights on/off" }));
    await waitFor(() => expect(useApp.getState().lighting?.phase).toBe("off"));
    await waitFor(() => expect(document.querySelector(".home-look")).toHaveTextContent("Lights off"));
    // lights off on a white TK68: plain white keycaps (Home draws the keyboard as it looks)
    await waitFor(() => expect(Object.values(screen.getByTestId("mirror").querySelectorAll(".cap")).every((c) => c.getAttribute("fill") === "#e9ecf2")).toBe(true));
    expect(screen.getByTestId("mirror").querySelector(".keyboard")).toHaveClass("finish-white");
  });

  it("follows a spell to its last picture and holds it", async () => {
    slowKeyboard();
    render(<App />);
    await go("Create");
    fireEvent.click(await screen.findByRole("tab", { name: /Spell/ }));
    fireEvent.change(screen.getByLabelText("Word 1"), { target: { value: "GG" } });
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save & spell it" })); });
    await waitFor(() => expect(useApp.getState().lighting?.shown?.progress).toMatchObject({ done: 1, total: 3 }));
    await waitFor(() => expect(useApp.getState().lighting?.shown?.progress).toMatchObject({ done: 3, total: 3 }), { timeout: 3000 });
    expect(useApp.getState().lighting?.shown?.running).toBe(false);
  });
});

describe("Settings: backup and restore", () => {
  afterEach(shared);
  const backupFile = JSON.stringify({ keylumeBackup: 1, device: "epomaker-tk68", firmware: 772, macros: [{ repeat: 1, events: [{ kind: "delay", ms: 5 }] }], effect: { mode: "breathing", speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#ff0000" } });

  it("checks a backup before asking, writes nothing for a bad one, and blocks other changes while restoring", async () => {
    const m = slowKeyboard();
    const asked: string[] = [];
    window.confirm = (q?: string) => { asked.push(q ?? ""); return true; };
    render(<App />);
    await go("Settings");
    const restore = await screen.findByRole("button", { name: "Restore…" });
    await waitFor(() => expect(restore).toBeEnabled()); // not while the keyboard's settings are read
    m.sim.pickFile("notes.json", "{}");
    fireEvent.click(restore);
    expect(await screen.findByText(/notes.json can't be restored: it isn't a Keylume backup/)).toBeInTheDocument();
    expect(asked).toEqual([]);
    m.sim.pickFile("tk68.json", backupFile);
    fireEvent.click(restore);
    await waitFor(() => expect(asked).toHaveLength(1));
    expect(asked[0]).toMatch(/Restore tk68.json\? .*1 macro .*saves what the keyboard holds now first/);
    await waitFor(() => expect(screen.getByRole("button", { name: "Back up…" })).toBeDisabled());
    expect(useApp.getState().status?.busy).toBe("restoring a backup");
    await act(async () => { await useApp.getState().apply(fixture.profiles[0].id); });
    expect(await screen.findByText(/busy \(restoring a backup\)/)).toBeInTheDocument();
    expect(await screen.findByText(/Backup restored/)).toBeInTheDocument();
    expect(useApp.getState().lighting?.shown?.origin).toBe("restored");
    await waitFor(() => expect(screen.getByRole("button", { name: "Back up…" })).toBeEnabled());
  });
});

describe("keys can be edited without a mouse", () => {
  it("moves between keys with the arrows and presses them with Enter or Space", () => {
    const pressed: string[] = [];
    render(<Keyboard layout={fixture.layout} colors={{ esc: "#ff0000" }} onKeyDown={(id) => pressed.push(id)} describe={(id) => (id === "esc" ? "#ff0000" : "unlit")} />);
    const esc = screen.getByRole("button", { name: "Esc, #ff0000" });
    expect(esc).toHaveAttribute("tabindex", "0");
    esc.focus();
    fireEvent.keyDown(esc, { key: "ArrowRight" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: /^1,/ }));
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement?.getAttribute("data-key")).toBe("q");
    fireEvent.keyDown(document.activeElement!, { key: "Enter" });
    fireEvent.keyDown(document.activeElement!, { key: " " });
    expect(pressed).toEqual(["q", "q"]);
    expect(screen.getAllByRole("button").filter((b) => b.getAttribute("tabindex") === "0")).toHaveLength(1);
  });
});

describe("with reduced motion", () => {
  // Windows reports it when "Animation effects" is off. Lighting previews still play (they're
  // what you point at or open to see a design); the interface's own transitions calm down.
  beforeEach(() => {
    window.matchMedia = ((q: string) => ({ matches: q.includes("reduce"), addEventListener() {}, removeEventListener() {} })) as unknown as typeof window.matchMedia;
  });
  afterEach(() => { delete (window as { matchMedia?: unknown }).matchMedia; vi.restoreAllMocks(); });
  const wave = () => fixture.profiles.find((p: Profile) => p.id === "hero-banner-guard-animated")!;

  it("a design you point at still plays", async () => {
    const fills: string[] = [];
    const g = new Proxy({}, {
      get: (_, k) => (k === "createLinearGradient" ? () => ({ addColorStop() {} }) : () => {}),
      set: (_, k, v) => { if (k === "fillStyle" && typeof v === "string") fills.push(v); return true; },
    });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(g as never);
    vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(160);
    render(<Thumb layout={fixture.layout} colors={{}} profile={wave()} hovering />);
    const still = new Set(fills).size; // unlit keycaps: a skirt, a face and a highlight
    expect(still).toBeLessThan(5);
    await waitFor(() => expect(new Set(fills).size).toBeGreaterThan(still + 5), { timeout: 3000 });
  });

  it("Home still mirrors a keyboard that moves", async () => {
    const p = wave();
    const lighting: LightingState = {
      seq: 1, phase: "applied", request: { id: 1, profileId: p.id, name: p.name }, error: null,
      shown: { request: 1, origin: "profile", profileId: p.id, name: p.name, lighting: p.lighting, brightness: 4, running: false, progress: null },
    };
    const { container } = render(<Mirror layout={fixture.layout} lighting={lighting} />);
    const caps = () => [...container.querySelectorAll("rect.cap")].map((r) => r.getAttribute("fill")).join();
    const first = caps();
    await waitFor(() => expect(caps()).not.toBe(first), { timeout: 3000 });
  });
});


describe("any keyboard", () => {
  beforeEach(shared);

  it("knows what the TK68 can do before it connects (boards/epomaker-tk68.json)", () => {
    const board = JSON.parse(readFileSync("boards/epomaker-tk68.json", "utf8"));
    expect(ALL_FEATURES).toEqual(board.features);
  });

  it("offers only what a keyboard that Keylume lights key by key can do", async () => {
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: fixture.layout.name });
    const mock = (await api()) as MockApi;
    await act(async () => mock.sim.useBoard("lamparray"));
    try {
      await waitFor(() => expect(useApp.getState().features.hostDriven).toBe(true));
      await go("Library");
      // no side light and no key remapping on this keyboard
      expect(within(rail()).queryByRole("button", { name: "Side light" })).toBeNull();
      expect(within(rail()).queryByRole("button", { name: /Keys/ })).toBeNull();
      // the animations that answer typing aren't offered; the rest are
      await go("Create");
      fireEvent.click(screen.getByRole("tab", { name: /Animated/ }));
      const modes = screen.getByRole("listbox", { name: "Effect" });
      expect(within(modes).queryByRole("option", { name: "Ripple" })).toBeNull();
      expect(within(modes).getByRole("option", { name: "Wave" })).toBeInTheDocument();
      expect(screen.getByText(/Keylume draws on the keys while it runs/)).toBeInTheDocument();
      // the Library hides designs it can't show
      const shown = useApp.getState().profiles.filter((p) => canShow(p, useApp.getState().features));
      expect(shown.some((p) => p.lighting.kind === "effect" && p.lighting.effect.mode === "ripple")).toBe(false);
      await go("Settings");
      expect(await screen.findByText(/Experimental/)).toBeInTheDocument();
      expect(screen.getByText(/When Keylume closes, the keyboard shows its own lighting again/)).toBeInTheDocument();
      expect(screen.queryByText("Polling rate")).toBeNull();
    } finally {
      await act(async () => mock.sim.useBoard("tk68"));
    }
  });

  it("ships packs in the sections, adds one from a file, and removes it", async () => {
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    const meta = JSON.parse(readFileSync("packs/solar-terms/pack.json", "utf8"));
    const themes = JSON.parse(readFileSync("packs/solar-terms/themes.json", "utf8"));
    const drop = (name: string, v: object) => act(async () => {
      fireEvent.drop(window, { dataTransfer: { types: ["Files"], files: [new File([JSON.stringify(v)], name)] } });
    });
    // it comes with Keylume: an unsigned file claiming to be it is refused
    await drop("solar-terms-1.0.0.keylumepack", { keylumePack: 1, ...meta, collections: [themes] });
    expect(await screen.findByText(/only an official file can update Solar Terms/)).toBeInTheDocument();
    // another pack is added: it opens in its section (Themes), marked as a community pack
    await drop("dawn.keylumepack", { keylumePack: 1, ...meta, id: "dawn-colours", name: "Dawn Colours", collections: [{ ...themes, collection: "Dawn Colours" }] });
    expect(await screen.findByText("Added the Dawn Colours pack: 24 designs (community)")).toBeInTheDocument();
    expect(within(sections()).getByRole("tab", { name: "Themes" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { level: 2, name: "Dawn Colours" })).toBeInTheDocument();
    expect(screen.getByText("Community")).toBeInTheDocument();
    window.confirm = () => true;
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Remove pack" })); });
    await waitFor(() => expect(useApp.getState().packs.some((p) => p.id === "dawn-colours")).toBe(false));
    // one that comes with Keylume is just a collection: no badge, nothing to remove
    fireEvent.click(await findTile("Solar Terms"));
    expect(await screen.findByRole("heading", { level: 2, name: "Solar Terms" })).toBeInTheDocument();
    expect(screen.queryByText("Included")).toBeNull();
    expect(screen.queryByRole("button", { name: "Remove pack" })).toBeNull();
  });

  it("makes a pack of your own designs, signed as yours, that others can add", async () => {
    const m = base as MockApi;
    const paint = (name: string): Profile => ({ id: "", name, category: "Mine", tags: [], description: "", source: "user",
      lighting: { kind: "perKey", brightness: 4, keys: { q: "#ff7a00", esc: "#5a1a8a" } } });
    await act(async () => { for (const n of ["Sunset Keys", "Ice Ribbon"]) await m.saveProfile(paint(n)); });
    render(<App />);
    await go("Library");
    await screen.findByRole("region", { name: "Favourites" });
    fireEvent.click(screen.getByRole("tab", { name: "Mine" }));
    fireEvent.click(await screen.findByRole("button", { name: /Make a pack/ }));
    const dialog = await screen.findByRole("dialog", { name: "Make a pack" });
    const save = () => act(async () => { fireEvent.click(within(dialog).getByRole("button", { name: "Save pack…" })); });
    await save();
    expect(within(dialog).getByRole("alert")).toHaveTextContent("Give the pack a name");
    fireEvent.change(within(dialog).getByPlaceholderText("Neon Nights"), { target: { value: "Neon Nights" } });
    fireEvent.change(within(dialog).getByPlaceholderText("What's in it, in a line"), { target: { value: "Two warm designs" } });
    fireEvent.change(within(dialog).getByPlaceholderText("Your name"), { target: { value: "PlayerOne" } });
    fireEvent.change(within(dialog).getByPlaceholderText(/^https/), { target: { value: "not a page" } });
    fireEvent.click(within(dialog).getByRole("radio", { name: /Only who gets it/ }));
    await save();
    expect(within(dialog).getByRole("alert")).toHaveTextContent("https://");
    fireEvent.change(within(dialog).getByPlaceholderText(/^https/), { target: { value: "https://example.com/packs" } });
    await save();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(await screen.findByText(/Saved neon-nights-1-0.keylumepack/)).toBeInTheDocument();
    const saved = m.sim.saved()!;
    const file = JSON.parse(saved.text);
    // every design of yours, unless you untick some (other tests left a few here too)
    const mine = useApp.getState().profiles.filter((p) => p.source === "user").length;
    expect(file.designs.map((d: { name: string }) => d.name)).toEqual(expect.arrayContaining(["Sunset Keys", "Ice Ribbon"]));
    expect(file.designs).toHaveLength(mine);
    expect([file.licence, file.publisher, file.url]).toEqual(["personal", "PlayerOne", "https://example.com/packs"]);
    expect(file.signature.key).toMatch(/^[0-9a-f]{64}$/);

    // someone adds it: a community pack under Themes, signed by its maker, for them only
    m.sim.pickFile(saved.name, saved.text);
    await act(async () => { await useApp.getState().uploadDialog(); });
    expect(await screen.findByText(`Added the Neon Nights pack: ${mine} designs (community)`)).toBeInTheDocument();
    expect(screen.getByText(/signed with their key/)).toBeInTheDocument();
    expect(screen.getByText(/please don't pass it on/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /More from PlayerOne/ })).toBeInTheDocument();
    expect(card("Sunset Keys")).toBeDefined();

    // the next one starts from what you filled in, one version on
    fireEvent.click(screen.getByRole("tab", { name: "Mine" }));
    fireEvent.click(await screen.findByRole("button", { name: /Make a pack/ }));
    const again = await screen.findByRole("dialog", { name: "Make a pack" });
    expect(within(again).getByPlaceholderText("Your name")).toHaveValue("PlayerOne");
    fireEvent.change(within(again).getByPlaceholderText("Neon Nights"), { target: { value: "Neon Nights" } });
    expect(within(again).getByDisplayValue("1.1")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    window.confirm = () => true;
    const id = useApp.getState().packs.find((p) => p.name === "Neon Nights")!.id;
    await act(async () => { await m.removePack(id); });
  });
});
