"""Parser for "Format A" replay logs -- the site's own narrative export
shape, structurally unrelated to replay_parser.py's bracketed `[Tn phase]`
format (that one was built against three hand-authored fixture logs which
predate this corpus; this module and its own tests are additive, and
replay_parser.py/its 27 hand-verified tests are untouched).

Format A has no fixed "Edify" perspective -- every match names two real
players, either of whom can be the block's active actor -- so nothing here
is allowed to hardcode a name. Two on-disk shapes share this one grammar:

  - "plain" (data/replays/*.log, split from COMBINED_TIER1_TIER2_44_MATCHES.txt):
    TURN N / ACTOR blocks, action sentences, no hand-reveal lines.
  - "with hand reveals" (data/replays/top_players_all20/*.txt): a strict
    superset -- same grammar, plus "Turn opened -- Souls N/N . Hand: ..."
    lines this module parses into structured soul/hand data rather than
    leaving as free text.

Same output schema as replay_parser.py: line_index, turn, phase, actor,
action, cards, targets, resolution, caused_by_line, text. `phase` is
always None here -- Format A has no phase-tag concept, only the block
actor. This module does NOT interpret card behaviour or rules, does NOT
resolve who truly "owns" a named card, and does NOT resolve what a
pronoun ("them"/"they") truly refers to -- same division of labor as
replay_parser.py: it structures what the log literally says, an engine/
replay driver resolves the rest.

## `actor`, precisely

Each block opens with a bare all-caps name line ("CHYLITH", "PINGU",
"OPPONENT" -- the literal placeholder a site anonymization setting
substitutes for a real name, see `opponent_anonymized` in the meta dict)
that sets the block's actor for every event in it. A sentence with its
own explicit subject ("Pingu interrupted with...") overrides the block
actor for THAT line only -- the same "explicit text wins over the
ambient tag" principle replay_parser.py already uses for its one
Edify-in-a-waiting-line exception, generalized here since Format A has
no fixed perspective to fall back on.

## Bare marker lines

Two single-token line shapes carry no verb of their own and only mean
something via what follows: a lone card name (a bare-form interrupt, or
the attacker's name repeated before a "destroyed" line -- its exact
rules meaning is not established, so this module records the fact of it
rather than guessing) and a lone "*" decorative marker (always
immediately preceding a Lucky-cancelled attack line in this corpus).
Both become `action: "bare_marker"` events; whatever structured event
immediately follows gets `caused_by_line` pointing back at it.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable, Optional

Groups = dict[str, str]
Builder = Callable[[Groups], tuple[list[str], list[str], Optional[dict[str, Any]]]]

# --- header / meta -----------------------------------------------------

_HEADER_DIVIDER_RE = re.compile(r"^=+$")
_SUMMARY_DIVIDER_RE = re.compile(r"^-{10,}$")

_MATCH_RE = re.compile(r"^MATCH:\s*(.*)$")
_RECORD_ID_RE = re.compile(r"^recordId:\s*(.*)$")
_RESULT_RE = re.compile(r"^RESULT:\s*(.*)$")
_NOTE_RE = re.compile(r"^NOTE:\s*(.*)$")

_RECONSTRUCTED_MARK = "FULLY RECONSTRUCTED FROM THE RAW TAGGED EVENT LOG"
_NO_GAP_MARK_RE = re.compile(r"no unexplained gap", re.IGNORECASE)
_ANON_OPPONENT_RE = re.compile(r'shown on-screen as\s*"Opponent"')
_QUOTED_RE = re.compile(r'"([^"]+)"')


def _participant_aliases(match_title: Optional[str]) -> dict[str, str]:
    """Maps casefolded alias -> canonical display form, derived from the
    header's own MATCH line -- never a hardcoded name. "Opponent" is
    always included: the site's own anonymization placeholder (see
    `opponent_anonymized`), which stands in for a real name on either
    side in some files. Each side contributes its leading name token
    (everything before the first "(" or "[") plus any quoted alias
    ("in-game name \"bel\"", "shown on-screen as \"Opponent\"") --
    covers every phrasing this corpus actually uses without parsing the
    annotation prose itself."""
    aliases = {"opponent": "Opponent"}
    if not match_title:
        return aliases
    for side in match_title.split(" vs ", 1):
        lead = re.split(r"[\(\[]", side, 1)[0].strip()
        if lead:
            aliases[lead.casefold()] = lead
        for quoted in _QUOTED_RE.findall(side):
            aliases[quoted.casefold()] = quoted
    return aliases


def _parse_header(lines: list[str], start: int) -> tuple[dict[str, Any], int]:
    """Parses the header block starting at `lines[start]` (the opening
    "====...====" divider) through its matching closing divider.
    Returns (meta, index_of_line_after_closing_divider)."""
    assert _HEADER_DIVIDER_RE.match(lines[start].strip())
    meta: dict[str, Any] = {"match_title": None, "recordId": None, "result": None, "notes": []}
    current_note: Optional[list[str]] = None

    i = start + 1
    while i < len(lines):
        line = lines[i].rstrip("\n")
        stripped = line.strip()
        if _HEADER_DIVIDER_RE.match(stripped):
            i += 1
            break

        if not stripped:
            i += 1
            continue

        m = _MATCH_RE.match(stripped)
        if m:
            meta["match_title"] = m.group(1).strip()
            current_note = None
            i += 1
            continue

        m = _RECORD_ID_RE.match(stripped)
        if m:
            meta["recordId"] = m.group(1).strip()
            current_note = None
            i += 1
            continue

        m = _RESULT_RE.match(stripped)
        if m:
            meta["result"] = m.group(1).strip()
            current_note = None
            i += 1
            continue

        m = _NOTE_RE.match(stripped)
        if m:
            current_note = [m.group(1).strip()]
            meta["notes"].append(current_note)
            i += 1
            continue

        # A continuation line (wrapped NOTE text, or an indented
        # sub-bullet like the bare-form-interrupt list) -- joined onto
        # the current NOTE the same way replay_parser.py's own
        # _parse_meta_line joins wrapped header lines.
        if current_note is not None:
            current_note.append(stripped)
        i += 1

    meta["notes"] = [" ".join(parts) for parts in meta["notes"]]
    meta["provenance"] = "reconstructed" if any(_RECONSTRUCTED_MARK in n for n in meta["notes"]) else "curated"
    meta["gap_expectation"] = "clean" if any(_NO_GAP_MARK_RE.search(n) for n in meta["notes"]) else "unstated"
    meta["opponent_anonymized"] = bool(meta["match_title"] and _ANON_OPPONENT_RE.search(meta["match_title"]))
    return meta, i


# --- body: control lines (consumed, no event) --------------------------

_TURN_RE = re.compile(r"^TURN (\d+)(?: \(.*\))?$")
_TRAILING_BRACKET_RE = re.compile(r"\s*\[[^\[\]]*\]\s*$")
_RESPONSE_SUFFIX_RE = re.compile(r"\s*\(response\)\s*$")
_BARE_MARKER_RE = re.compile(r"^(\w[\w' .\-]*[\w]|[✦✧✨★☆◆◇])$")


# --- body: event line patterns ------------------------------------------
# Each builder receives regex groups and returns (cards, targets, resolution).
# `subj` (when present) overrides the block actor for that one event --
# see the module doc comment.


def _split_hand(raw: str) -> list[str]:
    return [c.strip() for c in raw.split(",") if c.strip()]


def b_turn_opened(g: Groups):
    hand = _split_hand(g["hand"])
    return [], [], {"souls": int(g["cur"]), "souls_cap": int(g["cap"]), "hand": hand}


def b_deploy_slot(g: Groups):
    return [g["card"]], [], {"slot": int(g["slot"]), "cost": int(g["cost"])}


def b_card_only(g: Groups):
    return [g["card"]], [], None


def b_none(g: Groups):
    return [], [], None


def b_reveal_element(g: Groups):
    return [g["card"]], [], {"element": g["element"], "power_buff": int(g["amount"])}


def b_reveal_added(g: Groups):
    return [g["card"]], [], {"outcome": "added_to_hand"}


def b_attack_declare_player(g: Groups):
    return [g["card"]], [g["target"]], {"damage": int(g["dmg"])}


def b_attack_declare_card(g: Groups):
    return [g["card"]], [g["target"]], None


def b_attack_them(g: Groups):
    return (
        [g["card"]],
        [g["target"]],
        {
            "damage": int(g["dmg"]),
            "milled": int(g["milled"]),
            "lucky": False,
            "life_before": int(g["life_before"]),
            "life_after": int(g["life_after"]),
        },
    )


def b_attack_lucky(g: Groups):
    return (
        [g["card"]],
        [g["target"]],
        {"damage_attempted": int(g["dmg"]), "damage": 0, "milled": int(g["milled"]), "lucky": True},
    )


def _power_value(expr: str) -> int:
    expr = expr.strip()
    if "=" in expr:
        return int(expr.rsplit("=", 1)[1].strip())
    return int(expr)


def b_battle_attack(g: Groups):
    return (
        [g["attacker"]],
        [g["defender"]],
        {
            "attacker_power_expr": g["atk_power"].strip(),
            "attacker_power": _power_value(g["atk_power"]),
            "defender_power_expr": g["def_power"].strip(),
            "defender_power": _power_value(g["def_power"]),
        },
    )


def b_destroyed_vs(g: Groups):
    resolution = {"damage": int(g["dmg"]), "power": int(g["power"])}
    if g.get("both_sides"):
        resolution["both_sides"] = True
    return [g["card"]], [], resolution


def b_interrupt_soul(g: Groups):
    return [g["card"]], [], {"cost": {"type": "soul", "amount": int(g["cost"])}, "result": "nullified"}


def b_interrupt_discard(g: Groups):
    return [g["card"]], [], {"cost": {"type": "discard", "card": g["discard"]}, "result": "nullified"}


def b_drew_amount(g: Groups):
    return [], [], {"amount": int(g["amount"])}


def b_drew_for_turn(g: Groups):
    return [], [], {"amount": 1}


def b_rest_draw(g: Groups):
    return [], [], {"souls_spent": int(g["souls"]), "drew": int(g["amount"])}


def b_play_with_cost(g: Groups):
    return [g["card"]], [], {"cost": int(g["cost"])}


def b_power_buff(g: Groups):
    resolution: dict[str, Any] = {"amount": int(g["amount"])}
    if g.get("skill"):
        resolution["granted_skill"] = g["skill"]
    return [g["card"]], [], resolution


def b_mass_damage(g: Groups):
    return [], [], {"amount": int(g["amount"]), "scope": g["scope"], "self_sacrifice": True}


def b_discard_one(g: Groups):
    return [g["card"]], [], None


def b_discard_multi(g: Groups):
    return [c.strip() for c in g["cards"].split(",")], [], None


def b_must_discard(g: Groups):
    return [], [], {"amount": int(g["amount"])}


def b_status_effect_next_stand(g: Groups):
    return [g["card"]], [], {"effect": "wont_stand_next_stand_phase"}


def b_status_effect_while_source(g: Groups):
    return [g["card"]], [], {"effect": "wont_stand_while_source_in_play"}


def b_win_declared(g: Groups):
    return [], [], {"winner": g["winner"]}


def b_loss_reason(g: Groups):
    return [], [], {"loser": g["loser"], "reason": g["reason"]}


def b_final_life(g: Groups):
    return (
        [],
        [],
        {
            "winner": g["winner"],
            "winner_life": int(g["winner_life"]),
            "loser": g["loser"],
            "loser_life": int(g["loser_life"]),
        },
    )


def b_no_response(g: Groups):
    return [], [], {"actor": g["actor"]}


# (action, regex, builder). Order matters: more specific before general,
# same discipline as replay_parser.py's own PATTERNS table.
_SUBJ = r"(?:(?P<subj>\w[\w' .\-]*?) )?"

PATTERNS: list[tuple[str, "re.Pattern[str]", Builder]] = [
    ("setup", re.compile(r"^Shuffled 50 cards\. Drew 5\. Placed 2 Souls\. Main Phase\.$"), b_none),
    ("mulligan", re.compile(r"^Mulliganed\. Redrew 5\.$"), b_none),
    ("turn_opened", re.compile(r"^Turn opened — Souls (?P<cur>\d+)/(?P<cap>\d+) · Hand:\s*(?P<hand>.*)$"), b_turn_opened),
    (
        "no_response",
        re.compile(r"^\(no (?P<actor>.+?) response logged this turn(?: — .*)?\)$"),
        b_no_response,
    ),
    ("end_phase", re.compile(r"^End Phase, damage cleared\.$"), b_none),
    ("reveal_declined", re.compile(r"^Chose not to reveal a Dragon\.$"), b_none),
    ("no_valid_target", re.compile(r"^No valid target\.$"), b_none),
    (
        "reveal",
        re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?) \((?P<element>[\w/]+)\)\. Power \+(?P<amount>\d+) until end of turn\.$"),
        b_reveal_element,
    ),
    ("reveal", re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?) and added it to their hand\.$"), b_reveal_added),
    ("reveal", re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?), added to hand\.$"), b_reveal_added),
    (
        # A reveal-then-free-deploy Chillet chain narrated as one
        # sentence instead of two lines -- unlike the two-line form
        # (where _link_reveal_then_free_deploy connects a separate
        # reveal/deploy pair via caused_by_line), this shape is recorded
        # as a single event: nothing downstream needs it split into two
        # to know the reveal was deployed.
        "reveal_and_deploy",
        re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?) and deployed it\.$"),
        lambda g: ([g["card"]], [], {"deployed": True}),
    ),
    (
        "reveal",
        re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?) \((?P<element>[\w/]+)\)\.$"),
        lambda g: ([g["card"]], [], {"element": g["element"]}),
    ),
    ("reveal", re.compile(rf"^{_SUBJ}[Rr]evealed (?P<card>.+?)\.$"), b_card_only),
    (
        "deploy",
        re.compile(rf"^{_SUBJ}[Dd]eployed (?P<card>.+?) to slot (?P<slot>\d+) for (?P<cost>\d+) Souls?\.$"),
        b_deploy_slot,
    ),
    (
        "deploy",
        re.compile(rf"^{_SUBJ}[Dd]eployed (?P<card>.+?) from the graveyard \((?P<state>rested|standing)\)\.$"),
        lambda g: ([g["card"]], [], {"from_zone": "graveyard", "state": g["state"]}),
    ),
    ("deploy", re.compile(rf"^{_SUBJ}[Dd]eployed (?P<card>.+?)\.$"), b_card_only),
    (
        "attack_declare",
        re.compile(r"^(?P<subj>.+?)'s (?P<card>.+?) attacks (?P<target>.+?) for (?P<dmg>\d+)\.$"),
        b_attack_declare_player,
    ),
    (
        "attack_declare",
        re.compile(r"^(?P<subj>.+?)'s (?P<card>.+?) attacks (?P<target>.+?)'s (?P<target2>.+?)\.$"),
        lambda g: ([g["card"]], [g["target2"]], None),
    ),
    (
        "attack",
        re.compile(
            r"^(?P<card>.+?) (?:hit|attacked) (?P<target>.+?) for (?P<dmg>\d+)\. "
            r"Damage Check: (?P<milled>\d+) cards? milled, no Lucky\. "
            r"(?P<life_target>.+?) (?P<life_before>\d+) to (?P<life_after>\d+) life\.$"
        ),
        b_attack_them,
    ),
    (
        "attack",
        re.compile(
            r"^(?P<card>.+?) attacked (?P<target>.+?) for (?P<dmg>\d+); "
            r".+? Damage Check revealed a Lucky Pal, cancelled, no damage\. \((?P<milled>\d+) cards? milled\)$"
        ),
        b_attack_lucky,
    ),
    (
        "battle_attack",
        re.compile(r"^(?P<attacker>.+?) \((?P<atk_power>[^)]+)\) (?:attacked|struck) (?P<defender>.+?) \((?P<def_power>[^)]+)\)\.$"),
        b_battle_attack,
    ),
    ("destroyed", re.compile(r"^(?P<card>.+?) destroyed, (?P<dmg>\d+) damage vs (?P<power>-?\d+) Power(?P<both_sides> \(both sides\))?\.$"), b_destroyed_vs),
    ("destroyed", re.compile(r"^(?P<card>.+?) took (?P<dmg>\d+) damage and was destroyed\.$"), lambda g: ([g["card"]], [], {"damage": int(g["dmg"])})),
    ("destroyed", re.compile(r"^(?P<card>.+?) was destroyed\.$"), b_card_only),
    ("attack_interrupted", re.compile(r"^(?P<card>.+?)'s attack was interrupted\.$"), b_card_only),
    (
        "interrupt",
        re.compile(r"^(?P<subj>.+?) interrupted with (?P<card>.+?), paid (?P<cost>\d+) Souls?\. The attack was nullified\.$"),
        b_interrupt_soul,
    ),
    (
        "interrupt",
        re.compile(r"^(?P<subj>.+?) interrupted with (?P<card>.+?), discarded (?P<discard>.+?)\. The attack was nullified\.$"),
        b_interrupt_discard,
    ),
    ("rested", re.compile(r"^(?P<card>.+?) was Rested\.$"), b_card_only),
    ("status_effect", re.compile(r"^(?P<card>.+?) won't stand during its next Stand Phase\.$"), b_status_effect_next_stand),
    ("status_effect", re.compile(r"^(?P<card>.+?) won't stand while the source stays in play\.$"), b_status_effect_while_source),
    ("rest_draw", re.compile(r"^Rested (?P<souls>\d+) Souls?, drew (?P<amount>\d+) cards?\.$"), b_rest_draw),
    ("draw", re.compile(r"^Drew for the turn\.$"), b_drew_for_turn),
    ("draw", re.compile(r"^Drew (?P<amount>\d+) \(one per structures?\)\.$"), lambda g: ([], [], {"amount": int(g["amount"]), "note": "one per structure"})),
    ("draw", re.compile(r"^Drew (?P<amount>\d+) cards?\.$"), b_drew_amount),
    ("draw", re.compile(r"^Drew (?P<amount>\d+)\.$"), b_drew_amount),
    (
        "play_card",
        re.compile(rf"^{_SUBJ}[Pp]layed (?P<card>.+?) for (?P<cost>\d+) Souls?\.$"),
        b_play_with_cost,
    ),
    ("play_card", re.compile(rf"^{_SUBJ}[Pp]layed (?P<card>.+?)\.$"), b_card_only),
    (
        "power_buff",
        re.compile(r"^(?P<card>.+?) gets [Pp]ower [+-](?P<amount>\d+) and the (?P<skill>.+?) skill until end of turn\.$"),
        lambda g: ([g["card"]], [], {"amount": _signed(g), "granted_skill": g["skill"]}),
    ),
    (
        "power_buff",
        re.compile(r"^(?P<card>.+?) gets [Pp]ower (?P<sign>[+-])(?P<amount>\d+) until end of turn\.$"),
        lambda g: ([g["card"]], [], {"amount": int(g["amount"]) * (1 if g["sign"] == "+" else -1)}),
    ),
    (
        # Seen only as a follow-on line after another power_buff on the
        # same card with no subject repeated -- see the module doc
        # comment's general "structure what's there, don't guess"
        # principle: recorded with no card rather than assumed to be
        # whichever card the previous line named.
        "power_buff",
        re.compile(r"^power (?P<sign>[+-])(?P<amount>\d+) until end of turn\.$"),
        lambda g: ([], [], {"amount": int(g["amount"]) * (1 if g["sign"] == "+" else -1)}),
    ),
    (
        "gains_keyword",
        re.compile(r"^(?P<card>.+?) gains (?P<keyword>\w+) until end of turn\.$"),
        lambda g: ([g["card"]], [], {"keyword": g["keyword"]}),
    ),
    (
        "mass_damage",
        re.compile(r"^Dealt (?P<amount>\d+) to every (?P<scope>.+?) Pal, then sacrificed this card\.$"),
        b_mass_damage,
    ),
    (
        "mass_damage",
        re.compile(r"^Dealt (?P<amount>\d+) to every (?P<scope>.+?) Pal\.$"),
        lambda g: ([], [], {"amount": int(g["amount"]), "scope": g["scope"], "self_sacrifice": False}),
    ),
    ("must_discard", re.compile(r"^(?P<subj>.+?) must discard (?P<amount>\d+)\.$"), b_must_discard),
    (
        "discard",
        re.compile(rf"^{_SUBJ}discarded (?P<cards>.+?) to pay a cost\.$"),
        b_discard_multi,
    ),
    ("discard", re.compile(rf"^{_SUBJ}[Dd]iscarded (?P<card>.+?)\.$"), b_discard_one),
    (
        "return_to_hand",
        re.compile(r"^(?P<subj>.+?) returned (?P<card>.+?) from (?:their|its) graveyard to hand\.$"),
        b_card_only,
    ),
    ("return_to_hand", re.compile(r"^Returned (?P<card>.+?) from graveyard to hand\.$"), b_card_only),
    ("put_on_top", re.compile(r"^Put (?P<card>.+?) on top of the deck\.$"), b_card_only),
    (
        "activate",
        re.compile(rf"^{_SUBJ}[Aa]ctivated (?P<card>.+?)\.$"),
        b_card_only,
    ),
    (
        "look_and_take",
        re.compile(r"^(?P<subj>.+?) looked at (?P<n>\d+) cards? and took (?P<amount>\d+) into (?:their|its) hand\.$"),
        lambda g: ([], [], {"looked": int(g["n"]), "took": int(g["amount"])}),
    ),
    (
        "look_and_take",
        re.compile(r"^Looked at top (?P<n>\d+): took (?P<card>.+?) to hand\. Graveyard: (?P<graveyard>.+?)\.$"),
        lambda g: ([g["card"]], [], {"looked": int(g["n"]), "graveyard": _split_hand(g["graveyard"])}),
    ),
    ("look_and_take", re.compile(r"^Looked at top (?P<n>\d+): took (?P<card>.+?) to hand\.$"), lambda g: ([g["card"]], [], {"looked": int(g["n"])})),
    (
        "look_and_take",
        re.compile(r"^Looked at top (?P<n>\d+): deployed (?P<card>.+?)\.$"),
        lambda g: ([g["card"]], [], {"looked": int(g["n"]), "outcome": "deployed"}),
    ),
    ("gain_resource", re.compile(r"^Got (?P<amount>\d+) ?(?P<resource>.*?)\.$"), lambda g: ([], [], {"amount": int(g["amount"]), "resource": g["resource"] or None}),),
    (
        "mass_destroyed",
        re.compile(r"^All Pals with Power (?P<threshold>\d+) or less destroyed\.$"),
        lambda g: ([], [], {"power_threshold": int(g["threshold"])}),
    ),
    ("stand_self", re.compile(r"^Stood this card\.$"), b_none),
    ("stood", re.compile(r"^(?P<card>.+?) was Stood\.$"), b_card_only),
    (
        "assign",
        re.compile(r"^Assigned (?P<card>.+?) — its On Assign ability triggers\.$"),
        lambda g: ([g["card"]], [], {"triggers_on_assign": True}),
    ),
    ("assign", re.compile(r"^Assigned (?P<card>.+?)\.$"), lambda g: ([g["card"]], [], {"triggers_on_assign": False})),
    ("cost_reduction", re.compile(r"^Next Gear costs (?P<amount>\d+) less this turn\.$"), lambda g: ([], [], {"amount": int(g["amount"])})),
    (
        "spend_resource",
        re.compile(r"^Spent (?P<amount>\d+) ?(?P<resource>[^(]*?)\.?\s*(?:\((?P<left>\d+) left\))?\.$"),
        lambda g: ([], [], {"amount": int(g["amount"]), "resource": g["resource"].strip() or None, "left": int(g["left"]) if g.get("left") else None}),
    ),
    ("butcher", re.compile(r"^Butchered (?P<card>.+?)\.$"), b_card_only),
    ("damage_taken", re.compile(r"^(?P<card>.+?) took (?P<dmg>\d+) damage\.$"), lambda g: ([g["card"]], [], {"damage": int(g["dmg"])})),
    ("return_to_hand", re.compile(r"^(?P<card>.+?) returned to hand\.$"), b_card_only),
    ("reveal_declined", re.compile(r"^No Dragon Pal to reveal\.$"), b_none),
    ("cancelled", re.compile(r"^Cancelled\. The cost was refunded\.$"), b_none),
    ("declined_optional", re.compile(r"^Declined the optional effect\.$"), b_none),
    ("block", re.compile(rf"^{_SUBJ}blocked with (?P<card>.+?)\.$"), b_card_only),
    (
        "rest_for_cost",
        re.compile(r"^Rested (?P<souls>\d+) Souls? to pay a cost \((?P<standing>\d+) standing\)\.$"),
        lambda g: ([], [], {"souls_spent": int(g["souls"]), "standing": int(g["standing"])}),
    ),
    (
        "choose_to_stand",
        re.compile(r"^Choose up to (?P<amount>\d+) rested Pals? to stand\.$"),
        lambda g: ([], [], {"amount": int(g["amount"])}),
    ),
    ("night_begins", re.compile(r"^It becomes night\.(?: \U0001f319)?$"), b_none),
    ("soul_area_change", re.compile(r"^Soul Area (?P<sign>[+-])(?P<amount>\d+) \((?P<note>.+?)\)\.$"), lambda g: ([], [], {"amount": int(g["amount"]) * (1 if g["sign"] == "+" else -1), "note": g["note"]})),
    (
        "life_gain",
        re.compile(r"^(?P<card>.+?): gained (?P<amount>\d+) life \((?P<total>\d+)\)\.$"),
        lambda g: ([g["card"]], [], {"amount": int(g["amount"]), "total": int(g["total"])}),
    ),
    (
        "no_valid_target",
        re.compile(r"^No valid target — (?P<card>.+?)'s attack fizzled \(nothing to hit\)\.$"),
        lambda g: ([g["card"]], [], None),
    ),
    ("exiled", re.compile(r"^(?P<card>.+?) was exiled\.$"), b_card_only),
    (
        "returned_from_exile",
        re.compile(r"^(?P<card>.+?) returned from exile to the base \((?P<state>rested|standing)\)\.$"),
        lambda g: ([g["card"]], [], {"state": g["state"]}),
    ),
    (
        "retaliate",
        re.compile(r"^(?P<card>.+?) was dragged down by (?P<source>.+?)'s Retaliate\.$"),
        lambda g: ([g["card"]], [], {"source": g["source"]}),
    ),
    (
        "butcher_narration",
        re.compile(r"^(?:(?P<owner>.+?)'s )?(?P<source>\w[\w' ]*?): butchered (?P<victim_owner>.+?)'s (?P<victim>.+?)\.$"),
        lambda g: ([g["source"], g["victim"]], [], {"victim_owner": g["victim_owner"]}),
    ),
    (
        "butcher_narration",
        re.compile(r"^(?:(?P<owner>.+?)'s )?(?P<source>\w[\w' ]*?): (?P<victim_owner>.+?)'s (?P<victim>.+?) was put into the graveyard\.$"),
        lambda g: ([g["source"], g["victim"]], [], {"victim_owner": g["victim_owner"]}),
    ),
    (
        "butcher_narration",
        re.compile(r"^.+? butchered one of their Pals, put \d+ of .+? Pals into the graveyard\.$"),
        b_none,
    ),
    (
        "attack_interrupted",
        re.compile(r"^(?P<card>.+?)'s attack was interrupted, by (?P<interrupter_owner>.+?)'s (?P<interrupter>.+?)\.$"),
        lambda g: ([g["card"]], [], {"interrupter": g["interrupter"], "interrupter_owner": g["interrupter_owner"]}),
    ),
    (
        "attack_interrupted",
        re.compile(r"^(?P<card>.+?)'s attack was interrupted \(by (?P<interrupter_owner>.+?)'s (?P<interrupter>.+?)\)\.$"),
        lambda g: ([g["card"]], [], {"interrupter": g["interrupter"], "interrupter_owner": g["interrupter_owner"]}),
    ),
    (
        "mass_power_buff",
        re.compile(r"^All (?P<owner>.+?)'s (?P<scope>\w+) Pals get power [+-](?P<amount>\d+) until end of turn\.$"),
        lambda g: ([], [], {"owner": g["owner"], "scope": g["scope"], "stat": "power", "amount": int(g["amount"])}),
    ),
    (
        "mass_power_buff",
        re.compile(r"^All (?P<owner>.+?)'s (?P<scope>\w+) Pals get strike [+-](?P<amount>\d+) until end of turn\.$"),
        lambda g: ([], [], {"owner": g["owner"], "scope": g["scope"], "stat": "strike", "amount": int(g["amount"])}),
    ),
    (
        "mass_power_buff",
        re.compile(r"^All (?P<owner>.+?)'s Pals get power [+-](?P<amount>\d+) until end of turn\.$"),
        lambda g: ([], [], {"owner": g["owner"], "scope": None, "stat": "power", "amount": int(g["amount"])}),
    ),
    (
        "power_buff",
        re.compile(r"^strike (?P<sign>[+-])(?P<amount>\d+) until end of turn\.$"),
        lambda g: ([], [], {"stat": "strike", "amount": int(g["amount"]) * (1 if g["sign"] == "+" else -1)}),
    ),
    (
        "breakthrough_pending",
        re.compile(r"^(?P<card>.+?) breaks through for (?P<amount>\d+), pending their Damage Check\.$"),
        lambda g: ([g["card"]], [], {"amount": int(g["amount"])}),
    ),
]


def _signed(g: Groups) -> int:
    # Only reached by the "gets power +/-N and the X skill" pattern,
    # which -- unlike its no-skill sibling -- never observed a "-" sign
    # in this corpus; kept signed anyway for the same reason CLAUDE.md
    # says Power itself is never clamped.
    return int(g["amount"])


_END_PATTERNS: list[tuple[str, "re.Pattern[str]", Builder]] = [
    ("win_declared", re.compile(r"^(?P<winner>.+?) WINS$"), b_win_declared),
    (
        "loss_reason",
        re.compile(r'^"?(?P<loser>.+?)\'s life hit zero\.?"?$'),
        lambda g: ([], [], {"loser": g["loser"], "reason": "life_hit_zero"}),
    ),
    (
        "loss_reason",
        re.compile(r'^"?(?P<loser>.+?) ran out of (?P<reason>time|cards)\.?"?$'),
        lambda g: ([], [], {"loser": g["loser"], "reason": f"ran_out_of_{g['reason']}"}),
    ),
    (
        "loss_reason",
        re.compile(r'^"?(?P<loser>.+?) surrendered\.?"?$'),
        lambda g: ([], [], {"loser": g["loser"], "reason": "surrendered"}),
    ),
    (
        "final_life",
        re.compile(r"^FINAL LIFE: (?P<winner>.+?) \(WINNER\) (?P<winner_life>\d+), (?P<loser>.+?) (?P<loser_life>\d+)\.$"),
        b_final_life,
    ),
]


def classify(text: str) -> tuple[str, list[str], list[str], Optional[dict[str, Any]], Optional[str]]:
    """Returns (action, cards, targets, resolution, subj) -- `subj` is the
    explicit per-line actor override, if the matched pattern captured
    one, else None."""
    for action, regex, builder in PATTERNS + _END_PATTERNS:
        m = regex.fullmatch(text)
        if m:
            g = m.groupdict()
            cards, targets, resolution = builder(g)
            return action, cards, targets, resolution, g.get("subj")
    return "unrecognized_line_format", [], [], None, None


def parse_log(path: str | Path) -> dict[str, Any]:
    """Parses a Format A log (either on-disk shape -- see module doc
    comment) into {"meta": ..., "events": [...]}. Events are in file
    line order, exactly as read; nothing is sorted or grouped by turn
    number."""
    text = Path(path).read_text(encoding="utf-8")
    lines = text.splitlines()

    # Find the header block (first "====...====" divider through its
    # matching close) -- there is always exactly one, at the top.
    header_start = next(i for i, line in enumerate(lines) if _HEADER_DIVIDER_RE.match(line.strip()))
    meta, body_start = _parse_header(lines, header_start)
    aliases = _participant_aliases(meta.get("match_title"))

    body_lines = lines[body_start:]
    events: list[dict[str, Any]] = []
    current_turn: Optional[int] = None
    current_actor: Optional[str] = None
    last_bare_marker_line: Optional[int] = None
    word_wrap_joins = 0
    allow_word_wrap_join = meta.get("provenance") == "reconstructed"

    i = 0
    while i < len(body_lines):
        line_index = body_start + i + 1  # 1-based file line number
        line = body_lines[i].rstrip("\r\n")
        stripped = line.strip()

        if not stripped:
            i += 1
            continue
        if _SUMMARY_DIVIDER_RE.match(stripped):
            break  # everything from here is the PROSE SUMMARY appendix

        m = _TURN_RE.match(stripped)
        if m:
            current_turn = int(m.group(1))
            i += 1
            continue

        header_name = _RESPONSE_SUFFIX_RE.sub("", stripped)
        if header_name.casefold() in aliases:
            current_actor = aliases[header_name.casefold()]
            i += 1
            continue

        action, cards, targets, resolution, subj = classify(_TRAILING_BRACKET_RE.sub("", stripped))
        consumed = 1
        is_bare_marker_shaped = _BARE_MARKER_RE.match(stripped) and "." not in stripped

        if allow_word_wrap_join and action == "unrecognized_line_format" and not is_bare_marker_shaped and i + 1 < len(body_lines):
            # A sentence physically word-wrapped across two source
            # lines -- seen only in the four provenance="reconstructed"
            # files (their raw export wraps at a fixed column width the
            # rest of the corpus doesn't use), so only attempted there:
            # gating on provenance rather than "try it everywhere" keeps
            # this heuristic from ever getting a chance to misfire on
            # the other 60 files, which never wrap. Retried joined with
            # the next line before giving up; a trailing "[...]"
            # annotation that itself spans the wrap (opens on this
            # line, closes on the next) is what the bracket-strip needs
            # the join for. Never attempted for a bare-marker-shaped
            # line -- that would wrongly fuse a legitimate standalone
            # bare-form interrupt/lucky marker into whatever sentence
            # follows it (found the hard way: the first version of this
            # joined "Pyrin Noct" into the next line's attack_interrupted
            # sentence, in pingu_01, a curated file -- which is exactly
            # why joining is now gated to reconstructed files only).
            next_line = body_lines[i + 1].rstrip("\r\n")
            joined = _TRAILING_BRACKET_RE.sub("", f"{stripped} {next_line.strip()}")
            joined_result = classify(joined)
            if joined_result[0] != "unrecognized_line_format":
                action, cards, targets, resolution, subj = joined_result
                line = f"{line} {next_line.strip()}"
                consumed = 2
                word_wrap_joins += 1

        if action == "unrecognized_line_format" and is_bare_marker_shaped:
            events.append(
                {
                    "line_index": line_index,
                    "turn": current_turn,
                    "phase": None,
                    "actor": current_actor,
                    "action": "bare_marker",
                    "cards": [stripped] if stripped not in ("✦",) else [],
                    "targets": [],
                    "resolution": None,
                    "caused_by_line": None,
                    "text": line,
                }
            )
            last_bare_marker_line = line_index
            i += consumed
            continue

        actor = subj.strip() if subj else current_actor
        caused_by_line = None
        if action in ("attack_interrupted", "destroyed", "attack") and last_bare_marker_line == line_index - 1:
            caused_by_line = last_bare_marker_line
        last_bare_marker_line = None

        if (
            action == "power_buff"
            and not cards
            and events
            and events[-1]["action"] == "power_buff"
            and events[-1]["cards"]
            and events[-1]["line_index"] == line_index - 1
        ):
            # The subjectless "power +N until end of turn." continuation
            # -- see b_power_buff's sibling pattern above: recorded with
            # no card because none is named on this line, but adjacency
            # plus "the immediately preceding event was a power_buff
            # naming a card" is exactly as unambiguous as the bare-marker
            # links above, so it gets the same caused_by_line treatment
            # rather than leaving the association to be re-derived
            # downstream.
            caused_by_line = events[-1]["line_index"]

        events.append(
            {
                "line_index": line_index,
                "turn": current_turn,
                "phase": None,
                "actor": actor,
                "action": action,
                "cards": cards,
                "targets": targets,
                "resolution": resolution,
                "caused_by_line": caused_by_line,
                "text": line,
            }
        )
        i += consumed

    _link_reveal_then_free_deploy(events)
    meta["word_wrap_joins"] = word_wrap_joins
    return {"meta": meta, "events": events}


def _link_reveal_then_free_deploy(events: list[dict[str, Any]]) -> None:
    """Same Chillet-chain link as replay_parser.py's own function of this
    name: a bare "Revealed X." (resolution=None) immediately followed by
    a same-named bare "Deployed X." (resolution=None, no slot/cost) --
    matched by adjacency + identical card name, not card identity."""
    for prev, cur in zip(events, events[1:]):
        if (
            cur["action"] == "deploy"
            and cur["resolution"] is None
            and prev["action"] == "reveal"
            and prev["resolution"] is None
            and cur["cards"] == prev["cards"]
            and cur["cards"]
        ):
            cur["caused_by_line"] = prev["line_index"]
