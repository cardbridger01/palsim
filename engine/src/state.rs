//! Game state: zones, phases, souls, the standby/check-timing queue.
//!
//! Implements exactly what QA #10, #11, #12, #22, #23, #24, #25, #42,
//! #43 (x3), #75, #96 (`../tests/qa_rulings/flow_of_the_game.rs`)
//! require. `GameState::test_scenario()` is a minimal harness state for
//! exercising one rule at a time -- not a real 50-card game (no deck
//! construction legality, no real card data; every deployed "card" is a
//! synthetic filler entry identified only by the number the test
//! passes in).
//!
//! ## The standby/priority model (QA #42, #43)
//!
//! This is the piece CLAUDE.md calls out as the one most likely to be
//! silently wrong, so it's worth writing out explicitly rather than
//! trusting the code to speak for itself:
//!
//! - `trigger_auto` never resolves anything immediately. It only ever
//!   appends to `player`'s `pending_autos` queue (10.8.2: a fulfilled
//!   trigger enters standby state, even mid-resolution of something
//!   else -- see `effect_still_resolving`/QA #42).
//! - `priority_player` is **stateless**: it does not track "whose queue
//!   is currently being walked", it just asks "does the turn player have
//!   *any* pending AUTO right now?" and answers turn player if so, else
//!   the non-turn player. Re-deriving this fresh every time, rather than
//!   remembering "we were in the middle of resolving P2's queue", is
//!   what QA #43's third bullet ("a new turn-player AUTO returns
//!   priority to the turn player") falls out of for free: the instant
//!   the turn player has something pending, priority recomputes to
//!   them, with no separate "interrupt" logic needed.
//! - `resolve_auto` removes one AUTO from its controller's pending queue.
//!   Per QA #43's first bullet ("in any order they choose"), which one
//!   is never checked at resolve time -- only `resolution_order_is_legal`
//!   checks that a claimed order is a permutation of what's pending,
//!   because the ordering constraint is "any order among your own", not
//!   "this specific next one".
//! - Because it's a synthetic test scenario with no real card behind
//!   `source`, "resolving an AUTO generates a new one" (QA #43's second
//!   bullet) is modeled generically: every `resolve_auto` call enqueues
//!   exactly one new placeholder AUTO for the same controller and
//!   returns its id, so a test can assert that new id is eligible to
//!   resolve before older pending ones. This says nothing about any
//!   real card; it's exercising the ordering rule in the abstract.
//! - `pending_autos` is a plain `Vec`, not a set: **the same source can
//!   be in standby twice at once, deliberately.** This is required, not
//!   an accident of the data structure -- 10.8.2.1 ("when an automatic
//!   ability's trigger condition is fulfilled multiple times, that
//!   automatic ability enters the standby state an equal number of
//!   times") and 5.21.1 (an ability that "activates N times" enters
//!   pending N times, not once) both say so directly, and 10.8.3.1
//!   confirms the two instances are independently resolvable ("the
//!   player can choose their order of resolution" when the same AUTO is
//!   pending multiple times). `resolve_auto`'s `.position()` removing
//!   only the first match is exactly what makes that true here.
//!   Encoded end to end by the Shadowbeak/Katress cluster in
//!   `qa_about_cards.rs`: QA #84 is the specification for the whole
//!   model (an AUTO doubled into two standby instances; resolving one
//!   instance butchers its own source; that triggers a second card's
//!   AUTO twice more, counting the now-departed source's doubling
//!   effect at the moment of the trigger, not at resolution time --
//!   10.8.7; the remaining instances then resolve in any order). QA #85
//!   is the adjacent trap for whoever eventually builds the doubling
//!   CONT itself: two *redundant* copies of the identical doubling
//!   effect still only double (2x, never 4x or 3x) -- 5.21.2, a
//!   multiplier applied once per trigger, not once per copy of the CONT
//!   that grants it.
//!
//! ## The rule-action tier above the standby queue (10.5.3)
//!
//! A check timing is not just "resolve pending AUTOs" -- 10.5.3 puts a
//! whole tier above them: **all pending rule actions resolve first,
//! repeated until none remain, and only then do automatic abilities get
//! looked at at all.** `run_check_timing` drains `resolve_one_pending_
//! rule_action` to a genuine fixpoint (10.5.3.1: if resolving one
//! produces a new one, resolve that too, before ever touching AUTOs --
//! e.g. binning an overloaded Pal can put a *different* Pal at lethal
//! damage, which can in turn put a player at a losing verdict; one pass
//! would miss the second and third steps).
//!
//! 11.1.2 also splits rule actions into two kinds that must NOT share a
//! queue: **interrupt-type** ones (player damage resolution / the Lucky
//! mill, 11.2.1) resolve immediately, the instant their condition is
//! met, suspending whatever's in progress -- never deferred to a check
//! timing. **Check-type** ones (`RuleAction` below) only ever resolve at
//! a check timing. Queuing an interrupt-type action here would resolve
//! the Lucky mill at the wrong moment; `RuleAction` is deliberately
//! check-type only, and interrupt-type actions (not modeled yet -- no
//! combat exists in this module) will be ordinary function calls made
//! directly by whatever combat code triggers them, not variants added
//! to this enum.
//!
//! A losing verdict (11.3.3) beats everything else once true: per 1.2.1
//! the game ends immediately, so `run_check_timing` stops the instant
//! one resolves -- it never proceeds to hand priority to any pending
//! AUTO afterward, even the turn player's own. See
//! `rule_1_2_1_losing_verdict_ends_the_game_with_pending_autos_still_unresolved`
//! in flow_of_the_game.rs.

use std::collections::{HashMap, HashSet};

