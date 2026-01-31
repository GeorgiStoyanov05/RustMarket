(() => {
	function ensureWidget(slotEl, scriptSrc, config) {
		if (!slotEl) return;
		if (slotEl.dataset.tvInit === "1") return;
		slotEl.dataset.tvInit = "1";
		slotEl.innerHTML = "";

		const container = document.createElement("div");
		container.className = "tradingview-widget-container";

		const widget = document.createElement("div");
		widget.className = "tradingview-widget-container__widget";
		container.appendChild(widget);

		const script = document.createElement("script");
		script.type = "text/javascript";
		script.async = true;
		script.src = scriptSrc;
		script.text = JSON.stringify(config);

		container.appendChild(script);
		slotEl.appendChild(container);
	}

	function initHomeWidgets() {
		const homeRoot = document.querySelector('[data-home-page="1"]');
		if (!homeRoot) return;

		// 1) Market Overview
		ensureWidget(
			document.getElementById("tv-market-overview"),
			"https://s3.tradingview.com/external-embedding/embed-widget-market-overview.js",
			{
				colorTheme: "dark",
				dateRange: "12M",
				showChart: true,
				locale: "en",
				width: "100%",
				height: "420",
				isTransparent: false,
				showSymbolLogo: true,
				tabs: [
					{
						title: "Indices",
						symbols: [
							{ s: "FOREXCOM:SPXUSD", d: "S&P 500" },
							{ s: "FOREXCOM:NSXUSD", d: "US 100" },
							{ s: "FOREXCOM:DJI", d: "Dow 30" },
							{ s: "INDEX:NKY", d: "Nikkei 225" },
							{ s: "INDEX:DEU40", d: "DAX" },
						],
					},
					{
						title: "Forex",
						symbols: [
							{ s: "FX:EURUSD" },
							{ s: "FX:GBPUSD" },
							{ s: "FX:USDJPY" },
							{ s: "FX:USDCAD" },
							{ s: "FX:AUDUSD" },
						],
					},
				],
			},
		);

		// 2) Heatmap
		ensureWidget(
			document.getElementById("tv-heatmap"),
			"https://s3.tradingview.com/external-embedding/embed-widget-stock-heatmap.js",
			{
				colorTheme: "dark",
				locale: "en",
				width: "100%",
				height: "420",
				hasTopBar: true,
				isDataSetEnabled: true,
				isZoomEnabled: true,
				hasSymbolTooltip: true,
				isTransparent: false,
			},
		);

		// 3) Top Stories
		ensureWidget(
			document.getElementById("tv-top-stories"),
			"https://s3.tradingview.com/external-embedding/embed-widget-timeline.js",
			{
				colorTheme: "dark",
				locale: "en",
				width: "100%",
				height: "420",
				isTransparent: false,
				displayMode: "regular",
				feedMode: "all_symbols",
			},
		);

		// 4) Screener
		ensureWidget(
			document.getElementById("tv-screener"),
			"https://s3.tradingview.com/external-embedding/embed-widget-screener.js",
			{
				colorTheme: "dark",
				locale: "en",
				width: "100%",
				height: "420",
				isTransparent: false,
				defaultColumn: "overview",
				defaultScreen: "most_capitalized",
				market: "america",
				showToolbar: true,
			},
		);
	}

	document.addEventListener("DOMContentLoaded", initHomeWidgets);
	document.addEventListener("htmx:load", initHomeWidgets);
})();
