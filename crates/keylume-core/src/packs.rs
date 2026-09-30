//! Design packs: designs shared or sold on their own, as `.keylumepack` files. A pack
//! holds theme collections in the catalogue's own format
//! (`crates/keylume-profiles/themes/*.json`), which the app draws for whatever keyboard is
//! connected like the built-in library, and designs made in Keylume (per-key, animated,
//! live, spells: the user's own profiles), listed under the pack's name.
//!
//! A pack may carry an Ed25519 signature over its canonical JSON (every object's keys
//! sorted, no spaces, the `signature` field left out). Signed with a key listed in
//! [`PUBLISHERS`] it counts as **official** and shows its publisher's name from that
//! list. Anything else is a **community** pack: unsigned, or signed by its maker's own key
//! (packs made in Keylume are; the key travels in the file). A signature that doesn't
//! match is refused: the file was changed after it was signed. An installed signed pack
//! is only ever updated by a file signed with the same key ([`may_replace`]).
//! Everything is checked offline; nothing is ever fetched.

use std::collections::HashSet;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use keylume_profiles::themes::{self, Collection};
use keylume_profiles::{section, Lighting, Profile, Source};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The file format this build reads and writes.
pub const FORMAT: u64 = 1;
pub const EXTENSION: &str = "keylumepack";
pub const MAX_PACK_BYTES: u64 = 4 * 1024 * 1024;
const MAX_COLLECTIONS: usize = 8;
const MAX_THEMES: usize = 200;
const MAX_DESIGNS: usize = 200;
const MAX_URL: usize = 200;

/// The packs that come with Keylume: each pack's `pack.json`, then its collections.
const BUNDLED: &[(&str, &[&str])] = &[
    (include_str!("../../../packs/solar-terms/pack.json"), &[include_str!("../../../packs/solar-terms/themes.json")]),
    (include_str!("../../../packs/lifes-moments/pack.json"), &[include_str!("../../../packs/lifes-moments/themes.json")]),
];

/// The packs that come with Keylume, checked like any other against `taken` (and official
/// by coming with the app: the app itself is what vouches for them).
pub fn bundled(taken: &HashSet<String>) -> Vec<Result<Pack, String>> {
    let mut taken = taken.clone();
    BUNDLED
        .iter()
        .map(|(manifest, files)| {
            let mut v: Value = serde_json::from_str(manifest).map_err(|e| format!("pack.json: {e}"))?;
            let collections = files.iter().map(|f| serde_json::from_str::<Value>(f).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
            v["keylumePack"] = FORMAT.into();
            v["collections"] = collections.into();
            let mut pack = parse_with(v.to_string().as_bytes(), &taken, &[])?;
            pack.info.official = true;
            pack.info.bundled = true;
            for name in pack.names() {
                taken.insert(name);
            }
            Ok(pack)
        })
        .collect()
}

/// Keys whose signatures make a pack official: (key id, publisher, public key in hex).
/// Private keys never live in the repository (docs/PACKS.md, *Signing*).
pub const PUBLISHERS: &[(&str, &str, &str)] = &[("keylume-2026", "Keylume", "200d3cfac43bc11e526ba5e4363e4ada8b7324c1d4cc6abdc5e3aa8c69d6688c")];

/// What its maker lets people do with a pack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Licence {
    /// Anyone may pass it on.
    #[default]
    Share,
    /// For the person who got it (a bought pack, say): please don't pass it on.
    Personal,
}

/// What the Library says about an installed pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    /// From the signing key when official, else what the file says.
    pub publisher: String,
    pub version: String,
    pub description: String,
    pub official: bool,
    /// Its collections' names, as the Library lists them (its designs are listed under
    /// the pack's own name).
    pub collections: Vec<String>,
    pub themes: usize,
    /// Designs made in Keylume.
    pub designs: usize,
    /// It comes with Keylume (it can be updated by a newer file, not removed).
    pub bundled: bool,
    /// Signed by its maker's own key: the key's fingerprint ("3F9A B2C1"), so two makers
    /// of the same name can be told apart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maker_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence: Option<Licence>,
    /// The maker's page (https): where to find more of their packs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A pack that passed every check.
