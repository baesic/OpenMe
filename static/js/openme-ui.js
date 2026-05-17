(function () {
  const root = document.documentElement;
  const themeButton = document.querySelector("[data-theme-toggle]");
  const storedTheme = localStorage.getItem("openme-theme");

  if (storedTheme) {
    root.dataset.theme = storedTheme;
  }

  themeButton?.addEventListener("click", () => {
    const next = wasmNextThemeIsDark(root.dataset.theme || "light") ? "dark" : "light";
    root.dataset.theme = next;
    localStorage.setItem("openme-theme", next);
  });

  let wasmApi = null;

  async function loadWasm() {
    try {
      const response = await fetch("/wasm/openme_web_ui.wasm");
      if (!response.ok) return null;
      const { instance } = await WebAssembly.instantiateStreaming(response, {});
      wasmApi = instance.exports;
    } catch (_) {
      wasmApi = null;
    }
    return wasmApi;
  }

  function wasmScore(text, query) {
    if (!wasmApi?.openme_alloc || !wasmApi?.openme_match_score || !wasmApi?.memory) {
      return fallbackScore(text, query);
    }

    return callTwoStringWasm(wasmApi.openme_match_score, text, query);
  }

  function wasmShouldShowTag(tags, active) {
    if (!wasmApi?.openme_alloc || !wasmApi?.openme_should_show_tag || !wasmApi?.memory) {
      return active ? tags.split(",").includes(active) : true;
    }

    return callTwoStringWasm(wasmApi.openme_should_show_tag, tags, active) > 0;
  }

  function wasmNextThemeIsDark(current) {
    if (!wasmApi?.openme_alloc || !wasmApi?.openme_next_theme_is_dark || !wasmApi?.memory) {
      return current !== "dark";
    }

    const encoder = new TextEncoder();
    const bytes = encoder.encode(current);
    const ptr = wasmApi.openme_alloc(bytes.length);
    const memory = new Uint8Array(wasmApi.memory.buffer);
    memory.set(bytes, ptr);
    const result = wasmApi.openme_next_theme_is_dark(ptr, bytes.length);
    wasmApi.openme_dealloc(ptr, bytes.length);
    return result > 0;
  }

  function callTwoStringWasm(fn, left, right) {
    const encoder = new TextEncoder();
    const leftBytes = encoder.encode(left);
    const rightBytes = encoder.encode(right);
    const leftPtr = wasmApi.openme_alloc(leftBytes.length);
    const rightPtr = wasmApi.openme_alloc(rightBytes.length);
    const memory = new Uint8Array(wasmApi.memory.buffer);
    memory.set(leftBytes, leftPtr);
    memory.set(rightBytes, rightPtr);
    const result = fn(leftPtr, leftBytes.length, rightPtr, rightBytes.length);
    wasmApi.openme_dealloc(leftPtr, leftBytes.length);
    wasmApi.openme_dealloc(rightPtr, rightBytes.length);
    return result;
  }

  function fallbackScore(text, query) {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) return 1;
    const haystack = text.toLowerCase();
    return terms.reduce((sum, term) => sum + (haystack.includes(term) ? 10 : 0), 0);
  }

  function bindSearch() {
    const input = document.querySelector("[data-search-input]");
    const items = Array.from(document.querySelectorAll("[data-search-item]"));
    if (!input || !items.length) return;

    const apply = () => {
      const query = input.value.trim();
      items
        .map((item) => {
          const score = wasmScore(item.dataset.searchText || item.textContent || "", query);
          item.hidden = score <= 0;
          return [item, score];
        })
        .sort((a, b) => b[1] - a[1])
        .forEach(([item]) => item.parentElement?.appendChild(item));
    };

    input.addEventListener("input", apply);
    apply();
  }

  function bindTagFilters() {
    const buttons = Array.from(document.querySelectorAll("[data-tag-filter]"));
    const items = Array.from(document.querySelectorAll("[data-post-tags]"));
    if (!buttons.length || !items.length) return;

    buttons.forEach((button) => {
      button.addEventListener("click", () => {
        const tag = button.dataset.tagFilter || "";
        const active = button.getAttribute("aria-pressed") === "true" ? "" : tag;
        buttons.forEach((candidate) => {
          candidate.setAttribute("aria-pressed", String(candidate.dataset.tagFilter === active));
        });
        items.forEach((item) => {
          item.hidden = !wasmShouldShowTag(item.dataset.postTags || "", active);
        });
      });
    });
  }

  loadWasm().finally(() => {
    bindSearch();
    bindTagFilters();
  });
})();
