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
})();
