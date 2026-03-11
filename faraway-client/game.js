// ── Global state ────────────────────────────────────────────────────────────

let ws = null;
let state = null;   // latest ClientGameState from server
let mySeat = null;
let scoringRevealIndex = 0;  // 0 = not started, increments on click (N = N cards revealed)

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

const advancedCheckbox   = document.getElementById('advanced-variant');
const advancedModal      = document.getElementById('advanced-modal');
const advancedChoicesEl  = document.getElementById('advanced-choices');
const advancedConfirmBtn = document.getElementById('advanced-confirm-btn');

const sanctuaryModal   = document.getElementById('sanctuary-modal');
const sanctuaryChoices = document.getElementById('sanctuary-choices');


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
    // Join error: server sends a plain string like "GameAlreadyStarted".
    if (typeof data === 'string') {
      const friendly = {
        GameAlreadyStarted: 'That game has already started. Ask a player for the correct name.',
        RoomFull: 'That room is full.',
      };
      lobbyStatus.textContent = friendly[data] || `Error: ${data}`;
      connectBtn.disabled = false;
      location.hash = '';
      ws.close();
      return;
    }
    // Action error during gameplay.
    if (data.Err) {
      lobbyStatus.textContent = `Error: ${data.Err}`;
      connectBtn.disabled = false;
      return;
    }
    lobby.classList.add('hidden');
    gameBoard.classList.remove('hidden');
    state = data;
    mySeat = state.my_seat;
    // Persist room/name in URL hash so refresh reconnects.
    location.hash = `${encodeURIComponent(roomName)}/${encodeURIComponent(playerName)}`;
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
  renderAdvancedSetupModal();
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
    const msg = joined < 2
      ? `Waiting for players… (${joined} joined, need at least 2)`
      : `${joined} players joined${mySeat === 0 ? ' — click Start Game when ready' : ' — waiting for host to start'}`;
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
  } else if (phase === 'sanctuary_choice') {
    if (state.sanctuary_choices) {
      statusPhase.textContent = 'You found a Sanctuary! Choose one to keep.';
    } else {
      statusPhase.textContent = 'Waiting for other players to choose a sanctuary…';
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
      tableau.appendChild(regionCardEl(card, 'sm', false, true));
    }
    // Played-this-round placeholder
    if (p.played_this_round && state.phase === 'choosing_cards') {
      const ph = document.createElement('div');
      ph.className = 'card sm played-overlay';
      ph.innerHTML = '<img src="region/card-back.png" alt="face-down">';
      tableau.appendChild(ph);
    }
    panel.appendChild(tableau);

    // Sanctuaries
    if (p.sanctuaries.length > 0) {
      const sancts = document.createElement('div');
      sancts.className = 'opponent-sanctuaries';
      for (const s of p.sanctuaries) {
        sancts.appendChild(sanctuaryCardEl(s, 'sm', true));
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

  // Tableau (with live score badges)
  myTableau.innerHTML = '';
  const me = state.players.find(p => p.seat === mySeat);
  const liveRegionEntries = state.my_score_detail
    ? state.my_score_detail.filter(e => e.kind === 'region')
    : [];
  if (me) {
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
    // Show face-down placeholder immediately after playing a card
    if (me.played_this_round && state.phase === 'choosing_cards') {
      const ph = document.createElement('div');
      ph.className = 'card xl played-overlay';
      ph.innerHTML = '<img src="region/card-back.png" alt="face-down">';
      myTableau.appendChild(ph);
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

  // If waiting for players and I'm seat 0, show a Start Game button.
  // (Server will reject if not enough players have joined yet.)
  const startBtnId = 'start-game-btn';
  document.getElementById(startBtnId)?.remove();
  if (state.phase === 'waiting_for_players' && mySeat === 0) {
    const btn = document.createElement('button');
    btn.id = startBtnId;
    btn.textContent = 'Start Game';
    btn.style.cssText = 'padding:0.5rem 1.2rem;background:#c9a84c;color:#1a1a2e;border:none;border-radius:6px;font-weight:700;cursor:pointer;margin-left:1rem;';
    btn.addEventListener('click', () => send({ action: 'StartGame', advanced: advancedCheckbox.checked }));
    myHand.appendChild(btn);
  }
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
  advancedChoicesEl.innerHTML = '';
  advancedSelected.clear();
  updateAdvancedConfirmBtn();

  state.advanced_setup_choices.forEach((card, idx) => {
    const el = regionCardEl(card, 'lg', true);
    el.dataset.idx = idx;
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

// ── Game over (inline scoring) ────────────────────────────────────────────────

function renderGameOver() {
  const isGameOver = state.phase === 'game_over' && state.scores;
  const scoringBar = document.getElementById('scoring-bar');

  if (!isGameOver) {
    scoringRevealIndex = 0;
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

  const detail = state.my_score_detail;
  if (!detail) return;

  // Separate region entries (first 8) from sanctuary entries
  const regionEntries = detail.filter(e => e.kind === 'region');
  const sanctuaryEntries = detail.filter(e => e.kind === 'sanctuary');

  // --- Render tableau cards as face-down/revealed in place ---
  myTableau.innerHTML = '';
  const me = state.players.find(p => p.seat === mySeat);
  if (!me) return;

  // Region entries are right-to-left (index 7 first). Map them back to tableau order.
  // Tableau order is left-to-right (index 0=first played, 7=last played).
  // Score detail has index 0 = card 8 (rightmost), index 7 = card 1 (leftmost).
  // So tableau card i corresponds to regionEntries[7 - i].
  for (let i = 0; i < me.tableau.length; i++) {
    const card = me.tableau[i];
    const detailIdx = regionEntries.length - 1 - i;
    // Reveal order: rightmost card (i=7) is reveal 0, next (i=6) is reveal 1, etc.
    const revealOrder = me.tableau.length - 1 - i;
    const revealed = revealOrder < scoringRevealIndex;

    const el = document.createElement('div');
    el.className = 'card xl scoring-card-slot';
    if (revealed && detailIdx >= 0) {
      // Show face-up with score badge
      el.classList.add('scoring-revealed');
      const img = document.createElement('img');
      img.src = regionImagePath(card.number);
      img.alt = `Region ${card.number}`;
      el.appendChild(img);
      const entry = regionEntries[detailIdx];
      const badge = document.createElement('div');
      badge.className = 'score-badge' + (entry.points > 0 ? ' positive' : ' zero');
      badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
      el.appendChild(badge);
      // Highlight the just-revealed card
      if (revealOrder === scoringRevealIndex - 1) {
        el.classList.add('just-revealed');
      }
    } else {
      // Face down
      el.classList.add('face-down');
      const img = document.createElement('img');
      img.src = 'region/card-back.png';
      img.alt = 'Face down';
      el.appendChild(img);
    }
    myTableau.appendChild(el);
  }

  // --- Sanctuaries: always visible, score badges appear after region cards ---
  mySanctuaries.innerHTML = '';
  const sanctuariesScored = scoringRevealIndex > regionEntries.length;

  for (let i = 0; i < me.sanctuaries.length; i++) {
    const s = me.sanctuaries[i];
    const el = document.createElement('div');
    el.className = 'card sanctuary md';
    const img = document.createElement('img');
    img.src = sanctuaryImagePath(s.tile);
    img.alt = `Sanctuary ${s.tile}`;
    el.appendChild(img);
    if (sanctuariesScored && sanctuaryEntries[i]) {
      const entry = sanctuaryEntries[i];
      const badge = document.createElement('div');
      badge.className = 'score-badge-sm' + (entry.points > 0 ? ' positive' : ' zero');
      badge.textContent = entry.points > 0 ? `+${entry.points}` : '0';
      el.appendChild(badge);
    }
    mySanctuaries.appendChild(el);
  }

  // --- Scoring info bar / leaderboard (below sanctuaries) ---
  const runningTotal = computeRunningTotal(regionEntries, sanctuaryEntries, scoringRevealIndex);
  const totalRevealSteps = regionEntries.length + (sanctuaryEntries.length > 0 ? 1 : 0);
  const allDone = scoringRevealIndex > totalRevealSteps;

  // Clean up whichever element we're not using
  if (allDone) {
    document.getElementById('scoring-bar')?.remove();
    renderInlineLeaderboard();
  } else {
    document.getElementById('scoring-leaderboard')?.remove();
    renderScoringBar(regionEntries, sanctuaryEntries, runningTotal);
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
      ? (sanctuaryEntries.length > 0 ? 'Reveal sanctuaries' : 'See final scores')
      : 'Next card';
  } else {
    const sanctExps = sanctuaryEntries.filter(e => e.points > 0).map(e => `+${e.points}: ${e.explanation}`);
    explanation = sanctExps.length > 0 ? sanctExps.join(' | ') : 'No sanctuary points';
    btnLabel = 'See final scores';
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

  // Figure out how many rows are revealed based on scoringRevealIndex
  // (matches the inline reveal logic: 0=none, 1..N=regions, N+1=sanctuaries)
  const revealedRegions = Math.min(scoringRevealIndex, regionCount);
  const sanctuariesScored = scoringRevealIndex > regionCount;

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

function renderInlineLeaderboard() {
  // Remove scoring bar
  document.getElementById('scoring-bar')?.remove();

  let lb = document.getElementById('scoring-leaderboard');
  if (!lb) {
    lb = document.createElement('div');
    lb.id = 'scoring-leaderboard';
    const sanctRow = document.getElementById('my-sanctuaries-row');
    sanctRow.after(lb);
  }

  const sorted = [...state.scores].sort((a, b) => {
    if (b.total !== a.total) return b.total - a.total;
    return a.card_number_sum - b.card_number_sum;
  });

  const totals = sorted.map(s => s.total);
  const hasTie = (t) => totals.filter(v => v === t).length > 1;

  let html = '<div class="leaderboard-title">Final Scores</div><div class="leaderboard-rows">';
  sorted.forEach((s, i) => {
    const isWinner = i === 0;
    const tie = hasTie(s.total) ? ` <span class="tiebreaker">(sum: ${s.card_number_sum})</span>` : '';
    html += `<div class="score-row${isWinner ? ' winner' : ''}">
      <span>${isWinner ? '&#x1f3c6; ' : ''}${s.name}</span>
      <span>${s.total} fame${tie}</span>
    </div>`;
  });
  html += '</div>';
  html += '<button id="play-again-btn-inline" class="play-again-btn">Play Again</button>';
  lb.innerHTML = html;
  document.getElementById('play-again-btn-inline').addEventListener('click', () => {
    location.hash = '';
    location.reload();
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
  scoreTip.style.left = (rect.left + rect.width / 2) + 'px';
  scoreTip.style.top = (rect.top - 6) + 'px';
}

function hideScoreTip() {
  scoreTip.classList.add('hidden');
  scoreTip._anchor = null;
}

document.addEventListener('click', hideScoreTip);

connectBtn.addEventListener('click', connect);
playerNameEl.addEventListener('keydown', e => { if (e.key === 'Enter') connect(); });
roomNameEl.addEventListener('keydown', e => { if (e.key === 'Enter') connect(); });

// Pre-fill from URL hash if present (e.g. #room1/Alice) and auto-connect.
const hash = location.hash.slice(1);
if (hash) {
  const parts = hash.split('/');
  if (parts[0]) roomNameEl.value = decodeURIComponent(parts[0]);
  if (parts[1]) playerNameEl.value = decodeURIComponent(parts[1]);
  // Auto-connect if both fields are filled (e.g. page refresh).
  if (roomNameEl.value && playerNameEl.value) {
    connect();
  }
}
