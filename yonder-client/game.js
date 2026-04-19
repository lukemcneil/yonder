// ── Global state ────────────────────────────────────────────────────────────

let ws = null;
let state = null;   // latest ClientGameState from server
let mySeat = null;
let scoringRevealIndex = 0;  // 0 = not started, increments on click (N = N cards revealed)
let viewOtherSeat = null;  // whose board to show in main area (null = self)
const expandedOpponents = new Set();  // track which opponent panels are expanded on mobile

let currentRoomCode = '';   // room code for the current connection
let pingInterval = null;    // keepalive timer for game WebSocket
let lobbyWs = null;         // WebSocket for live lobby room list

// ── DOM refs ─────────────────────────────────────────────────────────────────

const lobby          = document.getElementById('lobby');
const playerNameEl   = document.getElementById('player-name');
const createBtn      = document.getElementById('create-btn');
const lobbyStatus    = document.getElementById('lobby-status');
const activeGamesList = document.getElementById('active-games-list');

const gameBoard      = document.getElementById('game-board');
const statusPhase    = document.getElementById('status-phase');
const statusRound    = document.getElementById('status-round');
const statusDeck     = document.getElementById('status-deck');
const statusSanctDeck = document.getElementById('status-sanctuary-deck');
const opponentsArea  = document.getElementById('opponents-area');
const marketCards    = document.getElementById('market-cards');
const myTableau      = document.getElementById('my-tableau');
const mySanctuaries  = document.getElementById('my-sanctuaries');
const myHand         = document.getElementById('my-hand');

const advancedModal      = document.getElementById('advanced-modal');
const advancedChoicesEl  = document.getElementById('advanced-choices');
const advancedConfirmBtn = document.getElementById('advanced-confirm-btn');


// ── Connection ───────────────────────────────────────────────────────────────

function generateCode() {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ'; // no I or O to avoid confusion
  let code = '';
  for (let i = 0; i < 4; i++) code += chars[Math.floor(Math.random() * chars.length)];
  return code;
}

function connect(roomCode) {
  const playerName = playerNameEl.value.trim();
  if (!playerName) {
    lobbyStatus.textContent = 'Enter your name first.';
    return;
  }
  if (!roomCode) {
    lobbyStatus.textContent = 'No room code.';
    return;
  }
  roomCode = roomCode.toUpperCase();
  currentRoomCode = roomCode;

  lobbyStatus.textContent = 'Connecting…';
  setLobbyButtonsDisabled(true);
  // Close any existing game WebSocket (e.g. rematch from a finished game).
  if (pingInterval) { clearInterval(pingInterval); pingInterval = null; }
  if (ws) { ws.close(); ws = null; }
  disconnectLobby();

  const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const params = new URLSearchParams(location.search);
  const serverHost = params.get('server') || location.host;
  const serverBase = `${wsProto}//${serverHost}`;
  const url = `${serverBase}/game/${encodeURIComponent(roomCode)}?player=${encodeURIComponent(playerName)}`;

  ws = new WebSocket(url);

  ws.addEventListener('open', () => {
    lobbyStatus.textContent = 'Connected. Waiting for state…';
    pingInterval = setInterval(() => {
      if (ws && ws.readyState === WebSocket.OPEN) ws.send('ping');
    }, 30000);
  });

  ws.addEventListener('message', (event) => {
    const data = JSON.parse(event.data);
    // Join error: server sends a plain string like "GameAlreadyStarted".
    if (typeof data === 'string') {
      const friendly = {
        GameAlreadyStarted: 'That game has already started.',
        RoomFull: 'That room is full.',
      };
      lobbyStatus.textContent = friendly[data] || `Error: ${data}`;
      setLobbyButtonsDisabled(false);
      location.hash = '';
      ws.close();
      connectLobby();
      return;
    }
    // Rematch response: server tells us the new room code.
    if (data.rematch_code) {
      connect(data.rematch_code);
      return;
    }
    // Action error during gameplay.
    if (data.Err) {
      if (data.Err === 'RoomExpired') {
        backToLobby();
        lobbyStatus.textContent = 'Game expired due to inactivity.';
        return;
      }
      lobbyStatus.textContent = `Error: ${data.Err}`;
      setLobbyButtonsDisabled(false);
      return;
    }
    lobby.classList.add('hidden');
    gameBoard.classList.remove('hidden');
    state = data;
    mySeat = state.my_seat;
    // Snap back to own board when you need to act
    if (viewOtherSeat !== null && needsMyAction(data)) {
      viewOtherSeat = null;
    }
    // Persist room/name in URL hash so refresh reconnects.
    location.hash = `${encodeURIComponent(currentRoomCode)}/${encodeURIComponent(playerName)}`;
    render();
  });

  ws.addEventListener('close', () => {
    if (pingInterval) { clearInterval(pingInterval); pingInterval = null; }
    // Don't overwrite backToLobby messages (state is cleared when returning to lobby).
    if (state && state.phase !== 'game_over') {
      showStatus('Disconnected from server.');
    }
  });

  ws.addEventListener('error', () => {
    lobbyStatus.textContent = 'Connection failed. Is the server running?';
    setLobbyButtonsDisabled(false);
    connectLobby();
  });
}

function setLobbyButtonsDisabled(disabled) {
  createBtn.disabled = disabled;
}

function backToLobby() {
  if (ws) { ws.close(); ws = null; }
  state = null;
  mySeat = null;
  currentRoomCode = '';
  location.hash = '';
  // Make sure the URL is the root (navigate handles the show/hide).
  if (location.pathname !== '/') {
    history.replaceState(null, '', '/');
  }
  lobby.classList.remove('hidden');
  gameBoard.classList.add('hidden');
  statsScreen.classList.add('hidden');
  gameDetailScreen.classList.add('hidden');
  lobbyStatus.textContent = '';
  setLobbyButtonsDisabled(false);
  // Restore Create Game button in case it was overridden by a share link
  createBtn.textContent = 'Create Game';
  createAction = () => connect(generateCode());
  connectLobby();
}

// ── Lobby WebSocket (live room list) ─────────────────────────────────────────

function connectLobby() {
  if (lobbyWs) return;
  const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const params = new URLSearchParams(location.search);
  const serverHost = params.get('server') || location.host;
  lobbyWs = new WebSocket(`${wsProto}//${serverHost}/lobby`);

  lobbyWs.addEventListener('message', (event) => {
    const rooms = JSON.parse(event.data);
    renderOpenGames(rooms);
  });

  lobbyWs.addEventListener('close', () => {
    lobbyWs = null;
    // Show stale state; don't auto-reconnect if we left the lobby intentionally.
  });

  lobbyWs.addEventListener('error', () => {
    activeGamesList.innerHTML = '<div class="no-games">Could not reach server</div>';
  });
}

function disconnectLobby() {
  if (lobbyWs) { lobbyWs.close(); lobbyWs = null; }
}

function renderOpenGames(rooms) {
  if (rooms.length === 0) {
    activeGamesList.innerHTML = '<div class="no-games">No open games. Create one!</div>';
    return;
  }
  activeGamesList.innerHTML = '';
  for (const room of rooms) {
    const row = document.createElement('div');
    row.className = 'active-game-row';
    row.innerHTML = `<span class="game-players">${room.players.join(', ')}</span>
      <span class="game-count">${room.player_count}/6</span>`;
    row.addEventListener('click', () => connect(room.code));
    activeGamesList.appendChild(row);
  }
}

function needsMyAction(st) {
  const me = st.players?.find(p => p.seat === st.my_seat);
  if (st.phase === 'choosing_cards' && me && !me.played_this_round) return true;
  if (st.phase === 'drafting' && st.current_drafter === st.my_seat) return true;
  if (st.sanctuary_choices && st.sanctuary_choices.length > 0) return true;
  if (st.phase === 'advanced_setup' && st.advanced_setup_choices) return true;
  if (st.phase === 'game_over') return true; // reset view on game end
  return false;
}

function send(action) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(action));
  }
}

// ── Main render ──────────────────────────────────────────────────────────────

function render() {
  if (!state) return;

  lobby.classList.add('hidden');
  gameBoard.classList.remove('hidden');

  const waitingRoom = document.getElementById('waiting-room');
  const gameUI = [
    document.getElementById('status-bar'),
    document.getElementById('opponents-area'),
    document.getElementById('market-area'),
    document.getElementById('my-area'),
  ];

  if (state.phase === 'waiting_for_players') {
    waitingRoom.classList.remove('hidden');
    gameUI.forEach(el => el.classList.add('hidden'));
    renderWaitingRoom();
    return;
  }

  waitingRoom.classList.add('hidden');
  gameUI.forEach(el => el.classList.remove('hidden'));

  renderStatusBar();
  renderOpponents();
  renderMarket();
  renderMyArea();
  renderAdvancedSetupModal();
  renderGameOver();
}

// ── Waiting room ─────────────────────────────────────────────────────────────

function renderWaitingRoom() {
  // Copy link button
  const copyBtn = document.getElementById('copy-link-btn');
  copyBtn.onclick = () => {
    const link = `${location.origin}${location.pathname}${location.search}#${encodeURIComponent(currentRoomCode)}`;
    navigator.clipboard.writeText(link).then(() => {
      copyBtn.textContent = 'Copied!';
      setTimeout(() => { copyBtn.textContent = 'Copy link to game'; }, 1500);
    });
  };

  // Back to lobby button
  document.getElementById('back-to-lobby-btn').onclick = backToLobby;

  const playersEl = document.getElementById('waiting-players');
  playersEl.innerHTML = '';
  for (const p of state.players) {
    const row = document.createElement('div');
    row.className = 'waiting-player' + (p.seat === 0 ? ' host' : '');
    row.textContent = p.name + (p.seat === 0 ? ' (host)' : '');
    playersEl.appendChild(row);
  }

  const controls = document.getElementById('waiting-controls');
  const startBtn = document.getElementById('waiting-start-btn');
  const advToggle = document.getElementById('waiting-advanced');
  const expToggle = document.getElementById('waiting-expansion');
  const hint = document.getElementById('waiting-hint');

  if (mySeat === 0) {
    controls.classList.remove('hidden');
    startBtn.disabled = state.players.length < 1;
    startBtn.textContent = state.players.length === 1
      ? 'Start Solo Game'
      : `Start Game (${state.players.length} players)`;
    startBtn.onclick = () => send({ action: 'StartGame', advanced: advToggle.checked, expansion: expToggle.checked });
    hint.textContent = '';
  } else {
    controls.classList.add('hidden');
    hint.textContent = 'Waiting for host to start the game…';
  }
}

// ── Status bar ───────────────────────────────────────────────────────────────

