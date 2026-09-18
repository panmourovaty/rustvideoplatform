"use strict";

const MOBILE_QUERY = "(max-width: 1000px)";
const hlsPreviewStates = new WeakMap();

function failHlsPreview(video, state) {
    if (hlsPreviewStates.get(video) === state) {
        stopHlsPreview(video);
    }
}

function showHlsPreview(video, state) {
    if (!state.active) return;

    const playPromise = video.play();
    if (playPromise && typeof playPromise.then === "function") {
        playPromise.then(() => {
            if (state.active) {
                state.started = true;
                video.closest(".thumbnail-container")?.classList.add("is-preview-playing");
            }
        }).catch(() => failHlsPreview(video, state));
    } else {
        state.started = true;
        video.closest(".thumbnail-container")?.classList.add("is-preview-playing");
    }
}

function rememberHlsPreviewTime(video) {
    const state = hlsPreviewStates.get(video);
    const link = video.closest("a[href]");
    if (!state?.started || !link) return;

    const currentTime = video.currentTime;
    if (Number.isFinite(currentTime) && currentTime > 0) {
        link.dataset.hlsPreviewTime = currentTime.toFixed(3);
    }
}

function startHlsPreview(video) {
    let state = hlsPreviewStates.get(video);
    if (state?.active) return;

    state = { active: true, started: false, hls: null, nativeErrorHandler: null };
    hlsPreviewStates.set(video, state);
    video.muted = true;
    const link = video.closest("a[href]");
    if (link) {
        delete link.dataset.hlsPreviewTime;
        const url = new URL(link.href, window.location.href);
        url.searchParams.delete("t");
        link.href = url.toString();
    }

    const source = video.dataset.hlsPreviewSrc;
    if (!source) {
        failHlsPreview(video, state);
        return;
    }

    if (window.Hls && Hls.isSupported()) {
        try {
            const hls = new Hls({
                capLevelToPlayerSize: true,
                maxBufferLength: 10,
                backBufferLength: 0,
            });
            state.hls = hls;
            hls.on(Hls.Events.MANIFEST_PARSED, () => showHlsPreview(video, state));
            hls.on(Hls.Events.ERROR, (_event, data) => {
                if (data.fatal) failHlsPreview(video, state);
            });
            hls.loadSource(source);
            hls.attachMedia(video);
        } catch (_error) {
            failHlsPreview(video, state);
        }
    } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
        state.nativeErrorHandler = () => failHlsPreview(video, state);
        video.addEventListener("error", state.nativeErrorHandler, { once: true });
        video.src = source;
        showHlsPreview(video, state);
    } else {
        failHlsPreview(video, state);
    }
}

function stopHlsPreview(video) {
    const state = hlsPreviewStates.get(video);
    if (state) {
        rememberHlsPreviewTime(video);
        state.active = false;
        if (state.nativeErrorHandler) {
            video.removeEventListener("error", state.nativeErrorHandler);
        }
        state.hls?.destroy();
        hlsPreviewStates.delete(video);
    }

    video.closest(".thumbnail-container")?.classList.remove("is-preview-playing");
    video.pause();
    video.removeAttribute("src");
    video.load();
}

