//! Drives a parsed Format A replay log (`parser/format_a_parser.py`'s own
//! JSON schema -- line_index, turn, phase, actor, action, cards, targets,
//! resolution, caused_by_line, text) through `engine::` + `cards::`,
//! scoped to exactly one hand-verified fixture: `pingu_01`
//! (`tests/parser/test_format_a_pingu01.py`'s own 138-event fixture,
//! `tests/format_a_replays/fixtures/pingu_01.json`). Not a general Format
//! A driver -- see `replay/`'s own module doc comment for why a driver
//! scoped to one known sequence is allowed judgment calls a real
//! player-facing driver couldn't make; every one of those judgment calls
//! applies here too, plus the ones below that are specific to this
//! format.
//!
//! ## Every card with a `cards::` script must be called for real
//!
//! Same discipline as `replay/`, built in from the start this time
//! instead of retrofitted: [`SCRIPTED_ACTIONS`] is a registry of (card
//! name, action) pairs that must never reach a generic-primitive
//! fallback. `apply_event`'s "draw" and "play_card" arms panic if a
//! registered pair falls through anyway.
//!
//! One addition this format needs that `replay/`'s registry didn't:
//! SCRIPTED_ACTIONS only lists a pair when the LOG gives enough evidence
//! to call the real script correctly. Strike from the Darkness is
//! genuinely implemented (`cards::td02::strike_from_the_darkness`) and
//! genuinely played twice in pingu_01 (lines 65, 76) -- but neither play
//! is followed by a "destroyed" line naming a target, unlike the
//! bracketed corpus's battle1. Calling the real function anyway would
//! mean fabricating a target the log never states, which is worse than
//! not calling it: SCRIPTED_ACTIONS enforces "don't silently substitute
//! a generic primitive when the real script COULD be called correctly,"
//! not "call the real script no matter what." Strike from the Darkness
//! is deliberately absent from the table for exactly this reason -- see
//! the "play_card" arm's own comment.
//!
//! Aurora Guide and Pal Sphere are the same shape as Strike from the
//! Darkness in one way but not another. Of Aurora Guide's 4 plays, only
//! the first (line 35-37) shows the resulting "Drew 1."/"Put X on top"
//! text; the other 3, and Pal Sphere's one play (line 138), show
//! neither. An earlier version of this driver treated that silence the
//! way it treats Strike from the Darkness's -- as evidence the effect
//! didn't happen. Live-client testing overturned that directly: both
//! draw every time they're cast, including twice in the same turn.
//! `aurora_guide`/`pal_sphere` are therefore called UNCONDITIONALLY at
//! every play now, same as `chillet_on_deploy`'s "add it to hand"
//! branch (also live-client-confirmed to persist every time, Pal or
//! Gear -- an earlier version of that arm had gated it to Pals only,
//! fitted to a hand-size mismatch that turned out to have a different
//! cause). The missing text is a rendering gap in this specific log
//! export, not evidence about the card.
//!
//! ## Hand resync, not hand check
//!
//! Format A's "Turn opened -- Souls N/N . Hand: ..." line is PREFERRED
//! over an unverifiable engine-derived hand, not AUTHORITATIVE --
//! confirmed weaker by a live-client test (see the correction below):
//! the line can omit cards the player genuinely holds, so treating it
//! as ground truth would be its own bug, just a differently-shaped one.
//! "Preferred" is the honest claim: `apply_event`'s "turn_opened" arm
//! discards every card currently tracked in that event's own player's
//! hand (`Zone::Hand` only -- never the opponent's, since this line
//! describes one seat) and materializes exactly the named cards the log
//! states, fresh, right there. Every following card play until the next
//! checkpoint resolves against real names and real stats from that
//! point on, which is what actually matters: what card script fires and
//! what targets are legal. It is still the better of the two available
//! options -- see "Why not reconstruct identity between checkpoints
//! instead" below -- just not a claim that nothing gets lost.
//!
//! Hand size stopped being an independently-asserted check
//! (`tests/format_a_replays/pingu_01.rs` counts `Check::Hand` results
//! against [`expected_hand_check_count`] only to guard against silently
//! skipping a checkpoint -- it does not compare `expected_size` against
//! the engine's own hand, because after a resync that comparison is
//! tautological). This is a smaller claim than the driver made before,
//! but a true one: hand size was never a SOUND independent check to
//! begin with. Format A's "draw" events never name what was drawn
//! (`event.cards` is always `[]`) -- only a LATER turn_opened line
//! reveals the resulting hand -- so any engine-derived hand the driver
//! built up call-by-call between checkpoints was unverifiable in
//! principle, not just in this implementation. See "Why not
//! reconstruct identity between checkpoints instead" below for why that
//! gap can't be closed by cleverer bookkeeping, either.
//!
//! **Correction: souls are resynced too, not independently verified --
//! an earlier version of this section got that backwards.** `increase_soul`
//! only ever moves `game.zone_count(player, Zone::SoulArea)` UP to match
//! the event's own `souls` field (Format A never logs a separate
//! "placed N souls" line, so this line is the only place that state is
//! ever stated) -- it is the same one-directional resync-toward-the-log
//! shape as the hand fix above, just narrower (never corrects downward,
//! for lack of any event in this fixture where the log's souls value is
//! ever lower than what's already tracked). Matching at all nine of
//! pingu_01's checkpoints therefore proves the resync mechanism is
//! self-consistent, not that the engine derives souls correctly from
//! first principles independent of the log -- there is no code path in
//! this driver that derives a soul count any other way. An earlier
//! diagnosis read "souls matched while hand drifted" as evidence the
//! turn_opened line itself was trustworthy; that inference doesn't
//! hold, since souls matched BECAUSE they were being pulled from the
//! same line, the same way hand now is. The line may still be
//! trustworthy -- but not for that reason, and this module doesn't
//! currently have one. Life totals remain the only state Format A
//! checks that the engine derives independently of the log and then
//! compares, rather than resyncing toward.
//!
//! **Confirmed by a separate live-client test: the "Turn opened" line
//! can omit a card the player genuinely holds.** pingu_01's own Pyrin
//! Noct (added to Chylith's hand via Chillet's reveal-declined branch,
//! line 29, turn 1) was live-client-confirmed to enter hand immediately
//! -- yet the log's own hand list doesn't name it until turn 18, with
//! zero intervening reference to it anywhere in between (checked
//! directly; see CLAUDE.md's Open Questions for the full trace). This
//! is exactly why "preferred," not "authoritative," above -- the resync
//! can silently drop a real card for as long as the log's own omission
//! lasts. Doesn't reverse the resync decision (still strictly better
//! than the identity corruption the alternative produced, below), but
//! it's a real, accepted cost of it, not a hypothetical one.
//!
//! ### Why not reconstruct identity between checkpoints instead
//!
//! Tried, twice in one session before this one and once in this
//! session -- reverted every time, and the reason is now understood
//! well enough to write down rather than rediscover. `take_from_hand`
//! (used to resolve every card played, deployed, or discarded by name)
//! falls back to fabricating a fresh card whenever no tracked card
//! matches. The tempting fix is: prefer renaming an existing anonymous
//! (blank, `main_name_of` == "") card over fabricating a new one, since
//! a named play is direct evidence some earlier anonymous draw WAS that
//! card. Implemented and measured directly against pingu_01: it does
//! make blanks stop accumulating, but it does NOT converge hand size to
//! zero drift, because the blank pool has more than one legitimate
//! claimant and nothing in a single log line distinguishes them. Two
//! different situations look identical from inside `take_from_hand` --
//! (1) a genuine unresolved anonymous draw, which really is the card
//! being looked up, and (2) a card this driver never tracked at all,
//! the same shape as pingu_01's own turn-1 Chillet (deployed at line 28
//! with no matching entry in the very first turn_opened hand list) --
//! and this recurs throughout the game (a second "Chillet" deployed at
//! line 38, a second "Pengullet" discarded at line 69 after the one
//! seeded copy had already left hand). A greedy first-blank match lets
//! case 2 steal a blank that rightfully belonged to case 1: line 36's
//! one anonymous draw should have resolved to "Jormuntide" (the only
//! candidate before it turns up by name in turn 5's hand) but line 38's
//! untracked second Chillet grabbed it first and relabeled it
//! "Chillet," permanently losing the real Jormuntide identity and
//! forcing its eventual `deploy` (line 128) to fabricate a substitute
//! anyway.
//!
//! That outcome is worse than the size drift it was meant to fix.
//! Undercounting hand size is visible, bounded, and leaves every card
//! correctly named. Silently relabeling a real Jormuntide as "Chillet"
//! is the failure class this whole project exists to guard against --
//! it can change which card script fires and which targets are legal,
//! and it does not announce itself. Resolving which of the two cases
//! applies would require reconstructing identity FORWARD from a draw to
//! whichever later name claims it, using information (the full
//! sequence of who-drew-what) this log never provides -- a genuinely
//! harder problem than hand-size bookkeeping, not a sharper version of
//! the same heuristic. Resyncing from each checkpoint sidesteps it
//! entirely instead of solving it: no blank pool, no claimants, no
//! guessing.
//!
//! ## Seat resolution -- derived from the body, never assumed
//!
//! Format A has no fixed "logging player" perspective (`replay/`'s
//! `player`/`opponent` split doesn't exist here) -- `event.actor` is
//! always the literal participant name or "Opponent," and a target can
//! be that same literal name OR the pronoun "them"/"they." Both are
//! resolved via `Participants`, built from the match's own `MATCH:`
//! line (never a hardcoded name), with a manifest cross-check left as
//! future work rather than done per-fixture (`match_list.tsv` isn't
//! wired up in this crate; see the corpus README's own attribution-traps
//! section for why a mismatch there should be an error, not a silent
//! fallback, once it is).
//!
//! ## The stand phase, and "most recent" meaning "most recently
//! deployed," not "most recently created"
//!
//! Format A logs neither a turn boundary nor a stand phase as their own
//! line -- `apply_event` detects both from `event.turn` changing (see
//! `ReplayDriver::last_turn`/`last_stood_owner`'s own doc comments),
//! since 7.2.1 (the stand phase stands only the turn player's own
//! cards) turned out to matter here: a Chillet that legitimately
//! attacked, then several lines later became an illegal Pal-attack
//! target because this driver never stood it back up, was the first bug
//! this fixture surfaced that had nothing to do with any specific card's
//! effect text.
//!
//! The second: `find_by_name` (used for every same-named-duplicate
//! lookup) checks `game.last_deployed()` before falling back to "most
//! recently CREATED matching card." Those differ once a card is
//! revived (graveyard -> hand -> base, e.g. via Zoe's Strategy): its
//! `CardId` is old, assigned once at creation, but it's the CURRENT
//! board representative of its name the moment it redeploys. Picking
//! by `CardId` order alone attacked with an already-rested, wrong
//! Chillet instead of the freshly-redeployed one the log actually
//! meant, then treated the fresh one (never having attacked, still
//! standing) as an illegal target three lines later -- the exact same
//! 9.2.3 panic the stand-phase fix above also produces symptoms for,
//! but from the opposite direction. Both were found by iterating this
//! ONE fixture to green, not designed in ahead of time.
use std::collections::HashMap;

