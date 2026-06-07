let animator;
let bubble;
let lastBubble = '';
let toolLockUntil = 0;
let idleStart = 0;
let idleAnimTimer = null;
let permissionPending = false;
let permissionTimer = null;

// Phase 1 new vars
let lastCoreSignal = '';
let lastOverlay = '';
let inputPending = false;
let lastEventTime = 0;
let fallbackTimer = null;
let tauriListenerActive = false;

// Phase 3 new vars
let clickStart = null;

async function init() {
  const resp = await fetch('validation.json');
  const validationData = await resp.json();
  const spriteEl = document.getElementById('sprite');
  const bubbleEl = document.getElementById('bubble');
  animator = new SpriteAnimator(spriteEl, validationData);
  bubble = new Bubble(bubbleEl);
  animator.play('waving');
  bubble.show('爱弥斯已上线~');
  window._petBubble = bubble;
  window._petAnimator = animator;

  const ipc = window.__TAURI_INTERNALS__;

  // Unified mouse handling: drag on move, click on release without move
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    clickStart = { x: e.clientX, y: e.clientY, time: Date.now() };
  });

  document.addEventListener('mousemove', (e) => {
    if (!clickStart) return;
    const dx = Math.abs(e.clientX - clickStart.x);
    const dy = Math.abs(e.clientY - clickStart.y);
    if (dx > 3 || dy > 3) {
      clickStart = null;
      try { if (ipc && ipc.invoke) ipc.invoke('start_drag'); } catch (_) {}
    }
  });

  document.addEventListener('mouseup', () => {
    clickStart = null;
  });

  // Right-click on sprite → quick menu
  spriteEl.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    bubble.showQuickMenu();
  });

  // Suppress browser right-click menu everywhere else
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  // Independent setups first (event listener goes last — it must not block others)
  setupInteractiveInput();
  setupQuickMenu();
  setupConfirmButtons();
  startFallbackPoll();
  setupEventListener();

  // Expose for bubble.js choice buttons
  window._sendUserInput = sendUserInput;
}

// ========== Tauri event listener (primary channel) ==========

function setupEventListener() {
  const ipc = window.__TAURI_INTERNALS__;
  if (!ipc || typeof ipc.listen !== 'function') {
    console.warn('Tauri IPC not available, state updates via polling only');
    return;
  }
  try {
    ipc.listen('state-change', (event) => {
      tauriListenerActive = true;
      lastEventTime = Date.now();
      const { animation, bubble: bubbleText, core_signal, tool_label, overlay,
              input_type, options } = event.payload;

      if (animation) {
        animator.play(animation);
      }

      updateBubbleStates(bubbleText, core_signal, overlay, input_type, options);
      handleIdleAnimation(animation);

      lastCoreSignal = core_signal;
      lastOverlay = overlay || '';
    });
  } catch (e) {
    console.warn('Tauri listen failed, using polling only', e);
  }
}

// ========== Fallback polling (2s, only when Tauri events stale) ==========

function startFallbackPoll() {
  if (fallbackTimer) return;
  // Poll every 2s; always poll as safety net (Tauri events are primary but may miss)
  fallbackTimer = setInterval(async () => {
    // If Tauri listener is active and recent, skip poll to save bandwidth
    if (tauriListenerActive && Date.now() - lastEventTime < 3000) return;

    try {
      const r = await fetch('http://127.0.0.1:9527/api/current');
      if (r.ok) {
        const data = await r.json();
        if (data.animation) {
          animator.play(data.animation);
        }
        if (data.core_signal) {
          updateBubbleStates(data.bubble, data.core_signal, data.overlay);
          handleIdleAnimation(data.animation);
        }
      }
    } catch (_) {}
    // Phase 3: also poll pending state to recover input UI
    try {
      const pr = await fetch('http://127.0.0.1:9527/api/user/pending');
      if (pr.ok) {
        const pd = await pr.json();
        if (pd.waiting && !inputPending) {
          inputPending = true;
          const it = pd.input_type || 'text';
          const opts = (pd.input_type === 'select') ? pd.options : null;
          bubble.showInteractive('等待输入...', it, opts, '输入...');
          bubble.startPendingPoll();
        }
      }
    } catch (_) {}
  }, 2000);
}

// ========== Bubble state machine (core_signal-driven) ==========

