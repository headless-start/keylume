//! Counter-Strike binds as lighting: what each key does in-game, by role.
//!
//! The binds come from the player's own config (read by `keylume-core::cs`); until
//! one is found, CS2's default binds are used.

use std::collections::BTreeMap;

use keylume_proto::{Layout, Rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CsRole {
    /// Walk around: +forward/+back/+left/+right.
    Movement,
    /// Jump, crouch, walk.
    Body,
    /// A jump-throw (or similar) bind.
    JumpThrow,
    /// Guns, knife, bomb: slot1-5, lastinv.
    Weapon,
    /// Grenades: slot6-10.
    Grenade,
    /// Use, reload, drop, inspect.
    Utility,
    /// Voice, chat, radio, ping.
    Comms,
    /// Scoreboard, buy menu, spray, console.
    Menu,
    /// Practice-server commands (noclip, scripts).
    Practice,
    Other,
}

impl CsRole {
    pub const ALL: [CsRole; 10] = [
        CsRole::Movement,
        CsRole::Body,
        CsRole::JumpThrow,
        CsRole::Weapon,
        CsRole::Grenade,
        CsRole::Utility,
        CsRole::Comms,
        CsRole::Menu,
        CsRole::Practice,
        CsRole::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CsRole::Movement => "Movement",
            CsRole::Body => "Jump / crouch / walk",
            CsRole::JumpThrow => "Jump-throw",
            CsRole::Weapon => "Weapons",
            CsRole::Grenade => "Grenades",
            CsRole::Utility => "Use / reload / drop",
            CsRole::Comms => "Voice & chat",
            CsRole::Menu => "Scoreboard & menus",
            CsRole::Practice => "Practice",
            CsRole::Other => "Other",
        }
    }
}

/// key id (layout) -> role
pub type CsBinds = BTreeMap<String, CsRole>;

/// Classify one bind's command line (after alias expansion; `;`-separated).
pub fn classify(command: &str) -> Option<CsRole> {
    let cmds: Vec<String> = command.split(';').map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty()).collect();
    if cmds.is_empty() || cmds.iter().any(|c| c == "<unbound>") {
        return None;
    }
    let has = |p: &dyn Fn(&str) -> bool| cmds.iter().any(|c| p(c.split_whitespace().next().unwrap_or("")));
    let jump = has(&|c| c == "+jump");
    let release_attack = has(&|c| c == "-attack" || c == "-attack2");
    Some(if jump && release_attack {
        CsRole::JumpThrow
    } else if has(&|c| matches!(c, "+forward" | "+back" | "+left" | "+right" | "+moveleft" | "+moveright")) {
        CsRole::Movement
    } else if jump || has(&|c| matches!(c, "+duck" | "+sprint" | "+speed")) {
        CsRole::Body
    } else if has(&|c| matches!(c, "slot6" | "slot7" | "slot8" | "slot9" | "slot10")) {
        CsRole::Grenade
    } else if has(&|c| matches!(c, "slot1" | "slot2" | "slot3" | "slot4" | "slot5" | "slot11" | "slot12" | "lastinv" | "invprev" | "invnext")) {
        CsRole::Weapon
    } else if has(&|c| matches!(c, "+use" | "+reload" | "drop" | "+lookatweapon" | "switchhands" | "+attack" | "+attack2")) {
        CsRole::Utility
    } else if has(&|c| c.starts_with("messagemode") || c.contains("radio") || c == "+voicerecord" || c == "player_ping") {
        CsRole::Comms
    } else if has(&|c| {
        matches!(
            c,
            "+showscores"
                | "buymenu"
                | "teammenu"
                | "+spray_menu"
                | "show_loadout_toggle"
                | "autobuy"
                | "rebuy"
                | "buyammo1"
                | "buyammo2"
                | "toggleconsole"
                | "cancelselect"
                | "sellbackall"
                | "cs_quit_prompt"
                | "jpeg"
        )
    }) {
        CsRole::Menu
    } else if has(&|c| matches!(c, "noclip" | "ent_fire" | "god" | "give" | "impulse") || c.starts_with("sv_") || c.starts_with("bot_")) {
        CsRole::Practice
    } else {
        CsRole::Other
    })
}

