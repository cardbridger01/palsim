//! Card-data loading shared by BOTH replay drivers (`replay/` for the
//! bracketed `[Tn phase]` format, `format_a_replay/` for Format A) --
//! generated from `data/11cardattributes-with-elements.csv` at first
//! use, not hand-typed by either driver separately.
//!
//! This crate exists because two independent hand-typed copies of the
//! same table is exactly the drift the CSV-generation change (made
//! first in `format_a_replay` alone) was meant to eliminate -- it had
//! only moved the risk from "table vs data" to "driver vs driver."
//! `replay/`'s own `card_meta`/`materialize` were a second hand-typed
//! copy, missing `color` for the identical reason `format_a_replay`'s
//! was (QA #53's Suzaku boost needs the damage SOURCE's own color, and
//! nothing ever set it), and its own `materialize` called `place()` --
//! `CardKind::Pal` for everything -- for the identical reason
//! `format_a_replay`'s did. Concretely: `battle3` (the bracketed
//! corpus's own Blue/Red fixture) materializes Single-Shot Rifle,
//! which is a Gear, and nothing asserted it wasn't silently tracked as
//! a Pal until this crate's own regression tests below. See
//! CLAUDE.md's Conventions section, "the pattern behind all of this,"
//! for the general principle -- this is instances five and six of it.
//!
//! ## Two lookup views over one shared cache
//!
//! The two drivers key differently, because their log formats do: the
//! bracketed format always states a card's FULL printed name
//! ("Elphidran – Gentle Radiance"), unambiguous per print, so
//! [`card_meta_by_full_name`] needs no disambiguation at all -- the
//! CSV's own `Name`/`Subtitle` columns already produce a unique key per
//! row. Format A gives only the bare MAIN name ("Elphidran"), which
//! collapses every print of that name together, so
//! [`card_meta_by_main_name`] needs [`PREFERRED_PRINT`] to resolve the
//! (rare) case where more than one print shares a main name -- hard
//! rule 3: a print is chosen only once a real log has actually forced
//! the question, never guessed ahead of need. Both views are built from
//! the same one CSV parse, cached once.

use std::collections::HashMap;
use std::sync::OnceLock;

use engine::state::{CardId, CardKind, GameState, PlayerId, Zone};

#[derive(Clone)]
pub struct CardMeta {
    pub cost: u32,
    pub strike: i32,
    pub elements: Vec<String>,
    pub color: String,
    pub kind: CardKind,
}

/// (main name -> preferred CardNumber) for the handful of names the
/// CSV prints more than once (checked directly: Foxparks, Pengullet,
/// Digtoise, Gumoss, Daedream, Lamball, as of this CSV snapshot). Only
/// consulted by [`card_meta_by_main_name`] -- [`card_meta_by_full_name`]
/// never needs it, since a full printed name is already unique per
/// print.
///
/// - Foxparks: BP01-006 (Cost 2, Strike 1, Brave 300), not TD01-004
///   (Cost 4, Strike 2, Interrupt) -- pingu_02's own Foxparks deals
///   exactly 1 direct player damage per unblocked attack, matching
///   BP01-006's printed Strike, not TD01-004's.
/// - Pengullet: BP01-028 "Yearning for the Sky" (Cost 4, Strike 2), not
///   TD01-014 "Frequent Flyer" (Cost 3, Strike 1) -- BP01-028 is the
///   print with the graveyard-draw trigger both drivers' own "activate"
///   arms already call for real when the Rocket Launcher's granted
///   sweep destroys its holder (pingu_01 and battle1 both).
/// - Digtoise, Gumoss, Daedream, Lamball: not yet resolved by main
///   name -- no Format A fixture materializes any of these BY BARE
///   NAME today (battle3's own Lamball, if any, would resolve through
///   `card_meta_by_full_name` instead, which needs no entry here at
///   all). Add an entry the moment a real Format A log forces the
///   choice, never ahead of need.
const PREFERRED_PRINT: &[(&str, &str)] = &[("Foxparks", "BP01-006"), ("Pengullet", "BP01-028")];

struct Loaded {
    by_full_name: HashMap<String, CardMeta>,
    by_main_name: HashMap<String, CardMeta>,
}

fn loaded() -> &'static Loaded {
    static CACHE: OnceLock<Loaded> = OnceLock::new();
    CACHE.get_or_init(load_from_csv)
}

fn csv_path() -> std::path::PathBuf {
    // Anchored on THIS crate's own manifest dir (env! is resolved at
    // this crate's compile time, not whichever crate depends on it) --
    // same pattern tests/format_a_replays/support.rs already uses for
    // its own fixture JSON.
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("data").join("11cardattributes-with-elements.csv")
}

