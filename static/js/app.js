(() => {
	function getFundsModalEl() {
		return document.getElementById("staticBackdrop");
	}

	function hideFundsModal() {
		const modalEl = getFundsModalEl();
		if (!modalEl || typeof bootstrap === "undefined") return;
		const instance =
			bootstrap.Modal.getInstance(modalEl) ||
			bootstrap.Modal.getOrCreateInstance(modalEl);
		instance.hide();

		// Optional: clear content after closing so the next open is clean.
		const content = document.getElementById("fundsModalContent");
		if (content) content.innerHTML = "";
	}

	// Small delay so success message renders first.
	document.body.addEventListener("closeFundsModal", () =>
		setTimeout(hideFundsModal, 250)
	);

	document.addEventListener("hidden.bs.modal", (e) => {
		if (e.target && e.target.id === "staticBackdrop") {
			const content = document.getElementById("fundsModalContent");
			if (content) content.innerHTML = "";
		}
	});

	function normalizeSymbol(sym) {
		return (sym || "").toString().trim().toUpperCase();
	}

	function refreshPortfolioPosition(symbol) {
		if (typeof htmx === "undefined") return;

		const sym = normalizeSymbol(symbol);
		if (!sym) return;

		const container = document.getElementById("portfolioPositions");
		const card = document.getElementById(`pos-${sym}`);

		// If the card exists, refresh only it
		if (card) {
			htmx.ajax("GET", `/portfolio/position/${encodeURIComponent(sym)}`, {
				target: card,
				swap: "outerHTML",
			});

			// If that was the last card and it got removed, reload the list to show the "No positions" message
			setTimeout(() => {
				if (container && !container.querySelector(".position-card")) {
					htmx.ajax("GET", "/portfolio/positions", {
						target: "#portfolioPositions",
						swap: "innerHTML",
					});
				}
			}, 200);

			return;
		}

		// If we don't have that card but we're on portfolio, reload the list
		if (container) {
			htmx.ajax("GET", "/portfolio/positions", {
				target: "#portfolioPositions",
				swap: "innerHTML",
			});
		}
	}

	// Fired by HX-Trigger on successful buy/sell
	document.body.addEventListener("positionUpdated", (e) => {
		const d = e.detail;
		const sym = d && d.symbol ? d.symbol : d;
		refreshPortfolioPosition(sym);
	});
})();
