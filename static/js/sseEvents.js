(() => {
  if (window.__gomarketSSEStarted) return;
  window.__gomarketSSEStarted = true;

  function fire(name) {
    // HTMX listens for DOM events (we use from:body in templates)
    if (window.htmx) {
      htmx.trigger(document.body, name);
    } else {
      document.body.dispatchEvent(new Event(name));
    }
  }

  function connect() {
    const es = new EventSource("/events");

    es.addEventListener("alertsUpdated", () => fire("alertsUpdated"));
    es.addEventListener("positionUpdated", () => fire("positionUpdated"));
    es.addEventListener("cashUpdated", () => fire("cashUpdated"));
    es.addEventListener("ordersUpdated", () => fire("ordersUpdated"));

    es.onerror = () => {
      try { es.close(); } catch {}
      // reconnect
      setTimeout(connect, 1500);
    };
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", connect);
  } else {
    connect();
  }
})();