function renderStatusBar() {
  const phase = state.phase;
  statusRound.textContent = state.round > 0 ? `Round ${state.round}/8` : '';
  statusDeck.textContent = `Deck: ${state.deck_size}`;
  statusSanctDeck.textContent = `Sanctuary deck: ${state.sanctuary_deck_size}`;

  if (phase === 'waiting_for_players') {
    const joined = state.players.length;
    const msg = mySeat === 0
      ? `${joined} player${joined > 1 ? 's' : ''} joined — click Start Game when ready`
      : `${joined} player${joined > 1 ? 's' : ''} joined — waiting for host to start`;
    statusPhase.textContent = msg;
  } else if (phase === 'advanced_setup') {
    if (state.advanced_setup_choices) {
      statusPhase.textContent = 'Advanced setup: choose 3 cards to keep.';
    } else {
      statusPhase.textContent = 'Advanced setup: waiting for other players to choose…';
    }
  } else if (phase === 'choosing_cards') {
    const waiting = state.players.filter(p => !p.played_this_round).map(p => p.name);
    if (waiting.includes(myName())) {
      statusPhase.textContent = 'Choose a card from your hand to play.';
    } else {
      statusPhase.textContent = `Waiting for: ${waiting.join(', ')}`;
    }
  } else if (phase === 'drafting') {
    const drafter = state.current_drafter;
    if (state.sanctuary_choices && state.drafter_choosing_sanctuary) {
      statusPhase.textContent = 'Choose a sanctuary to keep.';
    } else if (state.sanctuary_choices) {
      statusPhase.textContent = 'Choose a sanctuary.';
    } else if (drafter == null) {
      statusPhase.textContent = 'Waiting…';
    } else if (drafter === mySeat) {
      statusPhase.textContent = 'Your pick.';
    } else {
      const drafter_name = state.players.find(p => p.seat === drafter)?.name ?? '?';
      if (state.drafter_choosing_sanctuary) {
        statusPhase.textContent = `${drafter_name} choosing sanctuary…`;
      } else {
        statusPhase.textContent = `${drafter_name} drafting…`;
      }
    }
  } else if (phase === 'game_over') {
    statusPhase.textContent = 'Game over!';
  }
}

// ── Opponents ────────────────────────────────────────────────────────────────

function renderOpponents() {
  opponentsArea.innerHTML = '';
  // Show all players (including self). During drafting, sort by draft order.
  // Otherwise, sort by highest tableau card number (ascending = draft-like order).
  let players = [...state.players];
  if (state.phase === 'drafting' && state.draft_order.length > 0) {
    players.sort((a, b) =>
      state.draft_order.indexOf(a.seat) - state.draft_order.indexOf(b.seat));
  } else {
    players.sort((a, b) => {
      const aNum = a.tableau.length > 0 ? a.tableau[a.tableau.length - 1].number : 0;
      const bNum = b.tableau.length > 0 ? b.tableau[b.tableau.length - 1].number : 0;
      return aNum - bNum;
    });
  }
  const isGameOver = state.phase === 'game_over';
  for (const p of players) {
    const isMe = p.seat === mySeat;

    const panel = document.createElement('div');
    const isActiveDrafter = state.phase === 'drafting' && state.current_drafter === p.seat;
    const isViewSelected = viewOtherSeat !== null && viewOtherSeat === p.seat;
    panel.className = 'opponent-panel'
      + (isMe ? ' self-panel' : '')
      + (isActiveDrafter ? ' active-drafter' : '')
      + (isViewSelected ? ' view-selected' : '');

    const nameEl = document.createElement('div');
    nameEl.className = 'opponent-name';
    nameEl.textContent = isMe ? 'You' : p.name;
    if (isGameOver && state.scores) {
      const playerScore = state.scores.find(s => s.seat === p.seat);
      if (playerScore) {
        const badge = document.createElement('span');
        badge.className = 'draft-order-badge';
        badge.textContent = `${playerScore.total}`;
        nameEl.appendChild(badge);
      }
    } else if (p.tableau.length > 0) {
      const highest = p.tableau[p.tableau.length - 1].number;
      const badge = document.createElement('span');
      badge.className = 'draft-order-badge';
      badge.textContent = `#${highest}`;
      if (state.phase === 'drafting' && state.current_drafter === p.seat) badge.classList.add('active');
      nameEl.appendChild(badge);
    }
    panel.appendChild(nameEl);

    // Clicking any player switches the main area to show their board.
    panel.style.cursor = 'pointer';
    panel.addEventListener('click', () => {
      viewOtherSeat = p.seat === mySeat ? null : p.seat;
      render();
    });

    // During game over, opponent panels are just clickable names (no small card details).
    if (isGameOver) {
      opponentsArea.appendChild(panel);
      continue;
    }

    // During gameplay, show small card details for opponents.
    if (!isMe) {
      const details = document.createElement('div');
      details.className = 'opponent-details mobile-collapsible';

      // No expand chevron — clicking the panel switches to full-size view

      // Tableau
      const tableau = document.createElement('div');
      tableau.className = 'opponent-tableau';
      for (const card of p.tableau) {
        tableau.appendChild(regionCardEl(card, 'sm', false, true));
      }
      // Played-this-round placeholder
      if (p.played_this_round && state.phase === 'choosing_cards') {
        const ph = document.createElement('div');
        ph.className = 'card sm played-overlay';
        ph.innerHTML = '<img src="region/card-back.png" alt="face-down">';
        tableau.appendChild(ph);
      }
      details.appendChild(tableau);

      // Sanctuaries
      if (p.sanctuaries.length > 0) {
        const sancts = document.createElement('div');
        sancts.className = 'opponent-sanctuaries';
        for (const s of p.sanctuaries) {
          sancts.appendChild(sanctuaryCardEl(s, 'sm', true));
        }
        details.appendChild(sancts);
      }
      panel.appendChild(details);
    }

    opponentsArea.appendChild(panel);
  }
}

// ── Market ───────────────────────────────────────────────────────────────────

function renderMarket() {
  marketCards.innerHTML = '';
  const isDrafting = state.phase === 'drafting' && state.current_drafter === mySeat && !state.drafter_choosing_sanctuary;

  state.market.forEach((card, idx) => {
    const el = regionCardEl(card, 'xl', false);
    if (isDrafting) {
      el.classList.add('draftable');
      el.addEventListener('click', () => send({ action: 'DraftCard', market_index: idx }));
    }
    marketCards.appendChild(el);
  });
}

// ── My area ───────────────────────────────────────────────────────────────────

function renderMyArea() {
  // During game_over, renderGameOver handles the tableau/sanctuaries
  if (state.phase === 'game_over') return;

  const viewLabel = document.getElementById('my-tableau-label');
  const sanctLabel = document.getElementById('my-sanctuaries-label');

  // Viewing another player's board during gameplay (no scores, no hand)
  if (viewOtherSeat !== null) {
    const other = state.players.find(p => p.seat === viewOtherSeat);
    if (other) {
      viewLabel.textContent = `Viewing ${other.name}'s tableau`;
      sanctLabel.textContent = `${other.name}'s sanctuaries`;
      myTableau.innerHTML = '';
      for (const card of other.tableau) {
        myTableau.appendChild(regionCardEl(card, 'xl', false));
      }
      mySanctuaries.innerHTML = '';
      for (const s of other.sanctuaries) {
        mySanctuaries.appendChild(sanctuaryCardEl(s, 'md'));
      }
      // Show "Back to my board" button instead of hand
      myHand.innerHTML = '';
      document.getElementById('my-hand-row').classList.remove('hidden');
      document.getElementById('my-hand-label').textContent = '';
      const backBtn = document.createElement('button');
      backBtn.className = 'back-to-my-board-btn';
      backBtn.textContent = 'Back to my board';
      backBtn.addEventListener('click', () => { viewOtherSeat = null; render(); });
      myHand.appendChild(backBtn);
      return;
    }
  }

  viewLabel.textContent = 'Your tableau';
  sanctLabel.textContent = 'Sanctuaries';
  document.getElementById('my-hand-label').textContent = 'Your hand';
  document.getElementById('my-hand-row').classList.remove('hidden');

  // Tableau (with live score badges)
  myTableau.innerHTML = '';
  const me = state.players.find(p => p.seat === mySeat);
  const liveRegionEntries = state.my_score_detail
    ? state.my_score_detail.filter(e => e.kind === 'region')
    : [];
  if (me) {
    const filledCount = me.tableau.length +
      (state.my_played_card && state.phase === 'choosing_cards' ? 1 : 0);

    for (let i = 0; i < me.tableau.length; i++) {
      const card = me.tableau[i];
      const el = regionCardEl(card, 'xl', false);
      // Map tableau index to detail: detail is right-to-left, tableau is left-to-right
      const detailIdx = liveRegionEntries.length - 1 - i;
      if (detailIdx >= 0 && detailIdx < liveRegionEntries.length) {
        const entry = liveRegionEntries[detailIdx];
        const badge = document.createElement('div');
        badge.className = 'score-badge live' + (entry.points > 0 ? ' positive' : ' zero');
        badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
        el.appendChild(badge);
        el.style.cursor = 'pointer';
        el.addEventListener('click', (e) => {
          e.stopPropagation();
          showScoreTip(el, entry.explanation);
        });
      }
      myTableau.appendChild(el);
    }
    // Show the played card face-up immediately (with live score badge)
    if (state.my_played_card && state.phase === 'choosing_cards') {
      const card = state.my_played_card;
      const el = regionCardEl(card, 'xl', false);
      el.classList.add('played-overlay');
      // The score detail includes this card — it's the first entry (rightmost = just played)
      if (liveRegionEntries.length > 0) {
        const entry = liveRegionEntries[0];
        const badge = document.createElement('div');
        badge.className = 'score-badge live' + (entry.points > 0 ? ' positive' : ' zero');
        badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
        el.appendChild(badge);
        el.style.cursor = 'pointer';
        el.addEventListener('click', (e) => {
          e.stopPropagation();
          showScoreTip(el, entry.explanation);
        });
      }
      myTableau.appendChild(el);
    }
    // Fill remaining slots with empty numbered placeholders
    for (let i = filledCount; i < 8; i++) {
      const slot = document.createElement('div');
      slot.className = 'card-slot';
      slot.textContent = i + 1;
      myTableau.appendChild(slot);
    }
  }

  // Sanctuaries (with live score badges)
  mySanctuaries.innerHTML = '';
  const liveSanctEntries = state.my_score_detail
    ? state.my_score_detail.filter(e => e.kind === 'sanctuary')
    : [];
  if (me) {
    for (let i = 0; i < me.sanctuaries.length; i++) {
      const s = me.sanctuaries[i];
      const el = sanctuaryCardEl(s, 'md');
      if (i < liveSanctEntries.length) {
        const entry = liveSanctEntries[i];
        const badge = document.createElement('div');
        badge.className = 'score-badge-sm live' + (entry.points > 0 ? ' positive' : ' zero');
        badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
        el.appendChild(badge);
        el.style.cursor = 'pointer';
        el.addEventListener('click', (e) => {
          e.stopPropagation();
          showScoreTip(el, entry.explanation);
        });
      }
      mySanctuaries.appendChild(el);
    }
    // Inline sanctuary choices (pick one)
    if (state.sanctuary_choices && state.sanctuary_choices.length > 0) {
      const divider = document.createElement('div');
      divider.className = 'sanctuary-pick-label';
      divider.textContent = 'Pick one:';
      mySanctuaries.appendChild(divider);
      state.sanctuary_choices.forEach((card, idx) => {
        const el = sanctuaryCardEl(card, 'md', false);
        el.classList.add('sanctuary-pickable');
        el.addEventListener('click', () => send({ action: 'ChooseSanctuary', sanctuary_index: idx }));
        mySanctuaries.appendChild(el);
      });
    }
  }

  // Hand
  myHand.innerHTML = '';
  const canPlay = state.phase === 'choosing_cards' && !(me && me.played_this_round);

  state.my_hand.forEach((card, idx) => {
    const el = regionCardEl(card, 'xl', canPlay);
    if (canPlay) {
      el.addEventListener('click', () => send({ action: 'PlayCard', card_index: idx }));
    }
    myHand.appendChild(el);
  });

}

