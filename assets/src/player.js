// Import vidstack CSS — esbuild extracts these into player.css alongside player.js
import 'vidstack/player/styles/default/theme.css';
import 'vidstack/player/styles/default/layouts/video.css';

// Register core player custom elements (<media-player>, <media-provider>, <media-poster>, etc.)
import 'vidstack/player';
// Register default layout elements (<media-video-layout>, <media-audio-layout>, and all
// controls). This is a separate package from the core player and MUST be imported explicitly;
// without it the controls overlay is completely absent.
import 'vidstack/player/layouts/default';

import { LibASSTextRenderer } from 'vidstack';

// Set up the LibASS text renderer for any player elements that carry the data-libass-*
// attributes injected by the template.  This runs synchronously after customElements.define()
// so the renderer is registered before the player queues its async track-list processing —
// which is the only reliable way to ensure ASS tracks appear in the subtitle menu.
document.querySelectorAll('media-player[data-libass-worker]').forEach(function (player) {
    var options = {
        workerUrl: player.dataset.libassWorker,
        legacyWorkerUrl: player.dataset.libassWorkerLegacy,
    };
    var fontUrl = player.dataset.libassFontUrl;
    if (fontUrl) {
        options.availableFonts = { 'default': fontUrl };
        options.fallbackFont = 'default';
    }
    player.textRenderers.add(
        new LibASSTextRenderer(function () { return import('/jassub/jassub.js'); }, options)
    );
});