use crate::combat::{AttackId, AttackTarget, Battle};
use crate::targeting::{DeclareNameError, DeclaredName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerId {
    P1,
    P2,
}

impl PlayerId {
    fn other(self) -> PlayerId {
        match self {
            PlayerId::P1 => PlayerId::P2,
            PlayerId::P2 => PlayerId::P1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Hand,
    Deck,
    Base,
    Graveyard,
    SoulDeck,
    SoulArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    OwnMain,
    OwnTurnDuringBattle,
    OpponentTurnOutsideBattle,
    OpponentTurnDuringBattle,
}

/// `pub`, not module-private -- a caller outside this crate has to be
/// able to name a real kind to pass into `place_typed`. Split into
/// `Structure`/`Gear` (was one combined `StructureOrGear` variant)
/// because Victor's Strategy needs to count a controller's structures
/// specifically and Reptyro needs to find a gear specifically -- a
/// combined kind couldn't answer either question.
///
/// `Event` (the CSV's own `Type` for a Quick/spell card -- Aurora
/// Guide, Pal Sphere, Zoe's Strategy, Strike from the Darkness,
/// Medical Supplies) turned out NOT to be a "nothing needs it yet"
/// case the way an earlier version of this comment assumed: those are
/// 5 of `format_a_replay`'s own 24 fixture-referenced names, not a
/// hypothetical. `place_typed` making a real kind mandatory (see its
/// own doc comment) means every one of them needs a real answer to
/// "what kind is this," not just Pals/Structures/Gears -- tagging them
/// `Pal` because they never sit in `Zone::Base` anyway would have been
/// the exact silent-default shape this type exists to rule out, just
/// moved one step over. `Soul` (the fifth CSV value) still isn't a
/// variant here: Soul cards never occupy `Zone::Base` OR get
/// materialized by either driver at all (hard rule 5 excludes them
/// from the build pool entirely), so there is still no real card this
/// type would ever need to tag with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Pal,
    Event,
    Structure,
    Gear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierId(u32);

#[derive(Debug)]
pub struct DeployError(pub String);

/// Result of `deploy_from`/`last_deploy_result`. A struct rather than a
/// bare `bool` so a call site reads as `.on_deploy_triggered`, not an
/// unlabeled boolean, and so the deployed card's own id travels with the
/// result (QA #63 chains straight into `trigger_auto(result.card)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeployFromResult {
    pub card: CardId,
    pub on_deploy_triggered: bool,
}

/// QA #94's duration phrasing ("until end of the opponent's next turn").
/// Only one variant exists because only one is tested; `set_night_until`
/// doesn't yet act on it (see that method's doc comment) -- this is here
/// so the call site states its intent precisely rather than passing a
/// magic string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationSpec {
    EndOfOpponentsNextTurn,
}

struct CardInstance {
    owner: PlayerId,
    zone: Zone,
    kind: CardKind,
    /// Signed, unclamped (1.3.2.4) -- base value before any CONT
    /// modifier; see `reduce_power`/`increase_power` (direct, permanent)
    /// vs `apply_cont_power_modifier` (named, removable) and the
    /// `power()` accessor, which sums both.
    power: i32,
    damage_taken: u32,
    resting: bool,
    keywords: HashSet<String>,
    color: Option<String>,
    elements: Vec<String>,
    work_suitabilities: Vec<String>,
    /// Full printed name (e.g. "Elizabee – Queen of the Flower Garden"),
    /// set separately from `owner`/deploy-time `number` -- see QA #26.
    card_name: Option<String>,
    cont_ability: Option<String>,
    /// Set by `move_to_graveyard`/`butcher`: true iff this card's most
    /// recent move to the graveyard originated from `Zone::Base` (QA
    /// #41 -- the graveyard trigger fires only for that transition).
    entered_graveyard_from_base: bool,
    /// QA #82/#85/#86: "if it is night, your Pal's AUTO abilities
    /// activate twice" -- a dedicated, explicitly-set flag rather than
    /// matching against `cont_ability`'s free text (fragile) or the
    /// card's number (would put card-specific knowledge in the engine;
    /// see `has_active_doubling_cont`'s doc comment). A boolean, not a
    /// count: whether a SECOND redundant copy exists is a question for
    /// `has_active_doubling_cont`'s caller-side scan, not this field.
    doubles_auto_activations: bool,
    /// Mechanical facts about this card's involvement in a battle, set
    /// directly by `declare_attack`/`interrupt_attack`/`resolve_battle`
    /// -- NOT real ability activations (no per-card ability dispatch
    /// exists; see the qa_about_cards.rs QA #84 rewrite for why that
    /// stays out of the engine). QA #45/#49/#50/#71 each read one of
    /// these as "did this mechanical event happen", not "did some
    /// specific card's ability fire".
    attacked_a_structure: bool,
    was_declared_pal_target: bool,
    attack_was_interrupted: bool,
    end_of_battle_as_attacker_triggered: bool,
    /// Signed, unclamped, same convention as `power` -- see `strike()`/
    /// `reduce_strike`/`apply_cont_strike_modifier`. QA #64: distinct
    /// from Power -- Strike <= 0 blocks player damage outright rather
    /// than being merely referenced.
    strike: i32,
    /// Deploy cost. Not set by `deploy`/`place` (those are synthetic
    /// test fixtures with no real printed cost) -- `set_cost` states it
    /// explicitly wherever a ruling depends on it (QA #89).
    cost: u32,
    /// QA #53 (Suzaku): "when your red card would deal Damage other
    /// than battle damage to a Pal, deal +200 instead" -- a dedicated
    /// flag, same reasoning as `doubles_auto_activations`: explicit, not
    /// inferred from a card number.
    boosts_red_pal_damage: bool,
    /// QA #62 (Relaxaurus): set by `lock_from_standing`, checked by
    /// `stand_via_effect`. `Some(source)` means locked by `source` for
    /// as long as `source` remains in the base.
    locked_by: Option<CardId>,
    /// Names granted "in addition" to the printed one (QA #66/#97) --
    /// `card_names` returns the printed name plus all of these; the
    /// printed name itself is never removed by a grant.
    granted_names: Vec<String>,
}

struct ContModifier {
    target: CardId,
    amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrantedInstanceId(u32);

/// QA #57/#58: a specific instance of an ability granted to `target` by
/// `source`'s effect -- kept distinct from `target`'s own printed
/// abilities so two independently-granted copies of identical text are
/// two instances, not one. `text` is the granted ability's own text
/// (e.g. "AUTO OnAttack Choose up to 1 Pal, and deal 700 Damage"); the
/// engine doesn't interpret it beyond `resolve_granted_on_attack`
/// scanning for the "deal N Damage" number, so a new caller need not
/// touch this module to grant a differently-worded instance.
struct GrantedInstance {
    target: CardId,
    text: String,
}

#[derive(Default)]
struct PlayerState {
    pending_autos: Vec<CardId>,
    cards_played_this_turn: u32,
    /// (card, ability name) pairs already activated this turn -- QA #44:
    /// keyed on the card INSTANCE, not the card name, so two copies of
    /// the same card lock out independently; QA #48: keyed on the
    /// ability's own name too, so two distinct Once-Per-Turn abilities
    /// on the same card lock out independently of each other.
    /// `activate_act` (no ability name given) uses `""` as the name.
    acted_this_turn: HashSet<(CardId, String)>,
    materials: u32,
    ingredients: u32,
}

/// 11.1.2/`RuleAction`: check-type rule actions only. See the module doc
/// comment for why interrupt-type ones (player damage resolution / the
/// Lucky mill, 11.2.1) are deliberately NOT variants here.
///
/// A third check-type action, overloaded Pals (11.5.2, more than 5 Pals
/// in a base), is also not a variant: which Pal(s) go to the graveyard
/// is the controller's choice, not something `run_check_timing` can
/// auto-resolve the way it can for the two below. That stays
/// `bin_down_to_base_limit`, called explicitly by whoever is making the
/// choice (QA #11's test) -- a rule action that requires a choice sits
/// outside this automatic fixpoint, not inside it pretending to have
/// picked for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleAction {
    /// 11.3.3: a player at life <= 0 loses. Checked before
    /// `LethalDamage` in the same sweep -- per 1.2.1 the game ends
    /// immediately once this resolves, so nothing else should.
    LosingVerdict(PlayerId),
    /// 11.4.1/11.4.2: a Pal with `damage_taken > 0 && damage_taken >=
    /// power` goes to the graveyard. Both conjuncts, not just one --
    /// see card_abilities.rs QA #92/#93 and its `rule_11_4_1` boundary
    /// test for why.
    LethalDamage(CardId),
}

pub struct GameState {
    cards: HashMap<CardId, CardInstance>,
    next_card_id: u32,
    zone_counts: HashMap<(PlayerId, Zone), usize>,
    players: HashMap<PlayerId, PlayerState>,
    turn_player: PlayerId,
    resolving_effect: bool,
    life: HashMap<PlayerId, i32>,
    /// `Some(loser)` once a losing verdict has resolved. 1.2.1: the game
    /// is over at that point, permanently -- nothing un-sets this.
    game_over: Option<PlayerId>,
    /// QA #13: a plain settable state with no inherent side effects of
    /// its own -- becoming night doesn't, by itself, trigger anything.
    night: bool,
    cont_modifiers: HashMap<ModifierId, ContModifier>,
    next_modifier_id: u32,
    battles: HashMap<AttackId, Battle>,
    next_attack_id: u32,
    cont_strike_modifiers: HashMap<ModifierId, ContModifier>,
    granted_instances: HashMap<GrantedInstanceId, GrantedInstance>,
    next_granted_instance_id: u32,
    /// QA #67/#68: names a test has explicitly registered as real and
    /// released (`declare_card_name` succeeds) vs. real-but-unreleased
    /// (`declare_card_name` returns `NotYetReleased`) -- see
    /// `engine::targeting`'s module doc comment for why this isn't
    /// backed by the real CSV.
    known_card_names: HashSet<String>,
    known_unreleased_card_names: HashSet<String>,
    /// QA #72 (Elizabee): a minimal, single-card "what's on top" fact --
    /// not a full ordered deck model, just enough for `return_to_top_
    /// unchanged`/`top_of_deck` to round-trip.
    top_of_deck: HashMap<PlayerId, CardId>,
    /// QA #89 (Katress): the one-time "you may deploy at exactly this
    /// cost" allowance her AUTO grants, narrated explicitly by the
    /// caller (see qa_about_cards.rs) rather than computed by
    /// `resolve_auto` itself -- same reasoning as everywhere else a
    /// specific card's effect is stated by the test, not guessed by the
    /// engine.
    deploy_cost_allowance: Option<u32>,
    /// `engine::replay`'s `last_on_deploy_triggered`: which card
    /// `deploy`/`deploy_from` most recently placed. Not a general rules
    /// concept (no QA ruling needs "what was the last card deployed" as
    /// a queryable fact) -- a small, generically-useful bookkeeping
    /// convenience, kept here rather than in `engine::replay` because
    /// nothing outside `GameState` sees `deploy`/`deploy_from` mutate.
    last_deployed: Option<CardId>,
}

/// A generous, unremarkable starting supply for a scenario that's
/// exercising one rule at a time, not a real 50-card game -- see the
/// module doc comment. 10 souls in the Soul Deck matches CLAUDE.md's
/// documented "soul deck exactly 10" constant; 10 starting life matches
/// every observed value in the replay corpus (data/replays/) -- neither
/// is invented here.
const TEST_SCENARIO_DECK_SIZE: usize = 200;
const TEST_SCENARIO_SOUL_DECK_SIZE: usize = 10;
/// QA #76: souls in the Soul Area cap at 10.
const SOUL_AREA_CAP: usize = 10;
const TEST_SCENARIO_STARTING_LIFE: i32 = 10;
/// Baseline Power for a synthetic test Pal with no real card behind it.
/// Arbitrary but large enough that `reduce_power` can drive it well
/// negative without special-casing; never meant to resemble a real
/// card's printed Power.
const TEST_SCENARIO_PAL_POWER: i32 = 1000;
/// Baseline Strike for a synthetic test Pal, same rationale as Power
/// above. 3 matches the "fixture assumption: printed Strike is 3" QA
/// #65's test states directly, so it's not an arbitrary choice for that
/// specific test, but it's still a synthetic default, not read from any
/// real card.
const TEST_SCENARIO_PAL_STRIKE: i32 = 3;

impl GameState {
    pub fn test_scenario() -> Self {
        let mut zone_counts = HashMap::new();
        let mut players = HashMap::new();
        let mut life = HashMap::new();
        for player in [PlayerId::P1, PlayerId::P2] {
            zone_counts.insert((player, Zone::Hand), 0);
            zone_counts.insert((player, Zone::Deck), TEST_SCENARIO_DECK_SIZE);
            zone_counts.insert((player, Zone::Base), 0);
            zone_counts.insert((player, Zone::Graveyard), 0);
            zone_counts.insert((player, Zone::SoulDeck), TEST_SCENARIO_SOUL_DECK_SIZE);
            zone_counts.insert((player, Zone::SoulArea), 0);
            players.insert(player, PlayerState::default());
            life.insert(player, TEST_SCENARIO_STARTING_LIFE);
        }
        GameState {
            cards: HashMap::new(),
            next_card_id: 0,
            zone_counts,
            players,
            turn_player: PlayerId::P1,
            resolving_effect: false,
            life,
            game_over: None,
            night: false,
            cont_modifiers: HashMap::new(),
            next_modifier_id: 0,
            battles: HashMap::new(),
            next_attack_id: 0,
            cont_strike_modifiers: HashMap::new(),
            granted_instances: HashMap::new(),
            next_granted_instance_id: 0,
            known_card_names: HashSet::new(),
            known_unreleased_card_names: HashSet::new(),
            top_of_deck: HashMap::new(),
            deploy_cost_allowance: None,
            last_deployed: None,
        }
    }

    fn fresh_attack_id(&mut self) -> AttackId {
        let id = AttackId(self.next_attack_id);
        self.next_attack_id += 1;
        id
    }

    fn fresh_modifier_id(&mut self) -> ModifierId {
        let id = ModifierId(self.next_modifier_id);
        self.next_modifier_id += 1;
        id
    }

    /// Default field values for a brand-new synthetic test card. Kept in
    /// one place so every constructor (`draw`, `deploy`,
    /// `deploy_structure_or_gear`, `place`, `look_at_top`/`reveal_top`'s
    /// synthesized entries) stays consistent as more fields get added.
    fn new_card_instance(owner: PlayerId, zone: Zone, kind: CardKind) -> CardInstance {
        CardInstance {
            owner,
            zone,
            kind,
            power: TEST_SCENARIO_PAL_POWER,
            damage_taken: 0,
            resting: false,
            keywords: HashSet::new(),
            color: None,
            elements: Vec::new(),
            work_suitabilities: Vec::new(),
            card_name: None,
            cont_ability: None,
            entered_graveyard_from_base: false,
            doubles_auto_activations: false,
            attacked_a_structure: false,
            was_declared_pal_target: false,
            attack_was_interrupted: false,
            end_of_battle_as_attacker_triggered: false,
            strike: TEST_SCENARIO_PAL_STRIKE,
            cost: 0,
            boosts_red_pal_damage: false,
            locked_by: None,
            granted_names: Vec::new(),
        }
    }

    fn fresh_card_id(&mut self) -> CardId {
        let id = CardId(self.next_card_id);
        self.next_card_id += 1;
        id
    }

    fn player_state(&mut self, player: PlayerId) -> &mut PlayerState {
        self.players.entry(player).or_default()
    }

    // --- hand / draw --------------------------------------------------

    pub fn hand_size(&self, player: PlayerId) -> usize {
        self.zone_counts[&(player, Zone::Hand)]
    }

    /// Draws exactly 1 card from the deck to the hand. QA #22: this is
    /// the only Draw Phase action -- there is no opt-out.
    ///
    /// Consults `top_of_deck` first: TD01-012 (Elphidran Aqua)'s real
    /// On Deploy ends with "...put it on the top of the deck"
    /// (`move_to_top_of_deck`), and that fact must actually be what the
    /// NEXT draw returns, or "put on top" would be purely cosmetic --
    /// the next draw would materialize an unrelated card instead of the
    /// one just placed. Consuming (removing) the entry is what makes it
    /// apply to exactly the next draw, not every one after.
    pub fn draw(&mut self, player: PlayerId) -> CardId {
        let deck_count = self.zone_counts.get_mut(&(player, Zone::Deck)).unwrap();
        *deck_count = deck_count.saturating_sub(1);
        *self.zone_counts.get_mut(&(player, Zone::Hand)).unwrap() += 1;

        if let Some(card) = self.top_of_deck.remove(&player) {
            self.cards.get_mut(&card).expect("draw: top_of_deck card unknown").zone = Zone::Hand;
            return card;
        }

        let id = self.fresh_card_id();
        // kind is arbitrary (Pal) for a drawn-but-otherwise-untracked test card.
        self.cards.insert(id, Self::new_card_instance(player, Zone::Hand, CardKind::Pal));
        id
    }

    pub fn draw_phase(&mut self, player: PlayerId) {
        self.draw(player);
    }

    /// QA #10: no hand-size cap, so End Phase never discards anything.
    pub fn end_phase(&mut self, _player: PlayerId) {}

    // --- deploy / base zone ---------------------------------------------

    /// Deploys a Pal-kind card straight into `zone` (in every current
    /// test, `Zone::Base`). Always succeeds -- QA #11: there is no
    /// 5-Pal hard cap on the deploy action itself, only a post-deploy
    /// "bin down to 5" choice (`bin_down_to_base_limit`).
    pub fn deploy(&mut self, player: PlayerId, _number: &str, zone: Zone) -> Result<CardId, DeployError> {
        let id = self.fresh_card_id();
        self.cards.insert(id, Self::new_card_instance(player, zone, CardKind::Pal));
        *self.zone_counts.get_mut(&(player, zone)).unwrap() += 1;
        self.last_deployed = Some(id);
        Ok(id)
    }

    /// Puts a NEW card directly into `zone`, tracked with a real
    /// identity from the start (so `main_name_of`/`set_color`/etc. work
    /// on it), WITHOUT the "was this a deploy" semantics `deploy_from`
    /// carries -- QA #32's test uses this to stage a card in Hand/Deck/
    /// Graveyard before moving it into the base via `deploy_from`, and
    /// several other tests use it to seed a specific card in Hand/Deck
    /// for a selection ability to find. Kind defaults to Pal -- this is
    /// the test harness's own generic filler-card constructor ("a card,
    /// type irrelevant"), used by ~30 call sites across `cards/`'s own
    /// unit tests that genuinely don't care what kind they're placing.
    /// A real driver materializing an actual named card (which DOES
    /// know, from `card_meta`, whether it's a Pal/Structure/Gear) must
    /// use `place_typed` instead, never this one -- see that function's
    /// own doc comment for why the distinction is load-bearing, not
    /// stylistic.
    pub fn place(&mut self, player: PlayerId, _number: &str, zone: Zone) -> CardId {
        self.place_typed(player, _number, zone, CardKind::Pal)
    }

    /// Same as `place`, but the caller states the real kind instead of
    /// getting Pal by default. This exists because `place` defaulting
    /// silently was itself a real bug, not a hypothetical one:
    /// `format_a_replay`'s `materialize()` called `place` for EVERY
    /// card it ever built, including obviously-non-Pal ones (Pengullet
    /// Rocket Launcher, Single-Shot Rifle, Zoe's Strategy...), so every
    /// driver-materialized Gear or Structure has been silently tracked
    /// as `CardKind::Pal` since before this function existed --
    /// `pals_in_base` would have included them, meaning Pump-Action
    /// Shotgun's "all opponent Pals" sweep, the 5-Pal base limit
    /// (11.5.2), and Astegon's own "all Pals with Power 300 or less"
    /// would all have silently swept up materialized gears too. No
    /// fixture crossed one of those thresholds, so nothing caught it --
    /// see `qa_kind_regression_pals_in_base_excludes_a_materialized_gear`
    /// below, added specifically because nothing already existing
    /// would have. `materialize()` is now the only door either driver
    /// uses to build a real card, and it always calls this, never
    /// `place` -- see CLAUDE.md's Conventions section on why a second,
    /// shorter, wrong-by-default constructor staying reachable is its
    /// own risk independent of this specific bug.
    pub fn place_typed(&mut self, player: PlayerId, _number: &str, zone: Zone, kind: CardKind) -> CardId {
        let id = self.fresh_card_id();
        self.cards.insert(id, Self::new_card_instance(player, zone, kind));
        *self.zone_counts.get_mut(&(player, zone)).unwrap() += 1;
        id
    }

    /// Moves an EXISTING card (already tracked, e.g. via `place`) from
    /// `from_zone` to `to_zone`, and reports whether this transition is
    /// an On Deploy trigger. QA #32: On Deploy fires on ANY move into
    /// the base, regardless of source zone -- deck, graveyard, hand,
    /// wherever -- so `on_deploy_triggered` is simply `to_zone ==
    /// Zone::Base`. This reports the mechanical fact `cards::`'s actual
    /// On Deploy functions (or, in `engine::replay`, a check-timing step
    /// that decides whether to run one) key off of; it doesn't run any
    /// ability itself.
    pub fn deploy_from(&mut self, player: PlayerId, card: CardId, from_zone: Zone, to_zone: Zone) -> DeployFromResult {
        let instance = self.cards.get_mut(&card).expect("deploy_from: unknown card");
        assert_eq!(instance.zone, from_zone, "deploy_from: card is not actually in from_zone");
        instance.zone = to_zone;
        *self.zone_counts.get_mut(&(player, from_zone)).unwrap() -= 1;
        *self.zone_counts.get_mut(&(player, to_zone)).unwrap() += 1;
        if to_zone == Zone::Base {
            self.last_deployed = Some(card);
        }
        DeployFromResult { card, on_deploy_triggered: to_zone == Zone::Base }
    }

    pub fn last_deployed(&self) -> Option<CardId> {
        self.last_deployed
    }

    /// QA #12: structures/gears in the base, with no limit. `kind` is
    /// always stated explicitly by the caller (`CardKind::Structure` or
    /// `CardKind::Gear`) instead of guessed from the card number (hard
    /// rule 3) -- same principle `place_typed` follows, applied to the
    /// `deploy` side instead of the `place` side.
    pub fn deploy_structure_or_gear(&mut self, player: PlayerId, _number: &str, kind: CardKind) -> Result<CardId, DeployError> {
        let id = self.fresh_card_id();
        self.cards.insert(id, Self::new_card_instance(player, Zone::Base, kind));
        *self.zone_counts.get_mut(&(player, Zone::Base)).unwrap() += 1;
        Ok(id)
    }

    /// QA #24: no legal action deploys into the opponent's base.
    pub fn deploy_to_opponents_base(&mut self, _player: PlayerId, _number: &str) -> Result<CardId, DeployError> {
        Err(DeployError("cannot deploy into the opponent's base".to_string()))
    }

    pub fn pals_in_base(&self, player: PlayerId) -> Vec<CardId> {
        self.cards_in_zone_of_kind(player, Zone::Base, CardKind::Pal)
    }

    /// BP01-046 Victor's Strategy's own "draw X, X = your structure
    /// count" bullet is the reason this exists as its own function
    /// rather than staying folded into a combined structures-and-gears
    /// count -- Structure and Gear used to be one `CardKind` variant,
    /// which could never have answered this correctly.
    pub fn structures_in_base(&self, player: PlayerId) -> Vec<CardId> {
        self.cards_in_zone_of_kind(player, Zone::Base, CardKind::Structure)
    }

    /// BP01-005 Reptyro's own ACT ("choose up to 1 gear" among looked-at
    /// cards) is the reason this exists separately from
    /// `structures_in_base` -- same split, opposite half.
    pub fn gears_in_base(&self, player: PlayerId) -> Vec<CardId> {
        self.cards_in_zone_of_kind(player, Zone::Base, CardKind::Gear)
    }

    fn cards_in_zone_of_kind(&self, player: PlayerId, zone: Zone, kind: CardKind) -> Vec<CardId> {
        let mut ids: Vec<CardId> = self
            .cards
            .iter()
            .filter(|(_, c)| c.owner == player && c.zone == zone && c.kind == kind)
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        ids
    }

    pub fn zone_of(&self, card: CardId) -> Zone {
        self.cards[&card].zone
    }

    /// QA #11's controller-choice bin-down: moves each named card to the
    /// graveyard. No automatic "which ones" logic -- the caller already
    /// decided.
    pub fn bin_down_to_base_limit(&mut self, player: PlayerId, chosen: Vec<CardId>) {
        for card in chosen {
            assert_eq!(self.cards[&card].owner, player, "bin_down_to_base_limit: card belongs to the other player");
            self.move_to_graveyard(card, Zone::Base);
        }
    }

    /// Moves an existing card from `from_zone` to the graveyard, and
    /// records whether that transition originated from the base -- QA
    /// #41: "when this card is put into the graveyard" triggers fire
    /// only for the base->graveyard move, never hand->graveyard
    /// (discard) or deck->graveyard (mill). See `graveyard_trigger_fired`.
    pub fn move_to_graveyard(&mut self, card: CardId, from_zone: Zone) {
        let instance = self.cards.get_mut(&card).expect("move_to_graveyard: unknown card");
        assert_eq!(instance.zone, from_zone, "move_to_graveyard: card is not actually in from_zone");
        let owner = instance.owner;
        instance.zone = Zone::Graveyard;
        instance.entered_graveyard_from_base = from_zone == Zone::Base;
        *self.zone_counts.get_mut(&(owner, from_zone)).unwrap() -= 1;
        *self.zone_counts.get_mut(&(owner, Zone::Graveyard)).unwrap() += 1;
    }

    pub fn graveyard_trigger_fired(&self, card: CardId) -> bool {
        self.cards[&card].entered_graveyard_from_base
    }

    /// QA #81: "Butcher" means choosing N of your own base Pals and
    /// putting them into the graveyard -- always a base->graveyard
    /// move, so `graveyard_trigger_fired` is true for a butchered card.
    pub fn butcher(&mut self, card: CardId) {
        self.move_to_graveyard(card, Zone::Base);
    }

    // --- timing / turn structure -----------------------------------------

    pub fn set_turn_player(&mut self, player: PlayerId) {
        self.turn_player = player;
    }

    /// QA #25/#28/#29/#30, pinned down for real now that Card Abilities
    /// has been built: Quick has exactly two legal windows -- `OwnMain`
    /// (QA #28, played like an ordinary card) and
    /// `OpponentTurnDuringBattle` (QA #25/#30, one at a time) -- and
    /// NEVER `OwnTurnDuringBattle` (QA #29, explicit: "Quick can only be
    /// used during battle on the opponent's turn"). `is_quick` is passed
    /// explicitly rather than inferred from the card number -- see the
    /// module doc comment.
    ///
    /// Corrected from an earlier, wrong guess: this originally had
    /// `OwnTurnDuringBattle => is_quick` (written when only QA #25 was
    /// encoded, before #29 existed to test that branch) -- backwards
    /// from what #29 says, and nothing in Flow of the Game's tests ever
    /// exercised `OwnTurnDuringBattle` to catch it. Found by QA #29
    /// failing here, not by inspection.
    ///
    /// QA #73/10.5.4.1: play timing is always preceded by a check
    /// timing, so nothing is a legal play while an effect is still
    /// resolving -- checked first, ahead of the per-timing rules above.
    pub fn can_play_card(&self, _player: PlayerId, is_quick: bool, timing: Timing) -> bool {
        if self.resolving_effect {
            return false;
        }
        match timing {
            Timing::OwnMain => true,
            Timing::OpponentTurnDuringBattle => is_quick,
            Timing::OpponentTurnOutsideBattle => false,
            Timing::OwnTurnDuringBattle => false,
        }
    }

    /// QA #73/10.5.4.1, the same rule stated for attack declaration
    /// instead of playing a card: not legal while an effect is still
    /// resolving.
    pub fn declare_attack_is_legal(&self, _player: PlayerId) -> bool {
        !self.resolving_effect
    }

    /// QA #25: AUTO/CONT abilities are never turn-gated in this engine
    /// -- they activate/apply for either player whenever their trigger
    /// condition is met, regardless of whose turn it is. Always true;
    /// there is no state that could make this false.
    pub fn auto_and_cont_apply_on_opponents_turn(&self, _player: PlayerId) -> bool {
        true
    }

    pub fn cards_played_this_turn(&self, player: PlayerId) -> u32 {
        self.players.get(&player).map(|p| p.cards_played_this_turn).unwrap_or(0)
    }

    // --- standby / check timing (QA #42, #43) -----------------------------

    pub fn begin_resolving_effect(&mut self, _label: &str) {
        self.resolving_effect = true;
    }

    pub fn finish_resolving_effect(&mut self) {
        self.resolving_effect = false;
    }

    pub fn effect_still_resolving(&self) -> bool {
        self.resolving_effect
    }

    /// QA #42/10.8.2: never resolves anything, just enqueues -- even if
    /// called while `effect_still_resolving()` is true.
    ///
    /// QA #82/#85/#86 (5.21.1): if `player` currently has an active
    /// doubling CONT, the trigger enters standby TWICE instead of once.
    /// Checked fresh at THIS call (`has_active_doubling_cont`, evaluated
    /// against state as it is right now) -- never cached -- which is
    /// exactly what QA #83/#84 need: Katress's trigger must count
    /// Shadowbeak's doubling CONT as active at the moment Shadowbeak's
    /// own butcher effect fires it, even though the very next step
    /// removes Shadowbeak from the base. Callers narrate that ordering
    /// explicitly (trigger before butcher), the same way #43/#84 narrate
    /// "resolving this generates that" explicitly -- this function only
    /// answers "is a doubling CONT active right now", nothing about when
    /// "now" should be relative to some other effect.
    pub fn trigger_auto(&mut self, player: PlayerId, source: CardId) {
        let copies = if self.has_active_doubling_cont(player) { 2 } else { 1 };
        let state = self.player_state(player);
        for _ in 0..copies {
            state.pending_autos.push(source);
        }
    }

    /// QA #85/5.21.2: a boolean, not a count of doubling sources -- a
    /// second redundant copy of the identical doubling CONT must not
    /// push activations to 4x (or 3x). `.any()` rather than counting
    /// matches is what makes that true regardless of how many active
    /// doubling sources the player has.
    fn has_active_doubling_cont(&self, player: PlayerId) -> bool {
        self.night && self.cards.values().any(|c| c.owner == player && c.zone == Zone::Base && c.doubles_auto_activations)
    }

    /// QA #82/#85/#86: marks `card` as printing a doubling CONT ("if it
    /// is night, your Pal's AUTO abilities activate twice"), explicitly
    /// -- never inferred from a card number (hard rule 3; this is
    /// exactly the same reasoning as `can_play_card`'s `is_quick`
    /// parameter). A generic mechanism keyed on this flag plus
    /// `night_active()`, not a Shadowbeak special case: any card a test
    /// marks this way behaves identically.
    pub fn set_auto_doubling_cont(&mut self, card: CardId, active: bool) {
        self.cards.get_mut(&card).expect("set_auto_doubling_cont: unknown card").doubles_auto_activations = active;
    }

    pub fn pending_autos(&self, player: PlayerId) -> Vec<CardId> {
        self.players.get(&player).map(|p| p.pending_autos.clone()).unwrap_or_default()
    }

    /// Same underlying queue as `pending_autos`; see the module doc
    /// comment for why a check timing doesn't need separate state.
    pub fn next_check_timing_actions(&self, player: PlayerId) -> Vec<CardId> {
        self.pending_autos(player)
    }

    /// See the module doc comment: stateless, re-derived every call.
    pub fn priority_player(&self) -> PlayerId {
        if !self.pending_autos(self.turn_player).is_empty() {
            self.turn_player
        } else {
            self.turn_player.other()
        }
    }

    /// Removes exactly one pending instance of `which` from `player`'s
    /// standby queue (10.8.2.1 / 10.8.3.1: when the same source is
    /// pending multiple times, each instance resolves independently --
    /// `.position()` finds only the first match, leaving any duplicate
    /// instances still queued). Resolution's actual *effect* -- whether
    /// it generates a new trigger, on this card or another (QA #43's
    /// second bullet; QA #84's Shadowbeak-butchers-itself-then-Katress-
    /// triggers-twice chain) -- is real card behaviour and deliberately
    /// not modeled here; callers state that explicitly via their own
    /// `trigger_auto` calls (see `qa_043_newly_generated_auto_may_
    /// resolve_before_older_pending_one` for the abstract case, and
    /// Card Abilities/Q&A about Cards for concrete ones).
    pub fn resolve_auto(&mut self, player: PlayerId, which: CardId) {
        let state = self.player_state(player);
        let pos = state
            .pending_autos
            .iter()
            .position(|&id| id == which)
            .expect("resolve_auto: not pending for this player");
        state.pending_autos.remove(pos);
    }

    /// QA #43's "any order among your own" rule: legal iff `order` is
    /// exactly a permutation of what's currently pending for `player`.
    pub fn resolution_order_is_legal(&self, player: PlayerId, order: Vec<CardId>) -> bool {
        let mut pending = self.pending_autos(player);
        let mut order = order;
        pending.sort();
        order.sort();
        pending == order
    }

    /// Comprehensive rule 10.5.3, written as the literal loop: drain
    /// every pending rule action to a genuine fixpoint FIRST (10.5.3.1 --
    /// if resolving one produces another, resolve that too, before ever
    /// touching AUTOs), and only once none remain does an automatic
    /// ability get resolved at all -- one at a time, via `priority_player`
    /// (turn player if they have any pending AUTO, else the non-turn
    /// player), looping back to the rule-action check after every single
    /// resolution. That "back to the top after each" is what makes QA
    /// #43's two supplementary rules fall out for free rather than
    /// needing special-case code: a newly-generated AUTO is just as
    /// eligible as anything already pending the moment priority is
    /// re-derived, and a turn-player AUTO created mid-resolution of the
    /// non-turn player's queue reclaims priority on the very next
    /// iteration, because `priority_player` always checks the turn
    /// player first.
    ///
    /// If a losing verdict is among the rule actions resolved, this
    /// stops immediately (1.2.1: the game ends right there) and never
    /// reaches the AUTO-priority step at all, even for autos that were
    /// already pending before the verdict resolved.
    pub fn run_check_timing(&mut self) {
        loop {
            if self.resolve_one_pending_rule_action() {
                if self.game_over.is_some() {
                    break;
                }
                continue;
            }
            let player = self.priority_player();
            let Some(auto) = self.pending_autos(player).first().copied() else {
                break; // neither player has a pending rule action or AUTO -- done
            };
            // Resolves whichever pending AUTO happens to be first for
            // `player`. The real rule (QA #43) is that the CONTROLLING
            // player chooses the order among their own pending set; this
            // fully-automated drain doesn't model that choice (nothing
            // here pauses for one), so it applies a fixed FIFO default.
            // Fine for a rule action's sake -- lethal damage doesn't care
            // what order unrelated AUTOs resolve in -- but this is not
            // itself an implementation of "any order they choose"; that
            // stays `resolve_auto` + `resolution_order_is_legal`, called
            // directly by whoever (a player, an agent) is making the
            // choice.
            self.resolve_auto(player, auto);
        }
    }

    /// Finds ONE currently-true check-type rule action (11.1.2), if any,
    /// re-evaluated fresh from current state every call -- never a
    /// stale/precomputed list. That freshness is what makes
    /// `run_check_timing`'s loop a genuine fixpoint: binning an
    /// overloaded Pal (not resolved here, see `RuleAction`'s doc
    /// comment) could put a different Pal at lethal damage, which this
    /// function will see on the very next call, which could in turn put
    /// a player at a losing verdict, seen the call after that.
    ///
    /// Order between the two variants only matters because
    /// `LosingVerdict` halts everything (1.2.1) -- checking it first
    /// means a check timing where a player is already at 0 life never
    /// bothers resolving an unrelated Pal's lethal damage first.
    fn find_pending_rule_action(&self) -> Option<RuleAction> {
        for player in [PlayerId::P1, PlayerId::P2] {
            if self.life[&player] <= 0 {
                return Some(RuleAction::LosingVerdict(player));
            }
        }
        let lethal = self.cards.iter().find(|(id, c)| {
            c.zone == Zone::Base && c.kind == CardKind::Pal && c.damage_taken > 0 && (c.damage_taken as i64) >= (self.effective_power(**id) as i64)
        });
        lethal.map(|(id, _)| RuleAction::LethalDamage(*id))
    }

    fn apply_rule_action(&mut self, action: RuleAction) {
        match action {
            RuleAction::LosingVerdict(player) => {
                self.game_over = Some(player);
            }
            RuleAction::LethalDamage(card) => {
                let from_zone = self.cards[&card].zone;
                self.move_to_graveyard(card, from_zone);
            }
        }
    }

    fn resolve_one_pending_rule_action(&mut self) -> bool {
        match self.find_pending_rule_action() {
            Some(action) => {
                self.apply_rule_action(action);
                true
            }
            None => false,
        }
    }

    // --- power / damage / life (11.3.3, 11.4.1/#92/#93) -------------------

    /// Effective Power: base (mutated by `reduce_power`/`increase_power`)
    /// plus every currently-active `apply_cont_power_modifier` targeting
    /// this card. QA #74 depends on this being computed fresh every
    /// call, not cached -- removing a modifier must be reflected
    /// immediately, including by `find_pending_rule_action`'s lethal
    /// check (see there).
    pub fn power(&self, card: CardId) -> i32 {
        self.effective_power(card)
    }

    fn effective_power(&self, card: CardId) -> i32 {
        let base = self.cards[&card].power;
        let modifiers: i32 = self.cont_modifiers.values().filter(|m| m.target == card).map(|m| m.amount).sum();
        base + modifiers
    }

    /// A named, independently-removable Power modifier -- distinct from
    /// `reduce_power`/`increase_power`'s direct, permanent mutation of
    /// the base value. QA #74: a CONT ability's buff needs to be able to
    /// disappear (its condition stops being met) without that being
    /// confused with a one-off effect. `source` isn't used yet (nothing
    /// currently needs to know who granted a modifier, only that it
    /// exists and targets `target`), but is part of the signature
    /// because a real CONT ability always has a source card.
    pub fn apply_cont_power_modifier(&mut self, _source: CardId, target: CardId, amount: i32) -> ModifierId {
        let id = self.fresh_modifier_id();
        self.cont_modifiers.insert(id, ContModifier { target, amount });
        id
    }

    pub fn remove_cont_power_modifier(&mut self, id: ModifierId) {
        self.cont_modifiers.remove(&id);
    }

    /// Subtracts `magnitude` from `card`'s Power. Never clamped at 0
    /// (1.3.2.4): see QA #92's test, which drives Power deep negative on
    /// purpose. Unsigned on purpose, split from `increase_power` rather
    /// than one signed `adjust_power(delta)` -- a single signed-delta
    /// method is exactly what let QA #92's original call read as "pass
    /// a positive number to reduce" and silently increase Power instead
    /// (a naming/signature mismatch a doc comment didn't prevent). With
    /// no sign to misremember, that mistake isn't expressible.
    pub fn reduce_power(&mut self, card: CardId, magnitude: u32) {
        self.cards.get_mut(&card).expect("reduce_power: unknown card").power -= magnitude as i32;
    }

    /// See `reduce_power`. Not yet called by any test, but kept as a
    /// real sibling rather than left implicit -- Power-boosting effects
    /// (Chillet's Dragon reveal, +500 until end of turn) are common in
    /// the replay corpus and will need it.
    pub fn increase_power(&mut self, card: CardId, magnitude: u32) {
        self.cards.get_mut(&card).expect("increase_power: unknown card").power += magnitude as i32;
    }

    /// Sets base Power directly -- the `set_cost`/`set_strike` sibling
    /// for Power. `engine::replay` needs this to seed a Pal-vs-Pal
    /// `battle_attack` log line's two participants with the EXACT
    /// Power the client already computed (buffs, OnAttack reveals, and
    /// all), read straight off that event's own `attacker_power`/
    /// `defender_power` resolution fields -- reproducing that number
    /// from card_meta plus ability simulation isn't in scope for a
    /// driver that never scripted most of these cards (see the
    /// `replay` crate's own module doc comment).
    pub fn set_power(&mut self, card: CardId, power: i32) {
        self.cards.get_mut(&card).expect("set_power: unknown card").power = power;
    }

    pub fn damage_taken(&self, card: CardId) -> u32 {
        self.cards[&card].damage_taken
    }

    pub fn apply_damage(&mut self, card: CardId, amount: u32) {
        self.cards.get_mut(&card).expect("apply_damage: unknown card").damage_taken += amount;
    }

    /// Raw, unclamped stored value (3.2.1.2: life below 0 is
    /// *referenced* as 0 elsewhere, but not set to 0 in storage) -- so
    /// this can return a negative number. Nothing in this module
    /// currently "references" life for any purpose other than the
    /// losing-verdict check, which compares `<= 0` directly and doesn't
    /// need the clamp; a future caller that treats a negative life as a
    /// value (e.g. "double your life total") is the thing that would
    /// need to clamp at the read site.
    pub fn life(&self, player: PlayerId) -> i32 {
        self.life[&player]
    }

    pub fn reduce_life(&mut self, player: PlayerId, amount: i32) {
        *self.life.get_mut(&player).unwrap() -= amount;
    }

    /// `Some(loser)` once a losing verdict (11.3.3) has resolved via
    /// `run_check_timing`; `None` beforehand, including while a rule
    /// action making it true (e.g. `reduce_life` driving someone to 0)
    /// is sitting unresolved -- the verdict is a rule action, not an
    /// instant consequence, so it only becomes real at the next check
    /// timing (see the `rule_1_2_1_...` test in flow_of_the_game.rs).
    pub fn game_over(&self) -> Option<PlayerId> {
        self.game_over
    }

    // --- souls (QA #75, #96) ----------------------------------------------

    pub fn zone_count(&self, player: PlayerId, zone: Zone) -> usize {
        self.zone_counts[&(player, zone)]
    }

    /// QA #75: "increase soul" moves cards from the Soul Deck to the
    /// Soul Area -- a real, finite resource, not an abstract counter.
    /// QA #76: capped at 10 -- an increase that would overshoot only
    /// moves as many as fit; the rest stay in the Soul Deck (a rejected
    /// increase doesn't consume the resource; nothing here claims that's
    /// the right rule for an *overshooting* deploy/discard cost, only
    /// for this specific "increase soul" effect).
    pub fn increase_soul(&mut self, player: PlayerId, amount: usize) {
        let current = self.zone_counts[&(player, Zone::SoulArea)];
        let actual = amount.min(SOUL_AREA_CAP.saturating_sub(current));
        *self.zone_counts.get_mut(&(player, Zone::SoulDeck)).unwrap() -= actual;
        *self.zone_counts.get_mut(&(player, Zone::SoulArea)).unwrap() += actual;
    }

    /// QA #96 (rule 8.5): draws 1 card and, deliberately, does NOT touch
    /// `cards_played_this_turn` -- that's the entire content of the
    /// ruling.
    pub fn rest_souls_and_draw(&mut self, player: PlayerId, _souls: usize) {
        self.draw(player);
    }

    // --- night, materials/ingredients, work suitability, color/element,
    //     owner, main name (QA #13-19, #26) -----------------------------

    /// QA #13: a plain settable state; becoming night has no built-in
    /// side effect of its own (asserted directly by that test via
    /// `pending_autos` staying empty).
    pub fn night_active(&self) -> bool {
        self.night
    }

    pub fn set_night(&mut self, active: bool) {
        self.night = active;
    }

    /// QA #94: "until end of the opponent's next turn" -- the duration
    /// and source are accepted (a real implementation would need both
    /// to know when to clear `night` and to answer "what caused this"),
    /// but no expiration timer exists yet since nothing tests actual
    /// expiry, only that removing `source` from the base does NOT clear
    /// it early. That part this DOES get right: `night` isn't tied to
    /// `source`'s continued presence at all.
    pub fn set_night_until(&mut self, _duration: DurationSpec, _source: CardId) {
        self.night = true;
    }

    pub fn materials(&self, player: PlayerId) -> u32 {
        self.players.get(&player).map(|p| p.materials).unwrap_or(0)
    }

    pub fn ingredients(&self, player: PlayerId) -> u32 {
        self.players.get(&player).map(|p| p.ingredients).unwrap_or(0)
    }

    pub fn gain_materials(&mut self, player: PlayerId, amount: u32) {
        self.player_state(player).materials += amount;
    }

    pub fn gain_ingredients(&mut self, player: PlayerId, amount: u32) {
        self.player_state(player).ingredients += amount;
    }

    /// 3.3.3.1 (CLAUDE.md's numeric edge cases -- Resources below 0
    /// *become* 0, unlike Life which is merely referenced as 0 and
    /// Power which isn't clamped at all): saturating, not a bare `-=`.
    /// Found by QA #56, the first test to consume materials without
    /// gaining any first -- QA #14 (already passing) always gains
    /// before it consumes, so the missing saturation never panicked
    /// until this one exercised it.
    pub fn consume_materials(&mut self, player: PlayerId, amount: u32) {
        self.player_state(player).materials = self.player_state(player).materials.saturating_sub(amount);
    }

    /// QA #15/#16: replaces the full set (each Work Suitability is
    /// individually possessed; a card can have several at once).
    pub fn set_work_suitabilities(&mut self, card: CardId, suitabilities: Vec<String>) {
        self.cards.get_mut(&card).expect("set_work_suitabilities: unknown card").work_suitabilities = suitabilities;
    }

    pub fn has_work_suitability(&self, card: CardId, name: &str) -> bool {
        self.cards[&card].work_suitabilities.iter().any(|s| s == name)
    }

    /// QA #17/#18: Color and Element are stored independently -- setting
    /// one must never imply or overwrite the other (hard rule 2).
    pub fn set_color(&mut self, card: CardId, color: &str) {
        self.cards.get_mut(&card).expect("set_color: unknown card").color = Some(color.to_string());
    }

    /// "Colorless" if never set (QA #47 needs a Colorless Pal to compare
    /// against a colored one).
    pub fn color_of(&self, card: CardId) -> String {
        self.cards[&card].color.clone().unwrap_or_else(|| "Colorless".to_string())
    }

    pub fn set_elements(&mut self, card: CardId, elements: Vec<String>) {
        self.cards.get_mut(&card).expect("set_elements: unknown card").elements = elements;
    }

    pub fn elements_of(&self, card: CardId) -> Vec<String> {
        self.cards[&card].elements.clone()
    }

    pub fn owner_of(&self, card: CardId) -> PlayerId {
        self.cards[&card].owner
    }

    /// QA #26: Main Name is the portion of the full printed name before
    /// " – " (the en dash, surrounded by spaces, that separates a Pal's
    /// main name from its subtitle). Cards with no " – " (most
    /// synthetic filler in this suite, and real cards with no subtitle)
    /// have their whole name as their Main Name.
    pub fn set_card_name(&mut self, card: CardId, name: &str) {
        self.cards.get_mut(&card).expect("set_card_name: unknown card").card_name = Some(name.to_string());
    }

    pub fn main_name_of(&self, card: CardId) -> String {
        let full = self.cards[&card].card_name.clone().unwrap_or_default();
        full.split(" – ").next().unwrap_or(&full).to_string()
    }

    /// The untruncated name `set_card_name` stored -- `main_name_of`
    /// deliberately stops at the en dash (QA #26); `engine::replay`
    /// needs the whole printed name to match a client log's exact
    /// text, e.g. distinguishing "Pengullet – Yearning for the Sky"
    /// from "Pengullet – Frequent Flyer".
    pub fn full_name_of(&self, card: CardId) -> String {
        self.cards[&card].card_name.clone().unwrap_or_default()
    }

    // --- CONT zone-effectiveness, keywords/resting/Taunt (QA #27, #31,
    //     #40, #47, #51) --------------------------------------------------

    /// QA #31: a CONT ability's text is stored regardless of zone, but
    /// `cont_ability_active` -- the thing that actually matters -- is
    /// false unless the card is in the Base (the default rule; QA #51
    /// covers Interrupt's documented exception, activated from the hand
    /// instead, as its own explicit method rather than a special case
    /// here).
    pub fn set_cont_ability(&mut self, card: CardId, description: &str) {
        self.cards.get_mut(&card).expect("set_cont_ability: unknown card").cont_ability = Some(description.to_string());
    }

    pub fn cont_ability_active(&self, card: CardId) -> bool {
        let instance = &self.cards[&card];
        instance.cont_ability.is_some() && instance.zone == Zone::Base
    }

    pub fn rest(&mut self, card: CardId) {
        self.cards.get_mut(&card).expect("rest: unknown card").resting = true;
    }

    pub fn is_resting(&self, card: CardId) -> bool {
        self.cards[&card].resting
    }

    /// 7.2.1: the stand phase stands only the TURN PLAYER's own cards
    /// (CLAUDE.md's own combat-sequencing note -- a Pal that ends the
    /// opponent's turn rested is a legal attack target on yours,
    /// implying the opponent's own cards are untouched by this call).
    /// `wont_stand_next_stand_phase` (QA #61's "does not stand during
    /// your opponent's next stand phase") suppresses exactly ONE stand
    /// phase for a marked card -- the marker is cleared here regardless
    /// of whether it fired, since it's a one-shot duration, not a
    /// standing rule.
    pub fn stand_phase(&mut self, player: PlayerId) {
        for card in self.pals_in_base(player) {
            if self.wont_stand_next_stand_phase(card) {
                self.cards.get_mut(&card).unwrap().keywords.remove("__wont_stand_next");
            } else {
                self.cards.get_mut(&card).unwrap().resting = false;
            }
        }
    }

    pub fn grant_keyword(&mut self, card: CardId, keyword: &str) {
        self.cards.get_mut(&card).expect("grant_keyword: unknown card").keywords.insert(keyword.to_string());
    }

    fn has_keyword(&self, card: CardId, keyword: &str) -> bool {
        self.cards[&card].keywords.contains(keyword)
    }

    /// The Lucky icon (CLAUDE.md's deck-construction table, and
    /// `resolve_player_damage`'s mill loop) is modeled as an ordinary
    /// keyword -- `grant_keyword(card, "Lucky")` -- not a dedicated
    /// field, the same generic-primitive choice as Taunt/Interrupt.
    fn is_lucky(&self, card: CardId) -> bool {
        self.has_keyword(card, "Lucky")
    }

    /// QA #40: every one of the non-attacking player's rested Pals with
    /// Taunt is a legal target -- the attacker chooses among ALL of
    /// them, not a single forced one.
    pub fn taunt_compelled_targets(&self, attacking_player: PlayerId) -> Vec<CardId> {
        let mut ids: Vec<CardId> = self
            .cards
            .iter()
            .filter(|(_, c)| c.owner != attacking_player && c.zone == Zone::Base && c.resting && c.keywords.contains("Taunt"))
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        ids
    }

    /// QA #27/#47: "Choose all X" with no stated zone means the Base
    /// only (never hand/deck), and a color word in `description` (case-
    /// insensitively) filters to that color -- excluding Colorless even
    /// though its own name contains no color word to exclude BY (QA #47
    /// is explicit that Colorless never matches a color filter). No
    /// filter beyond zone (and color, if named) is applied -- kind
    /// (Pal vs structure/gear) isn't distinguished because no current
    /// test needs it to be.
    pub fn choose_all(&self, player: PlayerId, description: &str, _source: CardId) -> Vec<CardId> {
        let lower = description.to_lowercase();
        let color_filter = ["red", "blue", "green", "purple"].into_iter().find(|c| lower.contains(c));

        let mut ids: Vec<CardId> = self
            .cards
            .iter()
            .filter(|(_, c)| {
                c.owner == player
                    && c.zone == Zone::Base
                    && match color_filter {
                        Some(color) => c.color.as_deref().is_some_and(|actual| actual.eq_ignore_ascii_case(color)),
                        None => true,
                    }
            })
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        ids
    }

    /// QA #51: Interrupt is activatable only from the Hand -- the
    /// specific, named exception to QA #31's "Base only" default.
    pub fn activate_interrupt(&self, card: CardId) -> Result<(), String> {
        if self.cards[&card].zone == Zone::Hand {
            Ok(())
        } else {
            Err("Interrupt can only be activated from the hand".to_string())
        }
    }

    // --- Once Per Turn, declining, optional effects (QA #37, #44, #46,
    //     #48) -------------------------------------------------------------

    /// QA #37: an already-triggered AUTO can never be declined.
    /// Unconditional `Err` -- there is no state in which this succeeds.
    pub fn decline_auto(&mut self, _player: PlayerId, _card: CardId) -> Result<(), String> {
        Err("an AUTO whose trigger condition is met must always be activated".to_string())
    }

    /// QA #44/#48: locks out `(card, ability)` for the rest of the turn.
    /// Keyed on the card INSTANCE (not name) and the ability's own name,
    /// so two copies of one card lock out independently (#44), and two
    /// distinctly-named Once-Per-Turn abilities on the SAME card lock
    /// out independently of each other (#48). Nothing currently resets
    /// this at end of turn (no test exercises a second turn), so treat
    /// it as "locked out this session" rather than a fully turn-cycled
    /// model yet.
    ///
    /// A successful activation also begins resolving an effect (QA #90:
    /// an ACT's effect is "in progress" the same way any other effect
    /// is, so `effect_still_resolving()` reads true and a mid-resolution
    /// AUTO trigger correctly queues to standby instead of resolving
    /// immediately). The caller finishes it the same way any other
    /// resolution finishes -- `finish_resolving_effect()`.
    pub fn activate_act_named(&mut self, player: PlayerId, card: CardId, ability: &str) -> Result<(), String> {
        let key = (card, ability.to_string());
        let state = self.player_state(player);
        if state.acted_this_turn.contains(&key) {
            return Err(format!("ability {ability:?} on this card was already activated this turn"));
        }
        state.acted_this_turn.insert(key);
        self.resolving_effect = true;
        Ok(())
    }

    /// `activate_act_named` with the default (unnamed) ability -- QA #44
    /// doesn't distinguish which ability, only that each card instance
    /// has its own lockout.
    pub fn activate_act(&mut self, player: PlayerId, card: CardId) -> Result<(), String> {
        self.activate_act_named(player, card, "")
    }

    /// QA #46: "you may [X]" is never mandatory -- this is a fixed fact
    /// about the phrase, not something that varies by which optional
    /// effect it is, so `source`/`description` are accepted for the
    /// call site to read naturally but don't change the answer.
    pub fn optional_effect_may_be_declined(&self, _source: CardId, _description: &str) -> bool {
        true
    }

    // --- Quick timing, deck-out-safe look/reveal, choose-up-to-with-
    //     zero-matches (QA #28-30, #33-36, #38, #39, #59-61, #87, #95) ---

    /// QA #30: plays a Quick card, one at a time -- Err if the timing
    /// wouldn't allow it (reuses `can_play_card`'s exact rule so the two
    /// can never disagree). No real card effect resolves (no effect-text
    /// interpreter exists yet); this only records that a legal Quick
    /// play happened.
    pub fn play_quick(&mut self, player: PlayerId, _number: &str, timing: Timing) -> Result<(), String> {
        if self.can_play_card(player, true, timing) {
            Ok(())
        } else {
            Err("Quick is not playable at this timing".to_string())
        }
    }

    /// QA #38/#76's "set the deck to exactly N cards" test hook -- an
    /// abstract override of the deck's zone count, same as
    /// `test_scenario`'s own default, just settable per-test.
    pub fn set_deck_size_for_test(&mut self, player: PlayerId, size: usize) {
        *self.zone_counts.get_mut(&(player, Zone::Deck)).unwrap() = size;
    }

    pub fn refill_deck_for_test(&mut self, player: PlayerId, size: usize) {
        *self.zone_counts.get_mut(&(player, Zone::Deck)).unwrap() = size;
    }

    /// QA #38/#39: "look at"/"reveal the top X" resolve with whatever is
    /// actually there when the deck holds fewer than X -- never an
    /// error, never padded. Existing tracked cards already in this
    /// player's Deck zone (placed via `place`, so they carry a real name
    /// for `main_name_of`) are preferred; any remaining count needed is
    /// synthesized as anonymous filler. Deliberately decrements the
    /// deck's zone count by however many were taken (see #39's test,
    /// which explicitly `refill_deck_for_test`s afterward to demonstrate
    /// the deck temporarily hitting 0 mid-resolution is not itself a
    /// loss -- QA #39/CLAUDE.md's deck-out note) -- this module has no
    /// "shuffle the looked-at cards back in a new order" mechanic, so
    /// "looked at" and "no longer in the deck count" are the same event
    /// here.
    fn look_or_reveal_top_n(&mut self, player: PlayerId, x: usize) -> Vec<CardId> {
        let available = self.zone_counts[&(player, Zone::Deck)];
        let n = x.min(available);

        let mut tracked: Vec<CardId> = self
            .cards
            .iter()
            .filter(|(_, c)| c.owner == player && c.zone == Zone::Deck)
            .map(|(id, _)| *id)
            .collect();
        tracked.sort();
        tracked.truncate(n);

        let mut ids = tracked;
        while ids.len() < n {
            let id = self.fresh_card_id();
            self.cards.insert(id, Self::new_card_instance(player, Zone::Deck, CardKind::Pal));
            ids.push(id);
        }

        *self.zone_counts.get_mut(&(player, Zone::Deck)).unwrap() -= n;
        ids
    }

    /// Materializes the single top card of `player`'s deck WITHOUT
    /// decrementing the deck's zone count -- the card returned here is
    /// about to be actually moved by the caller's own subsequent call
    /// (`move_to_graveyard` in `resolve_player_damage`'s mill loop;
    /// `deploy_from` in BP01-025 Chillet's real On Deploy, `cards/`'s
    /// first consumer), and that call does the zone-count bookkeeping
    /// itself -- doing it here too would double-decrement. Unlike
    /// `look_or_reveal_top_n` (Look/Reveal-top-X, QA #38/#39/#59/#60/
    /// #72), which is for the "look and leave it in the deck" shape and
    /// decrements immediately since nothing moves it further. `None`
    /// once the deck's count has already reached 0.
    ///
    /// Consults (and consumes) `top_of_deck` first, same as `draw` --
    /// TD01-012's "put it on the top of the deck" has to mean the same
    /// thing to whichever primitive next reads "the top of the deck",
    /// not just to `draw` specifically. Before this checked the marker,
    /// a card placed on top and then immediately consumed by THIS
    /// method (BP01-025 Chillet's own On Deploy reveal, which moves the
    /// card out of Deck without ever clearing the marker) left the
    /// marker dangling -- pointing at a card that had already left the
    /// deck entirely -- so the next unrelated `draw` would still find it
    /// and yank that card, now sitting in Base or the graveyard, back
    /// into Hand.
    pub fn take_top_of_deck(&mut self, player: PlayerId) -> Option<CardId> {
        if let Some(card) = self.top_of_deck.remove(&player) {
            return Some(card);
        }
        if self.zone_counts[&(player, Zone::Deck)] == 0 {
            return None;
        }
        let top = self.cards.iter().filter(|(_, c)| c.owner == player && c.zone == Zone::Deck).map(|(id, _)| *id).min();
        let id = top.unwrap_or_else(|| {
            let id = self.fresh_card_id();
            self.cards.insert(id, Self::new_card_instance(player, Zone::Deck, CardKind::Pal));
            id
        });
        Some(id)
    }

    pub fn look_at_top(&mut self, player: PlayerId, x: usize) -> Vec<CardId> {
        self.look_or_reveal_top_n(player, x)
    }

    pub fn reveal_top(&mut self, player: PlayerId, x: usize) -> Vec<CardId> {
        self.look_or_reveal_top_n(player, x)
    }

    pub fn has_lost(&self, player: PlayerId) -> bool {
        self.game_over == Some(player)
    }

    /// QA #59/#60/#87/#95: "choose up to Y specific cards ... " with zero
    /// matches is legal -- choose 0 and the rest of the effect resolves.
    /// Not itself a full selection-and-move implementation (no
    /// "specific" card-name matching exists, and there's no distinct
    /// "hand"/"graveyard" source parameter -- every current test's
    /// `chosen` slice is already filtered to the source zone before
    /// calling this), just legality: at most `max` chosen.
    pub fn choose_up_to_n_to_hand(&mut self, _player: PlayerId, chosen: &[CardId], max: usize) -> Result<(), String> {
        self.choose_up_to_n(chosen, max)
    }

    pub fn choose_up_to_n_to_deploy(&mut self, _player: PlayerId, chosen: &[CardId], max: usize) -> Result<(), String> {
        self.choose_up_to_n(chosen, max)
    }

    fn choose_up_to_n(&self, chosen: &[CardId], max: usize) -> Result<(), String> {
        if chosen.len() <= max {
            Ok(())
        } else {
            Err(format!("chose {} but at most {max} are allowed", chosen.len()))
        }
    }

    pub fn cards_in_zone(&self, player: PlayerId, zone: Zone) -> Vec<CardId> {
        let mut ids: Vec<CardId> = self.cards.iter().filter(|(_, c)| c.owner == player && c.zone == zone).map(|(id, _)| *id).collect();
        ids.sort();
        ids
    }

    /// QA #61: choosing an already-rested Pal for a "choose up to X,
    /// rest them, they won't stand next" effect is legal -- resting an
    /// already-rested card is simply a no-op on the rest state itself,
    /// but the "won't stand next" duration still applies independent of
    /// whether resting did anything.
    pub fn choose_and_rest_wont_stand_next(&mut self, _player: PlayerId, card: CardId) {
        self.rest(card);
        self.cards.get_mut(&card).expect("choose_and_rest_wont_stand_next: unknown card").keywords.insert("__wont_stand_next".to_string());
    }

    pub fn wont_stand_next_stand_phase(&self, card: CardId) -> bool {
        self.has_keyword(card, "__wont_stand_next")
    }

    // --- combat (9.2-9.7; QA #45, #49, #50, #71) --------------------------

    /// 9.3: declares a complete battle. Rules enforced here, not just
    /// documented:
    ///
    /// - 9.2.3: a Pal target must be RESTED. A standing Pal is never a
    ///   legal target -- panics rather than silently allowing it, since
    ///   this is exactly the rule CLAUDE.md flags as deciding half the
    ///   deck analysis in this project.
    /// - 9.7.4: attacks are not a batch. Declaring a new attack while an
    ///   earlier one hasn't reached end of battle panics -- the turn
    ///   player only gets to decide on another attack after 9.7
    ///   concludes the current one.
    /// - 9.3.2: declaring an attack rests the attacker, as part of
    ///   declaration itself -- not a side effect of the damage step, and
    ///   not undone by anything that happens later in the battle
    ///   (interrupt_attack nullifies the attack's effect, 5.18.1, but
    ///   never un-rests; see `rule_5_18_1_interrupted_attacker_stays_rested`).
    ///
    /// On Attack fires AT declaration (9.3.2), before block declaration
    /// (9.4) -- QA #71's whole point. That's why the mechanical facts
    /// (`attacked_a_structure`, `was_declared_pal_target`) are recorded
    /// right here, not deferred to `block`/`resolve_battle`.
    pub fn declare_attack(&mut self, attacker: CardId, target: AttackTarget) -> AttackId {
        assert!(
            self.battles.values().all(|b| b.concluded),
            "declare_attack: an earlier battle hasn't reached end of battle yet (9.7.4) -- attacks are not a batch"
        );

        match target {
            AttackTarget::Pal(pal) => {
                assert!(
                    self.cards[&pal].resting,
                    "declare_attack: {pal:?} is standing -- 9.2.3: only the player, a structure, or a RESTED Pal is a legal attack target"
                );
                self.cards.get_mut(&pal).unwrap().was_declared_pal_target = true;
            }
            AttackTarget::Structure(_) => {
                self.cards.get_mut(&attacker).unwrap().attacked_a_structure = true;
            }
            AttackTarget::Player(_) => {}
        }

        self.rest(attacker); // 9.3.2

        let id = self.fresh_attack_id();
        self.battles.insert(id, Battle { attacker, target: Some(target), concluded: false });
        id
    }

    /// 9.7.4: at most one battle is ever unconcluded at a time
    /// (`declare_attack` panics otherwise), so "the pending attack" is
    /// unambiguous. `engine::replay` needs this for log lines that
    /// react to an attack already in progress (an Interrupt, a battle's
    /// own resolution) without themselves naming the attacker.
    pub fn pending_attack(&self) -> Option<AttackId> {
        self.battles.iter().find(|(_, b)| !b.concluded).map(|(&id, _)| id)
    }

    pub fn attacker_of(&self, attack: AttackId) -> CardId {
        self.battles[&attack].attacker
    }

    /// QA #49: real card behaviour (the attacker's own On Attack
    /// removing its target) narrated explicitly, the same way
    /// `trigger_auto` cascades are -- this method has no knowledge of
    /// which card's ability does this, only that the battle's target is
    /// now gone.
    pub fn trigger_on_attack_removes_target(&mut self, attacker: CardId) {
        let battle = self.active_battle_mut(attacker);
        battle.target = None;
    }

    pub fn battle_target(&self, attack: AttackId) -> Option<AttackTarget> {
        self.battles[&attack].target
    }

    /// QA #49: retargeting a battle is simply not a legal action --
    /// always `Err`, not conditionally.
    pub fn retarget_attack(&mut self, _attack: AttackId, _new_target: PlayerId) -> Result<(), String> {
        Err("a battle's target cannot be changed once declared".to_string())
    }

    /// 9.6-9.7. Two things this gets right, not just documented:
    ///
    /// - **9.6.2 short-circuit**: if EITHER the attacker or a Pal/
    ///   structure target ceased to exist for any reason (not just "the
    ///   target was removed", QA #49's case -- the attacker itself could
    ///   be gone too), skip straight to end of battle. No damage step,
    ///   no damage check, for either side. This is an early return, not
    ///   a zero computed by the normal damage logic.
    /// - **9.6.3.1: damage is simultaneous.** Both sides' damage amounts
    ///   are read from Power BEFORE either `apply_damage` call, then
    ///   both applied -- not attacker-deals-damage-then-check-if-
    ///   defender-survives-then-defender-deals-damage. This is what
    ///   makes an equal-Power trade kill both sides (the `>=` boundary
    ///   already pinned down by QA #93/`rule_11_4_1`). Lethal damage
    ///   itself is 9.6.4, a check-type rule action -- resolved by the
    ///   caller's own `run_check_timing()` afterward, same as everywhere
    ///   else `apply_damage` is used; this method never calls it itself.
    ///
    /// Player damage (the `AttackTarget::Player` arm) is 11.2.1's
    /// *interrupt-type* rule action, resolved by `resolve_player_damage`
    /// -- an ordinary call made directly from here, never queued,
    /// exactly as the module doc's "interrupt-type actions... will be
    /// ordinary function calls" note promises. Structure damage is a
    /// separate, still-unimplemented rule (no `Durability` field exists
    /// on `CardInstance`, and no currently-written test exercises it) --
    /// left as an explicit `todo!` rather than guessed at.
    pub fn resolve_battle(&mut self, attack: AttackId) {
        let attacker = self.battles[&attack].attacker;
        let target = self.battles[&attack].target;

        let attacker_gone = self.cards[&attacker].zone != Zone::Base;
        let target_gone = match target {
            None => true, // QA #49: target removed mid-battle
            Some(AttackTarget::Pal(p)) => self.cards[&p].zone != Zone::Base,
            Some(AttackTarget::Structure(s)) => self.cards[&s].zone != Zone::Base,
            Some(AttackTarget::Player(_)) => false, // a player can't "cease to exist"
        };

        if attacker_gone || target_gone {
            self.cards.get_mut(&attacker).unwrap().end_of_battle_as_attacker_triggered = true;
            self.battles.get_mut(&attack).unwrap().concluded = true;
            return;
        }

        match target.unwrap() {
            AttackTarget::Pal(defender) => {
                let attacker_power = self.power(attacker).max(0) as u32;
                let defender_power = self.power(defender).max(0) as u32;
                self.apply_damage(defender, attacker_power);
                self.apply_damage(attacker, defender_power);
            }
            AttackTarget::Player(player) => {
                self.resolve_player_damage(attacker, player);
            }
            AttackTarget::Structure(_) => {
                todo!("resolve_battle: structure damage step is not implemented yet -- no Durability field exists on CardInstance")
            }
        }

        self.cards.get_mut(&attacker).unwrap().end_of_battle_as_attacker_triggered = true;
        self.battles.get_mut(&attack).unwrap().concluded = true;
    }

    /// 11.2.1/11.2.2: player damage resolution. `deal_damage_to_player`
    /// already encodes QA #64 (Strike <= 0 deals no damage at all --
    /// the mill loop below never starts).
    ///
    /// 11.2.2.1/11.2.2.2: mill the top card of the deck one at a time.
    /// The instant a milled card has the Lucky icon, the loop stops and
    /// ALL remaining damage is cancelled outright -- no life is lost,
    /// even if only 1 of N cards has been milled so far. This is a
    /// `return`, not a `break`: falling through to the `reduce_life`
    /// call below would be exactly the bug this rule exists to prevent.
    ///
    /// 11.2.2.3: absent a Lucky card, the loop stops once the mill count
    /// reaches the damage amount, OR the deck runs out first -- and in
    /// BOTH of those exits, life drops by the FULL damage amount, never
    /// by however many cards were actually milled. Those two numbers
    /// only diverge in the endgame case (deck shorter than the damage),
    /// which is exactly the case worth a dedicated test for, since the
    /// common case can't tell the two readings apart.
    ///
    /// Milling the deck to 0 here is NOT itself a loss (QA #38/#39,
    /// same principle as `look_or_reveal_top_n`) -- the losing verdict
    /// (11.3.3) is a check-type rule action that only `run_check_timing`
    /// ever evaluates, via `life`, never `game_over` set directly here.
    /// (Drawing from an empty deck, 11.3.2, is a different rule action
    /// entirely and not modeled by this method.)
    fn resolve_player_damage(&mut self, attacker: CardId, target: PlayerId) {
        let damage = self.deal_damage_to_player(attacker, target);
        if damage == 0 {
            return;
        }

        let mut milled = 0u32;
        while let Some(card) = self.take_top_of_deck(target) {
            self.move_to_graveyard(card, Zone::Deck);
            milled += 1;
            if self.is_lucky(card) {
                return; // 11.2.2.2: cancels ALL remaining damage.
            }
            if milled >= damage {
                break;
            }
        }

        self.reduce_life(target, damage as i32); // 11.2.2.3: full damage, not `milled`.
    }

    /// 9.4.2: the non-turn player rests a STANDING Pal to become the
    /// battle's actual opponent -- blocking is only legal for a
    /// standing Pal, and rests it as a consequence, the same direction
    /// as `declare_attack`'s target check (9.2.3: only a rested Pal is
    /// attackable in the first place). Panics if `blocker` is already
    /// rested rather than silently re-resting it, for the same reason
    /// `declare_attack` panics on a standing target: this precondition
    /// is easy to get backwards (a blocker that's ALREADY rested cannot
    /// legally block at all).
    ///
    /// `blocker` becomes the battle opponent, but is NOT the originally
    /// *declared* target -- QA #50's whole point, which is why this
    /// doesn't set `was_declared_pal_target`.
    pub fn block(&mut self, attack: AttackId, blocker: CardId) {
        assert!(
            !self.cards[&blocker].resting,
            "block: {blocker:?} is already rested -- 9.4.2: only a STANDING Pal may block (blocking is what rests it)"
        );
        self.rest(blocker);
        // The blocker becomes the battle's actual opponent for the
        // damage step -- `resolve_battle` reads `target` straight off
        // the `Battle`, so without this reassignment it would still
        // resolve against whatever was originally declared (QA #50's
        // own fixture never calls `resolve_battle` afterward, which is
        // why this was missed). `was_declared_pal_target` deliberately
        // stays untouched -- that flag means "was the ORIGINAL declared
        // target," which a blocker never is (QA #50).
        self.battles.get_mut(&attack).unwrap().target = Some(AttackTarget::Pal(blocker));
    }

    /// QA #45: fires even when the attack was Interrupted -- 12.8.2:
    /// Interrupt nullifies the attack, but the battle still reaches end
    /// of battle (9.7), it just does so without a damage step.
    pub fn interrupt_attack(&mut self, attacker: CardId) {
        let battle = self.active_battle_mut(attacker);
        battle.concluded = true;
        let card = self.cards.get_mut(&attacker).unwrap();
        card.attack_was_interrupted = true;
        card.end_of_battle_as_attacker_triggered = true;
    }

    fn active_battle_mut(&mut self, attacker: CardId) -> &mut Battle {
        self.battles
            .values_mut()
            .find(|b| b.attacker == attacker && !b.concluded)
            .expect("no active (undeclared-concluded) battle for this attacker")
    }

    pub fn attack_was_interrupted(&self, card: CardId) -> bool {
        self.cards[&card].attack_was_interrupted
    }

    pub fn end_of_battle_this_card_attacked_triggered(&self, card: CardId) -> bool {
        self.cards[&card].end_of_battle_as_attacker_triggered
    }

    /// QA #71: a mechanical fact of what was declared, set at
    /// declaration and never un-set -- being blocked afterward doesn't
    /// retroactively change what already happened at 9.3.
    pub fn on_attacks_structure_triggered(&self, card: CardId) -> bool {
        self.cards[&card].attacked_a_structure
    }

    /// QA #50: true only for a card that was the *declared* target of
    /// an attack (`AttackTarget::Pal`) -- never for a card that merely
    /// became the battle opponent via `block`.
    pub fn on_attacked_triggered(&self, card: CardId) -> bool {
        self.cards[&card].was_declared_pal_target
    }

    // --- Q&A about Cards: generic primitives, each grounded in the real
    //     effect_text of the card(s) the ruling names (data/
    //     11cardattributes-with-elements.csv), never the QA paraphrase
    //     alone. See individual test doc comments in qa_about_cards.rs
    //     for the citation; this section is the mechanism, not the
    //     per-card behaviour, which stays out of engine/ (see the QA #84
    //     rewrite's reasoning, restated throughout below). -------------

    pub fn base_power(&self, card: CardId) -> i32 {
        self.cards[&card].power
    }

    /// QA #52 (Jormuntide Ignis, "ACT 1/Turn [3] OR [Discard 2 cards]
    /// Stand this card"): the lockout is per-ABILITY, not per-cost --
    /// `activate_act_named`'s key is `(card, ability)`, never the cost
    /// paid, so a second attempt with a *different* alternative cost is
    /// still the same ability and still locked out. `cost_description`
    /// is accepted so a call site can state which cost it's narrating,
    /// but never affects the lockout key.
    pub fn activate_act_paying(&mut self, player: PlayerId, card: CardId, _cost_description: &str) -> Result<(), String> {
        self.activate_act_named(player, card, "")
    }

    /// QA #56 (Mounted Machine Gun, "[Consume X Material, assign 1 Pal]
    /// Perform <Choose 1 Pal, deal 500> X times"): consumes the stated
    /// materials for `player`. Takes the card's number rather than a
    /// `CardId` because the ruling this backs never deploys the
    /// structure at all (it's about the "perform...X times" idiom, not
    /// this specific gear's own lockout), so there's nothing to key a
    /// per-instance lockout on here.
    pub fn activate_act_consuming(&mut self, player: PlayerId, _card_number: &str, materials: u32) {
        self.consume_materials(player, materials);
        self.resolving_effect = true;
    }

    /// QA #56: "perform <effect> X times" -- distinct from a single
    /// "Choose X" (QA #33/#34's `Selector`), repeats the WHOLE
    /// sub-effect, closure and all, X times. Generic on purpose: it
    /// doesn't know or care what the closure does.
    pub fn perform_n_times(&mut self, n: u32, mut effect: impl FnMut(&mut Self)) {
        for _ in 0..n {
            effect(self);
        }
    }

    pub fn choose_one_and_deal_damage(&mut self, target: CardId, amount: u32) {
        self.apply_damage(target, amount);
    }

    /// QA #53 (Suzaku, "CONT When your red card would deal Damage other
    /// than battle damage to a Pal, deal +200 Damage instead"): a
    /// dedicated flag, same reasoning as `set_auto_doubling_cont`.
    pub fn set_boosts_red_pal_damage(&mut self, card: CardId, active: bool) {
        self.cards.get_mut(&card).expect("set_boosts_red_pal_damage: unknown card").boosts_red_pal_damage = active;
    }

    /// QA #53: non-battle damage from a red source, boosted +200 if the
    /// source's controller has an active damage-boosting CONT in the
    /// base. "Other than battle damage" is exactly what this method
    /// name says -- `resolve_battle`'s damage step calls `apply_damage`
    /// directly, never this, so the boost can never double-apply to
    /// combat damage.
    pub fn deal_damage_from_source(&mut self, source: CardId, target: CardId, amount: u32) -> u32 {
        let owner = self.cards[&source].owner;
        let is_red = self.cards[&source].color.as_deref() == Some("Red");
        let boosted = is_red
            && self
                .cards
                .values()
                .any(|c| c.owner == owner && c.zone == Zone::Base && c.boosts_red_pal_damage);
        let total = if boosted { amount + 200 } else { amount };
        self.apply_damage(target, total);
        total
    }

    /// QA #57/#58 (Foxparks' Harness): grants `target` a new, distinct
    /// instance of the ability described by `text`, sourced from
    /// `source`. Two grants of identical text are two instances, not
    /// one -- `granted_ability_instances` returns every live one.
    pub fn grant_ability_instance(&mut self, _source: CardId, target: CardId, text: &str) -> GrantedInstanceId {
        let id = GrantedInstanceId(self.next_granted_instance_id);
        self.next_granted_instance_id += 1;
        self.granted_instances.insert(id, GrantedInstance { target, text: text.to_string() });
        id
    }

    pub fn granted_ability_instances(&self, target: CardId) -> Vec<GrantedInstanceId> {
        let mut ids: Vec<GrantedInstanceId> =
            self.granted_instances.iter().filter(|(_, g)| g.target == target).map(|(id, _)| *id).collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// QA #58: each granted instance resolves independently, so the same
    /// Pal may be `chosen` by more than one instance. The damage amount
    /// is read out of the instance's own granted text rather than
    /// hardcoded, so granting a differently-worded instance doesn't
    /// require touching this method -- the LAST whitespace-delimited
    /// numeric token (e.g. "...deal 700"), not every digit in the text
    /// concatenated. Text like "choose up to 1 Pal, deal 700" has TWO
    /// numbers in it ("1" and "700"); concatenating every digit
    /// character read that as 1700, not 700 -- caught by QA #58 itself
    /// (700*2=1400 expected, got 1700*2=3400), fixed before it shipped
    /// silently.
    pub fn resolve_granted_on_attack(&mut self, instance: GrantedInstanceId, chosen: Option<CardId>) {
        let Some(target) = chosen else { return };
        let text = &self.granted_instances[&instance].text;
        let amount: u32 = text.split_whitespace().rev().find_map(|word| word.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok()).unwrap_or(0);
        self.apply_damage(target, amount);
    }

    /// QA #62 (Relaxaurus, "...rest it. While this card is in the base,
    /// that card does not stand"): `locked` cannot be stood by a card
    /// effect while `source` remains in the base -- `stand_via_effect`
    /// enforces it, `lock_from_standing` records it.
    pub fn lock_from_standing(&mut self, locked: CardId, source: CardId) {
        self.cards.get_mut(&locked).expect("lock_from_standing: unknown card").locked_by = Some(source);
    }

    pub fn stand_via_effect(&mut self, locked: CardId) -> Result<(), String> {
        if let Some(source) = self.cards[&locked].locked_by {
            if self.cards[&source].zone == Zone::Base {
                return Err(format!("{locked:?} is locked from standing by {source:?}, still in the base"));
            }
        }
        self.cards.get_mut(&locked).unwrap().resting = false;
        Ok(())
    }

    /// QA #64 (Wumpo CONT: opponent's Pals get Strike -1). Signed,
    /// unclamped, same convention as `power`/`reduce_power` -- see
    /// `reduce_power`'s doc comment for why the split-method shape (vs.
    /// one signed-delta method) is deliberate.
    pub fn strike(&self, card: CardId) -> i32 {
        let base = self.cards[&card].strike;
        let modifiers: i32 = self.cont_strike_modifiers.values().filter(|m| m.target == card).map(|m| m.amount).sum();
        base + modifiers
    }

    /// See `apply_cont_power_modifier` -- identical shape, for Strike
    /// instead of Power.
    pub fn apply_cont_strike_modifier(&mut self, _source: CardId, target: CardId, amount: i32) -> ModifierId {
        let id = self.fresh_modifier_id();
        self.cont_strike_modifiers.insert(id, ContModifier { target, amount });
        id
    }

    /// QA #64: unlike Power (referenced but never clamped -- QA #92/93),
    /// Strike <= 0 blocks player damage outright. No real player-damage/
    /// Lucky-mill system exists yet (`resolve_battle`'s player-target
    /// arm is still a `todo!`) -- this only implements the specific
    /// Strike<=0 short-circuit QA #64 asks about, not the full 11.2.1
    /// resolution.
    pub fn deal_damage_to_player(&self, attacker: CardId, _target: PlayerId) -> u32 {
        let s = self.strike(attacker);
        if s <= 0 {
            0
        } else {
            s as u32
        }
    }

    /// 12.14.2 (Breakthrough, CLAUDE.md's keyword table): damage equals
    /// this card's CURRENT Strike, not its printed value -- QA #65 is
    /// exactly this, exercised through a CONT modifier.
    pub fn breakthrough_damage(&self, card: CardId) -> i32 {
        self.strike(card)
    }

    pub fn work_suitabilities_of(&self, card: CardId) -> Vec<String> {
        self.cards[&card].work_suitabilities.clone()
    }

    /// QA #66's exact point: a real attribute like Work Suitability is
    /// never satisfied by a granted NAME, however similar the text looks
    /// -- this reads `work_suitabilities`, never `granted_names`.
    pub fn assigned_structure_has_suitability(&self, structure: CardId, suitability: &str) -> bool {
        self.has_work_suitability(structure, suitability)
    }

    /// Registers `name` as a real, released card name -- see
    /// `engine::targeting`'s module doc comment for why this is an
    /// explicit test-scenario registry rather than the real CSV.
    pub fn register_known_card_name(&mut self, name: &str) {
        self.known_card_names.insert(name.to_string());
    }

    /// Registers `name` as real but not yet released (QA #67).
    pub fn register_unreleased_card_name(&mut self, name: &str) {
        self.known_unreleased_card_names.insert(name.to_string());
    }

    /// QA #67/#68 (Antique Dresser, "Declare 1 card name"): `Ok` only for
    /// a name registered via `register_known_card_name`; `NotYetReleased`
    /// for one registered via `register_unreleased_card_name`;
    /// `DoesNotExist` for anything else.
    pub fn declare_card_name(&mut self, _player: PlayerId, name: &str) -> Result<DeclaredName, DeclareNameError> {
        if self.known_card_names.contains(name) {
            Ok(DeclaredName(name.to_string()))
        } else if self.known_unreleased_card_names.contains(name) {
            Err(DeclareNameError::NotYetReleased)
        } else {
            Err(DeclareNameError::DoesNotExist)
        }
    }

    /// QA #66/#97: the declared name is granted "in addition" -- the
    /// printed name is never removed, so `card_names` grows, it doesn't
    /// replace. Takes `declared` by reference (rather than consuming it)
    /// so QA #97's "grant the SAME declared name to several cards" reads
    /// naturally in a loop instead of needing `.clone()` at every call.
    pub fn grant_name_until_end_of_turn(&mut self, card: CardId, declared: &DeclaredName) {
        self.cards.get_mut(&card).expect("grant_name_until_end_of_turn: unknown card").granted_names.push(declared.0.clone());
    }

    /// The printed name (if set via `set_card_name`) plus every granted
    /// name still in effect.
    pub fn card_names(&self, card: CardId) -> Vec<String> {
        let instance = &self.cards[&card];
        instance.card_name.iter().cloned().chain(instance.granted_names.iter().cloned()).collect()
    }

    /// QA #97: "3 Pals with DIFFERENT card names containing X" counts
    /// DISTINCT qualifying names across the player's base, not distinct
    /// Pals -- three Pals sharing one identical granted name contribute
    /// exactly one distinct name, so this returns false for them (QA
    /// #97's whole point: a name granted identically to several cards
    /// isn't "different" between them).
    pub fn has_n_pals_with_distinct_names_containing(&self, player: PlayerId, substring: &str, n: usize) -> bool {
        let mut names: Vec<String> = self
            .cards
            .iter()
            .filter(|(_, c)| c.owner == player && c.zone == Zone::Base)
            .flat_map(|(id, _)| self.card_names(*id))
            .filter(|name| name.contains(substring))
            .collect();
        names.sort();
        names.dedup();
        names.len() >= n
    }

    /// QA #70 (Victor's Strategy): marks that a card was played --
    /// reuses the same per-turn counter QA #96 already established
    /// (rest-3-draw-1 deliberately does NOT increment this one).
    pub fn play_card(&mut self, player: PlayerId, _card_number: &str) {
        self.player_state(player).cards_played_this_turn += 1;
    }

    /// QA #70: "choose 1" (not "choose up to 1") -- legal iff exactly
    /// `min(1, available.len())` were chosen. The specific "1" is
    /// Victor's Strategy's own printed wording, not a general "choose
    /// exactly N" parameter, because no other ruling here needs a
    /// different N; `engine::selection::Selector::choose_exact` is the
    /// general form if a future one does.
    pub fn choose_exactly_n_to_hand(&self, _player: PlayerId, available: &[CardId], chosen_count: usize) -> Result<(), String> {
        let required = 1usize.min(available.len());
        if chosen_count == required {
            Ok(())
        } else {
            Err(format!("must choose exactly {required}, chose {chosen_count}"))
        }
    }

    /// QA #72 (Elizabee ACT): puts `card` back on top of the deck,
    /// unchanged -- a minimal, single-card "what's on top" fact, not a
    /// full ordered deck model (nothing else here needs deck order).
    pub fn return_to_top_unchanged(&mut self, player: PlayerId, card: CardId) {
        self.top_of_deck.insert(player, card);
    }

    pub fn top_of_deck(&self, player: PlayerId) -> Option<CardId> {
        self.top_of_deck.get(&player).copied()
    }

    /// A generic "move `card` from `from_zone` to the top of `player`'s
    /// deck" primitive, with REAL zone bookkeeping -- unlike
    /// `return_to_top_unchanged`, which only ever puts back a card
    /// `look_at_top` already decremented the deck count for (so it
    /// deliberately leaves zone_counts alone, since that card never
    /// really left). Here the card is coming from a genuinely different
    /// zone (TD01-012 Elphidran Aqua's real On Deploy: "...choose 1
    /// card from your hand, and put it on the top of the deck"), so
    /// this decrements `from_zone` and increments Deck for real, then
    /// sets the same `top_of_deck` fact -- `draw`'s "consult
    /// top_of_deck first" is what makes that fact actually observed by
    /// the next draw, not just a queryable side-channel.
    pub fn move_to_top_of_deck(&mut self, player: PlayerId, card: CardId, from_zone: Zone) {
        let instance = self.cards.get_mut(&card).expect("move_to_top_of_deck: unknown card");
        assert_eq!(instance.zone, from_zone, "move_to_top_of_deck: card is not actually in from_zone");
        instance.zone = Zone::Deck;
        *self.zone_counts.get_mut(&(player, from_zone)).unwrap() -= 1;
        *self.zone_counts.get_mut(&(player, Zone::Deck)).unwrap() += 1;
        self.top_of_deck.insert(player, card);
    }

    /// QA #77 (Digtoise's Headband / Foxparks' Harness-shaped gears:
    /// "[Rest this card] Choose 1 Pal, +200 Power until EOT. If its main
    /// name is X, get 2 Material or 2 Ingredient"): generic over which
    /// main name qualifies for the resource bonus, so this isn't a
    /// Digtoise (or Foxparks) special case -- QA #77's specific point is
    /// that `target` may be the OPPONENT's Pal and still gets buffed.
    pub fn apply_gear_power_buff_and_bonus_resource(&mut self, player: PlayerId, target: CardId, matching_main_name: &str) {
        self.increase_power(target, 200);
        if self.main_name_of(target) == matching_main_name {
            self.gain_materials(player, 2);
        }
    }

    /// QA #78/#80 (Helzephyr): "your Pal with Nocturnal is deployed" --
    /// Nocturnal is a keyword like any other (`grant_keyword`/
    /// `has_keyword`), printed or granted, checked the same way either
    /// case.
    pub fn nocturnal_active(&self, card: CardId) -> bool {
        self.has_keyword(card, "Nocturnal")
    }

    /// The mechanical fact `deploy`/`deploy_from` would report if asked
    /// right after deploying `card`: On Deploy fires on any move into
    /// the base (QA #32), so this is simply "is `card` currently in the
    /// base" -- no separate tracking needed, since nothing currently
    /// moves a card into Base without also meaning to deploy it.
    pub fn last_deploy_result(&self, card: CardId) -> DeployFromResult {
        DeployFromResult { card, on_deploy_triggered: self.cards[&card].zone == Zone::Base }
    }

    pub fn set_cost(&mut self, card: CardId, cost: u32) {
        self.cards.get_mut(&card).expect("set_cost: unknown card").cost = cost;
    }

    /// Sets this card's base Strike directly -- distinct from
    /// `apply_cont_strike_modifier`, which adds a continuous modifier on
    /// top rather than replacing the base value. `engine::replay` needs
    /// this to seed a materialized card with its real printed Strike
    /// (`data/11cardattributes-with-elements.csv`), the same way
    /// `set_cost` already does for Cost.
    pub fn set_strike(&mut self, card: CardId, strike: i32) {
        self.cards.get_mut(&card).expect("set_strike: unknown card").strike = strike;
    }

    pub fn cost(&self, card: CardId) -> u32 {
        self.cards[&card].cost
    }

    /// QA #89 (Katress): the one-time "deploy at exactly [butchered
    /// cost - 1]" allowance her AUTO grants -- narrated explicitly by
    /// the caller (computed from `cost(butchered) - 1`), not derived
    /// here, since knowing WHICH butchered Pal grants WHICH allowance is
    /// Katress's specific effect text, not a generic engine fact.
    pub fn set_deploy_cost_allowance(&mut self, allowance: Option<u32>) {
        self.deploy_cost_allowance = allowance;
    }

    /// QA #89: exact match, not a ceiling -- a cost-4 Pal is legal, a
    /// cost-3 or cost-2 Pal is not, when the allowance is exactly 4.
    pub fn deploy_cost_allowance_permits(&self, cost: u32) -> bool {
        self.deploy_cost_allowance == Some(cost)
    }
}