function addHlsPreviewTimeToLink(event) {
    const link = event.target.closest?.("a[href]");
    const video = link?.querySelector("video[data-hls-preview-src]");
    if (!video) return;

    rememberHlsPreviewTime(video);
    const previewTime = Number(link.dataset.hlsPreviewTime);
    if (!Number.isFinite(previewTime) || previewTime <= 0) return;

    const url = new URL(link.href, window.location.href);
    url.searchParams.set("t", previewTime.toFixed(3));
    link.href = url.toString();
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

document.addEventListener("pointerdown", addHlsPreviewTimeToLink, true);
document.addEventListener("click", addHlsPreviewTimeToLink, true);

document.addEventListener("click", (event) => {
    const tab = event.target.closest?.(".nav-tabs .nav-link");
    if (!tab) return;

    if (tab.hasAttribute("data-block-while-uploading") && window.isUploading) {
        event.preventDefault();
        event.stopImmediatePropagation();
        alert("Please wait for the upload to complete before switching tabs.");
        return;
    }

    const tabList = tab.closest(".nav-tabs");
    tabList.querySelectorAll(".nav-link").forEach((item) => item.classList.remove("active"));
    tab.classList.add("active");
}, true);

document.addEventListener("htmx:afterRequest", (event) => {
    const trigger = event.detail?.elt;
    if (trigger?.hasAttribute("data-remove-after-request")) {
        trigger.remove();
    }
});

function resumeMediaFromQuery() {
    const resumeTime = Number(new URLSearchParams(window.location.search).get("t"));
    if (!Number.isFinite(resumeTime) || resumeTime <= 0) return;

    const mediaElement = document.querySelector("[data-media-element]");
    if (!mediaElement) return;
    const player = mediaElement.querySelector("video[slot=media]") || mediaElement;

    const seekToResumeTime = () => {
        const duration = Number(player.duration);
        if (!Number.isFinite(duration) || duration <= 0) return false;

        player.currentTime = Math.min(resumeTime, Math.max(0, duration - 0.01));
        return true;
    };

    const duration = Number(player.duration);
    if (Number.isFinite(duration) && duration > 0) {
        seekToResumeTime();
    } else {
        const seekWhenReady = () => {
            if (!seekToResumeTime()) return;
            player.removeEventListener("loadedmetadata", seekWhenReady);
            player.removeEventListener("durationchange", seekWhenReady);
            player.removeEventListener("canplay", seekWhenReady);
        };

        player.addEventListener("loadedmetadata", seekWhenReady);
        player.addEventListener("durationchange", seekWhenReady);
        player.addEventListener("canplay", seekWhenReady);
    }
}

function setupMediaCaptions() {
    const mediaElement = document.querySelector("[data-media-element][data-caption-tracks]");
    if (!mediaElement) return;

    const media = mediaElement.querySelector("video[slot=media]") || mediaElement;
    if (!(media instanceof HTMLMediaElement) || !media.textTracks) return;

    const fallbackFontUrl =
        "https://cdn.jsdelivr.net/gh/google/fonts@8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/nunito/Nunito[wght].ttf";
    // Bundle abslink with each entry to avoid duplicate proxy symbols in CDN subpath modules.
    const jassubModuleUrl = "https://esm.sh/jassub@2?bundle&deps=abslink@1";
    const jassubWorkerModuleUrl =
        "https://esm.sh/jassub@2/dist/worker/worker.js?bundle&deps=abslink@1";
    const jassubWasmUrl =
        "https://cdn.jsdelivr.net/npm/jassub@2/dist/wasm/jassub-worker.wasm";
    const jassubModernWasmUrl =
        "https://cdn.jsdelivr.net/npm/jassub@2/dist/wasm/jassub-worker-modern.wasm";

    let renderer = null;
    let activeSource = "";
    let renderGeneration = 0;
    let syncQueued = false;
    let workerBlobUrl = "";

    const getWorkerBlobUrl = () => {
        if (!workerBlobUrl) {
            workerBlobUrl = URL.createObjectURL(
                new Blob([`import "${jassubWorkerModuleUrl}";`], { type: "text/javascript" }),
            );
        }
        return workerBlobUrl;
    };

    const destroyRenderer = async () => {
        const previousRenderer = renderer;
        renderer = null;
        activeSource = "";
        if (previousRenderer) {
            await previousRenderer.destroy();
        }
    };

    const syncCaptionRenderer = async () => {
        syncQueued = false;

        const trackElements = Array.from(media.querySelectorAll("track")).filter(
            (track) => track.kind === "subtitles" || track.kind === "captions",
        );
        const selectedTrack = trackElements.find((track) => track.track.mode === "showing");

        for (const track of trackElements) {
            if (track !== selectedTrack && track.track.mode === "showing") {
                track.track.mode = "disabled";
            }
        }

        const assSource = selectedTrack?.dataset.assSrc;
        // Blob workers need absolute URLs, including when the source server is same-origin.
        const source = assSource ? new URL(assSource, document.baseURI).href : "";
        if (!source) {
            renderGeneration += 1;
            await destroyRenderer();
            return;
        }
        if (source === activeSource) return;
        if (!(media instanceof HTMLVideoElement)) return;

        const generation = ++renderGeneration;
        await destroyRenderer();
        activeSource = source;

        try {
            const { default: JASSUB } = await import(jassubModuleUrl);
            if (generation !== renderGeneration) return;

            const customFontUrl = mediaElement.dataset.assFontUrl
                ? new URL(mediaElement.dataset.assFontUrl, document.baseURI).href : "";
            const loadFont = async (url) => {
                const response = await fetch(url);
                if (!response.ok) throw new Error(`Subtitle font returned ${response.status}`);
                return new Uint8Array(await response.arrayBuffer());
            };
            const fallbackFont = await loadFont(fallbackFontUrl);
            const fonts = [fallbackFont];
            const availableFonts = { "Nunito ExtraLight": fallbackFont };
            const defaultFont = "Nunito ExtraLight";

            if (customFontUrl) {
                try {
                    const customFont = await loadFont(customFontUrl);
                    fonts.unshift(customFont);
                    availableFonts.default = customFont;
                } catch (error) {
                    console.warn("Unable to load the uploaded subtitle font", error);
                }
            }
            if (generation !== renderGeneration) return;

            const nextRenderer = new JASSUB({
                video: media,
                subUrl: source,
                workerUrl: getWorkerBlobUrl(),
                wasmUrl: jassubWasmUrl,
                modernWasmUrl: jassubModernWasmUrl,
                fonts,
                availableFonts,
                defaultFont,
            });

            await nextRenderer.ready;
            if (generation !== renderGeneration) {
                await nextRenderer.destroy();
                return;
            }

            renderer = nextRenderer;
            // A paused video will not deliver a new video-frame callback when captions change.
            if (media.paused && media.videoWidth && media.videoHeight) {
                await renderer.manualRender({
                    expectedDisplayTime: performance.now(),
                    width: media.videoWidth,
                    height: media.videoHeight,
                    mediaTime: media.currentTime,
                }, true);
            }
            const canvas = media.parentElement?.querySelector("canvas.JASSUB");
            if (canvas) canvas.style.zIndex = "2";
        } catch (error) {
            if (generation === renderGeneration) {
                activeSource = "";
                console.error("Failed to initialize ASS subtitles", error);
            }
        }
    };

    const queueCaptionSync = () => {
        if (syncQueued) return;
        syncQueued = true;
        queueMicrotask(syncCaptionRenderer);
    };

    media.textTracks.addEventListener("change", queueCaptionSync);
    window.addEventListener("pagehide", () => {
        renderGeneration += 1;
        destroyRenderer();
        if (workerBlobUrl) URL.revokeObjectURL(workerBlobUrl);
    }, { once: true });
    queueCaptionSync();
}

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
    setupMediaCaptions();
    resumeMediaFromQuery();
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