function updateBubbleStates(bubbleText, coreSignal, overlay, inputType, options) {
  // --- input overlay: show interactive bubble ---
  if (overlay === 'input') {
    if (!inputPending) {
      inputPending = true;
      const it = inputType || 'text';
      const opts = (inputType === 'select') ? options : null;
      bubble.showInteractive(bubbleText || '请输入...', it, opts, '输入...');
      bubble.startPendingPoll();
    }
    return;
  }

  // Hide interactive bubble when overlay clears
  if (inputPending && overlay !== 'input') {
    inputPending = false;
    bubble.hideInteractive();
  }

  // --- permission overlay ---
  if (overlay === 'permission') {
    if (!permissionPending) {
      permissionPending = true;
      permissionTimer = setInterval(() => {
        if (permissionPending && bubble) {
          bubble.hide();
          bubble.show('等待指示...');
        }
      }, 300);
      setTimeout(() => {
        if (permissionPending) { exitPermission(); bubble.hide(); }
      }, 120000);
    }
    if (bubbleText && bubbleText !== lastBubble) {
      lastBubble = bubbleText;
      bubble.showPersistent(bubbleText);
    }
    return;
  }

  // Clear permission when transitioning away
  if (permissionPending && overlay !== 'permission') {
    exitPermission();
  }

  const now = Date.now();

  // --- running: tool bubble, persistent ---
  if (coreSignal === 'running') {
    if (bubbleText && bubbleText !== lastBubble) {
      lastBubble = bubbleText;
      bubble.showPersistent(bubbleText);
      toolLockUntil = now + 1200;
    }
    return;
  }

  // --- waiting: persistent bubble ---
  if (coreSignal === 'waiting') {
    if (bubbleText && bubbleText !== lastBubble) {
      lastBubble = bubbleText;
      bubble.showPersistent(bubbleText);
    }
    return;
  }

  // --- ready: short bubble, auto-hide after 4s ---
  if (coreSignal === 'ready') {
    if (now < toolLockUntil) {
      // within tool lock, keep current bubble
      return;
    }
    lastBubble = '';
    if (bubbleText) {
      bubble.show(bubbleText);
    }
    return;
  }

  // --- idle: clear after tool lock expires ---
  if (coreSignal === 'idle') {
    if (now < toolLockUntil) {
      return; // keep tool bubble
    }
    lastBubble = '';
    if (bubbleText) {
      bubble.show(bubbleText);
    } else {
      bubble.hide();
    }
    return;
  }
}

// ========== Interactive input ==========

function setupInteractiveInput() {
  const askSend = document.getElementById('ask-send');
  const askInput = document.getElementById('ask-input');
  const askBack = document.getElementById('ask-back');

  askSend.addEventListener('click', () => {
    const value = askInput.value.trim();
    if (value) sendUserInput(value);
  });

  askInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const value = askInput.value.trim();
      if (value) sendUserInput(value);
    }
  });

  askBack.addEventListener('click', (e) => {
    e.stopPropagation();
    bubble.hideInteractive();
    bubble.showQuickMenu();
  });
}

// ========== Confirm buttons (Phase 3) ==========

function setupConfirmButtons() {
  const btnYes = document.getElementById('ask-confirm-yes');
  const btnNo = document.getElementById('ask-confirm-no');

  btnYes.addEventListener('click', () => {
    sendUserInput('true', 'confirm');
  });

  btnNo.addEventListener('click', () => {
    sendUserInput('false', 'confirm');
  });
}

// ========== Quick menu (Phase 3) ==========

function setupQuickMenu() {
  const menu = document.getElementById('quick-menu');
  const items = menu.querySelectorAll('.quick-menu-item');

  items.forEach(btn => {
    btn.addEventListener('click', () => {
      const action = btn.dataset.action;
      bubble.hideQuickMenu();
      switch (action) {
        case 'message':
          bubble.showInteractive('发消息给爱弥斯...', 'text', null, '输入消息...');
          break;
        case 'voice':
          bubble.show('语音输入暂不支持~');
          break;
        case 'webui':
          try { window.__TAURI_INTERNALS__?.invoke('open_webui'); } catch (_) {}
          break;
        case 'sleep':
          try { window.__TAURI_INTERNALS__?.invoke('hide_window'); } catch (_) {}
          bubble.show('爱弥斯已休眠，右键托盘唤醒~');
          break;
        case 'exit':
          try { window.__TAURI_INTERNALS__?.invoke('exit_app'); } catch (_) {}
          break;
        case 'cancel':
          break;
      }
    });
  });

  // Click outside quick menu to close
  document.addEventListener('click', (e) => {
    if (!menu.classList.contains('hidden') &&
        !menu.contains(e.target) &&
        e.target !== document.getElementById('sprite')) {
      bubble.hideQuickMenu();
    }
  });
}

// ========== Send user input ==========

async function sendUserInput(value, type = 'text') {
  // MCP-initiated input: forward to backend pending-input slot
  if (inputPending) {
    try {
      await fetch('http://127.0.0.1:9527/api/user/input', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ value, type }),
      });
    } catch (_) {}
    if (bubble) bubble.hideInteractive();
    inputPending = false;
    return;
  }

  // User-initiated message: relay to backend for Claude Code to pick up
  try {
    await fetch('http://127.0.0.1:9527/api/user/message', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
    });
  } catch (_) {}
  if (bubble) {
    lastBubble = value;
    bubble.hideInteractive();
    bubble.showPersistent(value);
  }
}

// ========== Permission helpers ==========

function exitPermission() {
  permissionPending = false;
  if (permissionTimer) { clearInterval(permissionTimer); permissionTimer = null; }
}

// ========== Idle animation ==========

function handleIdleAnimation(animation) {
  if (animation === 'idle') {
    if (!idleStart) idleStart = Date.now();
    scheduleIdleAnim();
  } else {
    idleStart = 0;
    cancelIdleAnim();
  }
}

function scheduleIdleAnim() {
  if (idleAnimTimer) return;
  idleAnimTimer = setTimeout(doIdleAnim, 15000 + Math.random() * 30000);
}

function doIdleAnim() {
  idleAnimTimer = null;
  if (!idleStart) return;
  const pick = ['jumping', 'waving', 'chatting'][Math.floor(Math.random() * 3)];
  window._petAnimator.play(pick);
  setTimeout(() => {
    if (window._petAnimator) window._petAnimator.play('idle');
    scheduleIdleAnim();
  }, 2000);
}

function cancelIdleAnim() {
  if (idleAnimTimer) { clearTimeout(idleAnimTimer); idleAnimTimer = null; }
}

document.addEventListener('DOMContentLoaded', () => { init(); setupAutostartToggle(); });