fn build_meta(row: &HashMap<String, String>, context_name: &str) -> Option<CardMeta> {
    let get = |col: &str| row.get(col).map(|s| s.trim()).filter(|s| !s.is_empty());
    let cost = get("Cost")
        .map(|s| s.parse::<f64>().unwrap_or_else(|e| panic!("card_data: bad Cost {s:?} for {context_name:?}: {e}")) as u32)
        .unwrap_or(0);
    let strike = get("Strike")
        .map(|s| s.parse::<f64>().unwrap_or_else(|e| panic!("card_data: bad Strike {s:?} for {context_name:?}: {e}")) as i32)
        .unwrap_or(0);
    let elements = get("Elements").map(|s| s.split(';').map(|e| e.trim().to_string()).collect()).unwrap_or_default();
    let color = get("Color").unwrap_or_else(|| panic!("card_data: missing Color for {context_name:?}")).to_string();
    let kind = match get("Type") {
        Some("Pal") => CardKind::Pal,
        Some("Structure") => CardKind::Structure,
        Some("Gear") => CardKind::Gear,
        Some("Event") => CardKind::Event,
        // Soul, or an unrecognized future Type value -- skipped rather
        // than guessed a kind for (hard rule 3).
        _ => return None,
    };
    Some(CardMeta { cost, strike, elements, color, kind })
}

fn load_from_csv() -> Loaded {
    let path = csv_path();
    let mut reader = csv::Reader::from_path(&path).unwrap_or_else(|e| panic!("card_data: failed to open {path:?}: {e}"));
    let rows: Vec<HashMap<String, String>> = reader
        .deserialize::<HashMap<String, String>>()
        .map(|r| r.unwrap_or_else(|e| panic!("card_data: failed to parse a row of {path:?}: {e}")))
        .collect();

    let mut by_full_name = HashMap::new();
    let mut rows_by_main_name: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
    for row in &rows {
        let name = row.get("Name").unwrap_or_else(|| panic!("card_data: a row of {path:?} has no Name column"));
        // Hard rule 5: Soul has no ability text and no place in the
        // 50-card pool -- excluded from both views, same as
        // parser/card_census.py excludes it from the build target.
        if name == "Soul" {
            continue;
        }
        let subtitle = row.get("Subtitle").map(|s| s.trim()).unwrap_or("");
        let full_name = if subtitle.is_empty() { name.clone() } else { format!("{name} – {subtitle}") };
        if let Some(meta) = build_meta(row, &full_name) {
            by_full_name.insert(full_name, meta);
        }
        rows_by_main_name.entry(name.clone()).or_default().push(row);
    }

    let preferred: HashMap<&str, &str> = PREFERRED_PRINT.iter().copied().collect();
    let mut by_main_name = HashMap::new();
    for (name, rows_for_name) in rows_by_main_name {
        let row: &HashMap<String, String> = match rows_for_name.len() {
            1 => rows_for_name[0],
            _ => match preferred.get(name.as_str()) {
                Some(&card_number) => *rows_for_name
                    .iter()
                    .find(|r| r.get("CardNumber").map(String::as_str) == Some(card_number))
                    .unwrap_or_else(|| panic!("card_data: PREFERRED_PRINT names {card_number:?} for {name:?}, but no such CardNumber exists in {path:?}")),
                // Multiple prints, no resolved preference -- don't
                // guess (hard rule 3). Skipped rather than defaulted
                // to either print; card_meta_by_main_name(name) reads
                // as None for it until PREFERRED_PRINT is updated
                // deliberately.
                None => continue,
            },
        };
        if let Some(meta) = build_meta(row, &name) {
            by_main_name.insert(name, meta);
        }
    }

    Loaded { by_full_name, by_main_name }
}

/// For the bracketed format (`replay/`): looks up by the card's exact
/// FULL printed name ("Elphidran – Gentle Radiance"), unambiguous per
/// print.
pub fn card_meta_by_full_name(name: &str) -> Option<CardMeta> {
    loaded().by_full_name.get(name).cloned()
}

/// For Format A (`format_a_replay/`): looks up by bare MAIN name
/// ("Elphidran"). See [`PREFERRED_PRINT`] for how a name with more than
/// one print resolves.
pub fn card_meta_by_main_name(name: &str) -> Option<CardMeta> {
    loaded().by_main_name.get(name).cloned()
}

