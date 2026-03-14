#!/usr/bin/env python3
"""End-to-end test: two players complete a full 8-round game via WebSocket."""

import asyncio
import json
import time
import websockets

ROOM = f"e2e_room_{int(time.time())}"
URL = f"ws://localhost:8765/game/{ROOM}"


async def drain(ws, timeout=0.5):
    """Drain all pending messages from ws, returning the last one (or None)."""
    last = None
    while True:
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=timeout)
            msg = json.loads(raw)
            if "Err" not in msg:
                last = msg
        except (asyncio.TimeoutError, websockets.exceptions.ConnectionClosed):
            break
    return last


async def send_action(ws, action):
    """Send action, return the direct response."""
    await ws.send(json.dumps(action))
    raw = await asyncio.wait_for(ws.recv(), timeout=5.0)
    return json.loads(raw)


async def sync_states(alice_ws, bob_ws, alice_state, bob_state):
    """Drain any pending broadcasts and update states."""
    a = await drain(alice_ws)
    b = await drain(bob_ws)
    return (a or alice_state), (b or bob_state)


async def main():
    alice_ws = await websockets.connect(f"{URL}?player=Alice")
    bob_ws = await websockets.connect(f"{URL}?player=Bob")

    # Initial state on connect
    alice_state = json.loads(await alice_ws.recv())
    bob_state = json.loads(await bob_ws.recv())
    print(f"Connected. Waiting for players.")

    # Start game
    alice_state = await send_action(alice_ws, {"action": "StartGame"})
    assert "Err" not in alice_state, f"StartGame failed: {alice_state}"
    alice_state, bob_state = await sync_states(alice_ws, bob_ws, alice_state, bob_state)
    # Bob gets the broadcast; alice already has hers from send_action
    bob_broadcast = await drain(bob_ws)
    if bob_broadcast:
        bob_state = bob_broadcast

    print(f"Game started. Round {alice_state['round']}, Phase: {alice_state['phase']}")
    assert alice_state["phase"] == "choosing_cards"

    for game_round in range(1, 9):
        print(f"\n=== Round {game_round} ===")

        # Both play a card — sync after each
        print(f"  Alice hand size: {len(alice_state['my_hand'])}, Bob hand size: {len(bob_state['my_hand'])}")
        alice_state = await send_action(alice_ws, {"action": "PlayCard", "card_index": 0})
        if "Err" in alice_state:
            raise RuntimeError(f"Alice PlayCard failed: {alice_state}, hand={len(alice_state.get('my_hand', []))}")
        bob_state = (await drain(bob_ws)) or bob_state  # Bob gets broadcast

        bob_state = await send_action(bob_ws, {"action": "PlayCard", "card_index": 0})
        if "Err" in bob_state:
            raise RuntimeError(f"Bob PlayCard failed: {bob_state}")
        alice_state = (await drain(alice_ws)) or alice_state  # Alice gets broadcast

        print(f"  Phase after reveal: alice={alice_state['phase']}, bob={bob_state['phase']}")

        if game_round == 8:
            # Round 8: may need sanctuary choices but no drafting.
            # Handle any sanctuary choices that appear during round 8 drafting.
            for _ in range(10):  # safety limit
                if alice_state["phase"] != "drafting":
                    break
                if alice_state.get("sanctuary_choices"):
                    print(f"  Alice chooses sanctuary (round 8)")
                    alice_state = await send_action(alice_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    bob_state = (await drain(bob_ws)) or bob_state
                elif bob_state.get("sanctuary_choices"):
                    print(f"  Bob chooses sanctuary (round 8)")
                    bob_state = await send_action(bob_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    alice_state = (await drain(alice_ws)) or alice_state
                else:
                    break
            print(f"  Round 8 complete. Phase: {alice_state['phase']}")
            break

        # Drafting (rounds 1-7)
        assert alice_state["phase"] == "drafting", f"Expected drafting, got alice={alice_state['phase']}, bob={bob_state['phase']}"
        draft_order = alice_state["draft_order"]
        print(f"  Drafting order: {draft_order}")

        for i, seat in enumerate(draft_order):
            if seat == 0:  # Alice
                # Choose sanctuary early if available (before drafting)
                if alice_state.get("sanctuary_choices"):
                    print(f"  Alice chooses sanctuary early (from {len(alice_state['sanctuary_choices'])} options)")
                    alice_state = await send_action(alice_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    bob_state = (await drain(bob_ws)) or bob_state

                alice_state = await send_action(alice_ws, {"action": "DraftCard", "market_index": 0})
                assert "Err" not in alice_state
                bob_state = (await drain(bob_ws)) or bob_state

                # If sanctuary pending after draft, must choose now
                if alice_state.get("sanctuary_choices"):
                    print(f"  Alice chooses sanctuary after draft (from {len(alice_state['sanctuary_choices'])} options)")
                    alice_state = await send_action(alice_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    bob_state = (await drain(bob_ws)) or bob_state
            else:  # Bob
                if bob_state.get("sanctuary_choices"):
                    print(f"  Bob chooses sanctuary early (from {len(bob_state['sanctuary_choices'])} options)")
                    bob_state = await send_action(bob_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    alice_state = (await drain(alice_ws)) or alice_state

                bob_state = await send_action(bob_ws, {"action": "DraftCard", "market_index": 0})
                assert "Err" not in bob_state
                alice_state = (await drain(alice_ws)) or alice_state

                if bob_state.get("sanctuary_choices"):
                    print(f"  Bob chooses sanctuary after draft (from {len(bob_state['sanctuary_choices'])} options)")
                    bob_state = await send_action(bob_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                    alice_state = (await drain(alice_ws)) or alice_state

        # Final sync after drafting
        a = await drain(alice_ws, timeout=1.0)
        b = await drain(bob_ws, timeout=1.0)
        if a: alice_state = a
        if b: bob_state = b

        print(f"  After draft: alice={alice_state['phase']} round={alice_state['round']}, hand={len(alice_state['my_hand'])}")
        assert alice_state["phase"] == "choosing_cards", f"Expected choosing_cards, got {alice_state['phase']}"
        assert alice_state["round"] == game_round + 1
        assert len(alice_state["my_hand"]) == 3

    # Ensure game over
    if alice_state["phase"] != "game_over":
        a = await drain(alice_ws, timeout=2.0)
        if a: alice_state = a

    print(f"\n=== Game Over ===")
    assert alice_state["phase"] == "game_over", f"Expected game_over, got {alice_state['phase']}"

    scores = alice_state["scores"]
    assert scores is not None and len(scores) == 2
    print(f"Scores: {json.dumps(scores, indent=2)}")

    for p in alice_state["players"]:
        n_tableau = len(p["tableau"])
        n_sanct = len(p["sanctuaries"])
        print(f"  {p['name']}: {n_tableau} tableau cards, {n_sanct} sanctuaries")
        assert n_tableau == 8, f"{p['name']} has {n_tableau} tableau cards, expected 8"

    print("\n✓ Full 8-round game completed end-to-end successfully!")
    await alice_ws.close()
    await bob_ws.close()


if __name__ == "__main__":
    asyncio.run(main())