// ── Advanced setup modal ──────────────────────────────────────────────────────

let advancedSelected = new Set();

function renderAdvancedSetupModal() {
  if (state.phase !== 'advanced_setup' || !state.advanced_setup_choices) {
    advancedModal.classList.add('hidden');
    advancedSelected.clear();
    return;
  }

  advancedModal.classList.remove('hidden');
  // Preserve selection across re-renders (broadcasts from other players submitting).
  advancedChoicesEl.innerHTML = '';
  updateAdvancedConfirmBtn();

  state.advanced_setup_choices.forEach((card, idx) => {
    const el = regionCardEl(card, 'xl', true);
    el.dataset.idx = idx;
    if (advancedSelected.has(idx)) el.classList.add('selected');
    el.addEventListener('click', () => {
      if (advancedSelected.has(idx)) {
        advancedSelected.delete(idx);
        el.classList.remove('selected');
      } else if (advancedSelected.size < 3) {
        advancedSelected.add(idx);
        el.classList.add('selected');
      }
      updateAdvancedConfirmBtn();
    });
    advancedChoicesEl.appendChild(el);
  });
}

function updateAdvancedConfirmBtn() {
  const n = advancedSelected.size;
  advancedConfirmBtn.disabled = n !== 3;
  advancedConfirmBtn.textContent = `Keep selected (${n} / 3)`;
}

advancedConfirmBtn.addEventListener('click', () => {
  const indices = Array.from(advancedSelected);
  send({ action: 'KeepCards', indices });
});

// (Sanctuary choices are now rendered inline in renderMyArea, no modal needed.)

// ── Scoring card factories ────────────────────────────────────────────────
//
// Shared by the live game-over screen and the read-only saved-game detail view.
// A "scoring region card" is a `.card.xl.scoring-card-slot` div that can be
// face-down, revealed with a score badge, or highlighted as just-revealed.
// A "scoring sanctuary card" is a `.card.sanctuary.md` with an optional
// score badge. Click handlers show the per-card score explanation.

function makeScoringRegionCard(card, entry, { revealed = true, justRevealed = false } = {}) {
  const el = document.createElement('div');
  el.className = 'card xl scoring-card-slot';
  if (!revealed || !entry) {
    el.classList.add('face-down');
    const img = document.createElement('img');
    img.src = 'region/card-back.png';
    img.alt = 'Face down';
    el.appendChild(img);
    return el;
  }
  el.classList.add('scoring-revealed');
  const img = document.createElement('img');
  img.src = regionImagePath(card.number);
  img.alt = `Region ${card.number}`;
  el.appendChild(img);
  const badge = document.createElement('div');
  badge.className = 'score-badge' + (entry.points > 0 ? ' positive' : ' zero');
  badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
  el.appendChild(badge);
  el.style.cursor = 'pointer';
  el.addEventListener('click', (e) => {
    e.stopPropagation();
    showScoreTip(el, entry.explanation);
  });
  if (justRevealed) el.classList.add('just-revealed');
  return el;
}

function makeScoringSanctuaryCard(card, entry, { scored = true } = {}) {
  const el = document.createElement('div');
  el.className = 'card sanctuary md';
  const img = document.createElement('img');
  img.src = sanctuaryImagePath(card.tile);
  img.alt = `Sanctuary ${card.tile}`;
  el.appendChild(img);
  if (scored && entry) {
    const badge = document.createElement('div');
    badge.className = 'score-badge-sm' + (entry.points > 0 ? ' positive' : ' zero');
    badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
    el.appendChild(badge);
    el.style.cursor = 'pointer';
    el.addEventListener('click', (e) => {
      e.stopPropagation();
      showScoreTip(el, entry.explanation);
    });
  }
  return el;
}

// ── Game over (inline scoring) ────────────────────────────────────────────────

function renderGameOver() {
  const isGameOver = state.phase === 'game_over' && state.scores;
  const scoringBar = document.getElementById('scoring-bar');

  if (!isGameOver) {
    scoringRevealIndex = 0;
    viewOtherSeat = null;
    if (scoringBar) scoringBar.remove();
    document.getElementById('scoring-leaderboard')?.remove();
    document.getElementById('scoring-table')?.remove();
    // Show normal game elements
    document.getElementById('market-area').classList.remove('hidden');
    document.getElementById('my-hand-row').classList.remove('hidden');
    return;
  }

  // Hide market and hand during scoring
  document.getElementById('market-area').classList.add('hidden');
  document.getElementById('my-hand-row').classList.add('hidden');

  // --- Always show leaderboard + buttons at the top ---
  renderLeaderboard();

  // Determine whose board to display
  const viewSeat = viewOtherSeat ?? mySeat;
  const viewPlayer = state.players.find(p => p.seat === viewSeat);
  // Get score detail for viewed player from all_score_details
  const allDetails = state.all_score_details || [];
  const viewDetail = allDetails.find(d => d.seat === viewSeat);
  const detail = viewDetail ? viewDetail.entries : [];

  // Separate region entries (first 8) from sanctuary entries
  const regionEntries = detail.filter(e => e.kind === 'region');
  const sanctuaryEntries = detail.filter(e => e.kind === 'sanctuary');

  // When viewing another player, skip the animation — show everything revealed.
  const viewingOther = viewSeat !== mySeat;
  const effectiveRevealIndex = viewingOther ? regionEntries.length + 2 : scoringRevealIndex;

  // --- Show whose board we're viewing ---
  const viewLabel = document.getElementById('my-tableau-label');
  const sanctLabel = document.getElementById('my-sanctuaries-label');
  if (viewSeat === mySeat) {
    viewLabel.textContent = 'Your tableau';
    sanctLabel.textContent = 'Sanctuaries';
  } else {
    const vname = viewPlayer?.name ?? '?';
    viewLabel.textContent = `${vname}'s tableau`;
    sanctLabel.textContent = `${vname}'s sanctuaries`;
  }

  // --- Render tableau cards as face-down/revealed in place ---
  myTableau.innerHTML = '';
  const me = viewPlayer;
  if (!me) return;

  for (let i = 0; i < me.tableau.length; i++) {
    const card = me.tableau[i];
    const detailIdx = regionEntries.length - 1 - i;
    const revealOrder = me.tableau.length - 1 - i;
    const revealed = revealOrder < effectiveRevealIndex;
    const entry = revealed && detailIdx >= 0 ? regionEntries[detailIdx] : null;
    const justRevealed = revealOrder === effectiveRevealIndex - 1;
    myTableau.appendChild(makeScoringRegionCard(card, entry, { revealed: !!entry, justRevealed }));
  }

  // --- Sanctuaries: always visible, score badges appear after region cards ---
  mySanctuaries.innerHTML = '';
  const sanctuariesScored = effectiveRevealIndex > regionEntries.length;

  for (let i = 0; i < me.sanctuaries.length; i++) {
    const entry = sanctuariesScored ? sanctuaryEntries[i] : null;
    mySanctuaries.appendChild(makeScoringSanctuaryCard(me.sanctuaries[i], entry, { scored: sanctuariesScored }));
  }

  // --- Scoring advance bar (below sanctuaries) ---
  const runningTotal = computeRunningTotal(regionEntries, sanctuaryEntries, effectiveRevealIndex);
  const totalRevealSteps = regionEntries.length + (sanctuaryEntries.length > 0 ? 1 : 0);
  const allDone = effectiveRevealIndex > totalRevealSteps;

  if (!allDone && !viewingOther) {
    renderScoringBar(regionEntries, sanctuaryEntries, runningTotal);
  } else {
    document.getElementById('scoring-bar')?.remove();
  }

  // Scoring table (all players)
  renderScoringTable();
}

function renderScoringBar(regionEntries, sanctuaryEntries, runningTotal) {
  let bar = document.getElementById('scoring-bar');
  if (!bar) {
    bar = document.createElement('div');
    bar.id = 'scoring-bar';
    document.getElementById('my-sanctuaries-row').after(bar);
  }

  let explanation = '';
  let btnLabel = '';
  if (scoringRevealIndex === 0) {
    explanation = 'Cards reveal right to left.';
    btnLabel = 'Start scoring';
  } else if (scoringRevealIndex <= regionEntries.length) {
    const lastRevealed = regionEntries[scoringRevealIndex - 1];
    if (lastRevealed) {
      explanation = lastRevealed.points > 0
        ? `+${lastRevealed.points} fame: ${lastRevealed.explanation}`
        : lastRevealed.explanation;
    }
    btnLabel = scoringRevealIndex >= regionEntries.length
      ? (sanctuaryEntries.length > 0 ? 'Reveal sanctuaries' : 'Done')
      : 'Next card';
  } else {
    const sanctExps = sanctuaryEntries.filter(e => e.points > 0).map(e => `+${e.points}: ${e.explanation}`);
    explanation = sanctExps.length > 0 ? sanctExps.join(' | ') : 'No sanctuary points';
    btnLabel = 'Done';
  }

  bar.className = 'scoring-bar';
  bar.innerHTML = `
    <div class="scoring-bar-left">
      <div class="scoring-bar-total">${runningTotal} fame</div>
      <div class="scoring-bar-explain">${explanation}</div>
    </div>
    <button id="scoring-advance-btn">${btnLabel}</button>
  `;
  document.getElementById('scoring-advance-btn').addEventListener('click', advanceScoringReveal);
}

