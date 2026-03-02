// ── Global state ────────────────────────────────────────────────────────────

let ws = null;
let state = null;   // latest ClientGameState from server
let mySeat = null;

// ── DOM refs ─────────────────────────────────────────────────────────────────

const lobby          = document.getElementById('lobby');
const playerNameEl   = document.getElementById('player-name');
const roomNameEl     = document.getElementById('room-name');
const connectBtn     = document.getElementById('connect-btn');
const lobbyStatus    = document.getElementById('lobby-status');

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

const sanctuaryModal   = document.getElementById('sanctuary-modal');
const sanctuaryChoices = document.getElementById('sanctuary-choices');

const gameOverOverlay = document.getElementById('game-over-overlay');
const scoresList      = document.getElementById('scores-list');
const playAgainBtn    = document.getElementById('play-again-btn');

// ── Connection ───────────────────────────────────────────────────────────────

function connect() {
  const playerName = playerNameEl.value.trim();
  const roomName   = roomNameEl.value.trim();
  if (!playerName || !roomName) {
    lobbyStatus.textContent = 'Enter your name and a room name.';
    return;
  }

  lobbyStatus.textContent = 'Connecting…';
  connectBtn.disabled = true;

  const serverBase = `ws://${location.hostname || 'localhost'}:8000`;
  const url = `${serverBase}/game/${encodeURIComponent(roomName)}?player=${encodeURIComponent(playerName)}`;

  ws = new WebSocket(url);

  ws.addEventListener('open', () => {
    lobbyStatus.textContent = 'Connected. Waiting for state…';
  });

  ws.addEventListener('message', (event) => {
    const data = JSON.parse(event.data);
    if (data.Err) {
      lobbyStatus.textContent = `Error: ${data.Err}`;
      connectBtn.disabled = false;
      return;
    }
    lobby.classList.add('hidden');
    gameBoard.classList.remove('hidden');
    state = data;
    mySeat = state.my_seat;
    render();
  });

  ws.addEventListener('close', () => {
    if (state && state.phase !== 'game_over') {
      showStatus('Disconnected from server.');
    }
  });

  ws.addEventListener('error', () => {
    lobbyStatus.textContent = 'Connection failed. Is the server running?';
    connectBtn.disabled = false;
  });
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

  renderStatusBar();
  renderOpponents();
  renderMarket();
  renderMyArea();
  renderSanctuaryModal();
  renderGameOver();
}

// ── Status bar ───────────────────────────────────────────────────────────────

function renderStatusBar() {
  const phase = state.phase;
  statusRound.textContent = state.round > 0 ? `Round ${state.round}/8` : '';
  statusDeck.textContent = `Deck: ${state.deck_size}`;
  statusSanctDeck.textContent = `Sanctuary deck: ${state.sanctuary_deck_size}`;

  if (phase === 'waiting_for_players') {
    const joined = state.players.length;
    const needed = state.player_count;
    statusPhase.textContent = `Waiting for players… (${joined}/${needed})${mySeat === 0 ? ' — click Start Game when ready' : ''}`;
  } else if (phase === 'choosing_cards') {
    const waiting = state.players.filter(p => !p.played_this_round).map(p => p.name);
    if (waiting.includes(myName())) {
      statusPhase.textContent = 'Choose a card from your hand to play.';
    } else {
      statusPhase.textContent = `Waiting for: ${waiting.join(', ')}`;
    }
  } else if (phase === 'sanctuary_choice') {
    if (state.sanctuary_choices) {
      statusPhase.textContent = 'You found a Sanctuary! Choose one to keep.';
    } else {
      const active = state.players.find(p => p.seat !== mySeat);
      statusPhase.textContent = `${active ? active.name : 'Opponent'} is choosing a sanctuary…`;
    }
  } else if (phase === 'drafting') {
    const drafter = state.current_drafter;
    if (drafter === mySeat) {
      statusPhase.textContent = 'Your turn to draft — pick a card from the market.';
    } else {
      const drafter_name = state.players.find(p => p.seat === drafter)?.name ?? '?';
      statusPhase.textContent = `${drafter_name} is drafting…`;
    }
  } else if (phase === 'game_over') {
    statusPhase.textContent = 'Game over!';
  }
}

// ── Opponents ────────────────────────────────────────────────────────────────

function renderOpponents() {
  opponentsArea.innerHTML = '';
  for (const p of state.players) {
    if (p.seat === mySeat) continue;

    const panel = document.createElement('div');
    panel.className = 'opponent-panel';

    const nameEl = document.createElement('div');
    nameEl.className = 'opponent-name';
    nameEl.textContent = p.name;
    panel.appendChild(nameEl);

    // Tableau
    const tableau = document.createElement('div');
    tableau.className = 'opponent-tableau';
    for (const card of p.tableau) {
      tableau.appendChild(regionCardEl(card, 'sm', false));
    }
    // Played-this-round placeholder
    if (p.played_this_round && state.phase === 'choosing_cards') {
      const ph = document.createElement('div');
      ph.className = 'card sm played-overlay';
      ph.innerHTML = '<img src="region/tile000.jpg" alt="face-down">';
      tableau.appendChild(ph);
    }
    panel.appendChild(tableau);

    // Sanctuaries
    if (p.sanctuaries.length > 0) {
      const sancts = document.createElement('div');
      sancts.className = 'opponent-sanctuaries';
      for (const s of p.sanctuaries) {
        sancts.appendChild(sanctuaryCardEl(s, 'sm'));
      }
      panel.appendChild(sancts);
    }

    // Meta
    const meta = document.createElement('div');
    meta.className = 'opponent-meta';
    meta.textContent = `Hand: ${p.hand_size}`;
    panel.appendChild(meta);

    opponentsArea.appendChild(panel);
  }
}