#[derive(Debug)]
pub struct Pack {
    pub info: PackInfo,
    /// Parsed once and kept for the app's lifetime (the designs borrow from them).
    pub collections: Vec<&'static Collection>,
    /// Its designs made in Keylume, as library profiles (`<pack id>-<design id>`), with
    /// every key they name (the store fits them to the keyboard).
    pub designs: Vec<Profile>,
    /// The key that signed it: a publisher key's id, or a maker's public key in hex.
    pub signer: Option<String>,
}

impl Pack {
    /// Ids of every theme in the pack (profiles are `<theme id>-<look>`).
    pub fn theme_ids(&self) -> impl Iterator<Item = &str> {
        self.collections.iter().flat_map(|c| c.themes.iter().map(|t| t.id.as_str()))
    }

    /// What it takes in the library, as [`taken_by`] writes it: its themes' ids and names,
    /// its designs' ids and names, and its collections.
    pub fn names(&self) -> Vec<String> {
        let themes = self.collections.iter().flat_map(|c| &c.themes).flat_map(|t| [t.id.clone(), t.name.to_lowercase()]);
        let designs = self.designs.iter().flat_map(|p| [p.id.to_lowercase(), family(&p.name)]);
        let collections = self.info.collections.iter().map(|c| collection_key(c));
        themes.chain(designs).chain(collections).collect()
    }
}

/// Whether `new` may take the place of the installed `old` (same id): a pack that comes
/// with Keylume only by an official file, a signed pack only by a file with the same key.
pub fn may_replace(old: &Pack, new: &Pack) -> Result<(), String> {
    if old.info.bundled && !new.info.official {
        return Err(format!("only an official file can update {}, which comes with Keylume", old.info.name));
    }
    if old.signer.is_some() && old.signer != new.signer {
        return Err(format!("it isn't signed by whoever made the {} pack you have. If you trust this file, remove that pack first", old.info.name));
    }
    Ok(())
}

/// A design made in Keylume, as a pack carries it: the profile's name, words and
/// lighting, with an id of its own inside the pack.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Design {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub lighting: Lighting,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackFile {
    keylume_pack: u64,
    id: String,
    name: String,
    publisher: String,
    version: String,
    description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    collections: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    designs: Vec<Design>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    licence: Option<Licence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<SignatureField>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignatureField {
    /// Which key signed it: a key id from [`PUBLISHERS`], or the maker's own public key
    /// (64 hex digits).
    key: String,
    /// The Ed25519 signature, in hex.
    sig: String,
}

/// The bytes a signature covers: the pack without its `signature`, every object's keys
/// sorted, no whitespace. Independent of how the file was formatted.
pub fn canonical(pack: &Value) -> Vec<u8> {
    fn write(v: &Value, out: &mut String) {
        match v {
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(k).expect("strings serialise"));
                    out.push(':');
                    write(&map[k], out);
                }
                out.push('}');
            }
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(item, out);
                }
                out.push(']');
            }
            other => out.push_str(&serde_json::to_string(other).expect("scalars serialise")),
        }
    }
    let mut body = pack.clone();
    if let Value::Object(map) = &mut body {
        map.remove("signature");
    }
    let mut out = String::new();
    write(&body, &mut out);
    out.into_bytes()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// A new signing key: (secret key in hex, to keep private; public key in hex, for
/// [`PUBLISHERS`]).
pub fn new_key() -> Result<(String, String), String> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no secure random numbers: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    Ok((hex(&seed), hex(key.verifying_key().as_bytes())))
}