use engine::combat::AttackTarget;
use engine::state::{CardId, GameState, PlayerId, Zone};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FormatAMeta {
    pub match_title: Option<String>,
    #[serde(rename = "recordId")]
    pub record_id: Option<String>,
    pub result: Option<String>,
    pub notes: Vec<String>,
    pub provenance: String,
    pub gap_expectation: String,
    pub opponent_anonymized: bool,
    pub word_wrap_joins: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormatAEvent {
    pub line_index: usize,
    pub turn: u32,
    pub phase: Option<String>,
    pub actor: String,
    pub action: String,
    pub cards: Vec<String>,
    pub targets: Vec<String>,
    pub resolution: Option<serde_json::Value>,
    pub caused_by_line: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormatALog {
    pub meta: FormatAMeta,
    pub events: Vec<FormatAEvent>,
}

pub struct LifeCheck {
    pub player: PlayerId,
    pub expected_life: i64,
}

pub struct HandCheck {
    pub player: PlayerId,
    pub expected_size: usize,
}

pub enum Check {
    Life(LifeCheck),
    Hand(HandCheck),
}

/// How many "turn_opened" lines the fixture itself contains -- what
/// `Check::Hand` results returned by a full replay should sum to. Same
/// spirit as `replay::LifeCheck`'s own count guard: a driver that
/// silently skips a line it doesn't recognize would otherwise still
/// "pass."
pub fn expected_hand_check_count(log: &FormatALog) -> usize {
    log.events.iter().filter(|e| e.action == "turn_opened").count()
}

pub fn expected_life_check_count(log: &FormatALog) -> usize {
    log.events.iter().filter(|e| e.resolution.as_ref().and_then(|r| r.get("life_after")).is_some()).count()
}

/// Maps every valid actor/target spelling to a seat, derived from the
/// match's own `MATCH:` line -- see the module doc comment's "Seat
/// resolution" section. `match_title`'s shape is `"<side A> vs <side
/// B>  [annotation]"`; each side's own leading token (before the first
/// `(`/`[`) plus any `"quoted alias"` on that side all map to the SAME
/// seat, positionally (side A = P1, side B = P2) -- mirrors
/// `format_a_parser.py`'s own `_participant_aliases`, but tracks WHICH
/// seat each alias belongs to, which that function deliberately doesn't
/// need to (it only structures the log; a replay driver resolves the
/// rest, per that module's own doc comment).
pub struct Participants {
    seats: HashMap<String, PlayerId>,
}

impl Participants {
    pub fn from_match_title(match_title: &str) -> Self {
        let mut seats = HashMap::new();
        let sides: Vec<&str> = match_title.splitn(2, " vs ").collect();
        for (side, seat) in sides.iter().zip([PlayerId::P1, PlayerId::P2]) {
            let lead = side.split(['(', '[']).next().unwrap_or("").trim();
            if !lead.is_empty() {
                seats.insert(lead.to_lowercase(), seat);
            }
            for quoted in extract_quoted(side) {
                seats.insert(quoted.to_lowercase(), seat);
            }
        }
        Participants { seats }
    }

    /// The seat for `event.actor`, or a named target that's actually a
    /// player reference (a participant alias, or the pronoun
    /// "them"/"they" -- see the module doc comment). `None` means `name`
    /// isn't a player at all, i.e. it names a Pal instead.
    pub fn resolve(&self, actor_seat: PlayerId, name: &str) -> Option<PlayerId> {
        let lower = name.to_lowercase();
        if lower == "them" || lower == "they" {
            return Some(opponent_of(actor_seat));
        }
        self.seats.get(&lower).copied()
    }

    pub fn seat_of_actor(&self, actor: &str) -> PlayerId {
        self.seats.get(&actor.to_lowercase()).copied().unwrap_or_else(|| panic!("Participants: {actor:?} is not a recognized participant alias"))
    }
}

fn extract_quoted(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('"') {
            out.push(&after[..end]);
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

pub fn opponent_of(player: PlayerId) -> PlayerId {
    if player == PlayerId::P1 {
        PlayerId::P2
    } else {
        PlayerId::P1
    }
}

/// Cross-event memory a couple of card effects need because the log
/// splits one card effect across two or more adjacent lines -- see
/// `replay::ReplayDriver`'s own doc comment for the same pattern.
#[derive(Default)]
pub struct ReplayDriver {
    /// Event indices strictly before which this index has already been
    /// fully handled by an earlier event's own lookahead (Aurora Guide's
    /// deferred "draw"/"put_on_top", Zoe's Strategy's deferred
    /// return/discard/destroy) -- checked first, before dispatch.
    narration_until: HashMap<usize, ()>,
    /// The last `FormatAEvent::turn` number this driver has crossed into
    /// (detects a new "TURN N" block -- format_a_parser consumes that
    /// line, plus the all-caps actor header, as control lines its own
    /// module doc comment describes; neither reaches `apply_event` as
    /// an event, so the boundary has to be read off `event.turn`
    /// changing instead).
    last_turn: Option<u32>,
    /// The seat this driver last actually ran 7.2.1's stand phase for.
    /// A new turn NUMBER (above) is necessary but not sufficient:
    /// pingu_01 has "TURN 11" and "TURN 12" both belonging to Chylith
    /// back to back (the same oddity already seen at "TURN 1"/"TURN 2"
    /// -- see CLAUDE.md's Open Questions), and standing Chylith's own
    /// cards again at "TURN 12" is what caused the very bug this field
    /// exists to fix: a Chillet that legitimately attacked during "TURN
    /// 11" got stood back up one block later and was illegally
    /// available as a Pal target three lines after that, contradicting
    /// the log's own successful battle_attack there. Two consecutive
    /// turn NUMBERS for the SAME owner are therefore treated as at most
    /// one real stand phase, not two -- only a turn number whose owner
    /// DIFFERS from the previous one's actually stands anything.
    last_stood_owner: Option<PlayerId>,
}

impl ReplayDriver {
    fn mark_narration(&mut self, index: usize) {
        self.narration_until.insert(index, ());
    }

    fn is_narration(&self, index: usize) -> bool {
        self.narration_until.contains_key(&index)
    }
}

/// (card name, `FormatAEvent::action`) pairs that have a dedicated
/// `cards::` script AND enough evidence in this specific log to call it
/// correctly -- see the module doc comment's two sections on this. Add
/// an entry here in the same commit that adds a script this driver
/// needs to call.
const SCRIPTED_ACTIONS: &[(&str, &str)] = &[("Aurora Guide", "play_card"), ("Pal Sphere", "play_card"), ("Zoe's Strategy", "play_card")];

pub fn new_game_for_replay(_meta: &FormatAMeta) -> GameState {
    GameState::test_scenario()
}

/// Per-card facts this replay needs that aren't in the JSON itself --
/// sourced from `data/11cardattributes-with-elements.csv`'s Cost/
/// Strike/Elements columns, keyed by bare MAIN name (Format A never
/// gives the full "Main -- Subtitle" form, unlike the bracketed
/// format's own `card_meta`) -- hard rule 1/3. Where a main name has
/// more than one print (Pengullet: BP01-028 and TD01-014), the print
/// this replay's own events actually need is the one used; nothing in
/// pingu_01 depends on Pengullet's own Strike, so the choice between
/// prints doesn't affect any checked outcome here.
struct CardMeta {
    cost: u32,
    strike: i32,
    elements: &'static [&'static str],
}

fn card_meta(main_name: &str) -> Option<CardMeta> {
    match main_name {
        "Astegon" => Some(CardMeta { cost: 8, strike: 3, elements: &["Dragon", "Dark"] }),
        "Aurora Guide" => Some(CardMeta { cost: 2, strike: 0, elements: &[] }),
        "Chillet" => Some(CardMeta { cost: 5, strike: 2, elements: &["Ice", "Dragon"] }),
        "Cryolinx" => Some(CardMeta { cost: 6, strike: 2, elements: &["Ice"] }),
        "Elphidran" => Some(CardMeta { cost: 4, strike: 2, elements: &["Dragon"] }),
        "Jormuntide" => Some(CardMeta { cost: 8, strike: 3, elements: &["Dragon", "Water"] }),
        "Lyleen Noct" => Some(CardMeta { cost: 8, strike: 3, elements: &["Dark"] }),
        "Pal Sphere" => Some(CardMeta { cost: 4, strike: 0, elements: &[] }),
        "Pengullet" => Some(CardMeta { cost: 4, strike: 2, elements: &["Water", "Ice"] }),
        "Pengullet Rocket Launcher" => Some(CardMeta { cost: 4, strike: 0, elements: &[] }),
        "Pyrin Noct" => Some(CardMeta { cost: 7, strike: 3, elements: &["Fire", "Dark"] }),
        "Reindrix" => Some(CardMeta { cost: 5, strike: 2, elements: &["Ice"] }),
        "Relaxaurus" => Some(CardMeta { cost: 7, strike: 3, elements: &["Dragon", "Water"] }),
        "Strike from the Darkness" => Some(CardMeta { cost: 4, strike: 0, elements: &[] }),
        "Zoe's Strategy" => Some(CardMeta { cost: 3, strike: 0, elements: &[] }),
        _ => None,
    }
}

/// Creates a fresh, real-named, real-statted card in `zone` for `owner`
/// -- the "first time this name appears" moment, same role as
/// `replay::materialize`.
fn materialize(game: &mut GameState, owner: PlayerId, zone: Zone, main_name: &str) -> CardId {
    let card = game.place(owner, main_name, zone);
    game.set_card_name(card, main_name);
    if let Some(meta) = card_meta(main_name) {
        game.set_cost(card, meta.cost);
        game.set_strike(card, meta.strike);
        game.set_elements(card, meta.elements.iter().map(|s| s.to_string()).collect());
    }
    card
}

/// The card representing `main_name` for `owner` in `zone` -- for
/// `Zone::Base` specifically, `game.last_deployed()` is checked FIRST if
/// it's a match (the same "current instance" signal `replay/` already
/// uses for OnDeploy chaining), before falling back to the most
/// recently CREATED matching card. Those two aren't the same thing once
/// a card is revived (graveyard -> hand -> base): its `CardId` is old
/// (assigned once, at creation), but it's the CURRENT board
/// representative of its name the moment it redeploys -- found the hard
/// way, in pingu_01, where a Chillet returned via Zoe's Strategy and
/// redeployed (line 119) while an OLDER-created, already-rested Chillet
/// from turns earlier was still alive; picking by CardId order alone
/// attacked with the wrong one and later made the freshly-redeployed
/// one (never having attacked, still standing) an illegal Pal target.
fn find_by_name(game: &GameState, owner: PlayerId, zone: Zone, main_name: &str) -> Option<CardId> {
    if zone == Zone::Base {
        if let Some(card) = game.last_deployed() {
            if game.owner_of(card) == owner && game.zone_of(card) == Zone::Base && game.main_name_of(card) == main_name {
                return Some(card);
            }
        }
    }
    game.cards_in_zone(owner, zone).into_iter().rev().find(|&id| game.main_name_of(id) == main_name)
}

fn find_by_name_either_owner(game: &GameState, zone: Zone, main_name: &str) -> Option<CardId> {
    find_by_name(game, PlayerId::P1, zone, main_name).or_else(|| find_by_name(game, PlayerId::P2, zone, main_name))
}

/// An existing base card by name if one's already tracked, or a fresh
/// materialization directly into Base if not -- for combat lookups
/// specifically (an attacker or defender named in an "attack"/
/// "attack_declare"/"battle_attack" line). pingu_01 only ever reveals
/// CHYLITH's hand, never Pingu's, so Pingu's own board history before
/// this replay's visible window (a Chillet already attacking at line 45,
/// with no earlier "deploy" line for it anywhere in this file) is
/// exactly as unknowable as the true 50-card deck's contents beyond what
/// the log names -- same judgment call `take_from_hand`/`replay/`'s own
/// module doc comment already make, applied to Base instead of Hand.
/// Deliberately NOT used for handlers whose target should already have
/// been established by an earlier VISIBLE event in this same window
/// (status_effect, power_buff, destroyed) -- a miss there is this
/// driver's own bug, not a legitimate off-screen gap, and should keep
/// failing loudly.
fn find_by_name_or_materialize(game: &mut GameState, owner: PlayerId, main_name: &str) -> CardId {
    find_by_name(game, owner, Zone::Base, main_name).unwrap_or_else(|| materialize(game, owner, Zone::Base, main_name))
}

/// Same fallback as `find_by_name_or_materialize`, for a DEFENDER
/// specifically (a Pal-vs-Pal attack's target). 9.2.3: only a RESTED Pal
/// is a legal attack target -- the log's own declaration is already
/// proof the real target was resting at this moment, so a fallback-
/// materialized one (freshly created, standing by default) is rested
/// immediately here rather than left in the state that would make the
/// very call site that needed it illegal.
fn find_defender_or_materialize(game: &mut GameState, owner: PlayerId, main_name: &str) -> CardId {
    match find_by_name(game, owner, Zone::Base, main_name) {
        Some(card) => card,
        None => {
            let card = materialize(game, owner, Zone::Base, main_name);
            game.rest(card);
            card
        }
    }
}

/// An existing hand card by name if one's already tracked, or a fresh
/// materialization if not -- see the module doc comment's hand-check
/// section: a card whose true hand-membership predates what this
/// replay can observe (pingu_01's own Chillet, deployed turn 1 before
/// ever appearing in a "Hand:" list) is exactly as unknowable as the
/// true 50-card deck's contents beyond what the log names (`replay/`'s
/// own module doc comment) -- materializing it fresh, right where it's
/// first needed, is the same judgment call that document already makes
/// for the bracketed format.
fn take_from_hand(game: &mut GameState, player: PlayerId, main_name: &str) -> CardId {
    find_by_name(game, player, Zone::Hand, main_name).unwrap_or_else(|| materialize(game, player, Zone::Hand, main_name))
}

fn res_u64(event: &FormatAEvent, key: &str) -> Option<u64> {
    event.resolution.as_ref()?.get(key)?.as_u64()
}

fn res_i64(event: &FormatAEvent, key: &str) -> Option<i64> {
    event.resolution.as_ref()?.get(key)?.as_i64()
}

fn res_bool(event: &FormatAEvent, key: &str) -> Option<bool> {
    event.resolution.as_ref()?.get(key)?.as_bool()
}

fn res_str_list(event: &FormatAEvent, key: &str) -> Vec<String> {
    event
        .resolution
        .as_ref()
        .and_then(|r| r.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Applies one parsed log event to `game`. `events`/`i` (the full
/// sequence and this event's own index) let a handler look ahead at
/// what immediately follows when a single card effect spans more than
/// one log line -- Aurora Guide's deferred draw+put-on-top, Zoe's
/// Strategy's three observed shapes -- consuming those indices via
/// `driver.mark_narration` so the main loop's later call for that index
/// is a no-op rather than double-applying the effect.
pub fn apply_event(driver: &mut ReplayDriver, game: &mut GameState, participants: &Participants, events: &[FormatAEvent], i: usize) -> Option<Check> {
    if driver.is_narration(i) {
        return None;
    }
    let event = &events[i];

    // 7.2.1: the stand phase stands only the TURN PLAYER's own cards --
    // detected here from `event.turn` changing, since Format A logs
    // neither a turn boundary nor a stand phase as their own event (see
    // `ReplayDriver::last_turn`'s own doc comment). This event's own
    // actor is the incoming turn's player: format_a_parser only ever
    // reaches a new turn number by first consuming that turn's own
    // all-caps actor header as a control line, so the first EVENT this
    // driver sees at a new turn number is always that header's actor,
    // never a stray reference from the other player.
    if driver.last_turn != Some(event.turn) {
        driver.last_turn = Some(event.turn);
        let turn_owner = participants.seat_of_actor(&event.actor);
        if driver.last_stood_owner != Some(turn_owner) {
            driver.last_stood_owner = Some(turn_owner);
            game.stand_phase(turn_owner);
        }
    }

    let player = participants.seat_of_actor(&event.actor);
    let opponent = opponent_of(player);

    match event.action.as_str() {
        // Pure narration/markers -- no state change of their own.
        // "rested" always precedes the "status_effect" line that does
        // the real work (Relaxaurus's lock), same as `replay/`.
        // "reveal_declined" is Elphidran's own OnAttack "you may reveal"
        // being skipped entirely. "no_valid_target" covers an
        // explicit-in-text declined "up to 1" choice (Relaxaurus's
        // OnDeploy, this replay's one occurrence). "bare_marker" is
        // covered by its own arm below, which still returns None but
        // documents why separately.
        "rested" | "reveal_declined" | "no_valid_target" | "activate" | "win_declared" | "loss_reason" | "final_life" => None,

        // A lone card name or "*" marker with no verb of its own --
        // format_a_parser's own module doc comment: "its exact rules
        // meaning is not established." Every occurrence in pingu_01
        // immediately precedes an event this driver already handles
        // directly (an attack_interrupted, a destroyed, a Lucky-cancel
        // attack) -- narration only here, same treatment `replay/` gives
        // "rested".
        "bare_marker" => None,

        "turn_opened" => {
            let hand = res_str_list(event, "hand");
            let souls = res_u64(event, "souls").unwrap_or(0) as usize;

            // Resync, not seed-once: this line is PREFERRED over an
            // unverifiable engine-derived hand for THIS event's own
            // player, every time it fires, not just the first -- not
            // authoritative (confirmed capable of omitting a real card;
            // see the module doc comment). Discard whatever the engine
            // currently holds for this player's hand (never the
            // opponent's -- this line describes one seat) and
            // materialize exactly what the log states. See the module
            // doc comment's "Hand resync, not hand check" section for
            // why.
            for card in game.cards_in_zone(player, Zone::Hand) {
                game.move_to_graveyard(card, Zone::Hand);
            }
            for name in &hand {
                materialize(game, player, Zone::Hand, name);
            }

            let current_souls = game.zone_count(player, Zone::SoulArea);
            if souls > current_souls {
                game.increase_soul(player, souls - current_souls);
            }

            Some(Check::Hand(HandCheck { player, expected_size: hand.len() }))
        }

        "deploy" => {
            let name = &event.cards[0];
            if event.caused_by_line.is_some() {
                // Already performed by the preceding "reveal" event's
                // chillet_on_deploy call.
                return None;
            }
            let card = take_from_hand(game, player, name);
            game.deploy_from(player, card, Zone::Hand, Zone::Base);
            None
        }

        "reveal" => {
            if let Some(resolution) = &event.resolution {
                if resolution.get("element").is_some() {
                    // BP01-098 Elphidran's OnAttack: the revealed card
                    // is named in `event.cards`, but the ATTACKER whose
                    // Power this buffs is not -- checked directly
                    // against every occurrence in pingu_01, the
                    // attacker is always Elphidran itself, already on
                    // the field by the time this fires.
                    let revealed_name = &event.cards[0];
                    let revealed = take_from_hand(game, player, revealed_name);
                    let attacker = find_by_name(game, player, Zone::Base, "Elphidran").expect("reveal: Elphidran's OnAttack fired but no Elphidran in base for this player");
                    cards::bp01::elphidran_gr_on_attack(game, attacker, Some(revealed));
                }
                // "outcome": "added_to_hand" -- Chillet's OWN OnDeploy
                // reveal, declined or ineligible branch. Live-client-
                // confirmed (not log-inferred): "if it's ineligible to
                // be deployed, it goes into the hand... cast two
                // Chillets in one turn, you get two chances" -- always
                // persists, Pal or Gear, every occurrence. An earlier
                // version of this arm gated persistence to Pal-only,
                // fitted to a hand-size mismatch that turned out to have
                // a different cause (see CLAUDE.md's Open Questions);
                // reverted once the real client behavior was checked
                // directly instead of inferred from log arithmetic.
                if resolution.get("outcome").map(|v| v == "added_to_hand").unwrap_or(false) {
                    let name = &event.cards[0];
                    materialize(game, player, Zone::Deck, name);
                    cards::bp01::chillet_on_deploy(game, player, false);
                }
                return None;
            }
            // BP01-025 Chillet's OnDeploy reveal, free-deploy branch --
            // same shape as `replay/`'s own "reveal" arm.
            let name = &event.cards[0];
            materialize(game, player, Zone::Deck, name);
            cards::bp01::chillet_on_deploy(game, player, true);
            None
        }

        "put_on_top" => {
            // Only reached if NOT already consumed by Aurora Guide's own
            // "play_card" lookahead (see there) -- i.e. never, in
            // pingu_01, but kept as a real fallback rather than an
            // unreachable panic, matching `replay/`'s own generic
            // "put_on_top" arm.
            let name = &event.cards[0];
            let card = take_from_hand(game, player, name);
            game.move_to_top_of_deck(player, card, Zone::Hand);
            None
        }

        "draw" => {
            let amount = res_u64(event, "amount").unwrap_or(1);
            // BP01-027 Jormuntide's OnDeploy draw -- Format A's own
            // "draw" event carries no card name (`event.cards` is
            // always `[]`), so `last_deployed` stands in for "which
            // card's OnDeploy this is," same discriminator `replay/`
            // already uses for this exact card. "Choose up to 1
            // opponent Pal" has no textual evidence here (no
            // "no_valid_target" line, unlike every bracketed
            // occurrence) -- `None`, consistent with the rest of this
            // driver's "only what's directly observable" scope.
            if amount == 1 && game.last_deployed().map(|c| game.main_name_of(c) == "Jormuntide" && game.zone_of(c) == Zone::Base).unwrap_or(false) {
                cards::bp01::jormuntide_on_deploy(game, player, None);
                return None;
            }
            for _ in 0..amount {
                game.draw(player);
            }
            None
        }

        // "Played X [for N Souls]." -- a Quick/spell/gear-ACT card
        // resolves and then always leaves hand for the graveyard as its
        // own base cost of being played, independent of whatever its
        // own effect does -- modeled here, once, for every play_card
        // event, rather than duplicated in each card-specific branch
        // below.
        "play_card" => {
            let name = &event.cards[0];
            let card = take_from_hand(game, player, name);
            game.deploy_from(player, card, Zone::Hand, Zone::Graveyard);

            match name.as_str() {
                "Aurora Guide" => {
                    // "Draw 1 card, choose up to 1 card from your hand,
                    // and put it on the top of the deck" is ONE function
                    // call, deferred here because the log splits it
                    // across up to two more lines when it renders them at
                    // all. Only the FIRST of pingu_01's 4 occurrences
                    // shows the "Drew 1."/"Put X on top" text (line
                    // 35-37); the other 3 show neither -- but this is a
                    // LOGGING gap, not a real one: live-client-confirmed
                    // (not inferred from this log), Aurora Guide draws
                    // every time it's cast, even twice in one turn. An
                    // earlier version of this arm only called the real
                    // function where the log gave direct textual
                    // evidence, on the theory the missing text meant a
                    // missing effect -- checked directly against the
                    // client and found wrong; see CLAUDE.md's Open
                    // Questions for what changed and why. Always called
                    // now; the "chosen" card is only knowable from a
                    // following "put_on_top" line when the log happens to
                    // render it, so it stays `None` otherwise -- same
                    // "only what's directly observable" scope as every
                    // other hardcoded choice in this driver, just no
                    // longer gating the UNCONDITIONAL draw behind it.
                    let mut chosen = None;
                    if let Some(draw_evt) = events.get(i + 1) {
                        if draw_evt.action == "draw" {
                            driver.mark_narration(i + 1);
                            if let Some(put_evt) = events.get(i + 2) {
                                if put_evt.action == "put_on_top" {
                                    let put_name = &put_evt.cards[0];
                                    chosen = Some(take_from_hand(game, player, put_name));
                                    driver.mark_narration(i + 2);
                                }
                            }
                        }
                    }
                    cards::bp01::aurora_guide(game, player, chosen);
                }
                // "Draw 3 cards" -- same logging gap as Aurora Guide
                // above, same live-client confirmation ("if you want to
                // cast it twice in one turn, you could draw 6"). Always
                // called now.
                "Pal Sphere" => cards::bp01::pal_sphere(game, player),
                "Zoe's Strategy" => {
                    // Three observed shapes -- see the doc comment on
                    // cards::bp01's own zoes_strategy_* functions for
                    // which bullets these are and the open question on
                    // whether two of them are really one combined
                    // bullet the CSV's flattened effect_text lost the
                    // grouping for.
                    if let Some(next) = events.get(i + 1) {
                        match next.action.as_str() {
                            "return_to_hand" => {
                                let returned_name = &next.cards[0];
                                // Same off-screen-history judgment call
                                // as `take_from_hand`/`find_by_name_or_
                                // materialize`: a Pal in THIS player's
                                // own graveyard whose death this replay
                                // never witnessed (Pingu's own board
                                // history before its visible window) is
                                // materialized fresh, directly there,
                                // rather than treated as a driver bug.
                                let returned = find_by_name(game, player, Zone::Graveyard, returned_name).unwrap_or_else(|| materialize(game, player, Zone::Graveyard, returned_name));
                                cards::bp01::zoes_strategy_return_to_hand(game, player, returned);
                                driver.mark_narration(i + 1);

                                if let Some(after) = events.get(i + 2) {
                                    if after.action == "must_discard" {
                                        // Combined shape: "your opponent
                                        // discards" bullet -- see the doc
                                        // comment on cards::bp01's own
                                        // zoes_strategy_* functions.
                                        driver.mark_narration(i + 2);
                                        if let Some(disc) = events.get(i + 3) {
                                            if disc.action == "discard" {
                                                let disc_name = &disc.cards[0];
                                                let disc_card = take_from_hand(game, opponent, disc_name);
                                                cards::bp01::zoes_strategy_force_discard(game, opponent, disc_card);
                                                driver.mark_narration(i + 3);
                                            }
                                        }
                                    } else if after.action == "discard" && after.cards == event.cards {
                                        // "Chylith discarded Zoe's
                                        // Strategy." -- narrating that
                                        // Zoe's Strategy ITSELF ended up
                                        // in the graveyard, already done
                                        // above by the generic play_card
                                        // step; not a forced discard of a
                                        // DIFFERENT card, so this must
                                        // NOT be routed through
                                        // `zoes_strategy_force_discard`.
                                        driver.mark_narration(i + 2);
                                    }
                                }
                            }
                            "destroyed" => {
                                let victim_name = &next.cards[0];
                                let victim = find_by_name(game, player, Zone::Base, victim_name).expect("Zoe's Strategy: butcher target not found in own base");
                                cards::bp01::zoes_strategy_butcher(game, victim);
                                driver.mark_narration(i + 1);
                            }
                            _ => {}
                        }
                    }
                }
                // Strike from the Darkness IS a real, implemented
                // script (cards::td02::strike_from_the_darkness) -- not
                // called here on purpose. Neither of pingu_01's two
                // plays (lines 65, 76) is followed by a "destroyed"
                // line naming a target, unlike battle1's bracketed
                // occurrence, so there is no evidence for what to pass
                // it. Deliberately absent from SCRIPTED_ACTIONS for the
                // same reason -- see the module doc comment.
                "Strike from the Darkness" => {}
                other => {
                    assert!(
                        !SCRIPTED_ACTIONS.contains(&(other, "play_card")),
                        "apply_event: {other:?} is registered in SCRIPTED_ACTIONS for \"play_card\" but this arm doesn't recognize it"
                    );
                }
            }
            None
        }

        "attack_declare" => {
            let name = &event.cards[0];
            let attacker = find_by_name_or_materialize(game, player, name);
            let target_name = &event.targets[0];
            let target = match participants.resolve(player, target_name) {
                Some(seat) => AttackTarget::Player(seat),
                None => {
                    let defender = find_defender_or_materialize(game, opponent, target_name);
                    AttackTarget::Pal(defender)
                }
            };
            game.declare_attack(attacker, target);
            None
        }

        "attack" => {
            let name = &event.cards[0];
            let attacker = find_by_name_or_materialize(game, player, name);
            let attack = game.pending_attack().unwrap_or_else(|| {
                let target_seat = event.targets.first().and_then(|t| participants.resolve(player, t)).unwrap_or(opponent);
                game.declare_attack(attacker, AttackTarget::Player(target_seat))
            });

            if res_bool(event, "lucky") == Some(true) {
                let target_seat = event.targets.first().and_then(|t| participants.resolve(player, t)).unwrap_or(opponent);
                if let Some(card) = game.take_top_of_deck(target_seat) {
                    game.grant_keyword(card, "Lucky");
                }
            }

            game.resolve_battle(attack);

            res_i64(event, "life_after").map(|expected_life| {
                let target_seat = event.targets.first().and_then(|t| participants.resolve(player, t)).unwrap_or(opponent);
                Check::Life(LifeCheck { player: target_seat, expected_life })
            })
        }

        "attack_interrupted" => {
            let name = &event.cards[0];
            // The attacker being interrupted isn't necessarily this
            // event's own actor's card -- pingu_01's bare-form
            // interrupts are always the NON-turn-player interrupting
            // the turn player's attack, so the attacker is searched
            // across both bases, same reasoning as `replay/`'s
            // "destroyed"/"status_effect" arms.
            let attacker = find_by_name_either_owner(game, Zone::Base, name).expect("attack_interrupted: attacker not found in either base");
            let attacker_owner = game.owner_of(attacker);
            if game.pending_attack().is_none() {
                game.declare_attack(attacker, AttackTarget::Player(opponent_of(attacker_owner)));
            }
            game.interrupt_attack(attacker);
            None
        }

        "interrupt" => {
            let attack = game.pending_attack().expect("interrupt: no pending attack to nullify");
            let attacker = game.attacker_of(attack);
            game.interrupt_attack(attacker);
            None
        }

        "battle_attack" => {
            let attacker_name = &event.cards[0];
            let attacker = find_by_name_or_materialize(game, player, attacker_name);

            let attack = match game.pending_attack() {
                Some(id) => id,
                None => {
                    let defender_name = &event.targets[0];
                    let defender = find_defender_or_materialize(game, opponent, defender_name);
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
            game.run_check_timing();
            None
        }

        "destroyed" => {
            let name = &event.cards[0];
            if let Some(victim) = find_by_name_either_owner(game, Zone::Base, name) {
                game.move_to_graveyard(victim, Zone::Base);
            }
            None
        }

        "discard" => {
            // A plain cost-payment discard (e.g. Lyleen Noct's own ACT
            // cost) -- Zoe's Strategy's forced-discard shape is already
            // fully consumed by its own "play_card" lookahead above and
            // never reaches here.
            for name in &event.cards {
                let card = take_from_hand(game, player, name);
                game.deploy_from(player, card, Zone::Hand, Zone::Graveyard);
            }
            None
        }

        "must_discard" => None, // narration; the following "discard" does the real move (or is consumed by Zoe's Strategy's lookahead)

        "return_to_hand" => {
            // Fallback only -- pingu_01's 3 occurrences are all
            // consumed by Zoe's Strategy's own "play_card" lookahead.
            let name = &event.cards[0];
            let card = find_by_name_either_owner(game, Zone::Graveyard, name).expect("return_to_hand: card not found in either graveyard");
            let owner = game.owner_of(card);
            game.deploy_from(owner, card, Zone::Graveyard, Zone::Hand);
            None
        }

        "power_buff" => {
            let name = &event.cards[0];
            let target = find_by_name(game, player, Zone::Base, name).expect("power_buff: target not found in base");
            let granted_skill = event.resolution.as_ref().and_then(|r| r.get("granted_skill")).and_then(|v| v.as_str());
            if granted_skill == Some("explode") {
                // BP01-044 Pengullet Rocket Launcher's own upgrade
                // branch -- Format A folds the launcher's own
                // activation directly into this line (no separate
                // "activate Pengullet Rocket Launcher" the way the
                // bracketed format logs it), so the launcher is found
                // by name across both bases rather than assumed to
                // belong to `player`.
                let launcher = find_by_name_either_owner(game, Zone::Base, "Pengullet Rocket Launcher").expect("power_buff: granted_skill \"explode\" but no Pengullet Rocket Launcher in either base");
                cards::bp01::rocket_launcher_act(game, launcher, target);
            } else if let Some(amount) = res_i64(event, "amount") {
                if amount >= 0 {
                    game.increase_power(target, amount as u32);
                } else {
                    // TD02-012 Astegon's own OnDeploy: "Power -1000
                    // until end of turn," the only negative power_buff
                    // in this corpus.
                    cards::td02::astegon_on_deploy(game, target);
                }
            }
            None
        }

        "mass_damage" => {
            // The Pengullet Rocket Launcher's own granted sweep,
            // resolving one power_buff-with-granted_skill event later.
            // `last_deployed` finds the same holder `rocket_launcher_act`
            // was just given -- nothing else deploys in between, in
            // either of pingu_01's two occurrences, same discriminator
            // `replay/` already uses for Jormuntide/Elphidran Aqua.
            //
            // Deliberately NOT using this event's own "scope" text to
            // pick the victim: the already-validated rules reading
            // (`rocket_launcher_granted_sweep`, corpus-validated against
            // battle1's bracketed replay) says the holder's OWN
            // opponent is hit -- live-client-confirmed directly (not
            // log-inferred): "Pengullet rocket launcher specifically
            // states target opponent's pals." pingu_01's SECOND
            // occurrence (line 110, "every Chylith Pal" = opponent of
            // the Pingu-owned holder) matches that. The FIRST (line 64,
            // "every Chylith Pal" = the SAME player who controls the
            // holder) does not -- meaning that one occurrence's own
            // "scope" text is simply wrong/misleading, not evidence of a
            // rules exception. Resolved in favor of the rules text, not
            // the log, per the client check.
            let holder = game.last_deployed().expect("mass_damage: no last_deployed card to identify the sweep's holder");
            let holder_owner = game.owner_of(holder);
            cards::bp01::rocket_launcher_granted_sweep(game, holder, holder_owner);
            None
        }

        "status_effect" => {
            // BP01-026 Relaxaurus's own OnDeploy targeting-rest, the
            // only status_effect shape in this corpus
            // ("wont_stand_while_source_in_play"). `last_deployed`
            // identifies the specific Relaxaurus instance that's the
            // lock's source, same discriminator as above.
            let name = &event.cards[0];
            let target = find_by_name_either_owner(game, Zone::Base, name).expect("status_effect: target not found in either base");
            let source = game.last_deployed().expect("status_effect: no last_deployed card to identify Relaxaurus");
            cards::bp01::relaxaurus_on_deploy(game, Some(target), source);
            None
        }

        other => panic!("apply_event: unhandled action {other:?} at line {} ({:?})", event.line_index, event.text),
    }
}

/// `Participants` + `ReplayDriver` for `log`, ready to feed into
/// `apply_event` in a loop over `0..log.events.len()` -- the caller owns
/// that loop (not a callback-taking helper here) so it can borrow
/// `game` immutably for its own assertions between calls, the same
/// shape `tests/replays/support.rs` already uses for the bracketed
/// format.
pub fn new_driver_for_replay(log: &FormatALog) -> (Participants, ReplayDriver) {
    (Participants::from_match_title(log.meta.match_title.as_deref().unwrap_or("")), ReplayDriver::default())
}
