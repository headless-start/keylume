//! Profiles and app settings on disk.
//!
//! Layout of the data directory:
//! ```text
//! <dir>/settings.json          app settings + favourites
//! <dir>/profiles/<id>.json     one file per user profile
//! <dir>/packs/<id>.keylumepack design packs added
//! <dir>/maker.json             the user's maker key and what they put in their packs
//! ```
//! Built-in profiles are regenerated at start and never written. Writes are atomic
//! (temp file + rename) so a crash can't leave a half-written profile.
//!
//! Everything that comes in from outside is checked here: uploads, saves, settings from
//! the window, and the files themselves on every start. A file that can't be read is
//! never deleted or overwritten: it's left where it is (a broken settings file is kept
//! aside as `settings.corrupt-<time>.json`) and reported through [`Store::notices`].

use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use keylume_profiles::{builtin, fit, is_key_id, pack_profiles, Lighting, Profile, Source};

use crate::packs::{self, Licence, MakeRequest, Pack, PackInfo};
use keylume_proto::Layout;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Profile ids, in the order the tray / hotkeys cycle through them.
    pub favorites: Vec<String>,
    pub last_profile: Option<String>,
    /// Picture layer (0-based) that per-key profiles are written into.
    pub live_layer: u8,
    /// Correct colours sent to the board for the LEDs' linear response (screen colours
    /// are gamma-encoded, the LEDs aren't); mid-tones look washed out on the keys without it.
    pub true_colors: bool,
    /// Use the simulator when no keyboard is connected.
    pub demo_mode: bool,
    /// Which keyboard the simulator plays.
    pub demo_board: crate::service::Demo,
    pub start_minimized: bool,
    pub launch_at_login: bool,
    /// Ctrl+Alt+→ / ← cycle favourites, Ctrl+Alt+L toggles lights.
    pub hotkeys: bool,
    /// Re-apply the last profile when the keyboard (re)connects.
    pub restore_on_connect: bool,
    /// The side light strip follows whatever the keys show.
    pub side_follow: bool,
    /// What the side strip shows when it doesn't follow.
    pub side_custom: keylume_proto::SideLight,
    /// Rotate through the favourites every N minutes (0 = off).
    pub shuffle_minutes: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            favorites: vec![
                "deep-ocean-cascade".into(),
                "deep-ocean-flame".into(),
                "live-ocean-flow".into(),
                "fx-ripple-cyan".into(),
                "midnight-gamer".into(),
            ],
            last_profile: None,
            live_layer: 2,
            true_colors: true,
            demo_mode: true,
            demo_board: crate::service::Demo::Tk68,
            start_minimized: false,
            launch_at_login: false,
            hotkeys: true,
            restore_on_connect: true,
            side_follow: true,
            side_custom: keylume_proto::SideLight::new(keylume_proto::SideMode::Breathing, keylume_proto::Rgb(0x1F, 0x45, 0xFF)),
            shuffle_minutes: 0,
        }
    }
}

/// Most favourites kept.
pub const MAX_FAVORITES: usize = 1000;
/// Longest shuffle interval, in minutes (a day).
pub const MAX_SHUFFLE_MINUTES: u32 = 24 * 60;
const MAX_ID: usize = 120;

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && !id.chars().any(char::is_control)
}

impl AppSettings {
    /// Settings from the window must make sense as they are. Err says what doesn't.
    pub fn validate(&self) -> Result<(), String> {
        if self.favorites.len() > MAX_FAVORITES || !self.favorites.iter().all(|f| id_ok(f)) {
            return Err(format!("up to {MAX_FAVORITES} favourites, each a profile id"));
        }
        if self.last_profile.as_deref().is_some_and(|id| !id_ok(id)) {
            return Err("the last profile isn't a profile id".into());
        }
        if self.live_layer > 2 {
            return Err("the per-key layer is 1, 2 or 3".into());
        }
        if self.shuffle_minutes > MAX_SHUFFLE_MINUTES {
            return Err(format!("shuffle at most every {MAX_SHUFFLE_MINUTES} minutes"));
        }
        self.side_custom.validate().map_err(|e| format!("side light: {e}"))
    }

    /// Settings from disk: repair what doesn't make sense. Returns what was repaired.
    fn repair(&mut self) -> Vec<&'static str> {
        let mut fixed = Vec::new();
        let d = AppSettings::default();
        let mut seen = HashSet::new();
        let before = self.favorites.len();
        self.favorites.retain(|f| id_ok(f) && seen.insert(f.clone()));
        self.favorites.truncate(MAX_FAVORITES);
        if self.favorites.len() != before {
            fixed.push("favourites");
        }
        if self.last_profile.as_deref().is_some_and(|id| !id_ok(id)) {
            self.last_profile = None;
            fixed.push("last profile");
        }
        if self.live_layer > 2 {
            self.live_layer = d.live_layer;
            fixed.push("per-key layer");
        }
        if self.shuffle_minutes > MAX_SHUFFLE_MINUTES {
            self.shuffle_minutes = 0;
            fixed.push("shuffle");
        }
        if self.side_custom.validate().is_err() {
            self.side_custom = d.side_custom;
            fixed.push("side light");
        }
        fixed
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("i/o: {0}")]
    Io(#[from] io::Error),
    #[error("bad json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("profile {0:?} not found")]
    NotFound(String),
    #[error("built-in profiles can't be modified; save a copy instead")]
    Builtin,
    /// Something from outside (an upload, the window) that Keylume won't store.
    #[error("{0}")]
    Invalid(String),
}

/// The user as a maker of packs (`maker.json`): the key their packs are signed with, and
/// what they put in the last one, to start the next from.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Maker {
    /// Their name as their packs say it.
    pub name: String,
    /// Their page (https), or empty.
    pub url: String,
    pub licence: Licence,
    /// The packs they made, newest last.
    pub made: Vec<Made>,
    /// The secret signing key, in hex (never sent to the window).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key: String,
}

/// A pack the user made.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Made {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
}

pub struct Store {
    dir: PathBuf,
    layout: Layout,
    /// Design packs the user added (`packs/*.keylumepack`), in file-name order.
    packs: Vec<Pack>,
    /// The built-in library and every pack's designs, drawn for `layout`.
    builtin: Vec<Profile>,
    user: Vec<Profile>,
    settings: AppSettings,
    /// Names of profile files that couldn't be loaded: never written over.
    reserved: HashSet<String>,
    /// Problems found while loading that the user should hear about.
    notices: Vec<String>,
}

/// A settings file is a few KB.
const MAX_SETTINGS_BYTES: u64 = 1 << 20;
/// A per-key profile is about 3 KB.
const MAX_PROFILE_BYTES: u64 = 1 << 20;
/// Most bytes one uploaded file may have.
pub const MAX_UPLOAD_BYTES: u64 = 4 << 20;
/// Most profiles one uploaded file may add.
const MAX_UPLOAD: usize = 500;

/// Read at most `max` bytes of `path`; Err(InvalidData) when it's bigger.
fn read_bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("larger than {} KB", max >> 10)));
    }
    Ok(bytes)
}