/// Builds a fresh, real-named, real-statted card in `zone` for `owner`
/// from an already-resolved `meta` -- the shared primitive both
/// drivers' own `materialize()` wrappers call after resolving
/// [`card_meta_by_full_name`]/[`card_meta_by_main_name`] (whichever
/// their own log format needs) and deciding how to handle a miss (both
/// currently panic rather than silently deploy an unstatted,
/// wrongly-kinded Pal -- see each wrapper's own doc comment, and
/// CLAUDE.md's Conventions section on why a missing value that stops
/// the build beats a default that compiles).
///
/// `place_typed`, not `place`: the whole reason this crate exists
/// alongside the CSV-generation change rather than as a pure
/// refactor -- see `engine::state::GameState::place_typed`'s own doc
/// comment for the bug this fixes (both drivers' own `place()` calls
/// silently tagged every materialized card `CardKind::Pal`).
pub fn materialize_with_meta(game: &mut GameState, owner: PlayerId, zone: Zone, stored_name: &str, meta: &CardMeta) -> CardId {
    let card = game.place_typed(owner, stored_name, zone, meta.kind);
    game.set_card_name(card, stored_name);
    game.set_cost(card, meta.cost);
    game.set_strike(card, meta.strike);
    game.set_elements(card, meta.elements.clone());
    game.set_color(card, &meta.color);
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the exact bug this crate exists to fix: before
    /// `place_typed`, `materialize` called `place`, which hardcodes
    /// `CardKind::Pal` -- so a materialized Gear would have been
    /// silently counted by `pals_in_base`. Nothing existing caught
    /// this: no fixture in either driver crosses a threshold where the
    /// extra count (Pump-Action Shotgun's "all opponent Pals" sweep,
    /// the 5-Pal base limit, Astegon's own "all Pals with Power 300 or
    /// less") would visibly diverge. This test would have failed on
    /// the pre-fix code; it must keep passing on every future one,
    /// for BOTH drivers, since both now go through this one function.
    #[test]
    fn materializing_a_gear_is_excluded_from_pals_in_base_and_included_in_gears_in_base() {
        let mut game = GameState::test_scenario();
        let meta = CardMeta { cost: 4, strike: 0, elements: vec![], color: "Blue".to_string(), kind: CardKind::Gear };
        let gear = materialize_with_meta(&mut game, PlayerId::P1, Zone::Base, "Pengullet Rocket Launcher", &meta);

        assert!(!game.pals_in_base(PlayerId::P1).contains(&gear), "a materialized Gear must not be counted as a Pal");
        assert!(game.gears_in_base(PlayerId::P1).contains(&gear));
    }

    /// Same check, opposite direction -- a materialized Pal must not be
    /// miscounted as a Gear either, now that the two are distinguished.
    #[test]
    fn materializing_a_pal_is_excluded_from_gears_in_base_and_included_in_pals_in_base() {
        let mut game = GameState::test_scenario();
        let meta = CardMeta { cost: 5, strike: 2, elements: vec!["Ice".to_string(), "Dragon".to_string()], color: "Blue".to_string(), kind: CardKind::Pal };
        let pal = materialize_with_meta(&mut game, PlayerId::P1, Zone::Base, "Chillet", &meta);

        assert!(!game.gears_in_base(PlayerId::P1).contains(&pal));
        assert!(game.pals_in_base(PlayerId::P1).contains(&pal));
    }

    /// Confirms the two lookup views actually agree with each other on
    /// overlapping data (Pengullet Rocket Launcher has no Subtitle, so
    /// its full name and main name are the literal same string) --
    /// catches a divergence between the two HashMaps that unit tests
    /// scoped to only one view wouldn't.
    #[test]
    fn full_name_and_main_name_lookups_agree_for_a_subtitle_less_card() {
        let by_full = card_meta_by_full_name("Pengullet Rocket Launcher").expect("Pengullet Rocket Launcher should resolve by full name");
        let by_main = card_meta_by_main_name("Pengullet Rocket Launcher").expect("Pengullet Rocket Launcher should resolve by main name");
        assert_eq!(by_full.cost, by_main.cost);
        assert_eq!(by_full.color, by_main.color);
        assert!(matches!(by_full.kind, CardKind::Gear));
        assert!(matches!(by_main.kind, CardKind::Gear));
    }

    #[test]
    fn preferred_print_resolves_pengullet_to_bp01_028_by_main_name() {
        let meta = card_meta_by_main_name("Pengullet").expect("Pengullet should resolve via PREFERRED_PRINT");
        assert_eq!(meta.cost, 4, "BP01-028 \"Yearning for the Sky\", not TD01-014 \"Frequent Flyer\" (Cost 3)");
        assert_eq!(meta.strike, 2);
    }
}