/// Sign a pack's JSON with a secret key (hex) under `key_id`; returns the signed JSON.
pub fn sign(json: &str, key_id: &str, secret_hex: &str) -> Result<String, String> {
    let secret = unhex::<32>(secret_hex.trim()).ok_or("the secret key should be 64 hex digits")?;
    let key = SigningKey::from_bytes(&secret);
    let mut value: Value = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let sig = key.sign(&canonical(&value));
    let field = serde_json::to_value(SignatureField { key: key_id.into(), sig: hex(&sig.to_bytes()) }).expect("serialises");
    value.as_object_mut().ok_or("a pack is a JSON object")?.insert("signature".into(), field);
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

/// Who signed a pack.
enum Signed {
    /// A key in [`PUBLISHERS`]: (key id, publisher).
    Publisher(String, String),
    /// Its maker's own key (public, in hex).
    Maker(String),
}

/// Who signed it: None when unsigned or signed by a key id Keylume doesn't know (it can't
/// be checked). Err when a signature doesn't match: the file was changed after signing.
fn signer(value: &Value, sig: Option<&SignatureField>, keys: &[(&str, &str, &str)]) -> Result<Option<Signed>, String> {
    let Some(sig) = sig else { return Ok(None) };
    let (public, signed) = if let Some((id, publisher, public)) = keys.iter().find(|(id, _, public)| *id == sig.key || *public == sig.key) {
        (public.to_string(), Signed::Publisher(id.to_string(), publisher.to_string()))
    } else if unhex::<32>(&sig.key).is_some() && sig.key == sig.key.to_lowercase() {
        (sig.key.clone(), Signed::Maker(sig.key.clone()))
    } else {
        return Ok(None);
    };
    let public = unhex::<32>(&public).and_then(|b| VerifyingKey::from_bytes(&b).ok()).ok_or("its signing key is malformed")?;
    let bytes = unhex::<64>(&sig.sig).ok_or("its signature is malformed")?;
    public
        .verify(&canonical(value), &Signature::from_bytes(&bytes))
        .map_err(|_| "its signature doesn't match: the file was changed after it was signed".to_string())?;
    Ok(Some(signed))
}

/// A maker key's fingerprint, as people compare it: "3F9A B2C1".
pub fn fingerprint(public_hex: &str) -> String {
    let h = public_hex.to_uppercase();
    format!("{} {}", &h[..4.min(h.len())], &h[4.min(h.len())..8.min(h.len())])
}

/// A design's family name as the library groups it (the part before " · "), lower-cased.
fn family(name: &str) -> String {
    name.split(" · ").next().unwrap_or(name).to_lowercase()
}

/// How a collection's name is kept among the ids and names in use.
fn collection_key(name: &str) -> String {
    format!("collection:{}", name.to_lowercase())
}

/// Ids, names (lower-cased, without the " · look" part) and collections that `profiles`
/// use: what a pack mustn't reuse.
pub fn taken_by<'a>(profiles: impl IntoIterator<Item = &'a keylume_profiles::Profile>) -> HashSet<String> {
    let mut taken = HashSet::new();
    for p in profiles {
        taken.insert(p.id.to_lowercase());
        taken.insert(family(&p.name));
        taken.insert(collection_key(&p.category));
    }
    taken
}

fn kebab(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn plain(what: &str, s: &str, max: usize) -> Result<(), String> {
    if s.trim().is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(format!("its {what} should be plain text of 1 to {max} characters"));
    }
    Ok(())
}

/// A maker's page: https, a host with a dot in it, no spaces, at most [`MAX_URL`] long.
pub fn valid_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    url.len() <= MAX_URL
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b':')
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '<' || c == '>' || c == '\\')
}

/// Read and check a pack. `taken` holds the ids, names (lower-cased) and collections
/// already used in the library ([`taken_by`]); every collection must pass the catalogue's
/// own checks against them, and every design the checks an upload gets.
pub fn parse(bytes: &[u8], taken: &HashSet<String>) -> Result<Pack, String> {
    parse_with(bytes, taken, publishers())
}

/// A publisher key only test builds know, so tests can make official packs.
#[cfg(test)]
pub(crate) const TEST_SECRET: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// [`PUBLISHERS`], plus the test key in test builds.
fn publishers() -> &'static [(&'static str, &'static str, &'static str)] {
    #[cfg(test)]
    {
        static KEYS: std::sync::OnceLock<Vec<(&str, &str, &str)>> = std::sync::OnceLock::new();
        return KEYS.get_or_init(|| {
            let public = hex(SigningKey::from_bytes(&unhex::<32>(TEST_SECRET).expect("hex")).verifying_key().as_bytes());
            let mut keys = PUBLISHERS.to_vec();
            keys.push(("test-publisher", "Keylume", Box::leak(public.into_boxed_str())));
            keys
        });
    }
    #[allow(unreachable_code)]
    PUBLISHERS
}

