import { memo, useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { AnimatedBoard, SHOWCASE } from "../components/AnimatedBoard";
import { Icon, type IconName } from "../components/Icon";
import { MakePack } from "../components/MakePack";
import { keyboardFocus, Thumb } from "../components/Thumb";
import { isDynamic } from "../lib/animate";
import { canShow } from "../lib/features";
import { byKind, groupFamilies, KIND_TITLE, variantLabel, type Family, type VariantKind } from "../lib/families";
import { colourFamily, HUES, kindLabel, passes, sections, stillColors, type Filter, type KindFilter } from "../lib/library";
import { api } from "../lib/api";
import { attempt, LIBRARY_START, useApp } from "../lib/store";
import type { Layout, PackInfo, Profile } from "../lib/types";

const KINDS: { value: KindFilter; label: string; title: string }[] = [
  { value: "any", label: "All", title: "Every kind" },
  { value: "perKey", label: "Per-key", title: "A colour on every key, stored on the keyboard" },
  { value: "effect", label: "Animated", title: "The keyboard's own animations: they run without Keylume" },
  { value: "live", label: "Live", title: "Streamed by Keylume while it runs: music, timers, games and more" },
];
const KIND_ICON: Record<VariantKind, IconName> = { perKey: "library", effect: "wand", live: "bolt", spell: "text" };

const PAGE = LIBRARY_START.limit;

/** Arrow keys move between cards (Enter applies, as on any button); up from the top row goes back to the search. */
function moveFocus(e: KeyboardEvent<HTMLElement>, search: HTMLInputElement | null) {
  const cards = [...e.currentTarget.children] as HTMLElement[];
  const i = cards.findIndex((c) => c.contains(e.target as Node));
  const cols = getComputedStyle(e.currentTarget).gridTemplateColumns.split(" ").length || 1;
  const to = ({ ArrowLeft: i - 1, ArrowRight: i + 1, ArrowUp: i - cols, ArrowDown: i + cols, Home: 0, End: cards.length - 1 } as Record<string, number>)[e.key];
  if (i < 0 || to === undefined) return;
  e.preventDefault();
  if (to < 0 && e.key === "ArrowUp") return search?.focus();
  cards[Math.max(0, Math.min(cards.length - 1, to))].querySelector<HTMLElement>(".card-main")?.focus();
}

/** One card in the grid: a design (every look of it), or a single profile. */
interface Card { key: string; family: Family; shown: Profile[]; single: boolean }

const DesignCard = memo(function DesignCard({ card, layout, active, applying = false, open, onOpen }: {
  card: Card; layout: Layout; active: boolean; applying?: boolean; open: boolean; onOpen: (p: Profile) => void;
}) {
  const { family, shown, single } = card;
  const cover = shown[0];
  const stills = useMemo(() => shown.filter((p) => p.lighting.kind === "perKey"), [shown]);
  const [hover, setHover] = useState(false);
  const [focused, setFocused] = useState(false); // keyboard focus previews too
  const [step, setStep] = useState(0);
  const dynamic = isDynamic(cover);
  // hovering a design with several per-key looks flips through them
  useEffect(() => {
    setStep(0);
    if (!(hover || focused) || dynamic || stills.length < 2) return;
    const t = setInterval(() => setStep((s) => s + 1), 750);
    return () => clearInterval(t);
  }, [hover, focused, dynamic, stills.length]);
  const showing = step ? stills[step % stills.length] : cover;
  const colors = useMemo(() => stillColors(showing, layout), [showing, layout]);
  const kinds = single ? [] : byKind(family.variants);
  return (
    <article className={`card ${active ? "active" : ""} ${open ? "open" : ""}`} data-testid="design-card"
      onPointerEnter={() => setHover(true)} onPointerLeave={() => setHover(false)}
      onFocus={(e) => setFocused(keyboardFocus(e.target))} onBlur={(e) => !e.currentTarget.contains(e.relatedTarget) && setFocused(false)}>
      <button className="card-main" onClick={() => onOpen(cover)} title={[single ? cover.name : family.name, cover.description].filter(Boolean).join(": ")}>
        <Thumb layout={layout} colors={colors} profile={dynamic ? cover : undefined} hovering={hover || focused} />
        <span className="card-meta">
          <span className="name">{single ? cover.name : family.name}</span>
          <span className="sub">
            {single ? kindLabel(cover) : family.collection}
            {kinds.length > 0 && family.variants.length > 1 && (
              <span className="kinds">
                {kinds.map(([k, list]) => (
                  <span key={k} className={`kc k-${k}`} title={`${list.length} ${KIND_TITLE[k].toLowerCase()}`}><Icon name={KIND_ICON[k]} size={11} />{list.length}</span>
                ))}
              </span>
            )}
          </span>
        </span>
      </button>
      {active && !applying && <span className="active-badge"><Icon name="check" size={11} />On keyboard</span>}
      {applying && <span className="active-badge applying" role="status">Applying…</span>}
    </article>
  );
});

/** Who made an open pack, what they allow, where to find more, and a way to remove it. */
function PackLine({ pack, onRemoved }: { pack: PackInfo; onRemoved: () => void }) {
  const toast = useApp((s) => s.toast);
  const remove = async () => {
    const a = await api();
    if (!(await a.confirm(`Remove the ${pack.name} pack and its designs? You can add it again from its file.`))) return;
    if (await attempt("Remove pack", () => a.removePack(pack.id))) {
      onRemoved();
      toast(`Removed the ${pack.name} pack`, "success");
    }
  };
  const who = pack.bundled ? "Included with Keylume" : pack.official ? `Signed by ${pack.publisher}` : pack.makerKey ? `By ${pack.publisher}, signed with their key ${pack.makerKey}` : `By ${pack.publisher} (unsigned)`;
  const host = pack.url ? pack.url.replace(/^https:\/\//, "").split(/[/?#]/)[0] : null;
  return (
    <div className="pack-line">
      <span className={`pill ${pack.official ? "ok" : ""}`}>{pack.bundled ? "Included" : pack.official ? "Official" : "Community"}</span>
      <span className="pack-text">
        <b>{pack.name} {pack.version}</b> · {who}.{pack.licence === "personal" ? " For the person who got it: please don't pass it on." : pack.licence === "share" ? " Free to pass on." : ""}
        {pack.description && <span className="pack-desc">{pack.description}</span>}
      </span>
      {host && (
        <button className="btn small ghost" title={pack.url} onClick={() => attempt("Open the maker's page", async () => (await api()).openPackPage(pack.id))}>
          More from {pack.publisher} <small>({host}) ↗</small>
        </button>
      )}
      {!pack.bundled && <button className="btn small ghost" onClick={remove}>Remove pack</button>}
    </div>
  );
}

/** A collection as a picture: four of its designs, its name and how many it has. */
const CollectionTile = memo(function CollectionTile({ name, families, layout, onOpen, badge }: {
  name: string; families: Family[]; layout: Layout; onOpen: (name: string) => void; badge?: string;
}) {
  const covers = useMemo(
    () => families.slice(0, 4).map((f) => stillColors(f.variants.find((v) => v.lighting.kind === "perKey") ?? f.variants[0], layout)),
    [families, layout],
  );
  return (
    <button className="ctile" onClick={() => onOpen(name)} data-testid="collection-tile" title={name}>
      <span className={`ctile-art n${covers.length}`}>
        {covers.map((c, i) => <Thumb key={i} layout={layout} colors={c} />)}
      </span>
      <span className="ctile-meta">
        <b>{name}</b>
        <small>{families.length} {families.length === 1 ? "design" : "designs"}</small>
      </span>
      {badge && <span className="ctile-badge">{badge}</span>}
    </button>
  );
});

function VariantTile({ p, layout, on, picked, onApply, onHover }: {
  p: Profile; layout: Layout; on: boolean; picked: boolean; onApply: () => void; onHover: (id: string | null) => void;
}) {
  const colors = useMemo(() => stillColors(p, layout), [p, layout]);
  return (
    <button className={`variant ${on ? "on" : ""} ${picked ? "picked" : ""}`} onClick={onApply} title={p.name}
      onPointerEnter={() => onHover(p.id)} onPointerLeave={() => onHover(null)} onFocus={() => onHover(p.id)} onBlur={() => onHover(null)}>
      <Thumb layout={layout} colors={colors} />
      <span>{variantLabel(p)}</span>
    </button>
  );
}

/** The design that's open, or what's on the keyboard: a big preview and every look of it. */
function Inspector({ family, picked, layout, onSurprise, onKeyboard }: {
  family: Family | null; picked: string | null; layout: Layout; onSurprise: () => void; onKeyboard: string | null;
}) {
  const { settings, apply, toggleFavorite, edit, setLibrary } = useApp();
  const current = onKeyboard;
  const [hoverId, setHoverId] = useState<string | null>(null);
  useEffect(() => setHoverId(null), [family]);
  if (!family) {
    return (
      <aside className="inspector welcome" aria-label="Design">
        <AnimatedBoard profile={SHOWCASE} layout={layout} className="insp-stage" />
        <h2>Pick a look for your keyboard</h2>
        <p>Every design goes straight to the keys. Open one to see all of its looks.</p>
        <button className="btn primary" onClick={onSurprise}><Icon name="wand" size={15} />Surprise me</button>
      </aside>
    );
  }
  const inFamily = (id: string | null | undefined) => (id ? family.variants.find((v) => v.id === id) : undefined);
  const shown = inFamily(hoverId) ?? inFamily(picked) ?? inFamily(current) ?? family.variants[0];
  const onKeys = !picked && !!inFamily(current);
  const fav = settings?.favorites.includes(shown.id) ?? false;
  const pick = (p: Profile) => { setLibrary({ selected: p.id }); apply(p.id); };
  return (
    <aside className="inspector" aria-label="Design">
      <header className="insp-head">
        <span className="eyebrow">{onKeys ? "On your keyboard" : family.collection}</span>
        <h2>{family.name}</h2>
      </header>
      <AnimatedBoard profile={shown} layout={layout} className="insp-stage" />
      <div className="insp-shown">
        <span className="insp-title">
          <b title={shown.name}>{family.variants.length > 1 ? variantLabel(shown) : shown.name}</b>
          <span className={`kind k-${shown.lighting.kind}`}>{kindLabel(shown)}</span>
        </span>
        <span className="insp-actions">
          <button className={`icon fav ${fav ? "on" : ""}`} onClick={() => toggleFavorite(shown.id)} aria-label={fav ? "Remove from favourites" : "Add to favourites"} title="Favourite">
            <Icon name="star" size={16} />
          </button>
          <button className="icon" onClick={() => edit(shown)} aria-label="Edit a copy" title="Edit a copy"><Icon name="edit" size={16} /></button>
        </span>
      </div>
      {shown.description && <p className="insp-desc">{shown.description}</p>}
      {shown.id !== current && family.variants.length === 1 && <button className="btn primary" onClick={() => pick(shown)}>Show on keyboard</button>}
      {family.variants.length > 1 && byKind(family.variants).map(([k, list]) => (
        <section key={k} className="insp-group" aria-label={`${KIND_TITLE[k]} looks`}>
          <h3>{KIND_TITLE[k]} <span className="count">{list.length}</span></h3>
          <div className="variants">
            {list.map((v) => (
              <VariantTile key={v.id} p={v} layout={layout} on={v.id === current} picked={v.id === shown.id} onApply={() => pick(v)} onHover={setHoverId} />
            ))}
          </div>
        </section>
      ))}
    </aside>
  );
}

/** The profile the keyboard really shows, and the one on its way (see the lighting state). */
function useOnKeyboard(): { shown: string | null; applying: string | null } {
  const lighting = useApp((s) => s.lighting);
  const reachable = lighting?.phase !== "disconnected" && lighting?.phase !== "paused";
  const pending = lighting?.phase === "requested" || lighting?.phase === "pending";
  return {
    shown: (reachable && lighting?.shown?.profileId) || null,
    applying: (pending && lighting?.request?.profileId) || null,
  };
}

export function LibraryPage() {
  const { profiles: every, layout, settings, current: chosen, apply, library: view, setLibrary, uploadDialog, features, packs } = useApp();
  const packOf = (collection: string) => packs.find((p) => p.collections.includes(collection));
  // packs sit in their sections like any collection; one you added says who it's from
  const badgeOf = (collection: string) => {
    const p = packOf(collection);
    return !p || p.bundled ? undefined : p.official ? "Official" : "Community";
  };
  const [making, setMaking] = useState(false);
  // only what this keyboard can show (a LampArray keyboard has no typing animations…)
  const profiles = useMemo(() => every.filter((p) => canShow(p, features)), [every, features]);
  const { shown: current, applying } = useOnKeyboard();
  const { filter } = view;
  const [limit, setLimit] = useState(view.limit);
  const scroller = useRef<HTMLDivElement>(null);
  const setFilter = (next: Filter) => { setLibrary({ filter: next }); setLimit(PAGE); if (scroller.current) scroller.current.scrollTop = 0; };
  const query = useDeferredValue(filter.query);
  const sentinel = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const grid = useRef<HTMLDivElement>(null);

  // Coming back to the Library shows the same view, scrolled to where you left.
  const kept = useRef({ scroll: view.scroll, limit });
  kept.current.limit = limit;
  useLayoutEffect(() => {
    if (scroller.current) scroller.current.scrollTop = kept.current.scroll;
    return () => setLibrary({ ...kept.current });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const groups = useMemo(() => sections(profiles), [profiles]);
  const { families, byProfile } = useMemo(() => groupFamilies(profiles), [profiles]);
  const favorites = settings?.favorites;
  const fav = useMemo(() => new Set(favorites ?? []), [favorites]);
  const f = useMemo(() => ({ ...filter, query }), [filter, query]);

  // What the grid shows: favourites one by one, else designs with at least one look that passes.
  const cards: Card[] = useMemo(() => {
    if (!layout) return [];
    if (f.scope === "favorites") {
      return (favorites ?? []).map((id) => profiles.find((p) => p.id === id)).filter((p): p is Profile => !!p && passes(p, f, fav))
        .map((p) => ({ key: p.id, family: byProfile.get(p.id)!, shown: [p], single: true }));
    }
    const out: Card[] = [];
    for (const family of families) {
      const shown = family.variants.filter((p) => passes(p, f, fav));
      if (!shown.length || (f.hue && colourFamily(shown[0], layout) !== f.hue)) continue;
      out.push({ key: family.key, family, shown, single: family.variants.length === 1 });
    }
    return out;
  }, [families, byProfile, profiles, f, fav, favorites, layout]);

  // Discover: each section's collections as tiles (the kind filter narrows them too).
  const browsing = f.scope === "all" && !f.category && !f.query && !f.hue;
  const shelves = useMemo(() => {
    if (!browsing) return [];
    const byCollection = new Map<string, Family[]>();
    for (const c of cards) {
      if (c.shown[0].source === "user") continue;
      const list = byCollection.get(c.family.collection) ?? [];
      list.push(c.family);
      byCollection.set(c.family.collection, list);
    }
    return [...groups].filter(([s]) => !f.section || s === f.section)
      .map(([s, cols]) => ({ section: s, tiles: cols.filter((c) => byCollection.has(c)).map((c) => ({ name: c, families: byCollection.get(c)! })) }))
      .filter((s) => s.tiles.length);
  }, [browsing, cards, groups, f.section]);

  // Infinite scroll: render cards in pages as the sentinel comes into view.
  useEffect(() => {
    const el = sentinel.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((es) => es.some((e) => e.isIntersecting) && setLimit((l) => l + PAGE), { root: scroller.current, rootMargin: "800px" });
    io.observe(el);
    return () => io.disconnect();
  }, [cards.length, browsing]);

  if (!layout || !settings) return null;
  const currentFamily = current ? byProfile.get(current) : chosen ? byProfile.get(chosen) : undefined;
  const onKeysFamily = current ? byProfile.get(current) : undefined;
  const applyingFamily = applying ? byProfile.get(applying) : undefined;
  const openFamily = (view.selected ? byProfile.get(view.selected) : undefined) ?? currentFamily ?? null;
  const open = (p: Profile) => { setLibrary({ selected: p.id }); apply(p.id); };
  const surprise = () => {
    const c = cards[Math.floor(Math.random() * cards.length)];
    if (c) open(c.shown[Math.floor(Math.random() * c.shown.length)]);
  };
  const tab = filter.section || (filter.scope === "all" ? "" : filter.scope);
  const favCards: Card[] = (favorites ?? []).map((id) => profiles.find((p) => p.id === id)).filter((p): p is Profile => !!p)
    .map((p) => ({ key: p.id, family: byProfile.get(p.id)!, shown: [p], single: true }));

  return (
    <div className="library">
      <h1 className="sr-only">Library</h1>
      <div className="lib-main" ref={scroller} onScroll={(e) => (kept.current.scroll = e.currentTarget.scrollTop)}>
        <div className="lib-top">
          <label className="search">
            <Icon name="search" size={16} />
            <input ref={search} type="search" placeholder="Search designs" value={filter.query}
              onChange={(e) => setFilter({ ...filter, query: e.target.value })} aria-label="Search profiles"
              onKeyDown={(e) => {
                if (e.key !== "ArrowDown") return;
                e.preventDefault();
                grid.current?.querySelector<HTMLElement>(".card-main")?.focus();
              }} />
          </label>
          <div className="hues" role="radiogroup" aria-label="Colour">
            {HUES.map((h) => (
              <button key={h.name} role="radio" aria-checked={filter.hue === h.name} aria-label={h.name} title={h.name}
                className={`hue ${filter.hue === h.name ? "on" : ""}`} style={{ ["--c" as string]: h.swatch }}
                onClick={() => setFilter({ ...filter, hue: filter.hue === h.name ? "" : h.name })} />
            ))}
          </div>
          <select className="kind-select" value={filter.kind} aria-label="Kind" title={KINDS.find((k) => k.value === filter.kind)?.title}
            onChange={(e) => setFilter({ ...filter, kind: e.target.value as KindFilter })}>
            {KINDS.map((k) => <option key={k.value} value={k.value}>{k.value === "any" ? "All kinds" : k.label}</option>)}
          </select>
          <button className="btn icon-btn" onClick={surprise} title="Surprise me: a random design" aria-label="Surprise me"><Icon name="wand" size={16} /></button>
        </div>

        <div className="lib-tabs" role="tablist" aria-label="Sections">
            {([["", "Discover"], ["favorites", "Favourites"], ["mine", "Mine"]] as const).map(([v, label]) => (
              <button key={label} role="tab" aria-selected={tab === v} className={`section ${tab === v ? "on" : ""}`}
                onClick={() => setFilter({ ...filter, scope: v || "all", section: "", category: "", kind: v ? "any" : filter.kind })}>{label}</button>
            ))}
            <span className="tab-sep" aria-hidden />
            {[...groups.keys()].map((sec) => (
              <button key={sec} role="tab" aria-selected={tab === sec} className={`section ${tab === sec ? "on" : ""}`}
                onClick={() => setFilter({ ...filter, scope: "all", section: sec, category: "" })}>{sec}</button>
            ))}
        </div>

        {browsing ? (
          <div className="discover">
            {!filter.section && favCards.length > 0 && (
              <section className="shelf" aria-label="Favourites">
                <header><h3>Favourites</h3><button className="link" onClick={() => setFilter({ ...filter, scope: "favorites", section: "", category: "" })}>See all</button></header>
                <div className="rail">
                  {favCards.slice(0, 12).map((c) => (
                    <DesignCard key={c.key} card={c} layout={layout} onOpen={open} active={c.shown[0].id === current} applying={c.shown[0].id === applying} open={false} />
                  ))}
                </div>
              </section>
            )}
            {shelves.map((s) => (
              <section key={s.section} className="shelf" aria-label={s.section}>
                <header><h3>{s.section}</h3><span className="count">{s.tiles.length} {s.tiles.length === 1 ? "collection" : "collections"}</span></header>
                <div className="tiles">
                  {s.tiles.map((t) => (
                    <CollectionTile key={t.name} name={t.name} families={t.families} layout={layout}
                      badge={badgeOf(t.name)}
                      onOpen={(name) => setFilter({ ...filter, section: s.section, category: name })} />
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : (
          <>
            <div className="results-head">
              {filter.category && (
                <button className="back" onClick={() => setFilter({ ...filter, category: "" })}><Icon name="left" size={15} />{filter.section || "Discover"}</button>
              )}
              <h2>{filter.category || (filter.scope === "favorites" ? "Favourites" : filter.scope === "mine" ? "Mine" : filter.hue ? `${filter.hue} designs` : "Results")}</h2>
              <span className="count">{cards.length.toLocaleString()} {filter.scope === "favorites" ? (cards.length === 1 ? "favourite" : "favourites") : cards.length === 1 ? "design" : "designs"}</span>
              {filter.scope === "mine" && (
                <span className="results-actions">
                  <button className="btn small" title="Add a profile file or a design pack (you can also drop it on the window)" onClick={uploadDialog}>
                    <Icon name="upload" size={14} />Upload
                  </button>
                  {every.some((p) => p.source === "user") && (
                    <button className="btn small" title="Put your designs in one file, to give away or sell" onClick={() => setMaking(true)}>
                      <Icon name="pack" size={14} />Make a pack
                    </button>
                  )}
                </span>
              )}
            </div>
            {filter.category && packOf(filter.category) && !packOf(filter.category)!.bundled && <PackLine pack={packOf(filter.category)!} onRemoved={() => setFilter({ ...filter, category: "" })} />}
            {cards.length === 0 ? (
              filter.scope === "mine" && !filter.query
                ? <div className="empty"><h3>Nothing of yours yet</h3><p>Make one in Create, or upload a profile file or a design pack (or drop it on the window).</p></div>
                : filter.scope === "favorites" && !filter.query
                  ? <div className="empty"><h3>No favourites yet</h3><p>Open a design and press ★ to keep it one click away, here and in the tray.</p></div>
                  : <div className="empty"><h3>Nothing matches</h3><p>Try another word or colour, or clear the filters.</p></div>
            ) : (
              <div ref={grid} className="grid" onKeyDown={(e) => moveFocus(e, search.current)}>
                {cards.slice(0, limit).map((c) => (
                  <DesignCard key={c.key} card={c} layout={layout} onOpen={open}
                    active={c.single ? c.shown[0].id === current : onKeysFamily === c.family}
                    applying={c.single ? c.shown[0].id === applying : applyingFamily === c.family}
                    open={openFamily === c.family && (!c.single || c.shown[0].id === view.selected)} />
                ))}
              </div>
            )}
            {limit < cards.length && <div ref={sentinel} className="sentinel"><button className="btn ghost" onClick={() => setLimit(limit + PAGE)}>Show more</button></div>}
          </>
        )}
      </div>
      <Inspector family={openFamily} picked={view.selected} layout={layout} onSurprise={surprise} onKeyboard={current} />
      {making && <MakePack onClose={() => setMaking(false)} />}
    </div>
  );
}
