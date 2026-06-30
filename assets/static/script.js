"use strict";

const MOBILE_QUERY = "(max-width: 1000px)";
const hlsPreviewStates = new WeakMap();

function showHlsPreview(video, state) {
    if (!state.active) return;

    const playPromise = video.play();
    if (playPromise && typeof playPromise.then === "function") {
        playPromise.then(() => {
            if (state.active) {
                video.closest(".thumbnail-container")?.classList.add("is-preview-playing");
            }
        }).catch(() => {});
    } else {
        video.closest(".thumbnail-container")?.classList.add("is-preview-playing");
    }
}

function startHlsPreview(video) {
    let state = hlsPreviewStates.get(video);
    if (state?.active) return;

    state = { active: true, hls: null };
    hlsPreviewStates.set(video, state);
    video.muted = true;

    const source = video.dataset.hlsPreviewSrc;
    if (!source) return;

    if (window.Hls && Hls.isSupported()) {
        const hls = new Hls({
            capLevelToPlayerSize: true,
            maxBufferLength: 10,
            backBufferLength: 0,
        });
        state.hls = hls;
        hls.on(Hls.Events.MANIFEST_PARSED, () => showHlsPreview(video, state));
        hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) stopHlsPreview(video);
        });
        hls.loadSource(source);
        hls.attachMedia(video);
    } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
        video.src = source;
        showHlsPreview(video, state);
    }
}

function stopHlsPreview(video) {
    const state = hlsPreviewStates.get(video);
    if (state) {
        state.active = false;
        state.hls?.destroy();
        hlsPreviewStates.delete(video);
    }

    video.closest(".thumbnail-container")?.classList.remove("is-preview-playing");
    video.pause();
    video.removeAttribute("src");
    video.load();
}

document.addEventListener("mouseover", (event) => {
    const container = event.target.closest?.(".thumbnail-container");
    if (!container || container.contains(event.relatedTarget)) return;

    const video = container.querySelector("video[data-hls-preview-src]");
    if (video) startHlsPreview(video);
});

document.addEventListener("mouseout", (event) => {
    const container = event.target.closest?.(".thumbnail-container");
    if (!container || container.contains(event.relatedTarget)) return;

    const video = container.querySelector("video[data-hls-preview-src]");
    if (video) stopHlsPreview(video);
});

document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
        document.querySelectorAll("video[data-hls-preview-src]").forEach(stopHlsPreview);
    }
});

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

    // Fade-in lazy-loaded HTMX content to reduce flicker
    document.body.addEventListener('htmx:beforeSwap', (event) => {
        const t = event.detail.target;
        if (t && t.classList && (
            t.classList.contains('hx-placeholder') ||
            t.classList.contains('subscribe-placeholder')
        )) {
            t.style.opacity = '0';
        }
    });

    document.body.addEventListener('htmx:afterSettle', (event) => {
        const t = event.detail.target;
        if (t && t.style && t.style.opacity === '0') {
            requestAnimationFrame(() => { t.style.opacity = '1'; });
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