function computeRunningTotal(regionEntries, sanctuaryEntries, revealIdx) {
  let total = 0;
  for (let r = 0; r < Math.min(revealIdx, regionEntries.length); r++) {
    total += regionEntries[r].points;
  }
  if (revealIdx > regionEntries.length) {
    for (const e of sanctuaryEntries) total += e.points;
  }
  return total;
}

function advanceScoringReveal() {
  scoringRevealIndex++;
  renderGameOver();
}

function renderScoringTable() {
  if (!state.all_score_details) return;

  let table = document.getElementById('scoring-table');
  if (!table) {
    table = document.createElement('div');
    table.id = 'scoring-table';
    document.getElementById('my-area').appendChild(table);
  }

  const details = state.all_score_details;
  // All players have same number of region entries; use first player to get row count
  const firstPlayer = details[0];
  const regionCount = firstPlayer.entries.filter(e => e.kind === 'region').length;
  const hasSanctuaries = firstPlayer.entries.some(e => e.kind === 'sanctuary');

  // Figure out how many rows are revealed
  const viewingOther = (viewOtherSeat ?? mySeat) !== mySeat;
  const revealIdx = viewingOther ? regionCount + 2 : scoringRevealIndex;
  const revealedRegions = Math.min(revealIdx, regionCount);
  const sanctuariesScored = revealIdx > regionCount;

  // Build table HTML
  let html = '<table><thead><tr><th></th>';
  for (const p of details) {
    const isMe = p.seat === mySeat;
    html += `<th class="${isMe ? 'me' : ''}">${p.name}</th>`;
  }
  html += '</tr></thead><tbody>';

  // Region rows (right-to-left, matching reveal order)
  for (let r = 0; r < regionCount; r++) {
    const revealed = r < revealedRegions;
    html += `<tr class="${revealed ? 'revealed' : 'hidden-row'}">`;
    html += `<td class="row-label">Card ${r + 1}</td>`;
    for (const p of details) {
      const entry = p.entries[r]; // entries are already right-to-left
      if (revealed) {
        const cls = entry.points > 0 ? 'pts positive' : 'pts zero';
        html += `<td class="${cls}" title="${entry.explanation}">${entry.points > 0 ? '+' + entry.points : '0'}</td>`;
      } else {
        html += '<td class="pts hidden-cell">—</td>';
      }
    }
    html += '</tr>';
  }

  // Sanctuary row
  if (hasSanctuaries) {
    html += `<tr class="${sanctuariesScored ? 'revealed' : 'hidden-row'}">`;
    html += '<td class="row-label">Sanct.</td>';
    for (const p of details) {
      const sanctEntries = p.entries.filter(e => e.kind === 'sanctuary');
      const sanctTotal = sanctEntries.reduce((s, e) => s + e.points, 0);
      const sanctExp = sanctEntries.map(e => `${e.number}: ${e.points > 0 ? '+' + e.points : '0'}`).join(', ');
      if (sanctuariesScored) {
        const cls = sanctTotal > 0 ? 'pts positive' : 'pts zero';
        html += `<td class="${cls}" title="${sanctExp}">${sanctTotal > 0 ? '+' + sanctTotal : '0'}</td>`;
      } else {
        html += '<td class="pts hidden-cell">—</td>';
      }
    }
    html += '</tr>';
  }

  // Total row
  html += '<tr class="total-row"><td class="row-label">Total</td>';
  for (const p of details) {
    const regionPts = p.entries.filter(e => e.kind === 'region').slice(0, revealedRegions)
      .reduce((s, e) => s + e.points, 0);
    const sanctPts = sanctuariesScored
      ? p.entries.filter(e => e.kind === 'sanctuary').reduce((s, e) => s + e.points, 0)
      : 0;
    const total = regionPts + sanctPts;
    html += `<td class="pts total">${total}</td>`;
  }
  html += '</tr>';

  html += '</tbody></table>';
  table.innerHTML = html;
}

function renderLeaderboard() {
  let lb = document.getElementById('scoring-leaderboard');
  if (!lb) {
    lb = document.createElement('div');
    lb.id = 'scoring-leaderboard';
    // Insert before opponents area so it's at the top of the game board
    const opArea = document.getElementById('opponents-area');
    opArea.parentNode.insertBefore(lb, opArea);
  }

  const sorted = [...state.scores].sort((a, b) => {
    if (b.total !== a.total) return b.total - a.total;
    return a.card_number_sum - b.card_number_sum;
  });

  const totals = sorted.map(s => s.total);
  const hasTie = (t) => totals.filter(v => v === t).length > 1;

  const medals = ['&#x1f947;', '&#x1f948;', '&#x1f949;'];
  const highlights = state.post_game_highlights || [];
  const hlBySeat = new Map(highlights.map(h => [h.seat, h]));

  let html = '<div class="leaderboard-buttons">';
  html += '<button id="play-again-btn-inline" class="play-again-btn">Play Again</button>';
  html += '<button id="back-to-lobby-btn-inline" class="play-again-btn secondary">Back to Lobby</button>';
  html += '</div>';
  html += '<div class="leaderboard-title">Game Over</div>';
  html += `<div class="leaderboard-winner">${sorted[0].name}</div>`;
  html += `<div class="leaderboard-winner-score">${sorted[0].total} fame</div>`;
  html += '<div class="leaderboard-rows">';
  sorted.forEach((s, i) => {
    const medal = i < 3 ? medals[i] : `${i + 1}.`;
    const tie = hasTie(s.total) ? ` <span class="tiebreaker">(tiebreak: ${s.card_number_sum})</span>` : '';
    const hl = hlBySeat.get(s.seat);
    const badges = [];
    if (hl) {
      if (hl.personal_best) {
        badges.push(hl.previous_best != null
          ? `<span class="hl-badge pb">personal best! (prev ${hl.previous_best})</span>`
          : `<span class="hl-badge pb">first game!</span>`);
      }
      if (hl.all_time_rank && hl.all_time_rank <= 10) {
        badges.push(`<span class="hl-badge rank">#${hl.all_time_rank} all-time</span>`);
      }
      // How this game compares to this player's historical average.
      if (hl.previous_player_avg != null) {
        const avg = hl.previous_player_avg;
        const diff = s.total - avg;
        const sign = diff > 0 ? '+' : (diff < 0 ? '−' : '±');
        const cls = diff > 0 ? 'avg-up' : (diff < 0 ? 'avg-down' : 'avg-even');
        badges.push(`<span class="hl-badge ${cls}">${sign}${Math.abs(diff).toFixed(1)} vs your avg (${avg.toFixed(1)})</span>`);
      }
      // How this game compares to the global average across all games.
      if (hl.previous_global_avg != null) {
        const avg = hl.previous_global_avg;
        const diff = s.total - avg;
        const sign = diff > 0 ? '+' : (diff < 0 ? '−' : '±');
        const cls = diff > 0 ? 'avg-up' : (diff < 0 ? 'avg-down' : 'avg-even');
        badges.push(`<span class="hl-badge ${cls}">${sign}${Math.abs(diff).toFixed(1)} vs all avg (${avg.toFixed(1)})</span>`);
      }
    }
    html += `<div class="score-row${i === 0 ? ' winner' : ''}">
      <span class="score-rank">${medal}</span>
      <span class="score-name">${s.name}</span>
      <span class="score-pts">${s.total}${tie}</span>
      ${badges.length ? `<span class="score-badges">${badges.join(' ')}</span>` : ''}
    </div>`;
  });
  html += '</div>';
  lb.innerHTML = html;
  document.getElementById('play-again-btn-inline').addEventListener('click', () => {
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      // WS died while idle on game over — reconnect to room, then player can retry
      connect(currentRoomCode);
      return;
    }
    send({ action: 'Rematch' });
  });
  document.getElementById('back-to-lobby-btn-inline').addEventListener('click', () => {
    backToLobby();
  });
}

// ── Card helpers ──────────────────────────────────────────────────────────────

function regionCardEl(card, size, clickable, zoomable) {
  const el = document.createElement('div');
  el.className = `card ${size}` + (clickable ? ' playable' : '');
  const img = document.createElement('img');
  img.src = regionImagePath(card.number);
  img.alt = `Region ${card.number}`;
  el.appendChild(img);
  if (zoomable) attachZoom(el, regionImagePath(card.number), 'region');
  return el;
}

function sanctuaryCardEl(card, size, zoomable) {
  const el = document.createElement('div');
  el.className = `card sanctuary ${size}`;
  const img = document.createElement('img');
  img.src = sanctuaryImagePath(card.tile);
  img.alt = `Sanctuary ${card.tile}`;
  el.appendChild(img);
  if (zoomable) attachZoom(el, sanctuaryImagePath(card.tile), 'sanctuary');
  return el;
}

// ── Hover / tap zoom ────────────────────────────────────────────────────────

const cardZoom = document.getElementById('card-zoom');
const cardZoomImg = cardZoom.querySelector('img');
const cardZoomBackdrop = document.getElementById('card-zoom-backdrop');
const isTouchDevice = window.matchMedia('(hover: none) and (pointer: coarse)').matches;

function attachZoom(el, imgSrc, kind) {
  if (isTouchDevice) {
    // Mobile: tap to show centered zoom overlay
    el.addEventListener('click', (e) => {
      // Don't intercept clicks on actionable cards (playable/draftable)
      if (el.classList.contains('playable') || el.classList.contains('draftable')) return;
      e.stopPropagation();
      cardZoomImg.src = imgSrc;
      cardZoom.className = kind + ' visible';
      // Center in viewport
      cardZoom.style.left = '50%';
      cardZoom.style.top = '50%';
      cardZoom.style.transform = 'translate(-50%, -50%)';
      cardZoomBackdrop.classList.remove('hidden');
    });
  } else {
    // Desktop: hover to show zoom near cursor
    el.addEventListener('mouseenter', (e) => {
      cardZoomImg.src = imgSrc;
      cardZoom.className = kind + ' visible';
      cardZoom.style.transform = '';
      positionZoom(e);
    });
    el.addEventListener('mousemove', positionZoom);
    el.addEventListener('mouseleave', () => {
      cardZoom.className = '';
    });
  }
}

function dismissZoom() {
  cardZoom.className = '';
  cardZoom.style.left = '';
  cardZoom.style.top = '';
  cardZoom.style.transform = '';
  cardZoomBackdrop.classList.add('hidden');
}

cardZoomBackdrop.addEventListener('click', dismissZoom);
cardZoom.addEventListener('click', dismissZoom);

