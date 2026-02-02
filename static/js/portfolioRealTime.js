// static/js/portfolioRealtime.js
// Opens ONE WS for all portfolio symbols and updates Last + P/L live.

(() => {
  function wsUrl(path) {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    return `${proto}://${location.host}${path}`;
  }

  let ws = null;
  let reconnectTimer = null;
  let activeKey = "";

  function fmt2(n) {
    return (Math.round(n * 100) / 100).toFixed(2);
  }

  function getSymbols() {
    const container = document.getElementById("portfolioPositions");
    if (!container) return [];

    const cards = container.querySelectorAll(".position-card[data-symbol]");
    const out = [];
    for (const c of cards) {
      const s = (c.dataset.symbol || "").trim().toUpperCase();
      if (s) out.push(s);
    }
    out.sort();
    // dedupe
    return out.filter((s, i) => i === 0 || s !== out[i - 1]);
  }

  function updateCard(symbol, price) {
    const card = document.getElementById(`pos-${symbol}`);
    if (!card) return;

    const qty = Number(card.dataset.qty);
    const avg = Number(card.dataset.avg);
    if (!Number.isFinite(qty) || !Number.isFinite(avg)) return;

    // Last
    const lastEl = card.querySelector(".js-last");
    if (lastEl) lastEl.textContent = `$${fmt2(price)}`;

    // P/L
    const pnl = (price - avg) * qty;
    const pct = avg > 0 ? ((price - avg) / avg) * 100 : 0;

    const pnlBox = card.querySelector(".js-pnl");
    const pnlVal = card.querySelector(".js-pnl-val");
    const pnlPct = card.querySelector(".js-pnl-pct");

    if (pnlVal) pnlVal.textContent = `${pnl >= 0 ? "+" : ""}${fmt2(pnl)}`;
    if (pnlPct) pnlPct.textContent = `${pct >= 0 ? "+" : ""}${fmt2(pct)}`;

    if (pnlBox) {
      pnlBox.classList.remove("text-success", "text-danger", "text-muted");
      pnlBox.classList.add(pnl > 0 ? "text-success" : pnl < 0 ? "text-danger" : "text-muted");
    }
  }

  function onTradeMessage(ev) {
    let msg;
    try { msg = JSON.parse(ev.data); } catch { return; }
    if (msg.type !== "trade" || !Array.isArray(msg.data)) return;

    for (const tr of msg.data) {
      const symbol = String(tr.s || "").toUpperCase();
      const price = Number(tr.p);
      if (!symbol || !Number.isFinite(price)) continue;

      updateCard(symbol, price);

      // Optional: keep compatibility with your existing alertsRealtime.js / chart flow
      document.dispatchEvent(new CustomEvent("rm:tradePrice", { detail: { symbol, price } }));
    }
  }

  function closeWs() {
    try { ws && ws.close(); } catch {}
    ws = null;
  }

  function connectFor(symbols) {
    const key = symbols.join(",");
    if (key === activeKey) return;

    activeKey = key;
    closeWs();

    if (!symbols.length) return;

    const url = wsUrl(`/ws/trades_multi?symbols=${encodeURIComponent(key)}`);
    ws = new WebSocket(url);

    ws.onmessage = onTradeMessage;

    ws.onclose = () => {
      if (reconnectTimer) clearTimeout(reconnectTimer);
      reconnectTimer = setTimeout(() => {
        // reconnect with latest symbols
        start();
      }, 2000);
    };

    ws.onerror = () => {
      try { ws.close(); } catch {}
    };
  }

  function start() {
    const container = document.getElementById("portfolioPositions");
    if (!container) {
      activeKey = "";
      closeWs();
      return;
    }
    connectFor(getSymbols());
  }

  // Start on load
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }

  // Re-scan after HTMX swaps portfolio content
  document.body.addEventListener("htmx:afterSwap", (e) => {
    // only if portfolioPositions was involved
    const t = e.target;
    if (t && (t.id === "portfolioPositions" || t.closest?.("#portfolioPositions"))) {
      start();
    }
  });

  // Also re-scan after your buy/sell events refresh cards
  document.body.addEventListener("positionUpdated", () => start());
})();