fn parse_with(bytes: &[u8], taken: &HashSet<String>, keys: &[(&str, &str, &str)]) -> Result<Pack, String> {
    if bytes.len() as u64 > MAX_PACK_BYTES {
        return Err(format!("it's larger than {} MB", MAX_PACK_BYTES / (1024 * 1024)));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "it isn't a Keylume pack (not JSON)".to_string())?;
    if value.get("keylumePack").is_none() {
        return Err("it isn't a Keylume pack".into());
    }
    let file: PackFile = serde_json::from_value(value.clone()).map_err(|e| format!("it isn't a valid pack ({e})"))?;
    if file.keylume_pack != FORMAT {
        return Err(format!("it's a format {} pack; this Keylume reads format {FORMAT}", file.keylume_pack));
    }
    if !kebab(&file.id) {
        return Err("its id should be lower-case letters, digits and dashes".into());
    }
    plain("name", &file.name, 40)?;
    plain("publisher", &file.publisher, 60)?;
    plain("version", &file.version, 20)?;
    plain("description", &file.description, 300)?;
    if file.collections.len() > MAX_COLLECTIONS || file.designs.len() > MAX_DESIGNS {
        return Err(format!("a pack holds up to {MAX_COLLECTIONS} collections of themes and {MAX_DESIGNS} designs"));
    }
    if file.collections.is_empty() && file.designs.is_empty() {
        return Err("it has no designs".into());
    }
    if let Some(url) = &file.url {
        if !valid_url(url) {
            return Err("its web page should be an https:// address".into());
        }
    }
    let signed = signer(&value, file.signature.as_ref(), keys)?;
    let mut collections = Vec::new();
    let mut taken = taken.clone();
    let mut themes = 0;
    let mut names = Vec::new();
    for c in &file.collections {
        let checked = themes::check(&c.to_string(), &taken)
            .map_err(|bad| format!("its collection {:?}: {}", c["collection"].as_str().unwrap_or("?"), bad.join("; ")))?;
        for t in &checked.themes {
            taken.insert(t.id.clone());
            taken.insert(t.name.to_lowercase());
        }
        themes += checked.themes.len();
        if themes > MAX_THEMES {
            return Err(format!("a pack has up to {MAX_THEMES} themes"));
        }
        names.push(checked.collection.clone());
        collections.push(&*Box::leak(Box::new(checked)));
    }
    // designs made in Keylume: checked as an upload is, on a keyboard with every key
    let every = keylume_proto::standard::every_key();
    let mut designs = Vec::new();
    let (mut ids, mut seen) = (HashSet::new(), HashSet::new());
    for d in &file.designs {
        let who = format!("its design {:?}", d.name);
        if !kebab(&d.id) || !ids.insert(d.id.as_str()) {
            return Err(format!("{who} needs an id of its own (lower-case letters, digits and dashes)"));
        }
        if !seen.insert(d.name.to_lowercase()) {
            return Err(format!("two of its designs are called {:?}", d.name));
        }
        let p = Profile {
            id: format!("{}-{}", file.id, d.id),
            name: d.name.trim().to_string(),
            category: file.name.clone(),
            tags: d.tags.clone(),
            description: d.description.clone(),
            source: Source::Builtin,
            section: section::THEMES.to_string(),
            lighting: d.lighting.clone(),
        };
        if taken.contains(&p.id) {
            return Err(format!("{who}: its id is already used in the library"));
        }
        p.validate(&every).map_err(|e| format!("{who}: {e}"))?;
        if let Some(word) = themes::not_ours(&format!("{} {} {}", p.name, p.description, p.tags.join(" "))).first() {
            return Err(format!("{who}: {word:?} is someone else's name; describe it in your own words"));
        }
        designs.push(p);
    }
    if !designs.is_empty() && !names.contains(&file.name) {
        names.push(file.name.clone());
    }
    for n in &names {
        if taken.contains(&collection_key(n)) || n.eq_ignore_ascii_case("mine") {
            return Err(format!("its collection {n:?} has the name of one already in the library"));
        }
    }
    let (official, publisher, maker_key, key) = match signed {
        Some(Signed::Publisher(id, publisher)) => (true, publisher, None, Some(id)),
        Some(Signed::Maker(public)) => (false, file.publisher, Some(fingerprint(&public)), Some(public)),
        None => (false, file.publisher, None, None),
    };
    let info = PackInfo {
        id: file.id,
        name: file.name,
        publisher,
        version: file.version,
        description: file.description,
        official,
        collections: names,
        themes,
        designs: designs.len(),
        bundled: false,
        maker_key,
        licence: file.licence,
        url: file.url,
    };
    Ok(Pack { info, collections, designs, signer: key })
}