#[cfg(test)]
thread_local! {
    /// Tests: how many more writes succeed before one fails (None: all succeed).
    static WRITES_LEFT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Write `data` to `path` via a temporary file beside it, so the file is either the
/// old one or the new one, never half of each.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = temp_path(path);
    write_file(&tmp, data)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

fn write_file(path: &Path, data: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if WRITES_LEFT.with(|w| match w.get() {
        Some(0) => {
            w.set(None);
            true
        }
        Some(n) => {
            w.set(Some(n - 1));
            false
        }
        None => false,
    }) {
        return Err(io::Error::other("disk full (test)"));
    }
    use std::io::Write;
    let mut f = fs::File::create(path)?;
    f.write_all(data)?;
    f.sync_all()
}

/// The date for file names, UTC: `2026-09-23`.
pub fn stamp_date() -> String {
    let s = stamp();
    format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..8])
}

/// Date and time for file names, UTC: `20260923-181502`.
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", rest / 3600, rest / 60 % 60, rest % 60)
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        "profile".into()
    } else {
        s
    }
}

/// A user profile id as Keylume makes them (`user-` and a slug): safe as a file name.
pub fn is_user_id(id: &str) -> bool {
    id.len() <= MAX_ID
        && id.strip_prefix("user-").is_some_and(|rest| {
            !rest.is_empty()
                && !rest.starts_with('-')
                && !rest.ends_with('-')
                && rest.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Load settings, repairing what can be and keeping a copy of a file that needed it.
fn load_settings(dir: &Path, notices: &mut Vec<String>) -> Result<AppSettings, StoreError> {
    let path = dir.join("settings.json");
    let bytes = match read_bounded(&path, MAX_SETTINGS_BYTES) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AppSettings::default()),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => Vec::new(), // too big: treated as unreadable
        Err(e) => return Err(e.into()),
    };
    let mut dropped: Vec<String> = Vec::new();
    let mut settings = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(serde_json::Value::Object(map)) => {
            // keep every field that reads on its own, so one bad value doesn't cost the rest
            let mut good = serde_json::Map::new();
            for (k, v) in map {
                let one = serde_json::Value::Object([(k.clone(), v.clone())].into_iter().collect());
                match serde_json::from_value::<AppSettings>(one) {
                    Ok(_) => drop(good.insert(k, v)),
                    Err(_) => dropped.push(k),
                }
            }
            serde_json::from_value(serde_json::Value::Object(good)).unwrap_or_default()
        }
        _ => {
            dropped.push("everything".into());
            AppSettings::default()
        }
    };
    let repaired = settings.repair();
    if !dropped.is_empty() || !repaired.is_empty() {
        let kept = dir.join(format!("settings.corrupt-{}.json", stamp()));
        let what = dropped.iter().map(String::as_str).chain(repaired.iter().copied()).collect::<Vec<_>>().join(", ");
        match fs::copy(&path, &kept) {
            Ok(_) => notices.push(format!(
                "Some settings couldn't be read ({what}) and went back to their defaults. The old file is kept as {}.",
                kept.display()
            )),
            Err(e) => notices
                .push(format!("Some settings couldn't be read ({what}) and went back to their defaults (the old file couldn't be copied: {e}).")),
        }
    }
    Ok(settings)
}

/// What an upload added.
#[derive(Debug)]
pub enum Imported {
    /// Ids of the new profiles.
    Profiles(Vec<String>),
    Pack(PackInfo),
}

/// The last keyboard's layout, remembered so the library opens drawn for it.
const LAYOUT_FILE: &str = "layout.json";
const MAX_LAYOUT_BYTES: u64 = 256 * 1024;

/// A remembered layout, if it's there and sane (anything else is ignored: the library is
/// simply drawn for the default until a keyboard connects).
fn load_layout(dir: &Path) -> Option<Layout> {
    let bytes = read_bounded(&dir.join(LAYOUT_FILE), MAX_LAYOUT_BYTES).ok()?;
    let l: Layout = serde_json::from_slice(&bytes).ok()?;
    let sane = !l.keys.is_empty()
        && l.keys.len() <= 512
        && l.slots <= 1024
        && l.width.is_finite()
        && l.height.is_finite()
        && l.keys.iter().all(|k| is_key_id(&k.id) && k.slot < l.slots.max(1) && [k.x, k.y, k.w, k.h].iter().all(|v| v.is_finite()));
    sane.then_some(l)
}

/// Load user profiles. Files that don't load stay where they are, reported and reserved.
fn load_profiles(dir: &Path, notices: &mut Vec<String>) -> Result<(Vec<Profile>, HashSet<String>), StoreError> {
    let any_board = &keylume_proto::standard::every_key();
    let mut user = Vec::new();
    let mut reserved = HashSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let loaded = (|| -> Result<Profile, String> {
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                return Err("it isn't a regular file".into());
            }
            if !is_user_id(&stem) {
                return Err("its name isn't a Keylume profile id".into());
            }
            let bytes = read_bounded(&path, MAX_PROFILE_BYTES).map_err(|e| e.to_string())?;
            let mut p: Profile = serde_json::from_slice(&bytes).map_err(|e| format!("it isn't a profile ({e})"))?;
            if p.id != stem {
                return Err(format!("it says its id is {:?}", p.id));
            }
            // any board's keys: a design made on one keyboard is kept when another connects
            p.validate(any_board)?;
            p.source = Source::User;
            p.section.clear();
            Ok(p)
        })();
        match loaded {
            Ok(p) => user.push(p),
            Err(why) => {
                reserved.insert(stem.to_lowercase());
                notices.push(format!("The profile file {} couldn't be loaded ({why}). It's been left as it is.", path.display()));
            }
        }
    }
    user.sort_by_key(|p| p.name.to_lowercase());
    Ok((user, reserved))
}

impl Store {
    /// Open the data folder. The library is drawn for the keyboard seen last
    /// (`layout.json`), else for `layout`.
    pub fn open(dir: impl Into<PathBuf>, layout: &Layout) -> Result<Store, StoreError> {
        let dir = dir.into();
        fs::create_dir_all(dir.join("profiles"))?;
        let mut notices = Vec::new();
        let settings = load_settings(&dir, &mut notices)?;
        let (user, reserved) = load_profiles(&dir.join("profiles"), &mut notices)?;
        let layout = load_layout(&dir).unwrap_or_else(|| layout.clone());
        let mut store = Store { dir, layout, packs: Vec::new(), builtin: Vec::new(), user, settings, reserved, notices };
        store.render();
        store.load_packs()?;
        store.migrate_retired_ids()?;
        Ok(store)
    }

    /// Draw the built-in library and the packs' designs for the current layout.
    fn render(&mut self) {
        let mut b = builtin(&self.layout);
        b.extend(signature_profiles(&self.layout));
        b.extend(self.pack_designs());
        self.builtin = b;
    }

    /// Every pack's designs, drawn for the current layout: its themes' looks, and its
    /// designs made in Keylume with only the keys this keyboard has (a per-key design with
    /// none of them left out).
    fn pack_designs(&self) -> Vec<Profile> {
        let collections: Vec<_> = self.packs.iter().flat_map(|p| p.collections.iter().copied()).collect();
        let mut out = pack_profiles(&self.layout, &collections);
        let mut made: Vec<Profile> = self.packs.iter().flat_map(|p| p.designs.iter().cloned()).collect();
        fit(&self.layout, &mut made);
        out.extend(made.into_iter().filter(|p| !matches!(&p.lighting, Lighting::PerKey { keys, .. } if keys.is_empty())));
        out
    }