/// CS2's default binds (game/csgo/cfg/user_keys_default.vcfg), as key ids.
pub fn default_binds() -> CsBinds {
    use CsRole::*;
    [
        ("w", Movement),
        ("a", Movement),
        ("s", Movement),
        ("d", Movement),
        ("space", Body),
        ("lctrl", Body),
        ("lshift", Body),
        ("1", Weapon),
        ("2", Weapon),
        ("3", Weapon),
        ("4", Weapon),
        ("5", Weapon),
        ("q", Weapon),
        ("6", Grenade),
        ("7", Grenade),
        ("8", Grenade),
        ("9", Grenade),
        ("0", Grenade),
        ("x", Weapon),
        ("e", Utility),
        ("r", Utility),
        ("f", Utility),
        ("g", Utility),
        ("h", Utility),
        ("c", Comms),
        ("v", Comms),
        ("z", Comms),
        ("u", Comms),
        ("y", Comms),
        ("tab", Menu),
        ("b", Menu),
        ("m", Menu),
        ("i", Menu),
        ("t", Menu),
        ("esc", Menu),
        ("grave", Menu),
        ("comma", Menu),
        ("period", Menu),
        ("delete", Menu),
    ]
    .into_iter()
    .map(|(k, r)| (k.to_string(), r))
    .collect()
}

/// A colour for each role: shades of blue, so the layout reads calmly in game; the
/// jump-throw key wears the crosshair colour.
pub fn role_color(role: CsRole, crosshair: Rgb) -> Rgb {
    let h = |s: &str| -> Rgb { s.parse().unwrap() };
    match role {
        CsRole::Movement => h("#00e5ff"),
        CsRole::Body => h("#2f6bff"),
        CsRole::JumpThrow => crosshair,
        CsRole::Weapon => h("#0a8cff"),
        CsRole::Grenade => h("#bfefff"),
        CsRole::Utility => h("#4d7dff"),
        CsRole::Comms => h("#7a5cff"),
        CsRole::Menu => h("#2536a8"),
        CsRole::Practice => h("#16306e"),
        CsRole::Other => h("#1a2e80"),
    }
}

/// Every key coloured by what it does in CS; unbound keys stay a deep navy.
pub fn binds_keys(layout: &Layout, binds: &CsBinds, crosshair: Rgb) -> BTreeMap<String, Rgb> {
    let unbound: Rgb = "#040b3a".parse().unwrap();
    layout
        .keys
        .iter()
        .map(|k| (k.id.clone(), binds.get(&k.id).map(|r| role_color(*r, crosshair)).unwrap_or(unbound)))
        .collect()
}

/// A crosshair in `crosshair`'s colour, centred on a blue board.
pub fn crosshair_keys(layout: &Layout, crosshair: Rgb, gap_tight: bool) -> BTreeMap<String, Rgb> {
    let (w, h) = (layout.width, layout.height);
    let navy: Rgb = "#050f4a".parse().unwrap();
    let blue: Rgb = "#1f45ff".parse().unwrap();
    layout
        .keys
        .iter()
        .map(|k| {
            let (x, y) = (k.x + k.w / 2.0, k.y + k.h / 2.0);
            let (dx, dy) = ((x - w / 2.0).abs(), (y - h / 2.0).abs());
            let start = if gap_tight { 0.6 } else { 1.0 };
            let arm = (dy < 0.5 && (start..4.6).contains(&dx)) || (dx < 0.6 && dy >= 0.9);
            let ring = ((dx / (w / 2.0)).max(dy / (h / 2.0)) * 3.0).floor() as i32 == 2;
            let c = if arm {
                crosshair
            } else if ring {
                crate::color::mix(navy, blue, 0.45)
            } else {
                navy
            };
            (k.id.clone(), c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_real_binds() {
        assert_eq!(classify("+forward"), Some(CsRole::Movement));
        assert_eq!(classify("+duck"), Some(CsRole::Body));
        assert_eq!(classify("+jump;"), Some(CsRole::Body));
        assert_eq!(classify("+jump;;-attack; -attack2"), Some(CsRole::JumpThrow), "jump-throw after alias expansion");
        assert_eq!(classify("slot7"), Some(CsRole::Grenade));
        assert_eq!(classify("slot2"), Some(CsRole::Weapon));
        assert_eq!(classify("+voicerecord"), Some(CsRole::Comms));
        assert_eq!(classify("+showscores"), Some(CsRole::Menu));
        assert_eq!(classify("noclip"), Some(CsRole::Practice));
        assert_eq!(classify("ent_fire script Enter"), Some(CsRole::Practice));
        assert_eq!(classify("<unbound>"), None);
        assert_eq!(classify("say gg"), Some(CsRole::Other));
    }

    #[test]
    fn binds_are_mostly_blue() {
        let l = Layout::tk68();
        let keys = binds_keys(&l, &default_binds(), Rgb(255, 255, 0));
        let blue = keys.values().filter(|c| c.2 >= c.0 && c.2 >= c.1).count();
        assert!(blue as f32 / keys.len() as f32 > 0.9, "{blue} of {}", keys.len());
    }
}