function positionZoom(e) {
  const pad = 12;
  const zw = cardZoom.classList.contains('sanctuary') ? 140 : 180;
  const zh = cardZoom.classList.contains('sanctuary') ? 216 : 180;

  let x = e.clientX + pad;
  let y = e.clientY - zh / 2;

  // Keep within viewport
  if (x + zw > window.innerWidth) x = e.clientX - zw - pad;
  if (y < 4) y = 4;
  if (y + zh > window.innerHeight - 4) y = window.innerHeight - zh - 4;

  cardZoom.style.left = x + 'px';
  cardZoom.style.top = y + 'px';
}

function regionImagePath(number) {
  return `region/tile${String(number).padStart(3, '0')}.jpg`;
}

function sanctuaryImagePath(tile) {
  return `sanctuary/tile${String(tile).padStart(3, '0')}.jpg`;
}

function myName() {
  return state.players.find(p => p.seat === mySeat)?.name ?? playerNameEl.value.trim();
}

function showStatus(msg) {
  statusPhase.textContent = msg;
}

// ── Wire up ───────────────────────────────────────────────────────────────────

// ── Score tooltip (floating, click to show) ──────────────────────────────────

const scoreTip = document.createElement('div');
scoreTip.id = 'score-tip';
scoreTip.className = 'hidden';
document.body.appendChild(scoreTip);

function showScoreTip(anchorEl, text) {
  const wasVisible = !scoreTip.classList.contains('hidden') && scoreTip._anchor === anchorEl;
  hideScoreTip();
  if (wasVisible) return; // toggle off
  scoreTip.textContent = text;
  scoreTip.classList.remove('hidden');
  scoreTip._anchor = anchorEl;
  const rect = anchorEl.getBoundingClientRect();
  // Position above card center, then clamp within viewport
  let left = rect.left + rect.width / 2;
  let top = rect.top - 6;
  scoreTip.style.left = left + 'px';
  scoreTip.style.top = top + 'px';
  // After rendering, check if it overflows and adjust
  const tipRect = scoreTip.getBoundingClientRect();
  if (tipRect.left < 4) {
    scoreTip.style.left = (4 + tipRect.width / 2) + 'px';
  } else if (tipRect.right > window.innerWidth - 4) {
    scoreTip.style.left = (window.innerWidth - 4 - tipRect.width / 2) + 'px';
  }
  if (tipRect.top < 4) {
    scoreTip.style.top = (rect.bottom + 6) + 'px';
  }
}

function hideScoreTip() {
  scoreTip.classList.add('hidden');
  scoreTip._anchor = null;
}

document.addEventListener('click', hideScoreTip);

// ── Stats screen ─────────────────────────────────────────────────────────────

const statsScreen      = document.getElementById('stats-screen');
const statsBtn         = document.getElementById('stats-btn');
const statsCloseBtn    = document.getElementById('stats-close-btn');
const statsTabs        = document.querySelectorAll('.stats-tab');
const statsPanelMe     = document.getElementById('stats-panel-me');
const statsPanelHs     = document.getElementById('stats-panel-highscores');
const statsPanelRecent = document.getElementById('stats-panel-recent');
const gameDetailScreen = document.getElementById('game-detail-screen');
const gameDetailBody   = document.getElementById('game-detail-body');
const gameDetailTitle  = document.getElementById('game-detail-title');
const gameDetailClose  = document.getElementById('game-detail-close-btn');

function apiBase() {
  const params = new URLSearchParams(location.search);
  const serverHost = params.get('server') || location.host;
  const proto = location.protocol === 'https:' ? 'https:' : 'http:';
  return `${proto}//${serverHost}`;
}

async function apiGet(path) {
  const res = await fetch(apiBase() + path);
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  return res.json();
}

// ── Router ──────────────────────────────────────────────────────────────────
//
// URL routes:
//   /                          → lobby (with optional #ROOM or #ROOM/Name hash)
//   /stats                     → stats hub (defaults to My Stats)
//   /stats/me                  → My Stats (uses saved name)
//   /stats/leaderboard         → all-time leaderboard (alias for high-scores)
//   /stats/high-scores?player= → optional filter (see /api/stats/leaderboard)
//   /stats/recent              → recent games list
//   /stats/player/<name>       → My Stats pre-filled for a specific player
//   /stats/games/<id>          → read-only detail view of a saved game

function parseRoute(pathname) {
  const path = pathname.replace(/\/+$/, '') || '/';
  if (path === '' || path === '/') return { view: 'lobby' };
  if (path === '/stats' || path === '/stats/me') return { view: 'stats', tab: 'me' };
  // /stats/leaderboard is kept as an alias for the old URL; new canonical is /stats/high-scores.
  if (path === '/stats/high-scores' || path === '/stats/leaderboard') return { view: 'stats', tab: 'highscores' };
  if (path === '/stats/recent') return { view: 'stats', tab: 'recent' };
  const playerMatch = path.match(/^\/stats\/player\/(.+)$/);
  if (playerMatch) return { view: 'stats', tab: 'me', name: decodeURIComponent(playerMatch[1]) };
  const gameMatch = path.match(/^\/stats\/games\/(\d+)$/);
  if (gameMatch) return { view: 'game-detail', gameId: parseInt(gameMatch[1], 10) };
  return { view: 'lobby' };
}

function navigate(path, { replace = false } = {}) {
  // Preserve the current query string (e.g. ?server=localhost:8085 when the
  // static files are served from live-server while the API/WS host is the
  // Rust server on a different port).
  // If `path` already includes a query (e.g. statsHighScoresUrl), do not append
  // location.search again — that would duplicate ?server= and break ?player=.
  const url = path.includes('?') ? path : path + (location.search || '');
  if (replace) history.replaceState(null, '', url);
  else history.pushState(null, '', url);
  applyRoute();
}

/** Merge `player` into the query string for `/stats/high-scores` (preserves e.g. `server=`). */
function statsHighScoresUrl(playerName) {
  const params = new URLSearchParams(location.search);
  const t = (playerName || '').trim();
  if (t) params.set('player', t);
  else params.delete('player');
  const q = params.toString();
  return '/stats/high-scores' + (q ? '?' + q : '');
}

function applyRoute() {
  const r = parseRoute(location.pathname);
  const inLiveGame = state && state.phase && state.phase !== 'waiting_for_players';

  // Hide every top-level screen; each branch below shows exactly one.
  lobby.classList.add('hidden');
  statsScreen.classList.add('hidden');
  gameDetailScreen.classList.add('hidden');
  gameBoard.classList.add('hidden');

  if (r.view === 'stats') {
    statsScreen.classList.remove('hidden');
    activateStatsTab(r.tab, r.name);
    return;
  }
  if (r.view === 'game-detail') {
    gameDetailScreen.classList.remove('hidden');
    loadAndRenderGameDetail(r.gameId);
    return;
  }
  // Lobby route.
  if (inLiveGame) gameBoard.classList.remove('hidden');
  else lobby.classList.remove('hidden');
}

function activateStatsTab(id, presetName) {
  statsTabs.forEach(b => b.classList.toggle('active', b.dataset.tab === id));
  statsPanelMe.classList.toggle('hidden', id !== 'me');
  statsPanelHs.classList.toggle('hidden', id !== 'highscores');
  statsPanelRecent.classList.toggle('hidden', id !== 'recent');
  if (id === 'me') renderMyStatsPanel(presetName);
  else if (id === 'highscores') renderHighScoresPanel();
  else if (id === 'recent') renderRecentGamesPanel();
}

window.addEventListener('popstate', applyRoute);

function formatDate(ts) {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  return d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

function ordinalSuffix(n) {
  const s = ['th', 'st', 'nd', 'rd'];
  const v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
}

function formatDateShort(ts) {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  return d.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
}

async function renderMyStatsPanel(presetName) {
  const name = (presetName || playerNameEl.value || '').trim();
  statsPanelMe.innerHTML = `
    <div class="stats-search">
      <label for="stats-name-input">Player name</label>
      <div class="stats-search-row">
        <input type="text" id="stats-name-input" value="${escapeHtml(name)}" placeholder="Your name" />
        <button id="stats-name-go">Look up</button>
      </div>
    </div>
    <div id="stats-me-body"></div>
  `;
  const input = document.getElementById('stats-name-input');
  const go = document.getElementById('stats-name-go');
  const doLookup = () => {
    const n = input.value.trim();
    if (n) {
      navigate(`/stats/player/${encodeURIComponent(n)}`, { replace: true });
    } else {
      document.getElementById('stats-me-body').innerHTML = '<div class="stats-empty">Enter a name to see stats.</div>';
    }
  };
  go.addEventListener('click', doLookup);
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); doLookup(); } });
  if (name) fetchAndRenderPlayerStats(name);
  else document.getElementById('stats-me-body').innerHTML = '<div class="stats-empty">Enter a name to see stats.</div>';
}

