(function () {
	const pending = new Set();

	function getDetailsWrap() {
		return document.querySelector('[data-symbol-details="1"]');
	}

	function shouldTrigger(cond, price, target) {
		if (!Number.isFinite(price) || !Number.isFinite(target)) return false;
		if (cond === "above") return price >= target;
		if (cond === "below") return price <= target;
		return false;
	}

	async function triggerAlert(id, wrap) {
		if (!id || pending.has(id)) return;
		pending.add(id);

		try {
			// Use HTMX if available so headers/events behave as expected
			if (window.htmx) {
				const msgEl = wrap.querySelector("#alertsMsg");
				htmx.ajax("POST", `/alerts/by-id/${encodeURIComponent(id)}/trigger`, {
					target: msgEl || "#alertsMsg",
					swap: "innerHTML",
				});
			} else {
				const res = await fetch(`/alerts/by-id/${encodeURIComponent(id)}/trigger`, {
					method: "POST",
				});
				const html = await res.text();
				const msgEl = wrap.querySelector("#alertsMsg");
				if (msgEl) msgEl.innerHTML = html;
			}

			// Refresh alert lists everywhere
			if (window.htmx) htmx.trigger(document.body, "alertsUpdated");
		} catch (e) {
		} finally {
			pending.delete(id);
		}
	}

	function onTrade(ev) {
		const detail = ev.detail || {};
		const symbol = detail.symbol;
		const price = Number(detail.price);

		const wrap = getDetailsWrap();
		if (!wrap) return;

		const pageSymbol = (wrap.dataset.symbol || "").toUpperCase();
		if (!pageSymbol || pageSymbol !== String(symbol || "").toUpperCase()) return;

		const list = wrap.querySelector("#alertsList");
		if (!list) return;

		const items = list.querySelectorAll('[data-alert-item="1"]');
		if (!items || items.length === 0) return;

		for (const el of items) {
			const id = el.dataset.alertId;
			const cond = (el.dataset.condition || "").toLowerCase();
			const target = Number(el.dataset.target);
			const triggered = el.dataset.triggered === "1";

			if (triggered) continue;
			if (!shouldTrigger(cond, price, target)) continue;

			triggerAlert(id, wrap);
		}
	}

	document.addEventListener("rm:tradePrice", onTrade);
})();
