"""Client replay log -> structured move sequence.

Reads a palworldtcg.gg client log (see data/replays/README.md) and emits
one JSON object per non-blank, non-comment line:

    line_index  1-based line number in the source file (for cross-reference)
    turn        the bracketed turn number, taken VERBATIM (see note below)
    phase       main | stand | draw | soul | end | waiting
    actor       "player" | "opponent"
    action      a short action-type tag (see PATTERNS below)
    cards       card/structure names the action's source (attacker,
                deployed card, etc.)
    targets     names of whatever the action targets (a player name,
                "opponent", or a card name)
    resolution  dict of numeric/boolean detail (damage, milled, Lucky,
                life totals, souls, slot, cost, ...) or null
    caused_by_line  line_index of the event that unambiguously caused this
                one, or null. Currently populated only for the
                reveal-then-free-deploy chain (README gotcha 4); see
                `_link_reveal_then_free_deploy`. Events are kept flat and
                faithful to the log rather than fused -- this field is a
                hook for the engine test, not a general causation graph.
    text        the message text, unmodified, for hand verification

Six parser gotchas (data/replays/README.md) this module is built around:

1. Turn numbers are NOT monotonic. A [Tn waiting] line describes the
   opponent's turn and can carry a lower turn number than the lines
   around it (see README example; battle 3 T7 is the sharp case).
   `parse_log` NEVER sorts or groups by the bracketed turn number --
   events are emitted strictly in file line order. Consumers must do
   the same.
2. `main`/`stand`/`draw`/`soul`/`end` tag the logging player's (Edify's)
   own turn; `waiting` tags the opponent's turn. Actor is derived from
   this tag, with one text-grounded exception: a `waiting` line whose
   message starts with "Edify" (an interrupt played during the
   opponent's turn) is attributed to the player, because the text says
   so directly -- this is not an inference about card behaviour.
3. The damage-check sentence carries the Lucky outcome inline; both the
   "no Lucky" and "cancelled" phrasings are parsed into the same
   `resolution` shape (`lucky`, `damage`, `milled`, ...).
4. A Chillet-style reveal is just "Revealed <card>." followed by either
   "Deployed <card>." (free deploy) or the card text continuing "...,
   added to hand." (declined / not a Dragon) -- these are ordinary,
   separately-emitted events; the parser does not fuse them.
5. Slot numbers are base-wide (structures and gears included), so slot
   6 on a 5-Pal-limit board is expected, not a bug.
6. "Rested 3 Souls, drew 1 card." (rule 8.5) is its own action
   (`rest_draw`), distinct from the plain per-turn `draw`.

This module does NOT interpret card behaviour or rules -- it only
structures what the log literally says.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable, Optional

LINE_RE = re.compile(r"^\[T(?P<turn>\d+)\s+(?P<phase>\w+)\]\s+(?P<text>.*)$")
PLAYER_PHASES = {"main", "stand", "draw", "soul", "end"}

Groups = dict[str, str]
Builder = Callable[[Groups], tuple[list[str], list[str], Optional[dict[str, Any]]]]


def _power_value(expr: str) -> int:
    expr = expr.strip()
    if "=" in expr:
        return int(expr.rsplit("=", 1)[1].strip())
    return int(expr)


# --- per-pattern builders -------------------------------------------------
# Each returns (cards, targets, resolution) from the regex's groupdict().

def b_none(g: Groups):
    return [], [], None


def b_stand_phase(g: Groups):
    return [], [], {"declared_turn": int(g["turn_num"])}


def b_drew_for_turn(g: Groups):
    return [], [], {"amount": 1}


def b_place_souls(g: Groups):
    return [], [], {"amount": int(g["amount"]), "total": int(g["total"]), "cap": int(g["cap"])}


def b_main_phase(g: Groups):
    return [], [], {"player": g["player"]}


def b_deploy_slot(g: Groups):
    return [g["card"]], [], {"slot": int(g["slot"]), "cost": int(g["cost"])}


def b_card_only(g: Groups):
    return [g["card"]], [], None


def b_reveal_element(g: Groups):
    return [g["card"]], [], {"element": g["element"], "power_buff": int(g["amount"])}


def b_reveal_added(g: Groups):
    return [g["card"]], [], {"outcome": "added_to_hand"}


def b_attack_them(g: Groups):
    return (
        [g["card"]],
        ["opponent"],
        {
            "damage": int(g["dmg"]),
            "milled": int(g["milled"]),
            "lucky": False,
            "life_before": int(g["life_before"]),
            "life_after": int(g["life_after"]),
        },
    )


def b_attack_them_lucky(g: Groups):
    return (
        [g["card"]],
        ["opponent"],
        {"damage_attempted": int(g["dmg"]), "damage": 0, "milled": int(g["milled"]), "lucky": True},
    )


def b_drew_amount(g: Groups):
    return [], [], {"amount": int(g["amount"])}


def b_drew_structures(g: Groups):
    return [], [], {"amount": int(g["amount"]), "note": "one per structure"}


def b_opp_power_buff(g: Groups):
    return [g["card"]], [], {"amount": int(g["amount"])}


def b_opp_attack_player(g: Groups):
    return [g["card"]], ["Edify"], {"damage": int(g["dmg"])}


def b_opp_attack_card(g: Groups):
    return [g["card"]], [g["target"]], None


def b_interrupt_soul(g: Groups):
    return [g["card"]], [], {"cost": {"type": "soul", "amount": int(g["cost"])}, "result": "nullified"}


def b_interrupt_discard(g: Groups):
    return [g["card"]], [], {"cost": {"type": "discard", "card": g["discard"]}, "result": "nullified"}


def b_destroyed_damage(g: Groups):
    return [g["card"]], [], {"damage": int(g["dmg"])}


def b_battle_attack(g: Groups):
    return (
        [g["attacker"]],
        [g["defender"]],
        {
            "attacker_power_expr": g["atk_power"].strip(),
            "attacker_power": _power_value(g["atk_power"]),
            "defender_power": int(g["def_power"]),
        },
    )


def b_destroyed_vs(g: Groups):
    return [g["card"]], [], {"damage": int(g["dmg"]), "power": int(g["power"])}


def b_hit_edify(g: Groups):
    return (
        [g["card"]],
        ["Edify"],
        {
            "damage": int(g["dmg"]),
            "milled": int(g["milled"]),
            "lucky": False,
            "life_before": int(g["life_before"]),
            "life_after": int(g["life_after"]),
        },
    )


def b_attack_edify_lucky(g: Groups):
    return (
        [g["card"]],
        ["Edify"],
        {"damage_attempted": int(g["dmg"]), "damage": 0, "milled": int(g["milled"]), "lucky": True},
    )


def b_rest_draw(g: Groups):
    return [], [], {"souls_spent": int(g["souls"]), "drew": int(g["amount"])}


def b_play_for_cost(g: Groups):
    return [g["card"]], [], {"cost": int(g["cost"])}


def b_look_take(g: Groups):
    return [g["card"]], [], {"looked": int(g["n"])}


def b_discard_multi(g: Groups):
    return [c.strip() for c in g["cards"].split(",")], [], None


def b_wont_stand(g: Groups):
    return [g["card"]], [], {"effect": "wont_stand_next_stand_phase"}


# --- pattern table ---------------------------------------------------------
# (action, regex, builder). Order matters: more specific patterns are tried
# before generic ones that would otherwise swallow their tail text (e.g. a
# slotted "Deployed X to slot N for M Souls." must be tried before the bare
# "Deployed X." pattern, or the lazy card group absorbs the slot/cost text).

PATTERNS: list[tuple[str, "re.Pattern[str]", Builder]] = [
    ("setup", re.compile(r"^Shuffled 50 cards\. Drew 5\. Placed 2 Souls\. Main Phase\.$"), b_none),
    ("mulligan", re.compile(r"^Mulliganed\. Redrew 5\.$"), b_none),
    ("end_phase", re.compile(r"^End Phase, damage cleared\.$"), b_none),
    ("stand_phase", re.compile(r"^Turn (?P<turn_num>\d+), Stand Phase\.$"), b_stand_phase),
    ("draw_phase", re.compile(r"^Draw Phase\.$"), b_none),
    ("draw", re.compile(r"^Drew for the turn\.$"), b_drew_for_turn),
    ("place_souls", re.compile(r"^Placed (?P<amount>\d+) Souls? \((?P<total>\d+)/(?P<cap>\d+)\)\.$"), b_place_souls),
    ("main_phase", re.compile(r"^Main Phase, (?P<player>\w+)'s move\.$"), b_main_phase),
    ("deploy", re.compile(r"^Deployed (?P<card>.+?) to slot (?P<slot>\d+) for (?P<cost>\d+) Souls?\.$"), b_deploy_slot),
    ("deploy", re.compile(r"^Deployed (?P<card>.+?)\.$"), b_card_only),
    ("reveal", re.compile(r"^Revealed (?P<card>.+?) \((?P<element>[\w/]+)\)\. Power \+(?P<amount>\d+) until end of turn\.$"), b_reveal_element),
    ("reveal", re.compile(r"^Revealed (?P<card>.+?), added to hand\.$"), b_reveal_added),
    ("reveal", re.compile(r"^Revealed (?P<card>.+?)\.$"), b_card_only),
    ("reveal_declined", re.compile(r"^Chose not to reveal a Dragon\.$"), b_none),
    ("attack", re.compile(
        r"^(?P<card>.+?) attacked them for (?P<dmg>\d+)\. Damage Check: (?P<milled>\d+) cards? milled, "
        r"no Lucky\. They (?P<life_before>\d+) to (?P<life_after>\d+) life\.$"
    ), b_attack_them),
    ("attack", re.compile(
        r"^(?P<card>.+?) attacked them for (?P<dmg>\d+); their Damage Check revealed a Lucky Pal, "
        r"cancelled, no damage\. \((?P<milled>\d+) cards? milled\)$"
    ), b_attack_them_lucky),
    ("no_valid_target", re.compile(r"^No valid target\.$"), b_none),
    ("draw", re.compile(r"^Drew (?P<amount>\d+) \(one per structures?\)\.$"), b_drew_structures),
    ("draw", re.compile(r"^Drew (?P<amount>\d+) cards?\.$"), b_drew_amount),
    ("draw", re.compile(r"^Drew (?P<amount>\d+)\.$"), b_drew_amount),
    ("attack_interrupted", re.compile(r"^(?P<card>.+?)'s attack was interrupted\.$"), b_card_only),
    ("deploy", re.compile(r"^FluffysFury deployed (?P<card>.+?)\.$"), b_card_only),
    ("play_card", re.compile(r"^FluffysFury played (?P<card>.+?)\.$"), b_card_only),
    ("activate", re.compile(r"^FluffysFury activated (?P<card>.+?)\.$"), b_card_only),
    ("power_buff", re.compile(r"^FluffysFury's (?P<card>.+?) gets power \+(?P<amount>\d+) until end of turn\.$"), b_opp_power_buff),
    ("attack_declare", re.compile(r"^FluffysFury's (?P<card>.+?) attacks Edify for (?P<dmg>\d+)\.$"), b_opp_attack_player),
    ("attack_declare", re.compile(r"^FluffysFury's (?P<card>.+?) attacks Edify's (?P<target>.+?)\.$"), b_opp_attack_card),
    ("interrupt", re.compile(r"^Edify interrupted with (?P<card>.+?), paid (?P<cost>\d+) Souls?\. The attack was nullified\.$"), b_interrupt_soul),
    ("interrupt", re.compile(r"^Edify interrupted with (?P<card>.+?), discarded (?P<discard>.+?)\. The attack was nullified\.$"), b_interrupt_discard),
    ("destroyed", re.compile(r"^(?P<card>.+?) took (?P<dmg>\d+) damage and was destroyed\.$"), b_destroyed_damage),
    ("battle_attack", re.compile(r"^(?P<attacker>.+?) \((?P<atk_power>[^)]+)\) attacked (?P<defender>.+?) \((?P<def_power>\d+)\)\.$"), b_battle_attack),
    ("destroyed", re.compile(r"^(?P<card>.+?) destroyed, (?P<dmg>\d+) damage vs (?P<power>\d+) Power\.$"), b_destroyed_vs),
    ("destroyed", re.compile(r"^(?P<card>.+?) was destroyed\.$"), b_card_only),
    ("block", re.compile(r"^They blocked with (?P<card>.+?)\.$"), b_card_only),
    ("attack", re.compile(
        r"^(?P<card>.+?) hit Edify for (?P<dmg>\d+)\. Damage Check: (?P<milled>\d+) cards? milled, "
        r"no Lucky\. Edify (?P<life_before>\d+) to (?P<life_after>\d+) life\.$"
    ), b_hit_edify),
    ("attack", re.compile(
        r"^(?P<card>.+?) attacked Edify for (?P<dmg>\d+); Edify's Damage Check revealed a Lucky Pal, "
        r"cancelled, no damage\. \((?P<milled>\d+) cards? milled\)$"
    ), b_attack_edify_lucky),
    ("rest_draw", re.compile(r"^Rested (?P<souls>\d+) Souls?, drew (?P<amount>\d+) cards?\.$"), b_rest_draw),
    ("play_card", re.compile(r"^Played (?P<card>.+?) for (?P<cost>\d+) Souls?\.$"), b_play_for_cost),
    ("look_and_take", re.compile(r"^Looked at top (?P<n>\d+): took (?P<card>.+?) to hand\.$"), b_look_take),
    ("put_on_top", re.compile(r"^Put (?P<card>.+?) on top of the deck\.$"), b_card_only),
    ("discard", re.compile(r"^FluffysFury discarded (?P<cards>.+?) to pay a cost\.$"), b_discard_multi),
    ("discard", re.compile(r"^Discarded (?P<card>.+?)\.$"), b_card_only),
    ("return_to_hand", re.compile(r"^FluffysFury returned (?P<card>.+?) from their graveyard to hand\.$"), b_card_only),
    ("reveal", re.compile(r"^FluffysFury revealed (?P<card>.+?) and added it to their hand\.$"), b_reveal_added),
    ("status_effect", re.compile(r"^(?P<card>.+?) won't stand during its next Stand Phase\.$"), b_wont_stand),
    ("rested", re.compile(r"^(?P<card>.+?) was Rested\.$"), b_card_only),
]

_PLAYER_ACTOR_RE = re.compile(r"^Edify\b")


def determine_actor(phase: str, text: str) -> str:
    """Actor per README note 2, with the one text-grounded exception
    (an opponent-turn line whose subject is named "Edify" -- e.g. an
    interrupt) noted in the module docstring."""
    if phase in PLAYER_PHASES:
        return "player"
    if _PLAYER_ACTOR_RE.match(text):
        return "player"
    return "opponent"


def classify(text: str) -> tuple[str, list[str], list[str], Optional[dict[str, Any]]]:
    for action, regex, builder in PATTERNS:
        m = regex.fullmatch(text)
        if m:
            cards, targets, resolution = builder(m.groupdict())
            return action, cards, targets, resolution
    return "unparsed", [], [], None


_META_LINE_RE = re.compile(r"^#\s*(?P<key>\w+):\s*(?P<value>.*)$")
_META_PLAYER_RE = re.compile(
    r"^(?P<player>.+?)\s+opponent:\s*(?P<opponent>.+?)\s+opponent_colors:\s*(?P<opponent_colors>.+)$"
)


def _parse_meta_line(meta: dict[str, Any], line: str, last_key: Optional[str]) -> Optional[str]:
    """Parse one '#'-prefixed header line. A line that isn't itself a
    'key: value' pair is treated as a wrapped continuation of the previous
    key's value (the header notes wrap across lines in battle3) and joined
    with a space; returns the key to track for the next line's continuation
    check."""
    m = _META_LINE_RE.match(line)
    if not m:
        text = line.lstrip("#").strip()
        if last_key == "note" and meta.get("notes"):
            meta["notes"][-1] = f"{meta['notes'][-1]} {text}".strip()
        elif last_key and isinstance(meta.get(last_key), str):
            meta[last_key] = f"{meta[last_key]} {text}".strip()
        else:
            meta.setdefault("unparsed_header_lines", []).append(line)
        return last_key

    key, value = m.group("key"), m.group("value").strip()
    if key == "note":
        meta.setdefault("notes", []).append(value)
        return "note"
    if key == "player":
        pm = _META_PLAYER_RE.match(value)
        if pm:
            meta["player"] = pm.group("player").strip()
            meta["opponent"] = pm.group("opponent").strip()
            meta["opponent_colors"] = pm.group("opponent_colors").strip()
            return "player"
    meta[key] = value
    return key


def parse_log(path: str | Path) -> dict[str, Any]:
    """Parse a client replay log into {"meta": ..., "events": [...]}.

    Events are always in file line order (see gotcha 1 in the module
    docstring) -- never sorted or grouped by the bracketed turn number.
    """
    meta: dict[str, Any] = {}
    events: list[dict[str, Any]] = []
    last_meta_key: Optional[str] = None

    with open(path, encoding="utf-8") as f:
        for line_index, raw_line in enumerate(f, start=1):
            line = raw_line.rstrip("\r\n")
            if not line.strip():
                continue
            if line.startswith("#"):
                last_meta_key = _parse_meta_line(meta, line, last_meta_key)
                continue

            m = LINE_RE.match(line)
            if not m:
                events.append(
                    {
                        "line_index": line_index,
                        "turn": None,
                        "phase": None,
                        "actor": None,
                        "action": "unrecognized_line_format",
                        "cards": [],
                        "targets": [],
                        "resolution": None,
                        "caused_by_line": None,
                        "text": line,
                    }
                )
                continue

            turn = int(m.group("turn"))
            phase = m.group("phase")
            text = m.group("text").strip()
            actor = determine_actor(phase, text)
            action, cards, targets, resolution = classify(text)

            events.append(
                {
                    "line_index": line_index,
                    "turn": turn,
                    "phase": phase,
                    "actor": actor,
                    "action": action,
                    "cards": cards,
                    "targets": targets,
                    "resolution": resolution,
                    "caused_by_line": None,
                    "text": text,
                }
            )

    _link_reveal_then_free_deploy(events)
    return {"meta": meta, "events": events}


def _link_reveal_then_free_deploy(events: list[dict[str, Any]]) -> None:
    """Populate `caused_by_line` for the one causal chain the log makes
    unambiguous without any card-specific knowledge: a bare "Revealed X."
    (action=reveal, resolution=None -- i.e. not the "(Element)... power
    buff" or ", added to hand" forms) immediately followed by a same-named
    free deploy ("Deployed X.", action=deploy, resolution=None -- no slot/
    cost, so not an ordinary paid deployment). This is exactly the
    reveal-then-free-deploy shape from README gotcha 4 (the Chillet
    pattern), matched generically by adjacency + identical card name, not
    by card identity -- it says nothing about *why* a card triggers a
    reveal, only that this specific pair of consecutive lines names the
    same card. It intentionally does NOT try to trace further (e.g. the
    freely-deployed Pal's own On Deploy lines that follow), since that
    would require card-behaviour knowledge this parser does not have.
    """
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


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Parse a client replay log into a structured move sequence (JSON).")
    parser.add_argument("log_path", type=Path)
    parser.add_argument("-o", "--output", type=Path, default=None, help="write JSON here instead of stdout")
    parser.add_argument("--indent", type=int, default=2)
    args = parser.parse_args(argv)

    result = parse_log(args.log_path)
    out = json.dumps(result, indent=args.indent, ensure_ascii=False)

    if args.output:
        args.output.write_text(out + "\n", encoding="utf-8")
    else:
        print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
