# Yonder — Complete Rules Reference

> Sources: Official English rulebook (rules-en.pdf), BoardGameGeek forums, Faraway_analysis.xlsx card data

---

## Overview

**Yonder** is a simultaneous card-drafting game for 2–6 players (15–30 min). Players explore the continent of Alula by playing 8 Region cards left-to-right over 8 rounds. The twist: at the end, scoring happens **right-to-left** — the last card you played is scored first (with almost nothing visible), while the first card is scored last (with everything visible).

- **Players:** 2–6
- **Rounds:** 8
- **Components:** 68 Region cards, 45 Sanctuary cards, score pad

---

## Card Anatomy

### Region Cards (68, numbered 1–68)

Each card has a unique **Exploration Duration** (1–68).

| Element | Location | Description |
|---|---|---|
| Exploration Duration | Top-left | Unique number 1–68 |
| Day/Night | Top-left | Sun = day, Moon = night |
| Clue icon | Top-left | Map symbol. Count toward Sanctuary draws |
| Wonder icons | Top-right | 0–2 icons: Uddu Stone (stone), Okiko Chimera (chimera), Goldlog Thistle (thistle) |
| Biome | Border colour | Forest=red, River=green, Desert=blue, City=yellow |
| Quest prerequisites | Bottom | Wonder icons that must be visible to score |
| Fame/scoring | Bottom | Points this card awards (see Scoring Types) |

### Sanctuary Cards (45)

Smaller rectangular cards with two sections:
- **Top:** Persistent bonuses — wonder icons, clues, night symbol, biome association
- **Bottom:** Optional own fame condition (scored after all 8 region cards)

Sanctuary biomes: Forest(red), River(green), Desert(blue), City(yellow), or Colorless (no biome).

---

## Biomes

| Biome | Colour | Count |
|---|---|---|
| Forest | Red | 17 |
| River | Green | 17 |
| Desert | Blue | 17 |
| City | Yellow | 17 |

Note: City cards never have wonder icons; they specialise in colour-pair multiplier scoring.

---

## Setup

1. Shuffle all 68 Region cards.
2. Deal **3 face-down** Region cards to each player. Players look at their own.
3. Reveal **player_count + 1** Region cards face-up to form the **Market**.
4. Shuffle all 45 Sanctuary cards into a face-down Sanctuary deck.

### Advanced Setup Variant (recommended after first game)
1. Deal **5** Region cards to each player instead of 3.
2. Each player secretly selects 3 to keep; removes the other 2 (without showing anyone).
3. Shuffle the removed cards back into the Region deck.
4. Then reveal the market as normal.

---

## Turn Structure (8 Rounds)

Each round has 3 phases:

### Phase 1: Exploring a Region (Simultaneous)

1. All players simultaneously select **1 Region card** from their hand of 3 and place it face-down.
2. All players **reveal simultaneously**.
3. Each player places their revealed card at the **right end** of their personal row (left-to-right).

*Round 1 exception: No player receives Sanctuaries on round 1 (no previous card to compare against).*

### Phase 2: Finding Sanctuaries

A player **qualifies** if the number on the card they just played is **strictly greater** than the number on their previously played card.

If qualified:
- Draw Sanctuary cards = **1 + (total visible Clue icons)** across all your face-up Region cards AND placed Sanctuary cards.
- Choose **1 Sanctuary** to keep face-up in your play area.
- Discard the rest to the **bottom** of the Sanctuary deck.

If not qualified: no Sanctuaries this round.

### Phase 3: Drafting from the Market

Players take turns in **ascending order** of the Exploration Duration number played this round (lowest number → first pick).

- Each player selects **1 Region card** from the market and adds it to their hand.
- After all players have picked, the **1 remaining card is discarded** from the game.
- Refill the market: reveal **player_count + 1** new cards.

*Round 8 exception: Players do NOT draft a new card (game is ending). Sanctuary phase still applies.*

---

## End of Game

After 8 complete rounds, each player has exactly 8 Region cards in their row plus any Sanctuaries.

---

## Scoring (Reverse Order)

### Procedure

1. Flip all 8 Region cards **face-down**. Sanctuary cards stay face-up.
2. Reveal the **rightmost** card (8th played) and score it. Only itself + Sanctuaries are visible.
3. Reveal each card to the left one at a time, scoring each. Each reveal adds one more card to the "visible" pool.
4. The **leftmost** card (1st played) is scored last with all 8 cards visible.
5. After all 8 Region cards, score each **Sanctuary's own fame condition** using the fully revealed tableau.
6. Sum all fame points.

**Tiebreaker:** Tied players compare the **sum of all 8 Exploration Duration numbers**. Lowest sum wins.

### Prerequisites

Prerequisites are wonder icons shown above a card's fame value. At scoring time, check whether the required icons are present across ALL currently visible Region cards + all Sanctuaries.

- Prerequisites **met** → card scores its fame.
- Prerequisites **not met** → card scores **0** (condition entirely ignored).
- A single wonder icon satisfies multiple prerequisites across different cards (icons are not "spent").

### Scoring Types

| Type | Example | How it scores |
|---|---|---|
| **Flat fame** | `8 fame` | Fixed number of points if prerequisites met |
| **Per-icon** | `2 fame per Uddu Stone` | Count visible instances of that wonder icon × score_per |
| **Per-colour** | `4 fame per Blue card` | Count visible cards of that biome × score_per |
| **Per-colour-pair** | `1 fame per Yellow or Green card` | Count visible cards of either biome × score_per |
| **Per-night** | `3 fame per night card` | Count visible night cards × score_per |
| **Per-clue** | `2 fame per clue icon` | Count visible clue icons × score_per |
| **Wonder-set** | `10 fame per wonder set` | floor(min(stone_count, chimera_count, thistle_count)) × score_per |
| **Colour-set** | `10 fame per colour set` | floor(min(red_count, green_count, blue_count, yellow_count)) × score_per |

**What "visible" means:**
- The card being scored itself + all cards to its RIGHT that have already been revealed
- ALL Sanctuary cards (always face-up)

Icons on Sanctuaries count for all purposes (prerequisites, wonder counts, clue counts, night counts, biome counts if they have a biome).

### Sanctuary Scoring

After all 8 Region cards are scored, Sanctuaries with a fame condition in their lower section are scored. At this point the full 8-card tableau is visible.

---

## Strategy Notes

- **Low numbers** = first pick from market, but rarely qualify for Sanctuaries (hard to always go up).
- **High numbers** = last pick from market, but easily qualify for Sanctuaries (easy to keep going up).
- **Clues** snowball: more clues → more Sanctuary cards drawn → better chance of a great Sanctuary.
- **Early cards** score with the most context (ideal for set-scoring like colorSet/wonderSet).
- **Late cards** score with little context (good for high flat-fame cards with heavy prerequisites — they'll score 0 unless you set up icons early).
- **City (yellow)** cards have no wonder icons. They thrive on colour-pair scoring with lots of yellow + one other colour.

---

## Card Distribution Summary

| Category | Count |
|---|---|
| Region cards total | 68 |
| Region cards with clues | 17 |
| Region cards that are night | 21 |
| Sanctuary cards total | 45 |
| Sanctuary cards with clues | ~10 |

---

## People from Below Expansion (not in scope for v1)

Adds a 5th biome: **Havens** (grey/colorless). 8 new Region cards (numbers 0, 69–76) and 8 new Sanctuary cards. Introduces "Guides" and "Three-Eyed Ones" cards. Enables 7-player games.