function formatDuration(secs) {
  if (!secs || secs <= 0) return '0m';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatShortDate(ts) {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  return d.toLocaleString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
}

// Render an inline SVG sparkline of the player's scores over time, with
// y-axis min/max labels, an average reference line, and x-axis date captions.
// Hover/tap interaction is wired up separately by setupSparklineHover after
// the SVG has been inserted into the DOM.
// SVG viewBox dimensions — chosen so the natural aspect ratio works well
// on both phones (~360 CSS) and wide laptops (~1000 CSS). Because we let
// preserveAspectRatio default to `xMidYMid meet`, circles stay circles and
// stroke widths stay uniform at every container width.
const SPARKLINE_W = 800;
const SPARKLINE_H = 160;
const SPARKLINE_PAD = { l: 36, r: 14, t: 12, b: 26 };

function renderSparkline(points, avg) {
  if (!points || points.length < 2) return '';
  const w = SPARKLINE_W, h = SPARKLINE_H;
  const padL = SPARKLINE_PAD.l, padR = SPARKLINE_PAD.r;
  const padT = SPARKLINE_PAD.t, padB = SPARKLINE_PAD.b;
  const chartW = w - padL - padR;
  const chartH = h - padT - padB;

  const scores = points.map(p => p.score);
  const max = Math.max(...scores);
  const min = Math.min(...scores);
  const range = Math.max(1, max - min);
  const stepX = chartW / Math.max(1, points.length - 1);

  const xOf = (i) => padL + i * stepX;
  const yOf = (score) => padT + chartH * (1 - (score - min) / range);

  // Main line + area fill.
  const linePath = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'}${xOf(i).toFixed(1)} ${yOf(p.score).toFixed(1)}`)
    .join(' ');
  const areaPath =
    linePath +
    ` L ${xOf(points.length - 1).toFixed(1)} ${(h - padB).toFixed(1)}` +
    ` L ${padL.toFixed(1)} ${(h - padB).toFixed(1)} Z`;

  // Dots — winners emphasised. Radii picked for an 800-wide viewBox.
  const dots = points.map((p, i) => {
    const cx = xOf(i).toFixed(1);
    const cy = yOf(p.score).toFixed(1);
    const cls = p.placement === 1 ? 'spark-dot winner' : 'spark-dot';
    return `<circle cx="${cx}" cy="${cy}" r="${p.placement === 1 ? 5.5 : 4}" class="${cls}" data-i="${i}" />`;
  }).join('');

  // Y-axis: min and max ticks. Include median if the series is tall enough.
  const yLabels = [
    `<text class="spark-yaxis" x="${padL - 4}" y="${(padT + 4).toFixed(1)}" text-anchor="end">${max}</text>`,
    `<text class="spark-yaxis" x="${padL - 4}" y="${(h - padB).toFixed(1)}" text-anchor="end">${min}</text>`,
  ].join('');

  // Average reference line — shown when avg falls within the chart's range.
  let avgLine = '';
  if (avg != null && avg >= min && avg <= max) {
    const yAvg = yOf(avg);
    avgLine = `
      <line class="spark-avg" x1="${padL}" y1="${yAvg.toFixed(1)}" x2="${(w - padR).toFixed(1)}" y2="${yAvg.toFixed(1)}" />
      <text class="spark-yaxis avg" x="${(w - padR - 2).toFixed(1)}" y="${(yAvg - 3).toFixed(1)}" text-anchor="end">avg ${avg.toFixed(1)}</text>
    `;
  }

  // X-axis date captions (first and most recent).
  const xCaption = `
    <text class="spark-xaxis" x="${padL}" y="${(h - 5).toFixed(1)}" text-anchor="start">${formatShortDate(points[0].finished_at)}</text>
    <text class="spark-xaxis" x="${(w - padR).toFixed(1)}" y="${(h - 5).toFixed(1)}" text-anchor="end">${formatShortDate(points[points.length - 1].finished_at)}</text>
  `;

  // Crosshair group — hidden until the user hovers/taps.
  const cursor = `
    <g class="spark-cursor hidden">
      <line class="spark-crosshair" x1="0" y1="${padT}" x2="0" y2="${(h - padB).toFixed(1)}" />
      <circle class="spark-marker" cx="0" cy="0" r="7" />
    </g>
  `;

  return `
    <svg class="sparkline" viewBox="0 0 ${w} ${h}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Score history">
      ${avgLine}
      <path d="${areaPath}" class="spark-fill" />
      <path d="${linePath}" class="spark-line" />
      ${dots}
      ${yLabels}
      ${xCaption}
      ${cursor}
    </svg>
  `;
}

// Attach hover / touch handlers to a sparkline SVG. Updates an info box
// below the chart with the currently-inspected point, and wires the info
// box's "view →" link to open that saved game.
function setupSparklineHover(svg, infoBox, points) {
  if (!svg || !infoBox || !points || points.length < 2) return;
  const cursor = svg.querySelector('.spark-cursor');
  const crosshair = svg.querySelector('.spark-crosshair');
  const marker = svg.querySelector('.spark-marker');
  if (!cursor || !crosshair || !marker) return;

  const w = SPARKLINE_W, h = SPARKLINE_H;
  const padL = SPARKLINE_PAD.l, padR = SPARKLINE_PAD.r;
  const padT = SPARKLINE_PAD.t, padB = SPARKLINE_PAD.b;
  const chartW = w - padL - padR;
  const chartH = h - padT - padB;
  const scores = points.map(p => p.score);
  const max = Math.max(...scores);
  const min = Math.min(...scores);
  const range = Math.max(1, max - min);
  const stepX = chartW / Math.max(1, points.length - 1);

  let activeIdx = -1;

  const update = (clientX) => {
    // Map client X to the SVG's user-space X, accounting for the aspect-ratio
    // preserving sizing: when the container is wider than the viewBox's
    // aspect, the viewBox is centered horizontally with equal side padding.
    const rect = svg.getBoundingClientRect();
    const svgAspect = w / h;
    const boxAspect = rect.width / rect.height;
    let xOffset = 0, visibleW = rect.width;
    if (boxAspect > svgAspect) {
      // Container wider than viewBox ratio → viewBox is letterboxed left/right.
      visibleW = rect.height * svgAspect;
      xOffset = (rect.width - visibleW) / 2;
    }
    const localX = (clientX - rect.left - xOffset) / visibleW;
    const svgX = Math.max(0, Math.min(1, localX)) * w;
    const raw = (svgX - padL) / stepX;
    const idx = Math.max(0, Math.min(points.length - 1, Math.round(raw)));
    if (idx === activeIdx) return;
    activeIdx = idx;
    const p = points[idx];
    const cx = padL + idx * stepX;
    const cy = padT + chartH * (1 - (p.score - min) / range);
    crosshair.setAttribute('x1', cx.toFixed(1));
    crosshair.setAttribute('x2', cx.toFixed(1));
    marker.setAttribute('cx', cx.toFixed(1));
    marker.setAttribute('cy', cy.toFixed(1));
    cursor.classList.remove('hidden');
    renderInfo(p);
  };

  const reset = () => {
    activeIdx = -1;
    cursor.classList.add('hidden');
    renderDefault();
  };

  const renderInfo = (p) => {
    infoBox.innerHTML = `
      <span class="spark-info-score">${p.score}</span>
      <span class="spark-info-meta">${formatDateShort(p.finished_at)} · ${ordinalSuffix(p.placement)} place</span>
      <a class="spark-info-link" data-game-id="${p.game_id}">view →</a>
    `;
    const link = infoBox.querySelector('.spark-info-link');
    if (link) {
      link.addEventListener('click', (e) => {
        e.stopPropagation();
        navigate(`/stats/games/${parseInt(link.dataset.gameId, 10)}`);
      });
    }
  };
  const renderDefault = () => {
    const last = points[points.length - 1];
    infoBox.innerHTML = `
      <span class="spark-info-hint">Hover or tap a point for details</span>
      <span class="spark-info-meta">latest: ${last.score} on ${formatDateShort(last.finished_at)}</span>
    `;
  };
  renderDefault();

  svg.addEventListener('mousemove', (e) => update(e.clientX));
  svg.addEventListener('mouseleave', reset);
  svg.addEventListener('touchstart', (e) => {
    if (e.touches[0]) { update(e.touches[0].clientX); e.preventDefault(); }
  }, { passive: false });
  svg.addEventListener('touchmove', (e) => {
    if (e.touches[0]) { update(e.touches[0].clientX); e.preventDefault(); }
  }, { passive: false });
  // Leave the last-inspected point visible after touchend — tapping elsewhere resets.
}

async function fetchAndRenderPlayerStats(name) {
  const body = document.getElementById('stats-me-body');
  body.innerHTML = '<div class="stats-empty">Loading…</div>';
  try {
    const s = await apiGet(`/api/stats/player/${encodeURIComponent(name)}`);
    if (!s.games_played) {
      body.innerHTML = `<div class="stats-empty">No games found for "${escapeHtml(name)}".</div>`;
      return;
    }

    // ── Header + sparkline ────────────────────────────────────────────
    const since = s.first_game_at
      ? `<div class="me-sub">playing since ${formatShortDate(s.first_game_at)}</div>`
      : '';
    // Only show the "recent form" pill once the player has more games than the
    // recent window (5). Below that, recent_avg == overall avg by definition
    // and the pill would just read "→ 0.0 vs overall".
    let trendBadge = '';
    if (s.recent_avg != null && s.games_played > 5) {
      const diff = s.recent_avg - s.avg_score;
      if (Math.abs(diff) >= 0.1) {
        const cls = diff > 0 ? 'avg-up' : 'avg-down';
        const arrow = diff > 0 ? '↑' : '↓';
        trendBadge = `<span class="hl-badge ${cls}">last 5 avg ${s.recent_avg.toFixed(1)} (${arrow}${Math.abs(diff).toFixed(1)} vs overall ${s.avg_score.toFixed(1)})</span>`;
      }
    }

    // ── Unified stat grid — every card is the same size/style ─────────
    const avg = s.avg_score.toFixed(1);
    const winPct = s.win_rate.toFixed(0) + '%';
    const hsAttr = s.high_score_game_id ? ` data-game-id="${s.high_score_game_id}"` : '';
    const hsClass = s.high_score_game_id ? ' stat-card-link' : '';
    const cards = [];
    cards.push(`<div class="stat-card"><div class="stat-label">Games</div><div class="stat-value">${s.games_played}</div></div>`);
    cards.push(`<div class="stat-card"><div class="stat-label">Win rate</div><div class="stat-value">${winPct}</div><div class="stat-sub">${s.wins} wins</div></div>`);
    cards.push(`<div class="stat-card${hsClass}"${hsAttr}><div class="stat-label">High score</div><div class="stat-value">${s.high_score}</div></div>`);
    cards.push(`<div class="stat-card"><div class="stat-label">Avg</div><div class="stat-value">${avg}</div></div>`);
    if (s.recent_avg != null) {
      cards.push(`<div class="stat-card"><div class="stat-label">Last 5 avg</div><div class="stat-value">${s.recent_avg.toFixed(1)}</div></div>`);
    }
    if (s.longest_win_streak > 0) {
      cards.push(`<div class="stat-card"><div class="stat-label">Win streak</div><div class="stat-value">${s.longest_win_streak}</div></div>`);
    }
    if (s.scoring_rate != null) {
      cards.push(`<div class="stat-card"><div class="stat-label">Cards scored</div><div class="stat-value">${s.scoring_rate.toFixed(0)}%</div></div>`);
    }
    if (s.total_play_time_secs > 0) {
      cards.push(`<div class="stat-card"><div class="stat-label">Play time</div><div class="stat-value">${formatDuration(s.total_play_time_secs)}</div></div>`);
    }
    const primaryHtml = `<div class="stats-summary">${cards.join('')}</div>`;

    // ── Placements chips (1st/2nd/…)  ─────────────────────────────────
    const placementsHtml = s.placements.map((count, i) => {
      if (!count) return '';
      return `<span class="placement-chip">${ordinalSuffix(i+1)}: ${count}</span>`;
    }).join('');

    // ── Sparkline ─────────────────────────────────────────────────────
    const sparkline = s.score_history && s.score_history.length >= 2
      ? `<div class="sparkline-wrap">
           <div class="sparkline-title">Score over time</div>
           ${renderSparkline(s.score_history, s.avg_score)}
           <div class="sparkline-info"></div>
         </div>`
      : '';

    // ── Best single-card play ─────────────────────────────────────────
    let bestCardHtml = '';
    if (s.best_card_score) {
      const b = s.best_card_score;
      const src = b.kind === 'sanctuary' ? sanctuaryImagePath(b.number) : regionImagePath(b.number);
      const kindLabel = b.kind === 'sanctuary' ? 'Sanctuary' : 'Region';
      bestCardHtml = `
        <h3 class="stats-section-title">Biggest single-card play</h3>
        <div class="best-card-row" data-game-id="${b.game_id}">
          <img class="best-card-img ${b.kind === 'sanctuary' ? 'sanctuary' : ''}" src="${src}" alt="${kindLabel} ${b.number}" />
          <div class="best-card-body">
            <div class="best-card-points">+${b.points}</div>
            <div class="best-card-explain">${escapeHtml(b.explanation)}</div>
            <div class="best-card-meta">${kindLabel} #${b.number} · ${formatDateShort(b.finished_at)}</div>
          </div>
        </div>
      `;
    }

    // ── Avg by player count ───────────────────────────────────────────
    let avgByPcHtml = '';
    if (s.avg_by_player_count.length > 0) {
      const maxAvg = Math.max(...s.avg_by_player_count.map(e => e.avg_score)) || 1;
      const rows = s.avg_by_player_count.map(e => {
        const pct = 100 * e.avg_score / maxAvg;
        const label = e.player_count === 1 ? 'Solo' : `${e.player_count}-player`;
        return `
          <div class="bar-row">
            <div class="bar-label">${label}</div>
            <div class="bar-track"><div class="bar-fill" style="width:${pct.toFixed(1)}%"></div></div>
            <div class="bar-value">${e.avg_score.toFixed(1)} <span class="bar-sub">· ${e.games}g</span></div>
          </div>
        `;
      }).join('');
      avgByPcHtml = `<h3 class="stats-section-title">Average by player count</h3><div class="bar-chart">${rows}</div>`;
    }

    // ── Biome section: region-only + sanctuary-only side by side ─────
    const renderBiomeBar = (prefs) => {
      if (!prefs || prefs.length === 0) return '';
      const total = prefs.reduce((sum, b) => sum + b.count, 0) || 1;
      const segs = prefs.map(b => {
        const pct = (100 * b.count / total).toFixed(1);
        return `<div class="biome-seg biome-${b.biome.toLowerCase()}" style="width:${pct}%" title="${b.biome}: ${b.count} (${pct}%)"></div>`;
      }).join('');
      const legend = prefs.map(b => {
        const pct = (100 * b.count / total).toFixed(0);
        return `<span class="biome-chip"><span class="biome-dot biome-${b.biome.toLowerCase()}"></span>${b.biome} ${pct}%</span>`;
      }).join('');
      return `<div class="biome-bar">${segs}</div><div class="biome-legend">${legend}</div>`;
    };
    let biomeSectionHtml = '';
    if (s.biome_preference_regions.length > 0 || s.biome_preference_sanctuaries.length > 0) {
      const regionPart = s.biome_preference_regions.length > 0
        ? `<div class="biome-block">
             <div class="biome-subtitle">Regions played</div>
             ${renderBiomeBar(s.biome_preference_regions)}
           </div>`
        : '';
      const sanctPart = s.biome_preference_sanctuaries.length > 0
        ? `<div class="biome-block">
             <div class="biome-subtitle">Sanctuaries kept</div>
             ${renderBiomeBar(s.biome_preference_sanctuaries)}
           </div>`
        : '';
      biomeSectionHtml = `
        <h3 class="stats-section-title">Biome preference</h3>
        <div class="biome-grid">${regionPart}${sanctPart}</div>
      `;
    }

    // ── Top region cards ─────────────────────────────────────────────
    let topCardsHtml = '';
    if (s.top_cards.length > 0) {
      const cards = s.top_cards.map(tc => `
        <div class="topcard">
          <img src="${regionImagePath(tc.number)}" alt="#${tc.number}" />
          <div class="topcard-meta">#${tc.number} · ${tc.times_played}×</div>
        </div>
      `).join('');
      topCardsHtml = `<h3 class="stats-section-title">Most played regions</h3><div class="topcards">${cards}</div>`;
    }

    // ── Sanctuary stats section ──────────────────────────────────────
    let sanctuarySectionHtml = '';
    const hasSanctuaryData =
      s.avg_sanctuaries_per_game > 0 ||
      s.sanctuary_scoring_rate != null ||
      s.best_sanctuary_score ||
      s.top_sanctuaries.length > 0 ||
      s.avg_by_sanctuary_count.length > 0;
    if (hasSanctuaryData) {
      const sCards = [];
      sCards.push(`<div class="stat-card"><div class="stat-label">Avg kept</div><div class="stat-value">${s.avg_sanctuaries_per_game.toFixed(1)}</div><div class="stat-sub">per game</div></div>`);
      if (s.sanctuary_scoring_rate != null) {
        sCards.push(`<div class="stat-card"><div class="stat-label">Sanct. scored</div><div class="stat-value">${s.sanctuary_scoring_rate.toFixed(0)}%</div></div>`);
      }
      if (s.best_sanctuary_score) {
        const b = s.best_sanctuary_score;
        sCards.push(`<div class="stat-card stat-card-link" data-game-id="${b.game_id}"><div class="stat-label">Best sanctuary</div><div class="stat-value">+${b.points}</div><div class="stat-sub">Tile #${b.number}</div></div>`);
      }

      let bestSanctRowHtml = '';
      if (s.best_sanctuary_score) {
        const b = s.best_sanctuary_score;
        bestSanctRowHtml = `
          <div class="best-card-row" data-game-id="${b.game_id}">
            <img class="best-card-img sanctuary" src="${sanctuaryImagePath(b.number)}" alt="Sanctuary ${b.number}" />
            <div class="best-card-body">
              <div class="best-card-points">+${b.points}</div>
              <div class="best-card-explain">${escapeHtml(b.explanation)}</div>
              <div class="best-card-meta">Tile #${b.number} · ${formatDateShort(b.finished_at)}</div>
            </div>
          </div>
        `;
      }

      let avgBySanctHtml = '';
      if (s.avg_by_sanctuary_count.length > 0) {
        const maxAvg = Math.max(...s.avg_by_sanctuary_count.map(e => e.avg_score)) || 1;
        const rows = s.avg_by_sanctuary_count.map(e => {
          const pct = 100 * e.avg_score / maxAvg;
          const label = e.sanctuary_count === 1 ? '1 sanctuary' : `${e.sanctuary_count} sanctuaries`;
          return `
            <div class="bar-row">
              <div class="bar-label">${label}</div>
              <div class="bar-track"><div class="bar-fill" style="width:${pct.toFixed(1)}%"></div></div>
              <div class="bar-value">${e.avg_score.toFixed(1)} <span class="bar-sub">· ${e.games}g</span></div>
            </div>
          `;
        }).join('');
        avgBySanctHtml = `<div class="subsection-title">Average score by sanctuaries kept</div><div class="bar-chart">${rows}</div>`;
      }

      let topSanctHtml = '';
      if (s.top_sanctuaries.length > 0) {
        const tcards = s.top_sanctuaries.map(tc => `
          <div class="topcard">
            <img class="sanctuary" src="${sanctuaryImagePath(tc.number)}" alt="Sanctuary ${tc.number}" />
            <div class="topcard-meta">#${tc.number} · ${tc.times_played}×</div>
          </div>
        `).join('');
        topSanctHtml = `<div class="subsection-title">Most-kept sanctuaries</div><div class="topcards">${tcards}</div>`;
      }

      sanctuarySectionHtml = `
        <h3 class="stats-section-title">Sanctuary stats</h3>
        <div class="stats-summary">${sCards.join('')}</div>
        ${bestSanctRowHtml}
        ${avgBySanctHtml}
        ${topSanctHtml}
      `;
    }

    // ── Head to head ─────────────────────────────────────────────────
    let h2hHtml = '';
    if (s.head_to_head.length > 0) {
      const rows = s.head_to_head.map(h => `
        <div class="h2h-row">
          <div class="h2h-name">${escapeHtml(h.name)}</div>
          <div class="h2h-record">
            <span class="h2h-w">${h.wins}W</span>
            <span class="h2h-l">${h.losses}L</span>
            ${h.ties > 0 ? `<span class="h2h-t">${h.ties}T</span>` : ''}
          </div>
          <div class="h2h-games">${h.games}g</div>
        </div>
      `).join('');
      h2hHtml = `<h3 class="stats-section-title">Head to head</h3><div class="h2h-list">${rows}</div>`;
    }

    // ── Recent games list ─────────────────────────────────────────────
    const recentHtml = s.recent.length === 0 ? '' : `
      <h3 class="stats-section-title">Recent games</h3>
      <div class="game-list">
        ${s.recent.map(r => `
          <div class="game-row" data-game-id="${r.game_id}">
            <div class="game-row-main">
              <div class="game-row-title">${ordinalSuffix(r.placement)} place <span class="game-row-sub">· ${r.player_count}p</span></div>
              <div class="game-row-meta">${formatDateShort(r.finished_at)}</div>
            </div>
            <div class="game-row-score">${r.score}</div>
          </div>
        `).join('')}
      </div>
    `;

    body.innerHTML = `
      <div class="me-header">
        <div class="me-name">${escapeHtml(s.name)}</div>
        ${since}
        ${trendBadge ? `<div class="me-trend">${trendBadge}</div>` : ''}
      </div>
      ${sparkline}
      ${primaryHtml}
      ${placementsHtml ? `<div class="placements-row">${placementsHtml}</div>` : ''}
      ${bestCardHtml}
      ${avgByPcHtml}
      ${biomeSectionHtml}
      ${topCardsHtml}
      ${sanctuarySectionHtml}
      ${h2hHtml}
      ${recentHtml}
    `;

    // Wire up every element with data-game-id to navigate to that saved game.
    body.querySelectorAll('[data-game-id]').forEach(el => {
      el.style.cursor = 'pointer';
      el.addEventListener('click', () => {
        const id = parseInt(el.dataset.gameId, 10);
        if (!isNaN(id)) navigate(`/stats/games/${id}`);
      });
    });

    // Wire up sparkline hover/tap mechanics (if the chart is rendered).
    const sparkSvg = body.querySelector('.sparkline');
    const sparkInfo = body.querySelector('.sparkline-info');
    if (sparkSvg && sparkInfo && s.score_history.length >= 2) {
      setupSparklineHover(sparkSvg, sparkInfo, s.score_history);
    }
  } catch (err) {
    body.innerHTML = `<div class="stats-empty">Error loading stats: ${escapeHtml(String(err))}</div>`;
  }
}

async function renderHighScoresPanel() {
  const params = new URLSearchParams(location.search);
  const playerFilter = (params.get('player') || '').trim();

  statsPanelHs.innerHTML = '<div class="stats-empty">Loading…</div>';
  try {
    let qs = 'limit=20';
    if (playerFilter) qs += '&player=' + encodeURIComponent(playerFilter);
    const rows = await apiGet('/api/stats/leaderboard?' + qs);

    statsPanelHs.innerHTML = '';
    const search = document.createElement('div');
    search.className = 'stats-search hs-player-filter';
    search.innerHTML = `
      <label for="hs-player-input">Filter by player</label>
      <div class="stats-search-row hs-player-filter-row">
        <input type="text" id="hs-player-input" value="${escapeHtml(playerFilter)}" placeholder="All players" autocomplete="off" />
        <button type="button" id="hs-player-apply">Apply</button>
        <button type="button" id="hs-player-clear" class="secondary">Clear</button>
      </div>
    `;
    statsPanelHs.appendChild(search);

    const input = document.getElementById('hs-player-input');
    const applyFilter = () => navigate(statsHighScoresUrl(input.value));
    document.getElementById('hs-player-apply').addEventListener('click', applyFilter);
    document.getElementById('hs-player-clear').addEventListener('click', () => navigate(statsHighScoresUrl('')));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); applyFilter(); }
    });

    if (rows.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'stats-empty';
      empty.textContent = playerFilter
        ? `No saved games for "${playerFilter}".`
        : 'No games saved yet. Play one!';
      statsPanelHs.appendChild(empty);
      return;
    }

    const heading = document.createElement('h3');
    heading.className = 'stats-section-title';
    heading.textContent = playerFilter
      ? `High scores — ${playerFilter}`
      : 'All-time high scores';
    statsPanelHs.appendChild(heading);

    const list = document.createElement('div');
    list.className = 'hs-list';
    statsPanelHs.appendChild(list);

    for (const r of rows) {
      const breakdown = Array.isArray(r.score_breakdown) ? r.score_breakdown : [];
      // Breakdown entries are stored right-to-left: region entry for tableau
      // index i lives at regionEntries.length - 1 - i.
      const regionEntries = breakdown.filter(e => e.kind === 'region');
      const sanctuaryEntries = breakdown.filter(e => e.kind === 'sanctuary');

      const entry = document.createElement('div');
      entry.className = 'hs-entry';
      entry.dataset.gameId = r.game_id;

      const header = document.createElement('div');
      header.className = 'hs-header';
      header.innerHTML = `
        <div class="hs-rank">#${r.rank}</div>
        <div class="hs-identity">
          <div class="hs-name">${escapeHtml(r.name)}</div>
          <div class="hs-meta">${formatDateShort(r.finished_at)} · ${r.player_count}p</div>
        </div>
        <div class="hs-score">${r.score}</div>
      `;
      entry.appendChild(header);

      // Tableau — reuse the same scoring-card helpers as the game-detail view.
      const tableauRow = document.createElement('div');
      tableauRow.className = 'my-tableau-row';
      const tableau = document.createElement('div');
      tableau.className = 'my-tableau';
      r.region_cards.forEach((num, i) => {
        const e = regionEntries[regionEntries.length - 1 - i];
        tableau.appendChild(makeScoringRegionCard({ number: num }, e, { revealed: true }));
      });
      tableauRow.appendChild(tableau);
      entry.appendChild(tableauRow);

      if (r.sanctuary_cards.length > 0) {
        const sanctRow = document.createElement('div');
        sanctRow.className = 'my-sanctuaries-row';
        const sancts = document.createElement('div');
        sancts.className = 'my-sanctuaries';
        r.sanctuary_cards.forEach((num, i) => {
          sancts.appendChild(makeScoringSanctuaryCard({ tile: num }, sanctuaryEntries[i], { scored: true }));
        });
        sanctRow.appendChild(sancts);
        entry.appendChild(sanctRow);
      }

      // Clicking the HEADER opens the game detail; clicking individual cards
      // only toggles the score tooltip (handled inside the card factories).
      header.style.cursor = 'pointer';
      header.addEventListener('click', () => navigate(`/stats/games/${r.game_id}`));
      list.appendChild(entry);
    }
  } catch (err) {
    statsPanelHs.innerHTML = `<div class="stats-empty">Error: ${escapeHtml(String(err))}</div>`;
  }
}

