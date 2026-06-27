/* Nourish site — progressive enhancement only.
   The page is fully readable with this file deleted. */
(function () {
  "use strict";

  // Mark that JS is running; CSS only hides .reveal elements under .js
  document.documentElement.classList.add("js");

  var reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- Header height -> CSS var (offsets the pinned hero) ---------- */
  var header = document.querySelector(".nav");
  var setHeaderH = function () {
    if (header) {
      document.documentElement.style.setProperty("--header-h", header.offsetHeight + "px");
    }
  };
  setHeaderH();
  window.addEventListener("resize", setHeaderH, { passive: true });

  /* ---------- Intro scroll-parallax via GSAP ScrollTrigger ----------
     The hero video scrolls past normally; this pins the full-viewport intro and
     rises + scales the title, description and globe in (staggered = parallax depth)
     so the video has completely cleared before any text shows. Degrades to the
     static centred layout if GSAP is unavailable or under reduced motion. */
  var introEl = document.getElementById("intro");
  if (introEl && window.gsap && window.ScrollTrigger && !reduceMotion) {
    gsap.registerPlugin(ScrollTrigger);
    document.documentElement.classList.add("gsap-ready");
    var tl = gsap.timeline({
      defaults: { ease: "power2.out" },
      scrollTrigger: {
        trigger: introEl,
        start: "top top",
        end: "+=110%",
        scrub: 0.6,
        pin: true,
        anticipatePin: 1,
        invalidateOnRefresh: true
      }
    });
    tl.fromTo(".intro .hero-title", { autoAlpha: 0, yPercent: 60, scale: 0.72 },
                                    { autoAlpha: 1, yPercent: 0, scale: 1 }, 0)
      .fromTo(".intro .hero-desc",  { autoAlpha: 0, yPercent: 80 },
                                    { autoAlpha: 1, yPercent: 0 }, 0.12)
      .fromTo(".intro .hero-globe", { autoAlpha: 0, yPercent: 90, scale: 0.6 },
                                    { autoAlpha: 1, yPercent: 0, scale: 1 }, 0.2)
      .fromTo(".intro .hero-cta",   { autoAlpha: 0, yPercent: 80 },
                                    { autoAlpha: 1, yPercent: 0 }, 0.32);
  }

  /* ---------- Starfield with 3 parallax depths (hero only) ---------- */
  var sky = document.getElementById("stars");
  if (sky) {
    var LAYERS = [
      { count: 36, size: [1, 1.6], speed: 0.05, op: [0.15, 0.45] }, // far
      { count: 24, size: [1.6, 2.4], speed: 0.12, op: [0.25, 0.7] }, // mid
      { count: 14, size: [2.4, 3.4], speed: 0.22, op: [0.35, 0.95] } // near
    ];
    var layerEls = LAYERS.map(function (cfg) {
      var layer = document.createElement("div");
      layer.style.position = "absolute";
      layer.style.inset = "0";
      layer.dataset.speed = cfg.speed;
      for (var i = 0; i < cfg.count; i++) {
        var s = document.createElement("span");
        s.className = "star";
        var size = cfg.size[0] + Math.random() * (cfg.size[1] - cfg.size[0]);
        s.style.width = s.style.height = size.toFixed(1) + "px";
        s.style.left = (Math.random() * 100).toFixed(2) + "%";
        s.style.top = (Math.random() * 100).toFixed(2) + "%";
        s.style.setProperty("--o1", cfg.op[0].toFixed(2));
        s.style.setProperty("--o2", cfg.op[1].toFixed(2));
        s.style.setProperty("--tw", (3 + Math.random() * 5).toFixed(1) + "s");
        layer.appendChild(s);
      }
      sky.appendChild(layer);
      return layer;
    });

    // Parallax: decoration only, transform-only, rAF-throttled
    if (!reduceMotion) {
      var ticking = false;
      var onScroll = function () {
        if (ticking) return;
        ticking = true;
        requestAnimationFrame(function () {
          var y = window.scrollY;
          layerEls.forEach(function (layer) {
            layer.style.transform =
              "translateY(" + (y * parseFloat(layer.dataset.speed)).toFixed(1) + "px)";
          });
          ticking = false;
        });
      };
      window.addEventListener("scroll", onScroll, { passive: true });
    }
  }

  /* ---------- Scroll reveals (once per element) ---------- */
  var reveals = document.querySelectorAll(".reveal");
  if (reduceMotion || !("IntersectionObserver" in window)) {
    reveals.forEach(function (el) { el.classList.add("in"); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) {
          e.target.classList.add("in");
          io.unobserve(e.target);
        }
      });
    }, { threshold: 0.12, rootMargin: "0px 0px -8% 0px" });
    reveals.forEach(function (el) { io.observe(el); });
  }

  /* ---------- Copy buttons on code blocks ---------- */
  document.querySelectorAll("[data-copy]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var code = btn.parentElement.querySelector("code");
      if (!code || !navigator.clipboard) return;
      navigator.clipboard.writeText(code.textContent).then(function () {
        var old = btn.textContent;
        btn.textContent = "Copied";
        setTimeout(function () { btn.textContent = old; }, 1600);
      });
    });
  });

  /* ---------- Tabbed sections (install methods, configuration) ----------
     Without JS the tablist is hidden and all panels stack (see CSS). Here we
     wire the tabs: click or arrow-key to switch, showing one panel at a time. */
  var tabActivators = {}; // id (of a tab button OR its panel) -> activate fn
  document.querySelectorAll("[data-tabs]").forEach(function (group) {
    var tabs = Array.prototype.slice.call(group.querySelectorAll("[role=tab]"));
    var panels = tabs.map(function (t) {
      return document.getElementById(t.getAttribute("aria-controls"));
    });
    var select = function (i) {
      tabs.forEach(function (t, j) {
        var on = i === j;
        t.setAttribute("aria-selected", on ? "true" : "false");
        t.tabIndex = on ? 0 : -1;
        if (panels[j]) panels[j].classList.toggle("is-active", on);
      });
    };
    tabs.forEach(function (t, i) {
      t.addEventListener("click", function () { select(i); });
      t.addEventListener("keydown", function (e) {
        var d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
        if (!d) return;
        e.preventDefault();
        var n = (i + d + tabs.length) % tabs.length;
        tabs[n].focus();
        select(n);
      });
      // Let in-page links target either the tab button or its panel.
      tabActivators[t.id] = function () { select(i); };
      if (panels[i]) tabActivators[panels[i].id] = function () { select(i); };
    });
    // Start on whichever tab is pre-marked selected (fallback: the first).
    var start = 0;
    tabs.forEach(function (t, i) {
      if (t.getAttribute("aria-selected") === "true") start = i;
    });
    select(start);
  });

  /* A link like href="#panel-install-source" should both switch to that tab
     and scroll to it. Activate first (so the target is visible), then let the
     browser's default hash jump run. Also handle a hash already in the URL. */
  var activateTabFor = function (hash) {
    var fn = hash && tabActivators[hash.replace(/^#/, "")];
    if (fn) { fn(); return true; }
    return false;
  };
  document.addEventListener("click", function (e) {
    var a = e.target.closest ? e.target.closest('a[href^="#"]') : null;
    if (a) activateTabFor(a.getAttribute("href"));
  });
  if (window.location.hash) activateTabFor(window.location.hash);
  window.addEventListener("hashchange", function () {
    activateTabFor(window.location.hash);
  });

  /* ---------- Feature loop videos: play only while visible ----------
     Activates automatically once placeholders are replaced by
     <video muted loop playsinline> elements. */
  var vids = document.querySelectorAll("video[loop]");
  if (vids.length && "IntersectionObserver" in window && !reduceMotion) {
    var vio = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) { e.target.play().catch(function () {}); }
        else { e.target.pause(); }
      });
    }, { threshold: 0.35 });
    vids.forEach(function (v) { vio.observe(v); });
  }
})();
