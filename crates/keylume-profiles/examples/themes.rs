//! Check catalogue files before they join the library: theme and game collections in
//! `themes/`, flags in `flags/`.
//! `cargo run -q -p keylume-profiles --example themes -- crates/keylume-profiles/themes/heroes.json`
//! (no path: every file in both folders). Prints each problem, or a summary per file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use keylume_profiles::{builtin, flags, patterns, render, themes, Lighting};
use keylume_proto::{Layout, Rgb};

fn json_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
        .unwrap_or_default()
}

/// Every per-key design a theme or flags file makes, by profile id.
fn designs_of(json: &str, layout: &Layout) -> Vec<(String, BTreeMap<String, Rgb>)> {
    if let Ok(r) = serde_json::from_str::<flags::Region>(json) {
        return r.flags.iter().map(|f| (f.id.clone(), f.render(layout))).collect();
    }
    let Ok(c) = serde_json::from_str::<themes::Collection>(json) else { return Vec::new() };
    let c: &'static themes::Collection = Box::leak(Box::new(c));
    let mut out = Vec::new();
    for t in &c.themes {
        let pal = t.palette(&c.collection);
        for pat in patterns::all() {
            out.push((format!("{}-{}", t.id, pat.id), render(layout, &pat, &pal)));
        }
        if !c.keys.is_empty() {
            out.push((format!("{}-keys", t.id), themes::game_keys(layout, &c.keys, t)));
        }
    }
    out
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let on_disk: Vec<PathBuf> = [json_files(&root.join("themes")), json_files(&root.join("flags"))].concat();
    let mut files: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        files = on_disk.clone();
    }
    files.sort();
    let layout = Layout::tk68();
    let lib = builtin(&layout);
    // every file's designs, drawn once: (file, profile id, key colours in layout order)
    let drawn: Vec<(PathBuf, String, Vec<Rgb>)> = on_disk
        .iter()
        .flat_map(|f| {
            let text = std::fs::read_to_string(f).unwrap_or_default();
            designs_of(&text, &layout).into_iter().map(move |(id, keys)| (f.clone(), id, keys.values().copied().collect()))
        })
        .collect();
    let mut failed = false;
    for file in &files {
        let json = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        let collection = value["collection"].as_str().unwrap_or_default().to_string();
        let is_flags = value.get("flags").is_some();
        // names and ids used by the rest of the library and by the other catalogue files
        let mut taken: HashSet<String> = HashSet::new();
        for p in lib.iter().filter(|p| p.category != collection) {
            taken.insert(p.id.to_lowercase());
            taken.insert(p.name.split(" · ").next().unwrap_or(&p.name).to_lowercase());
        }
        for other in on_disk.iter().filter(|o| o.canonicalize().ok() != file.canonicalize().ok()) {
            let Ok(v) = std::fs::read_to_string(other).map(|s| serde_json::from_str::<serde_json::Value>(&s)) else { continue };
            let Ok(v) = v else {
                println!("(skipping {}: it isn't valid JSON right now)", other.display());
                continue;
            };
            let items = v["themes"].as_array().or(v["flags"].as_array()).cloned().unwrap_or_default();
            for t in items {
                for k in ["id", "name"] {
                    if let Some(s) = t[k].as_str() {
                        taken.insert(s.to_lowercase());
                    }
                }
            }
        }
        let designs = designs_of(&json, &layout);
        let (mut bad, count) = if is_flags {
            match flags::check(&json, &taken) {
                Ok(r) => (Vec::new(), r.flags.len()),
                Err(b) => (b, 0),
            }
        } else {
            let mut bad = themes::check(&json, &taken).err().unwrap_or_default();
            let coll = serde_json::from_str::<themes::Collection>(&json).ok();
            let count = coll.as_ref().map_or(0, |c| c.themes.len());
            if bad.is_empty() && count == 0 {
                bad.push("no themes".into());
            }
            // a theme named after a colour should actually show it somewhere
            if let Some(c) = &coll {
                for t in &c.themes {
                    let words = themes::color_words(&t.name);
                    if words.is_empty() {
                        continue;
                    }
                    let ranges: Vec<(f32, f32)> = words.iter().flat_map(|(_, r)| r.iter().copied()).collect();
                    let has = std::iter::once(t.accent).chain(t.stops.iter().copied()).any(|c| {
                        let (hue, chroma) = themes::hue_chroma(c);
                        chroma >= themes::REAL_COLOR_CHROMA && ranges.iter().any(|&(a, b)| (a..=b).contains(&hue))
                    });
                    if !has {
                        let names: Vec<&str> = words.iter().map(|(w, _)| *w).collect();
                        bad.push(format!(
                            "{} ({}): the name says {} but no stop or accent is actually that colour (chroma >= {})",
                            t.id,
                            t.name,
                            names.join("/"),
                            themes::REAL_COLOR_CHROMA
                        ));
                    }
                }
            }
            (bad, count)
        };
        // what the designs look like: not mostly dark, and not a copy of another (in the
        // library or in the other files, which may not be registered yet)
        let mut others: HashMap<Vec<Rgb>, String> = lib
            .iter()
            .filter(|p| p.category != collection)
            .filter_map(|p| match &p.lighting {
                Lighting::PerKey { keys, .. } => Some((keys.values().copied().collect(), p.id.clone())),
                _ => None,
            })
            .collect();
        for (other, id, sig) in &drawn {
            if other.canonicalize().ok() != file.canonicalize().ok() {
                others.entry(sig.clone()).or_insert_with(|| id.clone());
            }
        }
        let mut mine: HashMap<Vec<Rgb>, String> = HashMap::new();
        for (id, keys) in designs {
            let dark = keys.values().filter(|c| (c.0 as u32 + c.1 as u32 + c.2 as u32) < 18).count();
            if dark * 10 >= keys.len() * 6 {
                bad.push(format!("{id}: {dark} of {} keys are nearly off (brighten the colours)", keys.len()));
            }
            let sig: Vec<Rgb> = keys.values().copied().collect();
            if let Some(o) = others.get(&sig).filter(|o| **o != id) {
                bad.push(format!("{id} looks exactly like {o}"));
            } else if let Some(o) = mine.insert(sig, id.clone()) {
                bad.push(format!("{id} looks exactly like {o}"));
            }
        }
        if bad.is_empty() {
            let what = if is_flags { format!("{count} flags, {} profiles", count * 2) } else { format!("{count} themes") };
            println!("ok  {}: {what}", file.display());
        } else {
            failed = true;
            println!("BAD {}:", file.display());
            for b in &bad {
                println!("    {b}");
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