    fn packs_dir(&self) -> PathBuf {
        self.dir.join("packs")
    }

    /// Ids and names in use, apart from those of pack `except` (the one being updated).
    fn names_in_use(&self, except: Option<&str>) -> HashSet<String> {
        let skip: HashSet<&str> = self
            .packs
            .iter()
            .filter(|p| Some(p.info.id.as_str()) == except)
            .flat_map(|p| p.info.collections.iter().map(String::as_str))
            .collect();
        // a pack's collection names are its own (no other collection may use them)
        packs::taken_by(self.builtin.iter().filter(|p| !skip.contains(p.category.as_str())))
    }

    /// Load the packs: the ones that come with Keylume, then the ones added. A file with a
    /// bundled pack's id replaces it (a newer version); one that can't be read is left
    /// where it is and reported. `builtin` must hold the built-in library only.
    fn load_packs(&mut self) -> Result<(), StoreError> {
        self.packs.clear();
        let mut taken = self.names_in_use(None);
        for p in packs::bundled(&taken) {
            match p {
                Ok(p) => {
                    taken.extend(p.names());
                    self.packs.push(p);
                }
                Err(why) => self.notices.push(format!("A pack that comes with Keylume couldn't be loaded ({why}).")),
            }
        }
        let mut files: Vec<PathBuf> = match fs::read_dir(self.packs_dir()) {
            Ok(d) => d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == packs::EXTENSION)).collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        files.sort();
        for path in files {
            let bytes = match read_bounded(&path, packs::MAX_PACK_BYTES) {
                Ok(b) => b,
                Err(e) => {
                    self.notices.push(format!("The pack file {} couldn't be loaded ({e}). It's been left as it is.", path.display()));
                    continue;
                }
            };
            let id = serde_json::from_slice::<serde_json::Value>(&bytes).ok().and_then(|v| v["id"].as_str().map(str::to_string));
            let mut against = taken.clone();
            if let Some(old) = self.packs.iter().find(|p| Some(&p.info.id) == id.as_ref()) {
                if !old.info.bundled {
                    self.notices.push(format!("The pack file {} is a second copy of {:?}; it was left out.", path.display(), old.info.name));
                    continue;
                }
                for n in old.names() {
                    against.remove(&n);
                }
            }
            let parsed = packs::parse(&bytes, &against).and_then(|p| match self.packs.iter().find(|o| o.info.id == p.info.id) {
                Some(old) => packs::may_replace(old, &p).map(|_| p),
                None => Ok(p),
            });
            match parsed {
                Ok(p) => {
                    if let Some(i) = self.packs.iter().position(|o| o.info.id == p.info.id) {
                        for n in self.packs[i].names() {
                            taken.remove(&n);
                        }
                        self.packs.remove(i);
                    }
                    taken.extend(p.names());
                    self.packs.push(p);
                }
                Err(why) => self.notices.push(format!("The pack file {} couldn't be loaded ({why}). It's been left as it is.", path.display())),
            }
        }
        // the built-ins are drawn already: add the packs' designs to them
        let designs = self.pack_designs();
        self.builtin.extend(designs);
        Ok(())
    }

    /// The packs added, for the Library.
    pub fn packs(&self) -> Vec<PackInfo> {
        self.packs.iter().map(|p| p.info.clone()).collect()
    }

    /// Add a pack (or update one with the same id), checked in full before it's kept.
    pub fn install_pack(&mut self, bytes: &[u8]) -> Result<PackInfo, StoreError> {
        let id = serde_json::from_slice::<serde_json::Value>(bytes).ok().and_then(|v| v["id"].as_str().map(str::to_string));
        let pack = packs::parse(bytes, &self.names_in_use(id.as_deref())).map_err(StoreError::Invalid)?;
        if let Some(old) = self.packs.iter().find(|p| p.info.id == pack.info.id) {
            packs::may_replace(old, &pack).map_err(StoreError::Invalid)?;
        }
        fs::create_dir_all(self.packs_dir())?;
        write_atomic(&self.packs_dir().join(format!("{}.{}", pack.info.id, packs::EXTENSION)), bytes)?;
        let info = pack.info.clone();
        self.packs.retain(|p| p.info.id != info.id);
        self.packs.push(pack);
        self.packs.sort_by(|a, b| a.info.id.cmp(&b.info.id));
        self.render();
        Ok(info)
    }

    /// Remove an added pack and its designs. A pack that comes with Keylume stays (if a
    /// newer file had replaced it, the one that came with Keylume is back).
    pub fn remove_pack(&mut self, id: &str) -> Result<(), StoreError> {
        let pack = self.packs.iter().find(|p| p.info.id == id).ok_or_else(|| StoreError::Invalid(format!("no pack {id:?} is installed")))?;
        if pack.info.bundled {
            return Err(StoreError::Invalid(format!("{} comes with Keylume", pack.info.name)));
        }
        match fs::remove_file(self.packs_dir().join(format!("{id}.{}", packs::EXTENSION))) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        self.packs.clear();
        self.render();
        self.load_packs()
    }

    /// What the user put in the packs they made before, to start the next one from.
    pub fn maker(&self) -> Maker {
        let mut m = self.maker_file().unwrap_or_default();
        m.key.clear();
        m
    }

    fn maker_file(&self) -> Option<Maker> {
        let bytes = read_bounded(&self.dir.join("maker.json"), MAX_SETTINGS_BYTES).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Make a pack of the user's own designs (`req.designs`, ids of their profiles),
    /// signed with their maker key (made the first time). Returns the pack and the file's
    /// bytes; nothing is kept until [`Store::made_pack`] says the file was saved.
    pub fn make_pack(&mut self, req: &MakeRequest) -> Result<(PackInfo, Vec<u8>), StoreError> {
        let designs: Vec<Profile> = req
            .designs
            .iter()
            .map(|id| self.user.iter().find(|p| &p.id == id).cloned().ok_or_else(|| StoreError::NotFound(id.clone())))
            .collect::<Result<_, _>>()?;
        if designs.is_empty() {
            return Err(StoreError::Invalid("pick at least one of your designs".into()));
        }
        let mut maker = self.maker_file().unwrap_or_default();
        if maker.key.is_empty() {
            maker.key = packs::new_key().map_err(StoreError::Invalid)?.0;
            self.save_maker(&maker)?;
        }
        let (info, json) = packs::make(req, &designs, &maker.key).map_err(StoreError::Invalid)?;
        Ok((info, json.into_bytes()))
    }

    /// Remember a pack the user made and saved: what they filled in, for the next one.
    pub fn made_pack(&mut self, req: &MakeRequest, info: &PackInfo) -> Result<(), StoreError> {
        let mut maker = self.maker_file().unwrap_or_default();
        maker.name = req.maker.trim().to_string();
        maker.url = req.url.trim().to_string();
        maker.licence = req.licence;
        maker.made.retain(|m| m.id != info.id);
        maker
            .made
            .push(Made { id: info.id.clone(), name: info.name.clone(), version: info.version.clone(), description: info.description.clone() });
        self.save_maker(&maker)
    }

    fn save_maker(&self, maker: &Maker) -> Result<(), StoreError> {
        let path = self.dir.join("maker.json");
        write_atomic(&path, &serde_json::to_vec_pretty(maker)?)?;
        // the maker key signs packs as theirs: only this account may read it
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// The keyboard the library is drawn for.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Draw the library for another keyboard, and remember it for the next start.
    /// Returns false when the library already fits `layout`.
    pub fn set_layout(&mut self, layout: &Layout) -> Result<bool, StoreError> {
        if *layout == self.layout {
            return Ok(false);
        }
        self.rebuild(layout);
        write_atomic(&self.dir.join(LAYOUT_FILE), &serde_json::to_vec_pretty(layout).map_err(io::Error::other)?)?;
        Ok(true)
    }

    /// Problems found while loading (files kept aside, settings repaired), in words.
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    /// Favourites / last profile pointing at profiles an update retired move to their
    /// closest replacement (or drop, if there's none), so nothing silently breaks.
    fn migrate_retired_ids(&mut self) -> Result<(), StoreError> {
        let fix = |id: &str| -> Option<String> {
            if self.get(id).is_some() {
                return Some(id.to_string());
            }
            replacement(id).filter(|r| self.get(r).is_some())
        };
        let s = &self.settings;
        let favorites: Vec<String> = s.favorites.iter().filter_map(|f| fix(f)).fold(Vec::new(), |mut v, f| {
            if !v.contains(&f) {
                v.push(f);
            }
            v
        });
        let last = s.last_profile.as_deref().and_then(fix);
        if favorites != s.favorites || last != s.last_profile {
            self.update_settings(|s| {
                s.favorites = favorites;
                s.last_profile = last;
            })?;
        }
        Ok(())
    }

    /// Regenerate the built-in library for `layout`.
    pub fn rebuild(&mut self, layout: &Layout) {
        self.layout = layout.clone();
        self.render();
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// All profiles: user ones first, then built-ins.
    pub fn profiles(&self) -> Vec<Profile> {
        self.iter().cloned().collect()
    }

    /// Your own profiles, then the built-in library.
    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.user.iter().chain(self.builtin.iter())
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.user.iter().chain(self.builtin.iter()).find(|p| p.id == id)
    }

    fn taken(&self) -> HashSet<&str> {
        self.user
            .iter()
            .chain(self.builtin.iter())
            .map(|p| p.id.as_str())
            .chain(self.reserved.iter().map(String::as_str))
            .collect()
    }

    /// A fresh `user-…` id: not a profile's, not a file's that failed to load, and not
    /// in `also` (ids handed out a moment ago).
    fn unique_id(&self, base: &str, also: &HashSet<String>) -> String {
        let taken = self.taken();
        let free = |id: &str| !taken.contains(id) && !also.contains(id) && !self.profile_file(id).exists();
        let base = format!("user-{}", slug(base.trim_start_matches("user-")));
        let base: String = base.chars().take(MAX_ID - 8).collect::<String>().trim_end_matches('-').to_string();
        if free(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|id| free(id)).unwrap()
    }

    /// Where a user profile lives. Only ids Keylume makes get a path, so none escapes
    /// the profiles folder.
    fn profile_path(&self, id: &str) -> Result<PathBuf, StoreError> {
        if !is_user_id(id) {
            return Err(StoreError::Invalid(format!("{id:?} isn't a profile id Keylume makes")));
        }
        Ok(self.profile_file(id))
    }

    fn profile_file(&self, id: &str) -> PathBuf {
        self.dir.join("profiles").join(format!("{id}.json"))
    }

    /// A profile as it's stored: yours, outside the built-in sections, and checked.
    fn prepare(&self, mut p: Profile) -> Result<Profile, StoreError> {
        p.source = Source::User;
        p.section.clear();
        p.name = p.name.trim().to_string();
        if p.category.trim().is_empty() {
            p.category = "Mine".into();
        }
        let name = p.name.clone();
        p.validate(&self.layout).map_err(|why| StoreError::Invalid(format!("“{name}” can't be used: {why}")))?;
        Ok(p)
    }

    /// Create or update a user profile. New profiles (or copies of built-ins) get a
    /// fresh `user-…` id. Returns the saved profile.
    pub fn save(&mut self, p: Profile) -> Result<Profile, StoreError> {
        let mut p = self.prepare(p)?;
        if !self.user.iter().any(|u| u.id == p.id) {
            p.id = self.unique_id(if p.id.is_empty() { &p.name } else { &p.id }, &HashSet::new());
        }
        write_atomic(&self.profile_path(&p.id)?, &serde_json::to_vec_pretty(&p)?)?;
        match self.user.iter_mut().find(|u| u.id == p.id) {
            Some(u) => *u = p.clone(),
            None => self.user.push(p.clone()),
        }
        Ok(p)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), StoreError> {
        if self.builtin.iter().any(|p| p.id == id) {
            return Err(StoreError::Builtin);
        }
        let i = self.user.iter().position(|p| p.id == id).ok_or_else(|| StoreError::NotFound(id.into()))?;
        match fs::remove_file(self.profile_path(id)?) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        self.user.remove(i);
        self.settings.favorites.retain(|f| f != id);
        self.save_settings()?;
        Ok(())
    }

    /// [`Store::import`] a file, reading at most [`MAX_UPLOAD_BYTES`] of it; a design
    /// pack (`.keylumepack`) is added with [`Store::install_pack`] instead.
    pub fn import_file(&mut self, path: &Path) -> Result<Imported, StoreError> {
        let bytes = read_bounded(path, MAX_UPLOAD_BYTES).map_err(|e| match e.kind() {
            io::ErrorKind::InvalidData => StoreError::Invalid("that file is too big to be a Keylume profile or pack".into()),
            _ => StoreError::Io(e),
        })?;
        let json = String::from_utf8(bytes).map_err(|_| StoreError::Invalid("Keylume profiles and packs are JSON text files".into()))?;
        if serde_json::from_str::<serde_json::Value>(&json).is_ok_and(|v| v.get("keylumePack").is_some()) {
            return self.install_pack(json.as_bytes()).map(Imported::Pack);
        }
        self.import(&json).map(Imported::Profiles)
    }

    /// Add the profiles in a file the user made: a pack (`{"profiles": [...]}`) or a single
    /// profile. All or nothing, even if a write fails part-way, and each gets a fresh id,
    /// so an upload never overwrites anything. Returns the new ids.
    pub fn import(&mut self, json: &str) -> Result<Vec<String>, StoreError> {
        let bad = |e: serde_json::Error| StoreError::Invalid(format!("not a Keylume profile file ({e})"));
        let v: serde_json::Value = serde_json::from_str(json).map_err(bad)?;
        let items = match v.get("profiles") {
            Some(serde_json::Value::Array(a)) => a.clone(),
            Some(_) => return Err(StoreError::Invalid("\"profiles\" should be a list".into())),
            None => vec![v],
        };
        if items.is_empty() || items.len() > MAX_UPLOAD {
            return Err(StoreError::Invalid(format!("a file can hold 1 to {MAX_UPLOAD} profiles")));
        }
        let mut ids = HashSet::new();
        let mut list = Vec::with_capacity(items.len());
        for mut item in items {
            // hand-written files may leave these out
            if let Some(o) = item.as_object_mut() {
                o.insert("id".into(), "".into());
                o.entry("category").or_insert_with(|| "Mine".into());
            }
            let mut p = self.prepare(serde_json::from_value(item).map_err(bad)?)?;
            p.id = self.unique_id(&p.name, &ids);
            ids.insert(p.id.clone());
            list.push(p);
        }
        // every file written aside first, then all moved in; any failure undoes it all
        let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut placed: Vec<PathBuf> = Vec::new();
        let result = (|| -> Result<(), StoreError> {
            for p in &list {
                let path = self.profile_path(&p.id)?;
                let tmp = temp_path(&path);
                write_file(&tmp, &serde_json::to_vec_pretty(p)?)?;
                staged.push((tmp, path));
            }
            for (tmp, path) in &staged {
                fs::rename(tmp, path)?;
                placed.push(path.clone());
            }
            Ok(())
        })();
        if let Err(e) = result {
            for (tmp, _) in &staged {
                let _ = fs::remove_file(tmp);
            }
            for path in &placed {
                let _ = fs::remove_file(path);
            }
            return Err(e);
        }
        let new_ids = list.iter().map(|p| p.id.clone()).collect();
        self.user.extend(list);
        Ok(new_ids)
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    /// Settings from the window, checked before they're kept.
    pub fn set_settings(&mut self, s: AppSettings) -> Result<(), StoreError> {
        s.validate().map_err(StoreError::Invalid)?;
        self.settings = s;
        self.save_settings()
    }

    pub fn update_settings(&mut self, f: impl FnOnce(&mut AppSettings)) -> Result<(), StoreError> {
        f(&mut self.settings);
        self.save_settings()
    }

    fn save_settings(&self) -> Result<(), StoreError> {
        write_atomic(&self.dir.join("settings.json"), &serde_json::to_vec_pretty(&self.settings)?)?;
        Ok(())
    }

    /// Favourite after/before the current one (for tray and hotkeys).
    pub fn cycle_favorite(&self, current: Option<&str>, step: i32) -> Option<String> {
        let favs: Vec<&String> = self.settings.favorites.iter().filter(|f| self.get(f).is_some()).collect();
        if favs.is_empty() {
            return None;
        }
        let n = favs.len() as i32;
        let i = current.and_then(|c| favs.iter().position(|f| f.as_str() == c)).map(|i| i as i32).unwrap_or(-1);
        let next = if i < 0 {
            if step > 0 {
                0
            } else {
                n - 1
            }
        } else {
            (i + step).rem_euclid(n)
        };
        Some(favs[next as usize].clone())
    }
}

/// Where a retired built-in id went, so favourites and the last look keep working after an
/// update. Built-in ids are stable: when one must go all the same, it gets a line in
/// `RENAMED` (old id, new id).
fn replacement(id: &str) -> Option<String> {
    const RENAMED: &[(&str, &str)] = &[
        // 0.5.3: the effect that followed a live match left with the game link
        ("live-cs2-live", "live-c4-countdown"),
    ];
    if let Some((_, to)) = RENAMED.iter().find(|(from, _)| *from == id) {
        return Some(to.to_string());
    }
    // A theme's or flag's extra animation mode or live kind is picked to avoid duplicating
    // one already in the library (see keylume-profiles::themes), so which one it lands on
    // can change between releases; an id that encodes the choice then falls back to the
    // plain extra, which always exists.
    if let Some((prefix, _mode)) = id.rsplit_once("-anim-") {
        return Some(format!("{prefix}-animated"));
    }
    if let Some((prefix, _kind)) = id.rsplit_once("-live-") {
        return Some(format!("{prefix}-live"));
    }
    None
}

/// The hand-designed blue profiles from the very first session, kept as signatures.
fn signature_profiles(layout: &Layout) -> Vec<Profile> {
    use keylume_profiles::Lighting;
    use std::collections::BTreeMap;
    let h = |s: &str| -> keylume_proto::Rgb { s.parse().unwrap() };
    let wasd = ["w", "a", "s", "d", "up", "down", "left", "right"];
    let support = ["q", "e", "r", "f", "z", "lshift", "lctrl", "space", "tab", "caps"];
    let nums = ["1", "2", "3", "4", "5"];
    let keys: BTreeMap<String, keylume_proto::Rgb> = layout
        .keys
        .iter()
        .map(|k| {
            let id = k.id.as_str();
            let c = if wasd.contains(&id) {
                h("#00ffff")
            } else if support.contains(&id) {
                h("#0060ff")
            } else if nums.contains(&id) {
                h("#0038c0")
            } else if id == "esc" {
                h("#7df9ff")
            } else {
                h("#001455")
            };
            (k.id.clone(), c)
        })
        .collect();
    vec![Profile {
        id: "midnight-gamer".into(),
        name: "Midnight Gamer".into(),
        category: "Blue".into(),
        tags: vec!["blue".into(), "gaming".into(), "signature".into()],
        description: "Dim midnight base; WASD and arrows blaze cyan, combat keys electric blue.".into(),
        source: Source::Builtin,
        section: keylume_profiles::section::COLOURS.to_string(),
        lighting: Lighting::PerKey { keys, brightness: 4 },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use keylume_profiles::section;
    use keylume_profiles::Lighting;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        (dir, s)
    }

    fn custom(name: &str) -> Profile {
        Profile {
            id: String::new(),
            name: name.into(),
            category: "Mine".into(),
            tags: vec![],
            description: String::new(),
            source: Source::User,
            section: String::new(),
            lighting: Lighting::Effect { effect: keylume_proto::Effect::new(keylume_proto::Mode::Breathing) },
        }
    }

    /// A pack like the real ones, built from one of the repo's pack folders (unsigned).
    fn sample_pack(folder: &str) -> Vec<u8> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs").join(folder);
        let mut v: serde_json::Value = serde_json::from_str(&fs::read_to_string(root.join("pack.json")).unwrap()).unwrap();
        let themes: serde_json::Value = serde_json::from_str(&fs::read_to_string(root.join("themes.json")).unwrap()).unwrap();
        v["keylumePack"] = packs::FORMAT.into();
        v["collections"] = vec![themes].into();
        serde_json::to_vec(&v).unwrap()
    }

    /// [`sample_pack`], signed by the publisher key test builds know: an official file.
    fn official_pack(folder: &str) -> Vec<u8> {
        packs::sign(&String::from_utf8(sample_pack(folder)).unwrap(), "test-publisher", packs::TEST_SECRET)
            .unwrap()
            .into_bytes()
    }

    /// A small unsigned pack of our own (not one of those that come with Keylume).
    fn small_pack(id: &str, theme: &str, name: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "keylumePack": 1, "id": id, "name": "Test Pack", "publisher": "Someone", "version": "1.0.0",
            "description": "A theme for the tests",
            "collections": [{ "collection": "Test Pack", "prefix": "tp", "themes": [{
                "id": theme, "name": name, "description": "Warm test colours for a test pack", "tags": ["test", "warm"],
                "stops": ["#ffb000", "#ff5000", "#c02000", "#801000"], "base": "#200800", "accent": "#ffffff",
                "animation": { "mode": "wave", "color": "#ff8000" }, "live": "flow" }] }]
        }))
        .unwrap()
    }

    #[test]
    fn packs_that_come_with_keylume_are_there_from_the_start() {
        let (_d, mut s) = store();
        let packs = s.packs();
        assert_eq!(packs.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["solar-terms", "lifes-moments"]);
        assert!(packs.iter().all(|p| p.bundled && p.official && p.publisher == "Keylume"));
        assert!(s.iter().any(|p| p.section == section::THEMES && p.category == "Solar Terms"), "a pack's collection sits under Themes");
        assert!(s.notices().is_empty(), "{:?}", s.notices());
        assert!(s.remove_pack("solar-terms").unwrap_err().to_string().contains("comes with Keylume"));
    }

    #[test]
    fn packs_install_reload_follow_the_keyboard_and_go_away() {
        let (dir, mut s) = store();
        let before = s.iter().count();
        let info = s.install_pack(&small_pack("test-pack", "tp-ember-dune", "Ember Dune")).unwrap();
        assert_eq!((info.id.as_str(), info.themes, info.official, info.bundled), ("test-pack", 1, false, false));
        let designs: Vec<&Profile> = s.iter().filter(|p| p.category == "Test Pack").collect();
        assert!(designs.len() > 30, "every look of the theme ({})", designs.len());
        assert!(designs.iter().all(|p| p.section == section::THEMES));
        assert_eq!(s.iter().count(), before + designs.len());
        // kept on disk: a new start finds it again, beside the bundled ones
        let mut s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.packs().len(), 3);
        assert!(s.notices().is_empty(), "{:?}", s.notices());
        // drawn for the keyboard that's connected, like the built-ins
        let full = keylume_proto::standard::standard("ansi-full").unwrap();
        assert!(s.set_layout(&full).unwrap());
        let p = s.iter().find(|p| p.category == "Test Pack" && matches!(p.lighting, keylume_profiles::Lighting::PerKey { .. })).unwrap();
        let keylume_profiles::Lighting::PerKey { keys, .. } = &p.lighting else { unreachable!() };
        assert!(keys.contains_key("kp5"), "the keypad is lit on a full-size board");
        // installing it again updates it rather than clashing with itself
        assert!(s.install_pack(&small_pack("test-pack", "tp-ember-dune", "Ember Dune")).is_ok());
        assert_eq!(s.packs().len(), 3);
        // another pack may not reuse its names, nor those of a pack that comes with Keylume
        let err = s.install_pack(&small_pack("copycat", "tp-ember-dune-2", "Ember Dune")).unwrap_err().to_string();
        assert!(err.contains("already used"), "{err}");
        let err = s.install_pack(&small_pack("copycat", "tp-spring", "Beginning of Spring")).unwrap_err().to_string();
        assert!(err.contains("already used"), "{err}");
        assert!(!dir.path().join("packs/copycat.keylumepack").exists(), "a refused pack isn't kept");
        s.remove_pack("test-pack").unwrap();
        assert!(s.iter().all(|p| p.category != "Test Pack"));
        assert!(!dir.path().join("packs/test-pack.keylumepack").exists());
        assert!(s.remove_pack("test-pack").is_err());
        assert_eq!(s.packs().len(), 2);
    }

    #[test]
    fn a_newer_file_replaces_a_bundled_pack_until_it_is_removed() {
        let (dir, mut s) = store();
        // only an official file may: anyone could write one that claims the same id
        let err = s.install_pack(&sample_pack("solar-terms")).unwrap_err().to_string();
        assert!(err.contains("only an official file"), "{err}");
        fs::create_dir_all(dir.path().join("packs")).unwrap();
        fs::write(dir.path().join("packs/solar-terms.keylumepack"), sample_pack("solar-terms")).unwrap();
        let s2 = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert!(s2.packs().iter().find(|p| p.id == "solar-terms").unwrap().bundled, "an unsigned copy put in the folder is left out");
        assert!(s2.notices()[0].contains("only an official file"), "{:?}", s2.notices());
        let info = s.install_pack(&official_pack("solar-terms")).unwrap();
        assert!(!info.bundled && info.official, "an official file");
        assert_eq!(s.packs().iter().filter(|p| p.id == "solar-terms").count(), 1);
        let again = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert!(!again.packs().iter().find(|p| p.id == "solar-terms").unwrap().bundled, "the file wins at the next start too");
        assert!(again.notices().is_empty(), "{:?}", again.notices());
        s.remove_pack("solar-terms").unwrap();
        assert!(s.packs().iter().find(|p| p.id == "solar-terms").unwrap().bundled, "the one that came with Keylume is back");
        assert!(s.iter().any(|p| p.category == "Solar Terms"));
    }

    #[test]
    fn a_pack_of_your_own_designs_is_signed_as_yours_and_updates_only_from_you() {
        let (dir, mut me) = store();
        let mut paint = custom("Sunset Keys");
        paint.lighting = Lighting::PerKey {
            keys: [("q".into(), keylume_proto::Rgb(255, 120, 0)), ("kp5".into(), keylume_proto::Rgb(0, 80, 255))].into(),
            brightness: 4,
        };
        let a = me.save(paint).unwrap().id;
        let b = me.save(custom("Calm")).unwrap().id;
        let req = MakeRequest {
            name: "Neon Nights".into(),
            maker: "PlayerOne".into(),
            version: "1.0".into(),
            description: "Two of mine".into(),
            licence: Licence::Share,
            url: "https://example.com".into(),
            designs: vec![a.clone(), b.clone()],
        };
        let (info, bytes) = me.make_pack(&req).unwrap();
        assert_eq!((info.designs, info.official, info.maker_key.is_some()), (2, false, true));
        assert!(me.maker().made.is_empty(), "nothing is remembered until the file is saved");
        me.made_pack(&req, &info).unwrap();
        let maker = me.maker();
        assert_eq!((maker.name.as_str(), maker.url.as_str(), maker.made.len()), ("PlayerOne", "https://example.com", 1));
        assert!(maker.key.is_empty(), "the key never leaves the store");
        let saved = fs::read_to_string(dir.path().join("maker.json")).unwrap();
        assert!(saved.contains("\"key\""), "the key is kept for the next pack");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(dir.path().join("maker.json")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // someone else adds it: the designs are under Packs, drawn for their keyboard
        let (_d2, mut them) = store();
        them.install_pack(&bytes).unwrap();
        let got: Vec<&Profile> = them.iter().filter(|p| p.category == "Neon Nights").collect();
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|p| p.section == section::THEMES && p.source == Source::Builtin));
        let Lighting::PerKey { keys, .. } = &got.iter().find(|p| p.name == "Sunset Keys").unwrap().lighting else { panic!() };
        assert_eq!(keys.len(), 1, "a TK68 has no keypad: only q is kept");
        // the maker's next version updates it; a pack of the same name by someone else is another pack
        let (newer, bytes2) = me.make_pack(&MakeRequest { version: "1.1".into(), ..req.clone() }).unwrap();
        assert_eq!(newer.id, info.id);
        assert_eq!(them.install_pack(&bytes2).unwrap().version, "1.1");
        let (_d3, mut other) = store();
        let c = other.save(custom("Calm")).unwrap().id;
        let (theirs, bytes3) = other.make_pack(&MakeRequest { designs: vec![c], ..req.clone() }).unwrap();
        assert_ne!(theirs.id, info.id);
        // …whose collection has the same name, so it can't sit beside the first
        assert!(them.install_pack(&bytes3).unwrap_err().to_string().contains("already in the library"));
        // and what isn't yours can't go in
        assert!(me.make_pack(&MakeRequest { designs: vec!["deep-ocean-cascade".into()], ..req.clone() }).is_err());
        assert!(me.make_pack(&MakeRequest { designs: vec![], ..req }).is_err());
    }

    #[test]
    fn a_broken_pack_file_is_left_alone_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("packs")).unwrap();
        fs::write(dir.path().join("packs/broken.keylumepack"), b"{ not json").unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert!(s.packs().iter().all(|p| p.bundled), "only the ones that come with Keylume");
        assert!(s.notices()[0].contains("broken.keylumepack"), "{:?}", s.notices());
        assert!(dir.path().join("packs/broken.keylumepack").exists());
    }

    #[test]
    fn uploads_tell_packs_from_profiles() {
        let (dir, mut s) = store();
        let file = dir.path().join("moments.keylumepack");
        fs::write(&file, official_pack("lifes-moments")).unwrap();
        assert!(matches!(s.import_file(&file).unwrap(), Imported::Pack(p) if p.id == "lifes-moments"));
        let profile = dir.path().join("mine.json");
        fs::write(&profile, serde_json::to_vec(&custom("Mine")).unwrap()).unwrap();
        assert!(matches!(s.import_file(&profile).unwrap(), Imported::Profiles(ids) if ids.len() == 1));
    }

    #[test]
    fn the_last_keyboard_s_layout_is_remembered() {
        let (dir, mut s) = store();
        let tkl = keylume_proto::standard::standard("ansi-tkl").unwrap();
        assert!(s.set_layout(&tkl).unwrap());
        assert!(!s.set_layout(&tkl).unwrap(), "same keyboard: nothing to redraw");
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.layout().id, "ansi-tkl");
        // a damaged file is ignored: the default layout is used
        fs::write(dir.path().join(LAYOUT_FILE), b"{\"keys\": 5}").unwrap();
        assert_eq!(Store::open(dir.path(), &Layout::tk68()).unwrap().layout().id, "epomaker-tk68");
    }

    #[test]
    fn library_includes_builtins_and_signatures() {
        let (_d, s) = store();
        assert!(s.profiles().len() > 650);
        assert!(s.get("midnight-gamer").is_some());
        for f in &s.settings().favorites {
            assert!(s.get(f).is_some(), "default favourite {f} must exist");
        }
    }

    #[test]
    fn retired_favourites_move_to_their_replacements() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"favorites":["deep-ocean-cascade","hero-web-line-anim-oldmode","nonsense"],
               "lastProfile":"hero-web-line-live-oldkind","gamerProfile":"cs-binds","ownerName":"someone"}"#,
        )
        .unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.settings().favorites, ["deep-ocean-cascade", "hero-web-line-animated"]);
        assert_eq!(s.settings().last_profile.as_deref(), Some("hero-web-line-live"));
        // and it's saved (retired fields such as ownerName and gamerProfile are dropped)
        let again = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(again.settings().favorites.len(), 2);
        let saved = fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(!saved.contains("ownerName") && !saved.contains("gamerProfile"));
    }

    #[test]
    fn unknown_anim_and_live_extra_ids_fall_back_to_the_plain_extra() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"favorites":["hero-web-line-anim-oldmode","hero-web-line-live-oldkind","nonsense-anim-wave"]}"#,
        )
        .unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        // the first two fall back to extras that do exist; the third's fallback doesn't,
        // so (like any other unknown id) it's just dropped
        assert_eq!(s.settings().favorites, ["hero-web-line-animated", "hero-web-line-live"]);
    }

    #[test]
    fn old_settings_files_gain_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), r#"{"favorites":["deep-ocean-flame"],"liveLayer":1}"#).unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.settings().favorites, ["deep-ocean-flame"]);
        assert_eq!(s.settings().live_layer, 1);
        assert!(s.settings().hotkeys);
        assert!(s.settings().true_colors, "true colours default on");
    }

    #[test]
    fn save_persists_and_reloads() {
        let (d, mut s) = store();
        let p = s.save(custom("My Blue Thing")).unwrap();
        assert_eq!(p.id, "user-my-blue-thing");
        let again = s.save(custom("My Blue Thing")).unwrap();
        assert_eq!(again.id, "user-my-blue-thing-2", "names may repeat, ids never do");
        let reopened = Store::open(d.path(), &Layout::tk68()).unwrap();
        assert_eq!(reopened.get("user-my-blue-thing").unwrap().name, "My Blue Thing");
        assert_eq!(reopened.profiles()[0].source, Source::User, "user profiles list first");
    }

    #[test]
    fn editing_a_builtin_saves_a_copy() {
        let (_d, mut s) = store();
        let mut p = s.get("deep-ocean-cascade").unwrap().clone();
        p.name = "Deeper Ocean".into();
        let saved = s.save(p).unwrap();
        assert!(saved.id.starts_with("user-"));
        assert_eq!(s.get("deep-ocean-cascade").unwrap().source, Source::Builtin);
        assert!(matches!(s.delete("deep-ocean-cascade"), Err(StoreError::Builtin)));
    }

    #[test]
    fn delete_removes_file_and_favourite() {
        let (d, mut s) = store();
        let p = s.save(custom("Temp")).unwrap();
        s.update_settings(|st| st.favorites.push(p.id.clone())).unwrap();
        s.delete(&p.id).unwrap();
        assert!(s.get(&p.id).is_none());
        assert!(!s.settings().favorites.contains(&p.id));
        assert!(!d.path().join("profiles").join(format!("{}.json", p.id)).exists());
    }

    #[test]
    fn uploads_add_packs_and_single_profiles_and_never_overwrite() {
        let (_d, mut s) = store();
        let p = s.save(custom("Shared")).unwrap();
        let pack = serde_json::json!({ "keylume": 1, "profiles": [p, s.get("deep-ocean-flame").unwrap()] }).to_string();
        let ids = s.import(&pack).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|i| i.starts_with("user-") && *i != p.id));
        assert_eq!(s.get(&ids[1]).unwrap().lighting, s.get("deep-ocean-flame").unwrap().lighting);
        assert_eq!(s.get(&ids[1]).unwrap().source, Source::User);

        // a hand-written single profile may leave out the id and category
        let one = r##"{ "name": "Hand Made", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##;
        let id = s.import(one).unwrap().remove(0);
        assert_eq!((s.get(&id).unwrap().name.as_str(), s.get(&id).unwrap().category.as_str()), ("Hand Made", "Mine"));
    }

    #[test]
    fn uploads_refuse_what_the_keyboard_cant_show() {
        let (_d, mut s) = store();
        let before = s.profiles().len();
        let fast = r##"{ "name": "Too Fast", "lighting": { "kind": "effect", "effect": { "mode": "wave", "speed": 9, "brightness": 4, "direction": 0, "rainbow": false, "color": "#ff0000" } } }"##;
        let good = r##"{ "name": "Fine", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##;
        for bad in [
            "not json".to_string(),
            r#"{ "profiles": [] }"#.into(),
            r##"{ "name": "", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##.into(),
            r#"{ "name": "Empty", "lighting": { "kind": "perKey", "keys": {} } }"#.into(),
            fast.into(),
            format!(r#"{{ "profiles": [{good}, {fast}] }}"#), // all or nothing
            r##"{ "name": "Bad Key", "lighting": { "kind": "perKey", "keys": { "../esc": "#ff0000" } } }"##.into(),
            r##"{ "name": "Long Spell", "lighting": { "kind": "spell", "background": "#000000", "words": [{ "text": "abcdefghijklmnopq", "color": "#ffffff" }] } }"##.into(),
            r#"{ "name": "No Colours", "lighting": { "kind": "live", "live": { "kind": "steps", "colors": [], "hold": 1 } } }"#.into(),
            format!(r#"{{ "name": "{}", "lighting": {{ "kind": "effect", "effect": {{ "mode": "wave" }} }} }}"#, "x".repeat(61)),
        ] {
            assert!(matches!(s.import(&bad), Err(StoreError::Invalid(_))), "{bad}");
        }
        assert_eq!(s.profiles().len(), before, "nothing was added");
    }

    #[test]
    fn unreadable_profile_files_are_kept_reported_and_never_overwritten() {
        let (d, s) = store();
        drop(s);
        let dir = d.path().join("profiles");
        fs::write(dir.join("broken.json"), "{ not json").unwrap();
        fs::write(dir.join("user-my-thing.json"), "{ not json either").unwrap();
        let mut s = Store::open(d.path(), &Layout::tk68()).unwrap();
        assert!(s.get("broken").is_none() && s.get("user-my-thing").is_none());
        assert_eq!(s.notices().len(), 2, "{:?}", s.notices());
        assert!(s.notices().iter().all(|n| n.contains("left as it is")));
        // a new profile with the same name gets another id: the broken file stays as it was
        let p = s.save(custom("My Thing")).unwrap();
        assert_eq!(p.id, "user-my-thing-2");
        assert_eq!(fs::read_to_string(dir.join("user-my-thing.json")).unwrap(), "{ not json either");
        let ids = s.import(r##"{ "name": "My Thing", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##).unwrap();
        assert_eq!(ids, ["user-my-thing-3"]);
    }

    #[test]
    fn stored_ids_cannot_reach_outside_the_profiles_folder() {
        let (d, s) = store();
        drop(s);
        let dir = d.path().join("profiles");
        let evil = r##"{ "id": "../settings", "name": "Evil", "category": "Mine", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##;
        fs::write(dir.join("user-evil.json"), evil).unwrap();
        let other =
            r##"{ "id": "user-someone-else", "name": "Other", "category": "Mine", "lighting": { "kind": "perKey", "keys": { "esc": "#ff0000" } } }"##;
        fs::write(dir.join("user-other.json"), other).unwrap();
        let mut s = Store::open(d.path(), &Layout::tk68()).unwrap();
        assert!(s.get("../settings").is_none() && s.get("user-someone-else").is_none(), "an id must match its file");
        assert_eq!(s.notices().len(), 2);
        // ids from the window never become paths either
        let mut p = custom("Escape");
        p.id = "../../settings".into();
        let saved = s.save(p).unwrap();
        assert!(is_user_id(&saved.id) && d.path().join("profiles").join(format!("{}.json", saved.id)).exists());
        assert!(matches!(s.delete("../settings"), Err(StoreError::NotFound(_))));
        let outside: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != "profiles" && n != "settings.json")
            .collect();
        assert!(outside.is_empty(), "{outside:?}");
    }

    #[test]
    fn a_broken_settings_file_is_kept_aside_and_the_rest_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "{ not json").unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.settings().live_layer, 2, "defaults");
        assert_eq!(s.notices().len(), 1);
        let kept: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("settings.corrupt-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(fs::read_to_string(dir.path().join(&kept[0])).unwrap(), "{ not json");

        // one bad value costs only itself
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"favorites":["deep-ocean-flame"],"liveLayer":"high","shuffleMinutes":999999,"hotkeys":false}"#,
        )
        .unwrap();
        let s = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert_eq!(s.settings().favorites, ["deep-ocean-flame"]);
        assert!(!s.settings().hotkeys);
        assert_eq!((s.settings().live_layer, s.settings().shuffle_minutes), (2, 0));
        assert!(s.notices()[0].contains("liveLayer") && s.notices()[0].contains("shuffle"), "{:?}", s.notices());
    }

    #[test]
    fn settings_from_the_window_are_checked() {
        let (dir, mut s) = store();
        for bad in [
            AppSettings { live_layer: 7, ..s.settings().clone() },
            AppSettings { shuffle_minutes: MAX_SHUFFLE_MINUTES + 1, ..s.settings().clone() },
            AppSettings { favorites: vec!["a\nb".into()], ..s.settings().clone() },
            AppSettings { side_custom: keylume_proto::SideLight { speed: 9, ..keylume_proto::SideLight::off() }, ..s.settings().clone() },
        ] {
            assert!(matches!(s.set_settings(bad), Err(StoreError::Invalid(_))));
        }
        s.set_settings(AppSettings { hotkeys: false, ..s.settings().clone() }).unwrap();
        assert!(!s.settings().hotkeys);
        // a field an older version kept is dropped, not carried along
        fs::write(dir.path().join("settings.json"), r#"{"hotkeys":false,"retiredField":"old"}"#).unwrap();
        let again = Store::open(dir.path(), &Layout::tk68()).unwrap();
        assert!(!again.settings().hotkeys);
        again.save_settings().unwrap();
        assert!(!fs::read_to_string(dir.path().join("settings.json")).unwrap().contains("retiredField"));
    }

    #[test]
    fn uploads_are_all_or_nothing_even_when_a_write_fails() {
        let (d, mut s) = store();
        let one = |n: &str| format!(r##"{{ "name": "{n}", "lighting": {{ "kind": "perKey", "keys": {{ "esc": "#ff0000" }} }} }}"##);
        let pack = format!(r#"{{ "profiles": [{}, {}, {}] }}"#, one("A"), one("B"), one("C"));
        let files = || fs::read_dir(d.path().join("profiles")).unwrap().count();
        WRITES_LEFT.with(|w| w.set(Some(2))); // the third file can't be written
        assert!(matches!(s.import(&pack), Err(StoreError::Io(_))));
        assert_eq!(files(), 0, "nothing is left behind");
        assert!(s.get("user-a").is_none());
        assert_eq!(s.import(&pack).unwrap(), ["user-a", "user-b", "user-c"]);
        assert_eq!(files(), 3);
    }

    #[test]
    fn uploads_read_at_most_a_few_megabytes() {
        let (d, mut s) = store();
        let big = d.path().join("big.json");
        fs::write(&big, vec![b' '; MAX_UPLOAD_BYTES as usize + 1]).unwrap();
        assert!(matches!(s.import_file(&big), Err(StoreError::Invalid(m)) if m.contains("too big")));
        let binary = d.path().join("binary.json");
        fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(s.import_file(&binary), Err(StoreError::Invalid(_))));
    }

    #[test]
    fn stamps_are_utc_dates() {
        let s = stamp();
        assert_eq!(s.len(), 15);
        assert!(s.starts_with("20") && s.as_bytes()[8] == b'-');
        let d = stamp_date();
        assert_eq!((d.len(), &d[..4]), (10, &s[..4]));
    }

    #[test]
    fn favourites_cycle_both_ways() {
        let (_d, s) = store();
        let favs = s.settings().favorites.clone();
        assert_eq!(s.cycle_favorite(None, 1).unwrap(), favs[0]);
        assert_eq!(s.cycle_favorite(Some(&favs[0]), 1).unwrap(), favs[1]);
        assert_eq!(s.cycle_favorite(Some(&favs[0]), -1).unwrap(), *favs.last().unwrap());
        assert_eq!(s.cycle_favorite(Some("not-a-fav"), 1).unwrap(), favs[0]);
    }
}
