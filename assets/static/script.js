"use strict";

const MOBILE_QUERY = "(max-width: 1000px)";

function toggleSidebar() {
    if (window.matchMedia(MOBILE_QUERY).matches) {
        document.getElementById("sidebar").classList.toggle("sidebar-open");
        document.getElementById("sidebarbackground").classList.toggle("sidebar-open");
    } else {
        document.body.classList.add("sidebar-animating");
        document.body.classList.toggle("sidebar-collapsed");
        try {
            localStorage.setItem("sidebar-collapsed", document.body.classList.contains("sidebar-collapsed") ? "1" : "0");
        } catch (e) {}
        setTimeout(() => {
            document.body.classList.remove("sidebar-animating");
        }, 300);
    }
}

function navbarSearch(event) {
    event.preventDefault();
    const input = document.getElementById('searchInput');
    const query = input && input.value.trim();
    if (query) {
        window.location.href = '/search?q=' + encodeURIComponent(query);
    }
    return false;
}

document.addEventListener('DOMContentLoaded', () => {
    const docEl = document.documentElement;
    if (docEl.classList.contains('sidebar-will-collapse')) {
        document.body.classList.add('sidebar-collapsed');
        docEl.classList.remove('sidebar-will-collapse');
    }

    const sidebarBg = document.getElementById("sidebarbackground");
    if (sidebarBg) {
        sidebarBg.addEventListener("click", function () {
            if (window.matchMedia(MOBILE_QUERY).matches) {
                document.getElementById("sidebar").classList.remove("sidebar-open");
                this.classList.remove("sidebar-open");
            }
        });
    }

    const searchInput = document.getElementById('searchInput');
    const suggestionsList = document.getElementById('suggestions');

    if (searchInput && suggestionsList) {
        searchInput.addEventListener('focus', () => {
            if (suggestionsList.children.length > 0) {
                suggestionsList.style.display = '';
            }
        });

        searchInput.addEventListener('blur', () => {
            setTimeout(() => {
                suggestionsList.style.display = 'none';
            }, 250);
        });
    }

    document.body.addEventListener('htmx:afterSwap', (event) => {
        const target = event.detail.target;
        if (target.id === 'suggestions' && document.getElementById('searchInput') === document.activeElement) {
            target.style.display = target.children.length > 0 ? '' : 'none';
        }
    });

    fitMediumTitle();
});

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    document.documentElement.setAttribute('data-bs-theme', e.matches ? 'dark' : 'light');
});

function closeListModal(event) {
    if (event.target.id === 'listModalOverlay') {
        event.target.style.display = 'none';
    }
}

function togglePdfFullscreen() {
    const wrapper = document.getElementById('pdfViewerWrapper');
    if (!document.fullscreenElement && !document.webkitFullscreenElement) {
        if (wrapper.requestFullscreen) {
            wrapper.requestFullscreen();
        } else if (wrapper.webkitRequestFullscreen) {
            wrapper.webkitRequestFullscreen();
        }
    } else {
        if (document.exitFullscreen) {
            document.exitFullscreen();
        } else if (document.webkitExitFullscreen) {
            document.webkitExitFullscreen();
        }
    }
}

function updateFullscreenIcon() {
    const icon = document.getElementById('pdfFullscreenIcon');
    if (!icon) return;
    const isFullscreen = document.fullscreenElement || document.webkitFullscreenElement;
    icon.classList.toggle('fa-compress', !!isFullscreen);
    icon.classList.toggle('fa-expand', !isFullscreen);
}

document.addEventListener('fullscreenchange', updateFullscreenIcon);
document.addEventListener('webkitfullscreenchange', updateFullscreenIcon);

function fitMediumTitle() {
    const title = document.getElementById('medium-title');
    if (!title || window.matchMedia('(min-width: 992px)').matches) return;

    title.style.fontSize = '';

    const style = window.getComputedStyle(title);
    const lineHeight = parseFloat(style.lineHeight);
    const maxHeight = lineHeight * 2 + 1;

    if (title.scrollHeight <= maxHeight) return;

    let lo = 10, hi = parseFloat(style.fontSize);
    while (hi - lo > 0.5) {
        const mid = (lo + hi) / 2;
        title.style.fontSize = mid + 'px';
        if (title.scrollHeight <= maxHeight) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    title.style.fontSize = lo + 'px';
}

window.addEventListener('resize', fitMediumTitle);

// Preload lazy-loaded HTMX content (hx-trigger="load") found inside mouseover-preloaded pages.
// The htmx-ext-preload extension caches the main page HTML on mouseover, but doesn't
// process nested hx-get elements that rely on "load" triggers. This script parses the
// preloaded HTML and prefetches those lazy-load URLs so they're browser-cached too.
(function() {
    const preloaded = new Set();

    function prefetchLazyContent(url) {
        if (preloaded.has(url)) return;
        preloaded.add(url);

        fetch(url, { credentials: 'include' }).then(function(res) {
            if (!res.ok) return;
            return res.text();
        }).then(function(html) {
            if (!html) return;
            var doc = new DOMParser().parseFromString(html, 'text/html');
            // Find all elements with hx-get whose trigger includes "load"
            var lazyEls = doc.querySelectorAll('[hx-get][hx-trigger]');
            lazyEls.forEach(function(el) {
                var trigger = el.getAttribute('hx-trigger');
                // Match triggers that fire on load (not just "revealed" or user events)
                if (!/\bload\b/.test(trigger)) return;
                var lazyUrl = el.getAttribute('hx-get');
                if (!lazyUrl || preloaded.has(lazyUrl)) return;
                preloaded.add(lazyUrl);
                // Prefetch with low priority - just warm the browser cache
                fetch(lazyUrl, { credentials: 'include', priority: 'low' }).catch(function() {});
            });
        }).catch(function() {});
    }

    document.addEventListener('mouseenter', function(e) {
        if (!e.target.closest) return;
        var el = e.target.closest('[preload="mouseover"]');
        if (!el) return;
        // Determine the URL the preload extension would fetch
        var url = el.getAttribute('href') || el.getAttribute('hx-get');
        if (!url || url.startsWith('javascript:') || url === '#') return;
        // Small delay to let the preload extension fire first
        setTimeout(function() { prefetchLazyContent(url); }, 50);
    }, true);
})();

