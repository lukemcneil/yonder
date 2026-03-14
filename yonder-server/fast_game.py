#!/usr/bin/env python3
"""Play a 2-player game to completion via WebSocket bots.

After running, open a browser to the same room and name to see the scoring UI.

Usage:
  1. Start server:  cd yonder-server && ROCKET_PORT=8001 cargo run
  2. Run this:       python3 yonder-server/fast_game.py
  3. Open browser with room "scoring-test" and name "Alice"
"""

import asyncio
import json
import sys
import time
import websockets

PORT = 8000
ROOM = sys.argv[1] if len(sys.argv) > 1 else f"score-{int(time.time())}"
URL = f"ws://localhost:{PORT}/game/{ROOM}"


async def drain(ws, timeout=0.1):
    last = None
    while True:
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=timeout)
            msg = json.loads(raw)
            if isinstance(msg, dict) and "Err" not in msg and "phase" in msg:
                last = msg
        except (asyncio.TimeoutError, websockets.exceptions.ConnectionClosed):
            break
    return last


async def send_action(ws, action):
    await ws.send(json.dumps(action))
    raw = await asyncio.wait_for(ws.recv(), timeout=5.0)
    msg = json.loads(raw)
    if isinstance(msg, dict) and "Err" in msg:
        raise RuntimeError(f"Action {action} failed: {msg}")
    return msg


async def main():
    print(f"Connecting Alice and Bob to room '{ROOM}' on port {PORT}...")
    alice_ws = await websockets.connect(f"{URL}?player=Alice")
    bob_ws = await websockets.connect(f"{URL}?player=Bob")

    alice_state = json.loads(await alice_ws.recv())
    bob_state = json.loads(await bob_ws.recv())
    # Drain any join broadcasts
    a = await drain(alice_ws, timeout=0.1)
    b = await drain(bob_ws, timeout=0.1)
    if a: alice_state = a
    if b: bob_state = b

    # Start game (Alice is seat 0)
    resp = await send_action(alice_ws, {"action": "StartGame"})
    if isinstance(resp, dict) and "phase" in resp:
        alice_state = resp
    # Drain broadcasts to get latest state for both
    a = await drain(alice_ws, timeout=0.1)
    b = await drain(bob_ws, timeout=0.1)
    if a: alice_state = a
    if b: bob_state = b

    print(f"Game started. Round {alice_state['round']}, phase: {alice_state['phase']}")

    for game_round in range(1, 9):
        # Both play card index 0
        alice_state = await send_action(alice_ws, {"action": "PlayCard", "card_index": 0})
        bob_state = (await drain(bob_ws)) or bob_state

        bob_state = await send_action(bob_ws, {"action": "PlayCard", "card_index": 0})
        alice_state = (await drain(alice_ws)) or alice_state

        # Handle sanctuary choices
        for _ in range(30):
            if alice_state["phase"] != "sanctuary_choice":
                break
            if alice_state.get("sanctuary_choices"):
                alice_state = await send_action(alice_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                bob_state = (await drain(bob_ws)) or bob_state
            elif bob_state.get("sanctuary_choices"):
                bob_state = await send_action(bob_ws, {"action": "ChooseSanctuary", "sanctuary_index": 0})
                alice_state = (await drain(alice_ws)) or alice_state
            else:
                b = await drain(bob_ws, timeout=0.1)
                if b:
                    bob_state = b
                a = await drain(alice_ws, timeout=0.1)
                if a:
                    alice_state = a

        if game_round == 8:
            break

        # Drafting
        if alice_state["phase"] != "drafting":
            print(f"  Warning: expected drafting, got {alice_state['phase']}")
            break

        for seat in alice_state["draft_order"]:
            if seat == 0:
                alice_state = await send_action(alice_ws, {"action": "DraftCard", "market_index": 0})
                bob_state = (await drain(bob_ws)) or bob_state
            else:
                bob_state = await send_action(bob_ws, {"action": "DraftCard", "market_index": 0})
                alice_state = (await drain(alice_ws)) or alice_state

        # Final sync after drafting
        a = await drain(alice_ws, timeout=0.1)
        b = await drain(bob_ws, timeout=0.1)
        if a: alice_state = a
        if b: bob_state = b

        print(f"  Round {game_round} done. Phase: {alice_state['phase']}, hands: A={len(alice_state['my_hand'])}, B={len(bob_state['my_hand'])}")

    # Final sync
    if alice_state["phase"] != "game_over":
        a = await drain(alice_ws, timeout=0.1)
        if a:
            alice_state = a

    assert alice_state["phase"] == "game_over", f"Expected game_over, got {alice_state['phase']}"

    scores = alice_state["scores"]
    detail = alice_state.get("my_score_detail", [])
    print(f"\n=== Game Over ===")
    for s in scores:
        print(f"  {s['name']}: {s['total']} fame (sum: {s['card_number_sum']})")
    print(f"\nAlice score detail ({len(detail)} entries):")
    total = 0
    for e in detail:
        total += e["points"]
        label = f"+{e['points']}" if e["points"] > 0 else "0"
        print(f"  {e['kind']} #{e['number']}: {label} — {e['explanation']}")
    print(f"  Total: {total}")

    print(f"\nNow open browser: room='{ROOM}', name='Alice' (or 'Bob')")
    print(f"Keeping connections open. Ctrl+C to stop.")

    try:
        await asyncio.Future()
    except asyncio.CancelledError:
        pass
    finally:
        await alice_ws.close()
        await bob_ws.close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nDone.")
