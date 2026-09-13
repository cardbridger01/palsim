//! Drives a parsed client-log replay (Phase 1's `parser/replay_parser.py`
//! output, `tests/replays/fixtures/*.json`) through `engine::` + `cards::`
//! and asserts the engine agrees with the real game at every line that
//! states a life total.
//!
//! `ReplayLog`/`ReplayEvent`/`ReplayMeta` derive `Deserialize` directly
//! from the parser's own JSON schema (line_index, turn, phase, actor,
//! action, cards, targets, resolution, caused_by_line, text --
//! `tests/replays/mod.rs`'s own doc comment asks for exactly this).
//! `resolution`'s shape varies per `action` value, so it stays a bare
//! `serde_json::Value` rather than an enum -- `apply_event` reads out of
//! it by key, per action.
//!
//! ## Scope: only what battle1 needs, only what's observable
//!
//! This is not a general-purpose interactive engine driver -- it
//! reproduces ONE SPECIFIC KNOWN SEQUENCE (a fixed log), so it is
//! allowed judgment calls a real player-facing driver couldn't make:
//!
//! - **The true 50-card deck's exact contents, beyond what the log
//!   names, are unknowable.** `apply_event` seeds exactly the one card
//!   about to be looked at (a Chillet-style reveal, or a Lucky mill
//!   card) with the identity/stats the log states, immediately before
//!   the primitive that consumes it runs -- never more.
//! - **A few battle1-specific choices are hardcoded** at the point each
//!   multi-step card effect starts (Jormuntide's "choose up to 1
//!   opponent Pal" always resolves to no eligible target across the
//!   WHOLE corpus, not just battle1 -- every occurrence's very next
//!   event is "no_valid_target", checked directly; Elphidran Aqua's
//!   "choose 1 from hand" is whatever the log names; Chillet always
//!   chooses to deploy an eligible reveal) rather than building general
//!   target-search/choice logic this replay never exercises
//!   non-trivially. None of the events in between are independently
//!   assertion-checked by `tests/replays/battle1.rs` -- only
//!   life-after-attack and on-deploy-triggered are -- so this is a
//!   replay-driver simplification, not a rules claim.
//! - **Player damage's mill/Lucky outcome is NOT bypassed.** Every
//!   card's Strike (set from `card_meta` below, sourced from
//!   `data/11cardattributes-with-elements.csv`) matches the log's
//!   "attacked for N" number exactly, so `resolve_battle`'s real 11.2.1/
//!   11.2.2 mill loop reproduces the log's damage naturally. The two
//!   Lucky-cancel attacks get exactly one Lucky-tagged card seeded on
//!   top of the defender's deck first, then the real mill loop runs
//!   unmodified.
//!
//! ## Every card with a `cards::` script must be called for real
//!
//! An earlier version of this driver reproduced several cards'
//! observable effects (a draw, a graveyard move) with generic engine
//! primitives instead of calling the card's own scripted function --
//! `game.draw()` standing in for `jormuntide_on_deploy`, and so on.
//! That made "this card is corpus-validated" a claim about the card's
//! NAME appearing in a log, not about its LOGIC ever running --
//! discovered only by grepping this file directly (CLAUDE.md's
//! calibration note). Two things fix that:
//!
//! 1. Every dedicated dispatch arm below (`"reveal"`, `"draw"`,
//!    `"put_on_top"`, `"destroyed"`, `"activate"`, `"power_buff"`) calls
//!    the real `cards::` function whenever the event names a card that
//!    has one, in place of the primitive that used to fake it.
//! 2. [`SCRIPTED_ACTIONS`] is a small registry of (card name, action)
//!    pairs that must never reach a generic-primitive fallback path.
//!    The `"draw"` and `"play_card"` arms consult it and panic if a
//!    registered pair falls through anyway -- so a *future* card script
//!    that this driver forgets to wire in fails loudly instead of
//!    silently reproducing the old bug. Same spirit as the `"power_buff"`
//!    arm's pre-existing `assert_eq!` guard: a check that fires when the
//!    assumption breaks, not a comment saying it might.

