// static/js/chartData.js
// Real-time Finnhub trades (WS) -> build candles live (no historical)
// Works with SSR + HTMX (safe init/cleanup), dark theme, sane candle sizing

function updateQuoteUI(price) {
	const cur = wrap.querySelector('[data-role="quote-current"]');
	if (!cur) return;
	cur.textContent = fmt2(price);
}

(function () {
	const RES_TO_SEC = { 1: 60, 5: 300, 15: 900, 30: 1800, 60: 3600 };

	function wsUrl(path) {
		const proto = location.protocol === "https:" ? "wss" : "ws";
		return `${proto}://${location.host}${path}`;
	}

	function bucketTimeSec(tSec, resSec) {
		return Math.floor(tSec / resSec) * resSec;
	}

	function buildBarsFromTicks(ticks, resSec) {
		const bars = [];
		let cur = null;

		for (const tick of ticks) {
			const bt = bucketTimeSec(tick.t, resSec);
			if (!cur || cur.time !== bt) {
				cur = {
					time: bt,
					open: tick.p,
					high: tick.p,
					low: tick.p,
					close: tick.p,
				};
				bars.push(cur);
			} else {
				cur.high = Math.max(cur.high, tick.p);
				cur.low = Math.min(cur.low, tick.p);
				cur.close = tick.p;
			}
		}
		return bars;
	}

	function init(root) {
		const wrap =
			root?.querySelector?.('[data-symbol-details="1"]') ||
			(root?.matches?.('[data-symbol-details="1"]') ? root : null);

		if (!wrap || wrap.dataset.chartInit === "1") return;
		wrap.dataset.chartInit = "1";

		const symbol = wrap.dataset.symbol;
		const chartEl = wrap.querySelector("#chart");
		const resEl = wrap.querySelector("#res");

		if (!symbol || !chartEl || !resEl) return;
		if (!window.LightweightCharts) {
			console.error("LightweightCharts not loaded");
			return;
		}

		// --- Chart ---
		const chart = LightweightCharts.createChart(chartEl, {
			width: chartEl.clientWidth,
			height: chartEl.clientHeight || 420,
			layout: {
				background: { type: "solid", color: "#212529" },
				textColor: "#e5e7eb",
			},
			grid: {
				vertLines: { color: "rgba(255,255,255,0.06)" },
				horzLines: { color: "rgba(255,255,255,0.06)" },
			},
			rightPriceScale: { borderColor: "rgba(255,255,255,0.12)" },
			timeScale: {
				borderColor: "rgba(255,255,255,0.12)",
				timeVisible: true,
				secondsVisible: false,
				rightOffset: 12,
				barSpacing: 6,
				minBarSpacing: 3,
				fixLeftEdge: true,
				fixRightEdge: true,
				lockVisibleTimeRangeOnResize: true,
			},
			crosshair: { mode: 1 },
			localization: {
				timeFormatter: (time) => {
					const d = new Date(time * 1000);
					return d.toLocaleString([], {
						month: "2-digit",
						day: "2-digit",
						hour: "2-digit",
						minute: "2-digit",
					});
				},
			},
		});

		const series = chart.addCandlestickSeries({
			upColor: "#22c55e",
			downColor: "#ef4444",
			wickUpColor: "#22c55e",
			wickDownColor: "#ef4444",
			borderVisible: false,
		});

		// --- Resize handling ---
		const ro = new ResizeObserver(() => {
			requestAnimationFrame(() => {
				chart.applyOptions({
					width: chartEl.clientWidth,
					height: chartEl.clientHeight || 420,
				});
			});
		});
		ro.observe(chartEl);

		// --- State ---
		let ticks = []; // {t: unixSec, p: price}
		let bars = [];
		let lastBar = null;
		let lastTradePrice = null;
		let posUpdateRAF = null;

		function fmt2(x) {
			return (Math.round(x * 100) / 100).toFixed(2);
		}

		function updatePositionUI(price) {
			const pos = wrap.querySelector('[data-position-panel="1"]');
			if (!pos) return;

			const qty = Number(pos.dataset.qty || 0);
			const avg = Number(pos.dataset.avg || 0);
			if (!Number.isFinite(qty) || qty <= 0) return;
			if (!Number.isFinite(avg) || avg <= 0) return;

			const lastEl = pos.querySelector('[data-role="pos-last-price"]');
			const pnlRow = pos.querySelector('[data-role="pos-pnl-row"]');
			const pnlVal = pos.querySelector('[data-role="pos-pnl-val"]');
			const pnlPct = pos.querySelector('[data-role="pos-pnl-pct"]');
			if (!lastEl || !pnlRow || !pnlVal || !pnlPct) return;

			lastEl.textContent = fmt2(price);

			const pnl = (price - avg) * qty;
			const pct = avg > 0 ? ((price - avg) / avg) * 100 : 0;

			pnlVal.textContent = (pnl > 0 ? "+" : "") + fmt2(pnl);
			pnlPct.textContent = (pct > 0 ? "+" : "") + fmt2(pct);

			pnlRow.classList.remove(
				"text-success",
				"text-danger",
				"text-muted",
			);
			pnlRow.classList.add(
				pnl > 0
					? "text-success"
					: pnl < 0
						? "text-danger"
						: "text-muted",
			);
		}

		function schedulePosUpdate() {
			if (posUpdateRAF) return;
			posUpdateRAF = requestAnimationFrame(() => {
				posUpdateRAF = null;
				if (lastTradePrice == null) return;
				updatePositionUI(lastTradePrice);
				updateQuoteUI(lastTradePrice);
			});
		}

		function resSec() {
			return RES_TO_SEC[resEl.value] || 300;
		}

		function trimTicks() {
			const cutoff = Math.floor(Date.now() / 1000) - 6 * 3600;
			while (ticks.length && ticks[0].t < cutoff) ticks.shift();
		}

		function rebuild() {
			bars = buildBarsFromTicks(ticks, resSec());
			series.setData(bars);
			lastBar = bars[bars.length - 1] || null;
		}

		// --- WS connect + reconnect ---
		let ws = null;
		let reconnectTimer = null;

		function connect() {
			if (reconnectTimer) clearTimeout(reconnectTimer);

			ws = new WebSocket(
				wsUrl(`/ws/trades?symbol=${encodeURIComponent(symbol)}`),
			);

			ws.onmessage = (ev) => {
				let msg;
				try {
					msg = JSON.parse(ev.data);
				} catch {
					return;
				}
				if (msg.type !== "trade" || !Array.isArray(msg.data)) return;

				const intervalSec = resSec();

				for (const tr of msg.data) {
					const price = Number(tr.p);
					lastTradePrice = price;
					const tSec = Math.floor(Number(tr.t) / 1000);
					if (!Number.isFinite(price) || !Number.isFinite(tSec))
						continue;

					ticks.push({ t: tSec, p: price });
					trimTicks();

					const bt = bucketTimeSec(tSec, intervalSec);

					if (!lastBar || lastBar.time !== bt) {
						lastBar = {
							time: bt,
							open: price,
							high: price,
							low: price,
							close: price,
						};
						bars.push(lastBar);
						series.update(lastBar);
					} else {
						lastBar.high = Math.max(lastBar.high, price);
						lastBar.low = Math.min(lastBar.low, price);
						lastBar.close = price;
						series.update(lastBar);
					}
				}

				// ✅ NEW: broadcast latest trade price to other components (alerts, etc.)
				if (lastTradePrice != null) {
					document.dispatchEvent(
						new CustomEvent("rm:tradePrice", {
							detail: { symbol, price: lastTradePrice },
						}),
					);
					schedulePosUpdate();
				}
			};

			ws.onclose = () => {
				reconnectTimer = setTimeout(connect, 2000);
			};

			ws.onerror = () => {
				try {
					ws.close();
				} catch {}
			};
		}

		resEl.addEventListener("change", rebuild);

		wrap._destroy = () => {
			if (reconnectTimer) clearTimeout(reconnectTimer);
			try {
				ws && ws.close();
			} catch {}
			ro.disconnect();
			chart.remove();
			if (posUpdateRAF) cancelAnimationFrame(posUpdateRAF);
			posUpdateRAF = null;
		};

		connect();
	}

	document.addEventListener("DOMContentLoaded", () => init(document));
	document.addEventListener("htmx:load", (e) => init(e.target));
	document.body.addEventListener("htmx:beforeCleanupElement", (e) => {
		const el = e.target;
		if (el?.dataset?.chartInit === "1" && el._destroy) el._destroy();
	});
})();