// ── Market ───────────────────────────────────────────────────────────────────

function renderMarket() {
  marketCards.innerHTML = '';
  const isDrafting = state.phase === 'drafting' && state.current_drafter === mySeat;

  state.market.forEach((card, idx) => {
    const el = regionCardEl(card, 'md', isDrafting);
    if (isDrafting) {
      el.addEventListener('click', () => send({ action: 'DraftCard', market_index: idx }));
    }
    marketCards.appendChild(el);
  });
}

// ── My area ───────────────────────────────────────────────────────────────────

function renderMyArea() {
  // Tableau
  myTableau.innerHTML = '';
  const me = state.players.find(p => p.seat === mySeat);
  if (me) {
    for (const card of me.tableau) {
      myTableau.appendChild(regionCardEl(card, 'md', false));
    }
  }

  // Sanctuaries
  mySanctuaries.innerHTML = '';
  if (me) {
    for (const s of me.sanctuaries) {
      mySanctuaries.appendChild(sanctuaryCardEl(s, 'md'));
    }
  }

  // Hand
  myHand.innerHTML = '';
  const canPlay = state.phase === 'choosing_cards' && !(me && me.played_this_round);

  state.my_hand.forEach((card, idx) => {
    const el = regionCardEl(card, 'lg', canPlay);
    if (canPlay) {
      el.addEventListener('click', () => send({ action: 'PlayCard', card_index: idx }));
    }
    myHand.appendChild(el);
  });

  // If waiting for players and I'm seat 0, show a Start Game button.
  // (Server will reject if not enough players have joined yet.)
  const startBtnId = 'start-game-btn';
  document.getElementById(startBtnId)?.remove();
  if (state.phase === 'waiting_for_players' && mySeat === 0) {
    const btn = document.createElement('button');
    btn.id = startBtnId;
    btn.textContent = 'Start Game';
    btn.style.cssText = 'padding:0.5rem 1.2rem;background:#c9a84c;color:#1a1a2e;border:none;border-radius:6px;font-weight:700;cursor:pointer;margin-left:1rem;';
    btn.addEventListener('click', () => send({ action: 'StartGame' }));
    myHand.appendChild(btn);
  }
}

// ── Sanctuary modal ───────────────────────────────────────────────────────────

function renderSanctuaryModal() {
  if (state.phase === 'sanctuary_choice' && state.sanctuary_choices) {
    sanctuaryModal.classList.remove('hidden');
    sanctuaryChoices.innerHTML = '';
    state.sanctuary_choices.forEach((card, idx) => {
      const el = document.createElement('div');
      el.className = 'sanctuary-choice-card';
      const img = document.createElement('img');
      img.src = sanctuaryImagePath(card.tile);
      img.alt = `Sanctuary ${card.tile}`;
      el.appendChild(img);
      el.addEventListener('click', () => send({ action: 'ChooseSanctuary', sanctuary_index: idx }));
      sanctuaryChoices.appendChild(el);
    });
  } else {
    sanctuaryModal.classList.add('hidden');
  }
}

// ── Game over ─────────────────────────────────────────────────────────────────

function renderGameOver() {
  if (state.phase !== 'game_over' || !state.scores) {
    gameOverOverlay.classList.add('hidden');
    return;
  }

  gameOverOverlay.classList.remove('hidden');
  scoresList.innerHTML = '';

  // Sort by score desc, tiebreaker asc
  const sorted = [...state.scores].sort((a, b) => {
    if (b.total !== a.total) return b.total - a.total;
    return a.card_number_sum - b.card_number_sum;
  });

  sorted.forEach((s, i) => {
    const row = document.createElement('div');
    row.className = 'score-row' + (i === 0 ? ' winner' : '');
    row.innerHTML = `
      <span>${i === 0 ? '🏆 ' : ''}${s.name}</span>
      <span>${s.total} fame <span class="tiebreaker">(sum: ${s.card_number_sum})</span></span>
    `;
    scoresList.appendChild(row);
  });
}

// ── Card helpers ──────────────────────────────────────────────────────────────

function regionCardEl(card, size, clickable) {
  const el = document.createElement('div');
  el.className = `card ${size}` + (clickable ? ' playable' : '');
  const img = document.createElement('img');
  img.src = regionImagePath(card.number);
  img.alt = `Region ${card.number}`;
  el.appendChild(img);
  return el;
}

function sanctuaryCardEl(card, size) {
  const el = document.createElement('div');
  el.className = `card ${size}`;
  const img = document.createElement('img');
  img.src = sanctuaryImagePath(card.tile);
  img.alt = `Sanctuary ${card.tile}`;
  el.appendChild(img);
  return el;
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

connectBtn.addEventListener('click', connect);
playerNameEl.addEventListener('keydown', e => { if (e.key === 'Enter') connect(); });
roomNameEl.addEventListener('keydown', e => { if (e.key === 'Enter') connect(); });
playAgainBtn.addEventListener('click', () => location.reload());

// Pre-fill from URL hash if present (e.g. #room1/Alice)
const hash = location.hash.slice(1);
if (hash) {
  const parts = hash.split('/');
  if (parts[0]) roomNameEl.value = decodeURIComponent(parts[0]);
  if (parts[1]) playerNameEl.value = decodeURIComponent(parts[1]);
}