use engine::combat::AttackTarget;
use engine::state::{CardId, GameState, PlayerId, Zone};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayMeta {
    #[serde(rename = "recordId")]
    pub record_id: String,
    pub deck: String,
    pub player: String,
    pub opponent: String,
    pub opponent_colors: String,
    pub result: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayEvent {
    pub line_index: usize,
    pub turn: u32,
    pub phase: String,
    pub actor: String,
    pub action: String,
    pub cards: Vec<String>,
    pub targets: Vec<String>,
    pub resolution: Option<serde_json::Value>,
    pub caused_by_line: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayLog {
    pub meta: ReplayMeta,
    pub events: Vec<ReplayEvent>,
}

/// What `apply_event` reports back when the log itself states an
/// expected life total for this event, so the caller can assert it.
pub struct LifeCheck {
    pub player: PlayerId,
    pub expected_life: i64,
}

/// Cross-event memory a couple of card effects need because the log
/// splits ONE card effect across two adjacent lines -- "played Dark
/// Cannon" and the "destroyed" event naming its victim are two log
/// lines for one Quick card's single effect; there is no engine-level
/// "last card played" fact to read this back off, unlike
/// `last_deployed` (used instead for Jormuntide/Elphidran Aqua below,
/// where such a fact already exists). Not engine state -- purely
/// replay-driver bookkeeping, hence its own type instead of a
/// `GameState` field.
#[derive(Default)]
pub struct ReplayDriver {
    pending_quick_destroy: Option<QuickDestroy>,
}

#[derive(Debug, Clone, Copy)]
enum QuickDestroy {
    DarkCannon,
    StrikeFromTheDarkness,
}

/// (card name, `ReplayEvent::action`) pairs that have a dedicated
/// `cards::` script and must never be handled by a generic engine
/// primitive instead. Consulted only at the specific fallback paths
/// that could otherwise silently substitute for one of these -- see the
/// module doc comment's "every card with a script must be called for
/// real" section. Add an entry here in the same commit that adds a
/// script this driver needs to call, so a future dispatch gap fails
/// loudly instead of quietly reproducing the original bug.
const SCRIPTED_ACTIONS: &[(&str, &str)] = &[
    ("Jormuntide – Surging Sea Serpent", "draw"),
    ("Elphidran Aqua – Gentle Ripples", "draw"),
    ("Dark Cannon", "play_card"),
    ("Strike from the Darkness", "play_card"),
];

/// A fresh `GameState` set up for replaying `meta`'s log -- `meta`
/// carries no fields `test_scenario()`'s defaults don't already match
/// (10 starting life is every observed value in the replay corpus, per
/// `engine::state`'s own doc comment).
pub fn new_game_for_replay(_meta: &ReplayMeta) -> GameState {
    GameState::test_scenario()
}

/// The parser's `"player"` field is always this log's own recording
/// perspective (e.g. `"Edify (P1)"`, or battle2's `"Edify (P2, on the
/// draw)"` -- the seat tag can carry extra text after it before the
/// closing paren, so this checks the `"(P2"` prefix, not the exact
/// `"(P2)"` substring) -- P1 unless explicitly tagged P2. The opponent
/// is simply the other seat.
pub fn opponent_of_replay_owner(meta: &ReplayMeta) -> PlayerId {
    if meta.player.contains("(P2") {
        PlayerId::P1
    } else {
        PlayerId::P2
    }
}

/// The other seat from `opponent_of_replay_owner` -- the log's own
/// recording perspective, e.g. battle2's LOSS result is asserted
/// against this player's life, not the opponent's.
pub fn replay_owner(meta: &ReplayMeta) -> PlayerId {
    if meta.player.contains("(P2") {
        PlayerId::P2
    } else {
        PlayerId::P1
    }
}

/// `engine::state::GameState::last_deployed`'s on-deploy fact --
/// battle1's own test for the Chillet-chain milestone (QA #32/#90)
/// reads this right after an event whose `caused_by_line` is set.
pub fn last_on_deploy_triggered(game: &GameState) -> bool {
    game.last_deployed().map(|card| game.last_deploy_result(card).on_deploy_triggered).unwrap_or(false)
}

/// Per-card facts this replay needs that aren't in the JSON itself --
/// sourced from `data/11cardattributes-with-elements.csv`'s Cost/
/// Strike/Elements columns (hard rule 1), keyed by the card's exact
/// printed name (hard rule 3's sanctioned alias table: the log only
/// ever records names, never card numbers).
struct CardMeta {
    cost: u32,
    strike: i32,
    elements: &'static [&'static str],
}

fn card_meta(name: &str) -> Option<CardMeta> {
    match name {
        "Elphidran – Gentle Radiance" => Some(CardMeta { cost: 4, strike: 2, elements: &["Dragon"] }),
        "Chillet – Dragon Whisperer" => Some(CardMeta { cost: 5, strike: 2, elements: &["Ice", "Dragon"] }),
        "Jormuntide – Surging Sea Serpent" => Some(CardMeta { cost: 8, strike: 3, elements: &["Dragon", "Water"] }),
        "Elphidran Aqua – Gentle Ripples" => Some(CardMeta { cost: 6, strike: 2, elements: &["Dragon", "Water"] }),
        "Victor's Strategy" => Some(CardMeta { cost: 3, strike: 0, elements: &[] }),
        "Pengullet Rocket Launcher" => Some(CardMeta { cost: 4, strike: 0, elements: &[] }),
        "Pengullet – Yearning for the Sky" => Some(CardMeta { cost: 4, strike: 2, elements: &["Water", "Ice"] }),
        "Strike from the Darkness" => Some(CardMeta { cost: 4, strike: 0, elements: &[] }),
        "Dark Cannon" => Some(CardMeta { cost: 3, strike: 0, elements: &[] }),
        "Relaxaurus – Hungry Gunner" => Some(CardMeta { cost: 7, strike: 3, elements: &["Dragon", "Water"] }),
        "Jormuntide Ignis – Savage Lava Dragon" => Some(CardMeta { cost: 8, strike: 4, elements: &["Dragon", "Fire"] }),
        "Suzaku – Hellfire Wings" => Some(CardMeta { cost: 7, strike: 3, elements: &["Fire"] }),
        "Kitsun – Wyrmbane Fangs" => Some(CardMeta { cost: 5, strike: 2, elements: &["Fire"] }),
        "Cryolinx – Arctic Ordeal" => Some(CardMeta { cost: 6, strike: 2, elements: &["Ice"] }),
        "Pyrin Noct – Steed of Azure Flames" => Some(CardMeta { cost: 7, strike: 3, elements: &["Fire", "Dark"] }),
        "Reindrix – Icy Gaze" => Some(CardMeta { cost: 5, strike: 2, elements: &["Ice"] }),
        "Dinossom – Radiant Fragrance" => Some(CardMeta { cost: 4, strike: 2, elements: &["Grass", "Dragon"] }),
        _ => None,
    }
}

/// Creates a fresh, real-named, real-statted card in `zone` for
/// `owner` -- every "this name appears for the first time" moment
/// (a real deploy from hand, a Chillet-style deck reveal, a card
/// materialized into hand for `put_on_top`).
fn materialize(game: &mut GameState, owner: PlayerId, zone: Zone, name: &str) -> CardId {
    let card = game.place(owner, name, zone);
    game.set_card_name(card, name);
    if let Some(meta) = card_meta(name) {
        game.set_cost(card, meta.cost);
        game.set_strike(card, meta.strike);
        game.set_elements(card, meta.elements.iter().map(|s| s.to_string()).collect());
    }
    card
}

/// The most recently created card owned by `owner`, in `zone`, whose
/// full printed name (`full_name_of`, not the truncated main name --
/// two "Pengullet" prints share a main name) matches `name` exactly.
/// "Most recent" is what lets battle1 tell apart the 2nd and 3rd
/// Jormuntide instances it deploys in the same turn (T5): each later
/// event always means the one just introduced, never an older
/// same-named copy still on the board.
fn find_by_name(game: &GameState, owner: PlayerId, zone: Zone, name: &str) -> Option<CardId> {
    game.cards_in_zone(owner, zone).into_iter().rev().find(|&id| game.full_name_of(id) == name)
}

/// `actor: "player"` always means Edify (the log owner), `"opponent"`
/// always means FluffysFury -- but WHICH engine `PlayerId` that maps to
/// depends on the log's own recorded seat (`replay_owner`/
/// `opponent_of_replay_owner`), not a hardcoded P1/P2 guess. Battle2's
/// meta ("Edify (P2, on the draw)") is the case that would otherwise
/// silently swap the two: without reading `meta` here, Edify's own
/// events would land on P1 while `replay_owner(&log.meta)` (what the
/// test's final assertion checks) resolves to P2 for that same log.
fn player_for(meta: &ReplayMeta, event: &ReplayEvent) -> PlayerId {
    if event.actor == "player" {
        replay_owner(meta)
    } else {
        opponent_of_replay_owner(meta)
    }
}

fn res_u64(event: &ReplayEvent, key: &str) -> Option<u64> {
    event.resolution.as_ref()?.get(key)?.as_u64()
}

fn res_i64(event: &ReplayEvent, key: &str) -> Option<i64> {
    event.resolution.as_ref()?.get(key)?.as_i64()
}

fn res_bool(event: &ReplayEvent, key: &str) -> Option<bool> {
    event.resolution.as_ref()?.get(key)?.as_bool()
}

/// Applies one parsed log event to `game`. Returns `Some(LifeCheck)`
/// exactly when the log itself states an expected life total for this
/// event (every non-Lucky "attack" event) -- `None` otherwise,
/// including for the Lucky-cancel attacks, where the log states no
/// life change to check.
pub fn apply_event(driver: &mut ReplayDriver, game: &mut GameState, meta: &ReplayMeta, event: &ReplayEvent) -> Option<LifeCheck> {
    let player = player_for(meta, event);
    let opponent = if player == PlayerId::P1 { PlayerId::P2 } else { PlayerId::P1 };

    match event.action.as_str() {
        // Pure narration/markers -- no state change of their own.
        // "mulligan" (QA #9: all 5 or none) is narration for the same
        // reason "setup" is: this driver never materializes an
        // unnamed starting hand in the first place (see the module doc
        // comment's "seeds exactly the one card about to be looked at"
        // scope), so there is nothing here for a redraw to act on.
        "setup" | "mulligan" | "draw_phase" | "main_phase" | "reveal_declined" | "no_valid_target" => None,

        // "Played X." (a Quick card, cast at instant speed) -- Dark
        // Cannon and Strike from the Darkness both name their victim on
        // a SEPARATE following "destroyed" event, not here, so this arm
        // only records which script (if either) that event needs to
        // finish. Every other played card either has no card script of
        // its own (nothing to remember) or is a plain "deploy"/
        // "activate" the corresponding arm already handles directly.
        "play_card" => {
            let name = event.cards.first().map(String::as_str);
            driver.pending_quick_destroy = match name {
                Some("Dark Cannon") => Some(QuickDestroy::DarkCannon),
                Some("Strike from the Darkness") => Some(QuickDestroy::StrikeFromTheDarkness),
                Some(other) => {
                    assert!(
                        !SCRIPTED_ACTIONS.contains(&(other, "play_card")),
                        "apply_event: {other:?} is registered in SCRIPTED_ACTIONS for \"play_card\" but this arm doesn't recognize it -- wire its real cards:: function in here instead of leaving it to fall through to a generic graveyard move at \"destroyed\""
                    );
                    None
                }
                None => None,
            };
            None
        }

        "end_phase" => {
            game.end_phase(player);
            None
        }

        "stand_phase" => {
            game.stand_phase(player);
            None
        }

        // Covers BOTH an ordinary Draw Phase draw (phase == "draw") and
        // an On Deploy trigger's own draw step (phase == "main",
        // Jormuntide/Elphidran Aqua). Unlike an ordinary draw, those two
        // have a real `cards::` script this driver must call instead of
        // the generic loop below -- the event itself carries no card
        // name to dispatch on (`event.cards` is always `[]` here), so
        // `last_deployed`/`on_deploy_triggered` stands in as "which
        // card's On Deploy this step belongs to," the same way the
        // Chillet-chain tests already use it. Holds across every occurrence in
        // the corpus checked directly: nothing else ever deploys
        // between one of these two cards' own deploy and its
        // immediately-following draw event.
        "draw" => {
            let amount = res_u64(event, "amount").unwrap_or(1);
            if event.phase == "main" && last_on_deploy_triggered(game) {
                if let Some(last) = game.last_deployed() {
                    let last_name = game.full_name_of(last);
                    match (last_name.as_str(), amount) {
                        ("Jormuntide – Surging Sea Serpent", 1) => {
                            // "choose up to 1 opponent Pal" always finds no
                            // eligible target in this corpus -- verified
                            // directly: every occurrence's very next event
                            // is "no_valid_target".
                            cards::bp01::jormuntide_on_deploy(game, player, None);
                            return None;
                        }
                        ("Elphidran Aqua – Gentle Ripples", 2) => {
                            // Both draws happen as part of the single
                            // `elphidran_aqua_on_deploy` call made at the
                            // "put_on_top" event that always follows --
                            // that function does "draw 2, then put 1 back"
                            // atomically, so nothing draws here yet.
                            return None;
                        }
                        (other, _) => {
                            assert!(
                                !SCRIPTED_ACTIONS.contains(&(other, "draw")),
                                "apply_event: {other:?}'s OnDeploy draw is registered in SCRIPTED_ACTIONS but this event ({amount} card(s)) didn't match a recognized shape -- a generic draw would silently substitute for its real script"
                            );
                        }
                    }
                }
            }
            for _ in 0..amount {
                game.draw(player);
            }
            None
        }

        "place_souls" => {
            let amount = res_u64(event, "amount").unwrap_or(0) as usize;
            game.increase_soul(player, amount);
            None
        }

        "rest_draw" => {
            let souls = res_u64(event, "souls_spent").unwrap_or(0) as usize;
            game.rest_souls_and_draw(player, souls);
            None
        }

        "deploy" => {
            let name = &event.cards[0];
            if event.caused_by_line.is_some() {
                // Already performed by the preceding "reveal" event's
                // chillet_on_deploy call, whose own deploy_from already
                // moved the seeded card to Base -- this event is the
                // log's observable confirmation, not new work.
                return None;
            }
            let card = materialize(game, player, Zone::Hand, name);
            game.deploy_from(player, card, Zone::Hand, Zone::Base);
            None
        }

        "reveal" => {
            if let Some(resolution) = &event.resolution {
                if resolution.get("element").is_some() {
                    // BP01-098 Elphidran GR's OnAttack: "reveal 1 card
                    // from your hand" -- the revealed card is named in
                    // `event.cards`, but the ATTACKER (whose Power this
                    // buffs) is not; checked directly against every
                    // occurrence in the corpus (battle1 x1, battle3 x3):
                    // the attacker is always Elphidran GR itself, already
                    // on the field by the time this fires, so it's found
                    // by name rather than tracked across events.
                    let revealed_name = &event.cards[0];
                    let revealed = materialize(game, player, Zone::Hand, revealed_name);
                    let attacker = find_by_name(game, player, Zone::Base, "Elphidran – Gentle Radiance")
                        .expect("reveal: Elphidran GR's OnAttack fired but no Elphidran GR in base for this player");
                    cards::bp01::elphidran_gr_on_attack(game, attacker, Some(revealed));
                }
                // Any other resolution shape (e.g. "outcome":
                // "added_to_hand", seen for "Found an Egg!"/Pyrin Noct)
                // belongs to an ability with no `cards::` script here --
                // narration only.
                return None;
            }
            // BP01-025 Chillet's OnDeploy reveal: seed the deck's top
            // card with exactly the identity the log says was
            // revealed, then run the real primitive against it.
            let name = &event.cards[0];
            materialize(game, player, Zone::Deck, name);
            cards::bp01::chillet_on_deploy(game, player, true); // battle1 always chooses to deploy
            None
        }

        "put_on_top" => {
            let name = &event.cards[0];
            // TD01-012 Elphidran Aqua's "choose 1 card from your hand" --
            // the counterpart to the deferred "draw" arm above: same
            // `last_deployed` discriminator, same corpus-wide invariant
            // (nothing deploys in between). `elphidran_aqua_on_deploy`
            // performs both draws AND this move as one call, so this is
            // the only place that function is ever invoked.
            if last_on_deploy_triggered(game) {
                if let Some(last) = game.last_deployed() {
                    if game.full_name_of(last) == "Elphidran Aqua – Gentle Ripples" {
                        let chosen = materialize(game, player, Zone::Hand, name);
                        cards::td01::elphidran_aqua_on_deploy(game, player, chosen);
                        return None;
                    }
                }
            }
            let card = materialize(game, player, Zone::Hand, name);
            game.move_to_top_of_deck(player, card, Zone::Hand);
            None
        }

        // Direct player damage, from either side: "X attacked them for
        // N..." (the log owner's own turn, target "opponent") or "X hit
        // Edify for N..."/"X attacked Edify for N; ...Lucky..." (the
        // opponent's turn, target "Edify"). An opponent-side hit is
        // usually preceded by its own "attack_declare" event, which
        // already opened the battle -- reuse it via `pending_attack`
        // rather than declaring a second time (declare_attack panics on
        // a still-open battle, 9.7.4).
        "attack" => {
            let name = &event.cards[0];
            let attacker = find_by_name(game, player, Zone::Base, name).expect("attack: attacker not found in base");
            assert!(
                event.targets.iter().any(|t| t == "opponent" || t == "Edify"),
                "attack: expected a direct player-damage attack, found targets {:?}",
                event.targets
            );

            let attack = game.pending_attack().unwrap_or_else(|| game.declare_attack(attacker, AttackTarget::Player(opponent)));

            if res_bool(event, "lucky") == Some(true) {
                // Seed exactly the one card about to be milled as
                // Lucky -- take_top_of_deck only peeks (see its own
                // doc comment), so resolve_battle's real mill loop
                // below picks up this exact same card again.
                if let Some(card) = game.take_top_of_deck(opponent) {
                    game.grant_keyword(card, "Lucky");
                }
            }

            game.resolve_battle(attack);

            res_i64(event, "life_after").map(|expected_life| LifeCheck { player: opponent, expected_life })
        }

        "attack_interrupted" => {
            let name = &event.cards[0];
            let attacker = find_by_name(game, player, Zone::Base, name).expect("attack_interrupted: attacker not found in base");
            game.declare_attack(attacker, AttackTarget::Player(opponent));
            game.interrupt_attack(attacker);
            None
        }

        // "FluffysFury's X attacks Edify [for N | 's Y]." -- the
        // opponent's own attack declaration, which the later "attack"/
        // "battle_attack" event resolves. A named target is always a
        // specific Pal (b_opp_attack_card never emits "opponent"/
        // "Edify" as the target string), never the player.
        "attack_declare" => {
            let name = &event.cards[0];
            let attacker = find_by_name(game, player, Zone::Base, name).expect("attack_declare: attacker not found in base");
            let target_name = &event.targets[0];
            let target = if target_name == "Edify" || target_name == "opponent" {
                AttackTarget::Player(opponent)
            } else {
                let defender = find_by_name(game, opponent, Zone::Base, target_name).expect("attack_declare: target Pal not found in base");
                AttackTarget::Pal(defender)
            };
            game.declare_attack(attacker, target);
            None
        }

        // "Edify interrupted with X, [paid N Souls|discarded Y]. The
        // attack was nullified." -- nullifies whichever attack is
        // currently open. The interrupting card's own identity and its
        // cost (Souls vs. discard) aren't modeled -- see the module doc
        // comment's "battle1-specific choices hardcoded" precedent;
        // nothing in this replay's scope (life totals, On Deploy
        // triggers) depends on which cost was paid.
        "interrupt" => {
            let attack = game.pending_attack().expect("interrupt: no pending attack to nullify");
            let attacker = game.attacker_of(attack);
            game.interrupt_attack(attacker);
            None
        }

        // "They blocked with X." -- 9.4.2 rests the blocker, making it a
        // legal `AttackTarget::Pal` for the `battle_attack` event that
        // follows. This driver resolves the actual retarget at
        // `battle_attack` time instead of via `GameState::block` (which
        // needs the attack's `AttackId` already in hand): one of
        // battle3's two blocks has no preceding event that names the
        // attacker at all (the anomalous T7 sequence README flags), so
        // there is no `AttackId` to give it. Resting the blocker here is
        // the only side effect `battle_attack`'s own direct
        // `AttackTarget::Pal` declaration actually depends on.
        "block" => {
            let name = &event.cards[0];
            let blocker = find_by_name(game, player, Zone::Base, name).expect("block: blocker not found in base");
            game.rest(blocker);
            None
        }

        // Pal-vs-Pal battle resolution: "X (power) attacked Y (power)."
        // If `attack_declare` already opened this battle (the opponent
        // declaring straight at a Pal), reuse it; otherwise (the log
        // owner's own turn, where declaration is never logged as its
        // own line) declare fresh against the named defender. Either
        // way, seed both sides with the EXACT Power the client already
        // computed -- OnAttack buffs this driver treats as narration-
        // only (see "reveal" below) still show up here, in the log's own
        // numbers, so this is reading them, not guessing them.
        "battle_attack" => {
            let attacker_name = &event.cards[0];
            let attacker = find_by_name(game, player, Zone::Base, attacker_name).expect("battle_attack: attacker not found in base");

            let attack = match game.pending_attack() {
                Some(id) => id,
                None => {
                    let defender_name = &event.targets[0];
                    let defender = find_by_name(game, opponent, Zone::Base, defender_name).expect("battle_attack: defender not found in base");
                    game.declare_attack(attacker, AttackTarget::Pal(defender))
                }
            };

            if let Some(AttackTarget::Pal(defender)) = game.battle_target(attack) {
                if let Some(p) = res_i64(event, "attacker_power") {
                    game.set_power(attacker, p as i32);
                }
                if let Some(p) = res_i64(event, "defender_power") {
                    game.set_power(defender, p as i32);
                }
            }

            game.resolve_battle(attack);
            game.run_check_timing(); // 9.6.4: lethal damage is a check-type rule action

            None
        }

        // Covers both the spell-removal case (battle1: a card destroyed
        // outright, still in Base when this fires) and the Pal-vs-Pal
        // battle case (battle2/3: `resolve_battle` + `run_check_timing`
        // above already moved the card to the graveyard, so this is
        // pure narration by the time it's reached). Searching both
        // players rather than trusting `player`/`opponent` is needed
        // because either side of a battle -- or, for the spell case, the
        // ACTOR's opponent -- can be the one destroyed.
        "destroyed" => {
            let name = &event.cards[0];
            // Consumed unconditionally, whichever branch below runs, so
            // a stale flag never leaks into a later, unrelated
            // "destroyed" event (e.g. an ordinary battle kill).
            let pending = driver.pending_quick_destroy.take();
            if let Some(victim) = find_by_name(game, PlayerId::P1, Zone::Base, name).or_else(|| find_by_name(game, PlayerId::P2, Zone::Base, name)) {
                match pending {
                    Some(QuickDestroy::DarkCannon) => cards::bp01::dark_cannon(game, victim),
                    Some(QuickDestroy::StrikeFromTheDarkness) => cards::td02::strike_from_the_darkness(game, victim),
                    None => game.move_to_graveyard(victim, Zone::Base),
                }
            }
            None
        }

        // Pengullet Rocket Launcher's own activation is deferred to the
        // "power_buff" event that follows, which is where its target is
        // actually named. A card that already holds a granted ability
        // instance (the launcher's own granted sweep skill, the only
        // shape this replay corpus exercises) runs that instead. Every
        // other "activate" here -- Grappling Gun, Single-Shot Rifle --
        // is a plain `ACT [Rest this card] ... Power +200 ...` with no
        // card script of its own in `cards::` (only the Pengullet
        // Rocket Launcher's Pengullet-specific branching earned one);
        // resting it here is the ACT's own stated cost, and the
        // Power+200 itself is applied generically at "power_buff".
        "activate" => {
            let name = &event.cards[0];
            if name == "Pengullet Rocket Launcher" {
                return None;
            }
            if let Some(holder) = find_by_name(game, player, Zone::Base, name) {
                if !game.granted_ability_instances(holder).is_empty() {
                    let holder_name = game.full_name_of(holder);
                    cards::bp01::rocket_launcher_granted_sweep(game, holder, player);
                    if holder_name == "Pengullet – Yearning for the Sky" {
                        // BP01-028's OWN AUTO ability ("when this card is
                        // put into the graveyard, draw 1 card"), entirely
                        // separate from the granted sweep above -- it
                        // fires here because the sweep's own "put this
                        // card into the graveyard" clause is what moves
                        // the holder base -> graveyard (QA #41's
                        // condition). NOTE: battle1's own log records no
                        // "drew 1 card" line for the holder's owner right
                        // after this -- see the README's "known anomaly"
                        // entry. The trigger's condition text is
                        // unconditional (no "may"), so this driver follows
                        // the rules text rather than the log's silence.
                        cards::bp01::pengullet_graveyard_trigger(game, player);
                    }
                    return None;
                }
                game.rest(holder);
            }
            None
        }

        // A Pengullet Rocket Launcher present for `player` means this
        // buff is its own Pengullet-specific ACT (`rocket_launcher_act`,
        // which branches on the target's main name); otherwise it's a
        // generic `Power +N until end of turn` from whichever plain
        // gear's "activate" deferred to this event, applied directly --
        // see the "activate" arm above.
        //
        // TODO(replay): "a launcher exists for this player" is a
        // presence check, not a causation check -- it can't tell a
        // launcher's own buff apart from some OTHER Power-buffing gear
        // active for the same player at the same time. True for
        // battle1-3 (none of them ever has both at once) but not a
        // general rule; it will not survive a larger corpus. The real
        // fix is tracking which specific source the preceding "activate"
        // named (the way `pending_attack`/`attacker_of` already track
        // the open attack) instead of re-deriving it here from board
        // state. Not worth building for three known logs -- but rather
        // than silently misrouting a future case into the wrong ACT,
        // this cross-checks the buff amount against what
        // `rocket_launcher_act` would actually produce and panics on a
        // mismatch, so a log that breaks the assumption fails loudly
        // here instead of quietly corrupting Power.
        "power_buff" => {
            let name = &event.cards[0];
            let target = find_by_name(game, player, Zone::Base, name).expect("power_buff: target not found in base");
            let amount = res_u64(event, "amount");
            if let Some(launcher) = find_by_name(game, player, Zone::Base, "Pengullet Rocket Launcher") {
                let expected = if game.main_name_of(target) == "Pengullet" { 500 } else { 200 };
                assert_eq!(
                    amount,
                    Some(expected),
                    "power_buff: a Pengullet Rocket Launcher is present for {player:?}, but this buff's amount ({amount:?}) doesn't match what rocket_launcher_act would produce ({expected}) -- likely a second Power-buffing gear active for the same player, which the launcher-presence discriminator can't tell apart from the launcher itself (see the TODO above)"
                );
                cards::bp01::rocket_launcher_act(game, launcher, target);
            } else if let Some(amount) = amount {
                game.increase_power(target, amount as u32);
            }
            None
        }

        // "Discarded X." (own cost payment) or "FluffysFury discarded X,
        // Y to pay a cost." (multiple) -- every named card is materialized
        // into hand and immediately moved to the graveyard, the same
        // "first time this name appears" treatment `materialize` already
        // gives `put_on_top`/`look_and_take`.
        "discard" => {
            for name in &event.cards {
                let card = materialize(game, player, Zone::Hand, name);
                game.deploy_from(player, card, Zone::Hand, Zone::Graveyard);
            }
            None
        }

        // "Looked at top N: took X to hand." / "FluffysFury returned X
        // from their graveyard to hand." -- both are "a card with this
        // exact identity now exists in hand," the deck-lookup and
        // graveyard-return mechanics themselves abstracted away the same
        // way Jormuntide's/Elphidran Aqua's own choices are (module doc
        // comment): nothing downstream needs the specific deck/graveyard
        // instance, only the named card being in hand from here on.
        "look_and_take" | "return_to_hand" => {
            let name = &event.cards[0];
            materialize(game, player, Zone::Hand, name);
            None
        }

        // "X was Rested." -- always immediately followed, in this
        // corpus, by the "status_effect" event that performs the same
        // card's actual state change (see below); narration only here.
        "rested" => None,

        // "X won't stand during its next Stand Phase." QA #61's shape
        // exactly: `choose_and_rest_wont_stand_next` both rests (a no-op
        // if the paired "rested" line already did, or if the target was
        // already resting) and marks the one-shot suppression. Searched
        // across both players, same reasoning as "destroyed" -- the
        // effect's source and its target are different players here
        // (FluffysFury's ability, Edify's Chillet), and
        // `choose_and_rest_wont_stand_next`'s own `player` parameter is
        // unused by the engine, so which one is passed doesn't matter.
        "status_effect" => {
            let name = &event.cards[0];
            let card = find_by_name(game, PlayerId::P1, Zone::Base, name)
                .or_else(|| find_by_name(game, PlayerId::P2, Zone::Base, name))
                .expect("status_effect: card not found in base");
            game.choose_and_rest_wont_stand_next(player, card);
            None
        }

        other => panic!("apply_event: unhandled action {other:?} at line {} ({:?})", event.line_index, event.text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(action: &str, actor: &str, phase: &str, cards: &[&str], targets: &[&str], resolution: Option<serde_json::Value>, caused_by_line: Option<usize>) -> ReplayEvent {
        ReplayEvent {
            line_index: 0,
            turn: 1,
            phase: phase.to_string(),
            actor: actor.to_string(),
            action: action.to_string(),
            cards: cards.iter().map(|s| s.to_string()).collect(),
            targets: targets.iter().map(|s| s.to_string()).collect(),
            resolution,
            caused_by_line,
            text: String::new(),
        }
    }

    fn meta() -> ReplayMeta {
        ReplayMeta {
            record_id: "test".to_string(),
            deck: "test".to_string(),
            player: "Edify (P1)".to_string(),
            opponent: "Opponent".to_string(),
            opponent_colors: "Blue/Purple".to_string(),
            result: "WIN turn 1".to_string(),
            notes: vec![],
        }
    }

    #[test]
    fn opponent_of_replay_owner_defaults_to_p2() {
        assert_eq!(opponent_of_replay_owner(&meta()), PlayerId::P2);
    }

    #[test]
    fn replay_owner_defaults_to_p1() {
        assert_eq!(replay_owner(&meta()), PlayerId::P1);
    }

    #[test]
    fn a_direct_deploy_lands_in_base_with_real_stats() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Elphidran – Gentle Radiance"], &[], Some(serde_json::json!({"slot": 1, "cost": 4})), None));

        let card = find_by_name(&game, PlayerId::P1, Zone::Base, "Elphidran – Gentle Radiance").unwrap();
        assert_eq!(game.cost(card), 4);
        assert_eq!(game.strike(card), 2);
        assert!(last_on_deploy_triggered(&game));
    }

    #[test]
    fn a_chillet_reveal_free_deploys_an_eligible_dragon() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("reveal", "player", "main", &["Jormuntide – Surging Sea Serpent"], &[], None, None));

        let revealed = find_by_name(&game, PlayerId::P1, Zone::Base, "Jormuntide – Surging Sea Serpent").unwrap();
        assert_eq!(game.cost(revealed), 8);
        assert!(last_on_deploy_triggered(&game), "the free-deployed card's own On Deploy must be reported as triggered");
    }

    #[test]
    fn a_non_lucky_attack_reduces_life_by_the_attackers_strike() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Elphidran – Gentle Radiance"], &[], None, None));

        let check = apply_event(
            &mut driver,
            &mut game,
            &meta(),
            &event(
                "attack",
                "player",
                "main",
                &["Elphidran – Gentle Radiance"],
                &["opponent"],
                Some(serde_json::json!({"damage": 2, "milled": 2, "lucky": false, "life_before": 10, "life_after": 8})),
                None,
            ),
        )
        .expect("a non-Lucky attack with life_after in resolution must return a LifeCheck");

        assert_eq!(check.player, PlayerId::P2);
        assert_eq!(check.expected_life, 8);
        assert_eq!(game.life(PlayerId::P2), 8, "the real mill loop, given the correct Strike, must land on the log's own number");
    }

    #[test]
    fn a_lucky_attack_cancels_and_returns_no_life_check() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Chillet – Dragon Whisperer"], &[], None, None));
        let starting_life = game.life(PlayerId::P2);

        let check = apply_event(
            &mut driver,
            &mut game,
            &meta(),
            &event(
                "attack",
                "player",
                "main",
                &["Chillet – Dragon Whisperer"],
                &["opponent"],
                Some(serde_json::json!({"damage_attempted": 2, "damage": 0, "milled": 1, "lucky": true})),
                None,
            ),
        );

        assert!(check.is_none(), "no life_after in the log's resolution means nothing to assert");
        assert_eq!(game.life(PlayerId::P2), starting_life, "the seeded Lucky card must actually cancel the damage");
    }

    #[test]
    fn an_interrupted_attack_concludes_the_battle_without_damage() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Jormuntide – Surging Sea Serpent"], &[], None, None));
        let starting_life = game.life(PlayerId::P2);

        apply_event(&mut driver, &mut game, &meta(), &event("attack_interrupted", "player", "main", &["Jormuntide – Surging Sea Serpent"], &[], None, None));

        let attacker = find_by_name(&game, PlayerId::P1, Zone::Base, "Jormuntide – Surging Sea Serpent").unwrap();
        assert!(game.attack_was_interrupted(attacker));
        assert_eq!(game.life(PlayerId::P2), starting_life);

        // 9.7.4: the battle must have actually concluded, or the next
        // declare_attack would panic -- exercise that directly.
        apply_event(&mut driver, &mut game, &meta(), &event("stand_phase", "player", "stand", &[], &[], None, None));
        apply_event(
            &mut driver,
            &mut game,
            &meta(),
            &event("attack", "player", "main", &["Jormuntide – Surging Sea Serpent"], &["opponent"], Some(serde_json::json!({"damage": 3, "milled": 3, "lucky": false, "life_before": 10, "life_after": 7})), None),
        );
        assert_eq!(game.life(PlayerId::P2), 7);
    }

    #[test]
    fn destroyed_removes_the_named_opponent_owned_card() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Jormuntide – Surging Sea Serpent"], &[], None, None));

        apply_event(&mut driver, &mut game, &meta(), &event("play_card", "opponent", "waiting", &["Strike from the Darkness"], &[], None, None));
        apply_event(&mut driver, &mut game, &meta(), &event("destroyed", "opponent", "waiting", &["Jormuntide – Surging Sea Serpent"], &[], None, None));

        let victim = find_by_name(&game, PlayerId::P1, Zone::Graveyard, "Jormuntide – Surging Sea Serpent");
        assert!(victim.is_some(), "the named card must end up in ITS OWNER's graveyard");
    }

    #[test]
    fn rocket_launcher_act_then_granted_sweep_via_power_buff_and_activate_events() {
        let mut game = new_game_for_replay(&meta());
        let mut driver = ReplayDriver::default();
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "opponent", "waiting", &["Pengullet Rocket Launcher"], &[], None, None));
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "opponent", "waiting", &["Pengullet – Yearning for the Sky"], &[], None, None));
        apply_event(&mut driver, &mut game, &meta(), &event("deploy", "player", "main", &["Elphidran – Gentle Radiance"], &[], None, None)); // a victim for the sweep

        apply_event(&mut driver, &mut game, &meta(), &event("activate", "opponent", "waiting", &["Pengullet Rocket Launcher"], &[], None, None));
        apply_event(&mut driver, &mut game, &meta(), &event("power_buff", "opponent", "waiting", &["Pengullet – Yearning for the Sky"], &[], Some(serde_json::json!({"amount": 500})), None));

        let pengullet = find_by_name(&game, PlayerId::P2, Zone::Base, "Pengullet – Yearning for the Sky").unwrap();
        assert_eq!(game.power(pengullet), 1000 + 500, "test_scenario's default Pal Power (1000) plus the Pengullet upgrade's +500");
        let hand_before_sweep = game.hand_size(PlayerId::P2);

        apply_event(&mut driver, &mut game, &meta(), &event("activate", "opponent", "waiting", &["Pengullet – Yearning for the Sky"], &[], None, None));

        let victim = find_by_name(&game, PlayerId::P1, Zone::Base, "Elphidran – Gentle Radiance").unwrap();
        assert_eq!(game.damage_taken(victim), 1500, "X = the holder's Power at sweep time");
        assert_eq!(game.zone_of(pengullet), Zone::Graveyard, "the holder puts itself into the graveyard");
        assert_eq!(
            game.hand_size(PlayerId::P2),
            hand_before_sweep + 1,
            "BP01-028's own AUTO graveyard trigger (draw 1) must fire here too -- base -> graveyard via the granted sweep's own effect satisfies QA #41's condition, even though battle1's own log never shows this draw (see the README's known-anomaly entry)"
        );
    }
}