/// What a maker fills in to make a pack of their own designs in Keylume.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeRequest {
    pub name: String,
    /// Who made it, as the pack will say.
    pub maker: String,
    pub version: String,
    pub description: String,
    pub licence: Licence,
    /// The maker's page (https), or empty.
    #[serde(default)]
    pub url: String,
    /// The user's own profiles to put in it.
    pub designs: Vec<String>,
}

/// `text` as a kebab-case id of at most `max` characters ("Neon Nights!" → "neon-nights").
pub fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out.chars().take(max).collect();
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Make a pack of `designs` (the user's own profiles), signed with the maker's secret key
/// (hex). Its id is the name's slug and the start of the maker's public key, so the same
/// maker's next version updates it and two makers' packs of the same name don't collide.
/// Returns the pack, checked as the app will check it, and its JSON.
pub fn make(req: &MakeRequest, designs: &[Profile], secret_hex: &str) -> Result<(PackInfo, String), String> {
    let secret = unhex::<32>(secret_hex.trim()).ok_or("the maker key is malformed")?;
    let public = hex(SigningKey::from_bytes(&secret).verifying_key().as_bytes());
    let base = slug(&req.name, 33);
    if base.is_empty() {
        return Err("give the pack a name with some letters in it".into());
    }
    let url = req.url.trim();
    if !url.is_empty() && !valid_url(url) {
        return Err("the web page should be an https:// address".into());
    }
    // every design gets an id and a name of its own ("My design", "My design 2", …)
    let (mut ids, mut names) = (HashSet::new(), HashSet::new());
    let mut out = Vec::new();
    for p in designs {
        let stem = match slug(&p.name, 34) {
            s if s.is_empty() => "design".to_string(),
            s => s,
        };
        let id = (1..)
            .map(|n| if n == 1 { stem.clone() } else { format!("{stem}-{n}") })
            .find(|id| !ids.contains(id))
            .expect("unbounded");
        let base: String = p.name.trim().chars().take(keylume_profiles::limits::NAME - 4).collect();
        let name = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") })
            .find(|n| !names.contains(&n.to_lowercase()))
            .expect("unbounded");
        ids.insert(id.clone());
        names.insert(name.to_lowercase());
        out.push(Design { id, name, tags: p.tags.clone(), description: p.description.clone(), lighting: p.lighting.clone() });
    }
    let file = PackFile {
        keylume_pack: FORMAT,
        id: format!("{base}-{}", &public[..6]),
        name: req.name.trim().to_string(),
        publisher: req.maker.trim().to_string(),
        version: req.version.trim().to_string(),
        description: req.description.trim().to_string(),
        collections: Vec::new(),
        designs: out,
        licence: Some(req.licence),
        url: (!url.is_empty()).then(|| url.to_string()),
        signature: None,
    };
    let json = serde_json::to_string(&file).map_err(|e| e.to_string())?;
    let signed = sign(&json, &public, secret_hex)?;
    let pack = parse(signed.as_bytes(), &HashSet::new())?;
    Ok((pack.info, signed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(id: &str, name: &str) -> Value {
        serde_json::json!({
            "id": id, "name": name, "description": "Warm test colours for a test pack", "tags": ["test", "warm"],
            "stops": ["#ffb000", "#ff5000", "#c02000", "#801000"], "base": "#200800", "accent": "#ffffff",
            "animation": { "mode": "wave", "color": "#ff8000" }, "live": "flow"
        })
    }

    fn pack() -> Value {
        serde_json::json!({
            "keylumePack": 1, "id": "test-pack", "name": "Test Pack", "publisher": "Someone", "version": "1.0.0",
            "description": "Two themes for the tests",
            "collections": [{ "collection": "Test Pack", "prefix": "tp", "themes": [theme("tp-ember-dune", "Ember Dune")] }]
        })
    }

    #[test]
    fn canonical_form_ignores_formatting_and_the_signature() {
        let a: Value = serde_json::from_str(r#"{"b": [1, {"y": 2, "x": 1}], "a": "é", "signature": {"key": "k", "sig": "00"}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":"é","b":[1,{"x":1,"y":2}]}"#).unwrap();
        assert_eq!(canonical(&a), canonical(&b));
        assert_eq!(String::from_utf8(canonical(&b)).unwrap(), r#"{"a":"é","b":[1,{"x":1,"y":2}]}"#);
    }

    #[test]
    fn signed_packs_are_official_and_tampering_is_refused() {
        let (secret, public) = new_key().unwrap();
        let keys: &[(&str, &str, &str)] = &[("test-key", "Keylume", Box::leak(public.into_boxed_str()))];
        let signed = sign(&pack().to_string(), "test-key", &secret).unwrap();
        let p = parse_with(signed.as_bytes(), &HashSet::new(), keys).unwrap();
        assert!(p.info.official);
        assert_eq!(p.info.publisher, "Keylume", "the publisher comes from the key, not the file");
        assert_eq!((p.info.themes, p.info.collections.clone()), (1, vec!["Test Pack".to_string()]));
        // re-formatting keeps the signature valid
        let pretty = serde_json::to_string_pretty(&serde_json::from_str::<Value>(&signed).unwrap()).unwrap();
        assert!(parse_with(pretty.as_bytes(), &HashSet::new(), keys).unwrap().info.official);
        // any change breaks it
        let tampered = signed.replace("Ember Dune", "Ember Dunes");
        assert!(parse_with(tampered.as_bytes(), &HashSet::new(), keys).unwrap_err().contains("changed after it was signed"));
        // unsigned, or signed by a key this build doesn't know: a community pack
        let plain = parse_with(pack().to_string().as_bytes(), &HashSet::new(), keys).unwrap();
        assert!(!plain.info.official);
        assert_eq!(plain.info.publisher, "Someone");
        let (other, _) = new_key().unwrap();
        let stranger = sign(&pack().to_string(), "someone-else", &other).unwrap();
        assert!(!parse_with(stranger.as_bytes(), &HashSet::new(), keys).unwrap().info.official);
    }

    fn per_key(name: &str) -> Profile {
        serde_json::from_value(serde_json::json!({
            "id": "user-x", "name": name, "category": "Mine", "tags": ["warm"], "description": "Orange letters",
            "lighting": { "kind": "perKey", "brightness": 4, "keys": { "q": "#ff7a00", "esc": "#5a1a8a" } }
        }))
        .unwrap()
    }

    fn request(name: &str) -> MakeRequest {
        MakeRequest {
            name: name.into(),
            maker: "PlayerOne".into(),
            version: "1.0".into(),
            description: "Bright nights".into(),
            licence: Licence::Personal,
            url: "https://example.com/packs".into(),
            designs: vec![],
        }
    }

    #[test]
    fn packs_carry_designs_made_in_keylume() {
        let mut v = pack();
        v.as_object_mut().unwrap().remove("collections");
        v["designs"] = serde_json::json!([
            { "id": "sunset-keys", "name": "Sunset Keys", "lighting": { "kind": "perKey", "brightness": 4, "keys": { "q": "#ff7a00", "kp5": "#5a1a8a" } } },
            { "id": "calm", "name": "Calm Breath", "tags": ["calm"], "lighting": { "kind": "live", "live": { "kind": "breathe", "colors": ["#ff0080", "#00c8ff"], "bpm": 12 } } }
        ]);
        v["licence"] = "personal".into();
        v["url"] = "https://example.com/packs".into();
        let p = parse(v.to_string().as_bytes(), &HashSet::new()).unwrap();
        assert_eq!((p.info.themes, p.info.designs), (0, 2));
        assert_eq!(p.info.collections, ["Test Pack"], "designs are listed under the pack's name");
        assert_eq!(p.designs[0].id, "test-pack-sunset-keys");
        assert!(p.designs.iter().all(|d| d.category == "Test Pack" && d.section == section::THEMES && d.source == Source::Builtin));
        assert_eq!((p.info.licence, p.info.url.as_deref()), (Some(Licence::Personal), Some("https://example.com/packs")));
        // what it takes in the library: its designs' ids and its collection
        assert!(p.names().contains(&"test-pack-calm".to_string()) && p.names().contains(&"collection:test pack".to_string()));

        // each design is checked like an upload, and in the pack's own words
        let broken = |f: &dyn Fn(&mut Value)| {
            let mut b = v.clone();
            f(&mut b);
            parse(b.to_string().as_bytes(), &HashSet::new()).unwrap_err()
        };
        assert!(broken(&|b| b["designs"][0]["lighting"]["keys"] = serde_json::json!({ "Not A Key": "#ffffff" })).contains("Sunset Keys"));
        assert!(broken(&|b| b["designs"][1]["name"] = "Sunset Keys".into()).contains("two of its designs"));
        assert!(broken(&|b| b["designs"][1]["name"] = "Pikachu Glow".into()).contains("someone else's name"));
        assert!(broken(&|b| b["designs"][1]["id"] = "Calm!".into()).contains("id of its own"));
        assert!(broken(&|b| b["url"] = "http://example.com".into()).contains("https"));
        assert!(broken(&|b| b["url"] = "https://bad host.com".into()).contains("https"));
        assert!(broken(&|b| b["designs"] = serde_json::json!([])).contains("no designs"));
        assert!(broken(&|b| b["licence"] = "resell".into()).contains("valid pack"));
        // a collection with the name of one in the library, or "Mine", is refused
        let taken: HashSet<String> = ["collection:test pack".to_string()].into();
        assert!(parse(v.to_string().as_bytes(), &taken).unwrap_err().contains("already in the library"));
        assert!(broken(&|b| b["name"] = "Mine".into()).contains("already in the library"));
    }

    #[test]
    fn packs_made_in_keylume_are_signed_by_their_maker() {
        let (secret, public) = new_key().unwrap();
        let mine = [per_key("My design"), per_key("My design"), per_key("Sunset Keys")];
        let (info, json) = make(&request("Neon Nights!"), &mine, &secret).unwrap();
        assert_eq!(info.id, format!("neon-nights-{}", &public[..6]), "the name, then the maker's key");
        assert_eq!((info.official, info.maker_key.clone()), (false, Some(fingerprint(&public))));
        assert_eq!((info.publisher.as_str(), info.designs, info.licence), ("PlayerOne", 3, Some(Licence::Personal)));
        let p = parse(json.as_bytes(), &HashSet::new()).unwrap();
        let names: Vec<&str> = p.designs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["My design", "My design 2", "Sunset Keys"], "every design keeps a name of its own");
        assert_eq!(p.signer.as_deref(), Some(public.as_str()));
        // changed after it was made: refused
        let tampered = json.replace("Sunset Keys", "Sunrise Keys");
        assert!(parse(tampered.as_bytes(), &HashSet::new()).unwrap_err().contains("changed after it was signed"));
        // and what makes no pack
        assert!(make(&request("!!!"), &mine, &secret).unwrap_err().contains("name"));
        assert!(make(&MakeRequest { url: "example.com".into(), ..request("Neon") }, &mine, &secret).unwrap_err().contains("https"));
        assert!(make(&request("Neon"), &[per_key("Batman Night")], &secret).unwrap_err().contains("someone else's name"));
    }

    #[test]
    fn only_the_same_maker_updates_a_signed_pack() {
        let (a, _) = new_key().unwrap();
        let (b, b_public) = new_key().unwrap();
        let mine = [per_key("Sunset Keys")];
        let old = parse(make(&request("Neon"), &mine, &a).unwrap().1.as_bytes(), &HashSet::new()).unwrap();
        let newer = parse(make(&MakeRequest { version: "1.1".into(), ..request("Neon") }, &mine, &a).unwrap().1.as_bytes(), &HashSet::new()).unwrap();
        assert_eq!(old.info.id, newer.info.id);
        assert!(may_replace(&old, &newer).is_ok());
        // the same id from anyone else: signed by another key, or not signed at all
        let mut forged: Value = serde_json::from_str(&make(&request("Neon"), &mine, &a).unwrap().1).unwrap();
        forged.as_object_mut().unwrap().remove("signature");
        let unsigned = parse(forged.to_string().as_bytes(), &HashSet::new()).unwrap();
        assert!(may_replace(&old, &unsigned).unwrap_err().contains("isn't signed by whoever made"));
        let other = parse(sign(&forged.to_string(), &b_public, &b).unwrap().as_bytes(), &HashSet::new()).unwrap();
        assert!(may_replace(&old, &other).is_err());
        // an unsigned pack may be updated by anyone
        assert!(may_replace(&unsigned, &other).is_ok());
        // a pack that comes with Keylume: only by an official file
        let mut bundled = parse(pack().to_string().as_bytes(), &HashSet::new()).unwrap();
        bundled.info.bundled = true;
        let plain = parse(pack().to_string().as_bytes(), &HashSet::new()).unwrap();
        assert!(may_replace(&bundled, &plain).unwrap_err().contains("official"));
        let official = parse(sign(&pack().to_string(), "test-publisher", TEST_SECRET).unwrap().as_bytes(), &HashSet::new()).unwrap();
        assert!(official.info.official);
        assert!(may_replace(&bundled, &official).is_ok());
    }

    #[test]
    fn slugs_and_fingerprints() {
        assert_eq!(slug("Neon Nights!", 40), "neon-nights");
        assert_eq!(slug("  --Café Été-- ", 40), "caf-t");
        assert_eq!(slug("abcdef", 3), "abc");
        assert_eq!(fingerprint("3f9ab2c1deadbeef"), "3F9A B2C1");
        assert!(valid_url("https://example.com") && valid_url("https://shop.example.com/keylume?x=1"));
        assert!(!valid_url("https://localhost") && !valid_url("javascript:alert(1)") && !valid_url("https://a.com/ b"));
    }

    #[test]
    fn broken_or_clashing_packs_are_refused_with_a_reason() {
        let taken: HashSet<String> = ["ember dune".to_string()].into();
        assert!(parse(pack().to_string().as_bytes(), &taken).unwrap_err().contains("already used"));
        assert!(parse(b"not json", &HashSet::new()).is_err());
        assert!(parse(br#"{"profiles": []}"#, &HashSet::new()).unwrap_err().contains("isn't a Keylume pack"));
        let mut future = pack();
        future["keylumePack"] = 2.into();
        assert!(parse(future.to_string().as_bytes(), &HashSet::new()).unwrap_err().contains("format 2"));
        let mut bad_id = pack();
        bad_id["id"] = "Not Kebab".into();
        assert!(parse(bad_id.to_string().as_bytes(), &HashSet::new()).is_err());
        let mut unknown = pack();
        unknown["extra"] = 1.into();
        assert!(parse(unknown.to_string().as_bytes(), &HashSet::new()).is_err(), "unknown fields aren't silently dropped");
        let mut branded = pack();
        branded["collections"][0]["themes"][0]["name"] = "Batman Night".into();
        assert!(parse(branded.to_string().as_bytes(), &HashSet::new()).is_err());
        assert!(parse(&vec![b' '; (MAX_PACK_BYTES + 1) as usize], &HashSet::new()).unwrap_err().contains("larger"));
    }
}