async function renderRecentGamesPanel() {
  statsPanelRecent.innerHTML = '<div class="stats-empty">Loading…</div>';
  try {
    const rows = await apiGet('/api/stats/games?limit=30');
    if (rows.length === 0) {
      statsPanelRecent.innerHTML = '<div class="stats-empty">No games saved yet.</div>';
      return;
    }
    const html = `
      <h3 class="stats-section-title">Recent games</h3>
      <div class="game-list">
        ${rows.map(r => `
          <div class="game-row" data-game-id="${r.game_id}">
            <div class="game-row-main">
              <div class="game-row-title">${escapeHtml(r.winner_name)} <span class="game-row-sub">· ${r.player_count}p</span></div>
              <div class="game-row-meta">${formatDateShort(r.finished_at)}</div>
            </div>
            <div class="game-row-score">${r.winner_score}</div>
          </div>
        `).join('')}
      </div>
    `;
    statsPanelRecent.innerHTML = html;
    statsPanelRecent.querySelectorAll('.game-row').forEach(el => {
      el.addEventListener('click', () => navigate(`/stats/games/${parseInt(el.dataset.gameId, 10)}`));
    });
  } catch (err) {
    statsPanelRecent.innerHTML = `<div class="stats-empty">Error: ${escapeHtml(String(err))}</div>`;
  }
}

