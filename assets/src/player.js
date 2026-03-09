// Register core player custom elements (<media-player>, <media-provider>, <media-poster>, etc.)
import 'vidstack/player';
// Register all UI custom elements used by the default layout: buttons, sliders,
// menus, captions, tooltips, etc.  Without this import the layout template renders
// inert HTML tags and controls are invisible / non-functional.
import 'vidstack/player/ui';
// Register the default layout element (<media-video-layout>, <media-audio-layout>).
import 'vidstack/player/layouts/default';

import { LibASSTextRenderer } from 'vidstack';

// Set up the LibASS text renderer for any player elements that carry the
// data-libass-worker attribute injected by the template.
document.querySelectorAll('media-player[data-libass-worker]').forEach(function (player) {
    // Use absolute paths so URL resolution is unambiguous regardless of whether
    // the string is evaluated in the main thread or inside the worker.
    var options = {
        workerUrl: player.dataset.libassWorker,
        wasmUrl: player.dataset.libassWasm,
        // jassub-worker.wasm.js is the JS fallback for browsers without native
        // WASM support.  The default relative path resolves to the page root;
        // spell it out so it always points to the correct /jassub/ directory.
        legacyWasmUrl: '/jassub/jassub-worker.wasm.js',
        // Explicit absolute URL for the bundled Liberation Sans fallback font.
        // JASSUB's built-in default ("./default.woff2") is relative and resolves
        // correctly inside the worker, but being explicit avoids any ambiguity.
        availableFonts: { 'liberation sans': '/jassub/default.woff2' },
        fallbackFont: 'liberation sans',
        // Pre-load Liberation Sans eagerly so it is available before the first
        // render frame fires.  Without this, fonts load async after
        // createTrackMem(), the first render sees no fonts, and libass logs
        // "failed to find any fallback with glyph 0x0" while the canvas stays
        // blank until the next frame.
        fonts: ['/jassub/default.woff2'],
        // Provide a minimal empty ASS document so JASSUB does not try to fetch
        // subUrl="" on first init (which fetches the worker script itself as
        // junk subtitle content).  The real track is loaded via setTrackByUrl
        // when the user selects a subtitle.
        subContent: '[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n',
    };
    var fontUrl = player.dataset.libassFontUrl;
    if (fontUrl) {
        options.availableFonts['default'] = fontUrl;
        options.fallbackFont = 'default';
        // Also pre-load the custom font so it is ready before the first render.
        options.fonts.push(fontUrl);
    }
    player.textRenderers.add(
        new LibASSTextRenderer(function () { return import('jassub'); }, options)
    );
});