// ── Saved-game detail view ──────────────────────────────────────────────────

async function loadAndRenderGameDetail(gameId) {
  gameDetailBody.innerHTML = '<div class="stats-empty">Loading…</div>';
  gameDetailTitle.textContent = `Game #${gameId}`;
  try {
    const g = await apiGet(`/api/stats/games/${gameId}`);
    if (!g) {
      gameDetailBody.innerHTML = '<div class="stats-empty">Game not found.</div>';
      return;
    }
    renderGameDetail(g);
  } catch (err) {
    gameDetailBody.innerHTML = `<div class="stats-empty">Error: ${escapeHtml(String(err))}</div>`;
  }
}

function renderGameDetail(g) {
  const sortedPlayers = [...g.players].sort((a, b) => a.placement - b.placement || a.seat - b.seat);
  gameDetailTitle.textContent = `Game #${g.game_id}`;

  gameDetailBody.innerHTML = '';

  const meta = document.createElement('div');
  meta.className = 'game-detail-meta';
  const bits = [
    formatDate(g.finished_at),
    `${g.player_count} player${g.player_count === 1 ? '' : 's'}`,
  ];
  if (g.advanced) bits.push('Advanced');
  if (g.expansion) bits.push('Expansion');
  meta.textContent = bits.join(' · ');
  gameDetailBody.appendChild(meta);

  for (const p of sortedPlayers) {
    const breakdown = Array.isArray(p.score_breakdown) ? p.score_breakdown : [];
    // Breakdown entries are stored right-to-left. Region entry for tableau
    // index i lives at regionEntries.length - 1 - i.
    const regionEntries = breakdown.filter(e => e.kind === 'region');
    const sanctuaryEntries = breakdown.filter(e => e.kind === 'sanctuary');

    const panel = document.createElement('div');
    panel.className = 'gd-player';

    const header = document.createElement('div');
    header.className = 'gd-player-header';
    header.innerHTML = `
      <span class="gd-placement">${ordinalSuffix(p.placement)}</span>
      <span class="gd-name">${escapeHtml(p.name)}</span>
      <span class="gd-score">${p.final_score} fame</span>
    `;
    panel.appendChild(header);

    // Mirror the live-game scoring row structure so the exact same CSS applies:
    //   <div class="my-tableau-row"><div class="my-tableau">…cards…</div></div>
    //   <div class="my-sanctuaries-row"><div class="my-sanctuaries">…cards…</div></div>
    const tableauRow = document.createElement('div');
    tableauRow.className = 'my-tableau-row';
    const tableau = document.createElement('div');
    tableau.className = 'my-tableau';
    p.region_cards.forEach((num, i) => {
      const entry = regionEntries[regionEntries.length - 1 - i];
      tableau.appendChild(makeScoringRegionCard({ number: num }, entry, { revealed: true }));
    });
    tableauRow.appendChild(tableau);
    panel.appendChild(tableauRow);

    if (p.sanctuary_cards.length > 0) {
      const sanctRow = document.createElement('div');
      sanctRow.className = 'my-sanctuaries-row';
      const sancts = document.createElement('div');
      sancts.className = 'my-sanctuaries';
      p.sanctuary_cards.forEach((num, i) => {
        sancts.appendChild(makeScoringSanctuaryCard({ tile: num }, sanctuaryEntries[i], { scored: true }));
      });
      sanctRow.appendChild(sancts);
      panel.appendChild(sanctRow);
    }

    gameDetailBody.appendChild(panel);
  }
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  })[c]);
}
function escapeAttr(s) { return escapeHtml(s); }

statsBtn.addEventListener('click', () => navigate('/stats'));
statsCloseBtn.addEventListener('click', () => navigate('/'));
gameDetailClose.addEventListener('click', () => navigate('/stats'));
statsTabs.forEach(b => b.addEventListener('click', () => {
  const tab = b.dataset.tab;
  const path = tab === 'me' ? '/stats/me'
    : tab === 'highscores' ? '/stats/high-scores'
    : '/stats/recent';
  navigate(path);
}));

// Restore saved name from localStorage
const savedName = localStorage.getItem('yonder-player-name');
if (savedName) playerNameEl.value = savedName;

// Save name on change
playerNameEl.addEventListener('input', () => {
  localStorage.setItem('yonder-player-name', playerNameEl.value.trim());
});

// Create game (or join via share link — overridden below if hash present)
let createAction = () => connect(generateCode());
createBtn.addEventListener('click', () => createAction());

// Initial routing: path takes priority over hash. Only the root path honours
// the `#ROOM/Name` hash-based auto-connect.
const initialRoute = parseRoute(location.pathname);
let autoConnecting = false;

if (initialRoute.view === 'lobby') {
  const hash = location.hash.slice(1);
  if (hash) {
    const parts = hash.split('/');
    const hashCode = parts[0] ? decodeURIComponent(parts[0]) : '';
    const hashName = parts[1] ? decodeURIComponent(parts[1]) : '';
    if (hashName) playerNameEl.value = hashName;
    if (hashCode && hashName) {
      autoConnecting = true;
      connect(hashCode);
    } else if (hashCode) {
      lobbyStatus.textContent = `Enter your name to join room ${hashCode.toUpperCase()}.`;
      createBtn.textContent = `Join ${hashCode.toUpperCase()}`;
      createAction = () => connect(hashCode);
    }
  }
  if (!autoConnecting) connectLobby();
} else {
  // Stats / game-detail route: render the right view and start lobby WS
  // in the background so navigating back to "/" shows fresh rooms.
  applyRoute();
  connectLobby();
}
