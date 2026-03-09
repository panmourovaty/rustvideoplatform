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
    var options = {
        workerUrl: player.dataset.libassWorker,
        wasmUrl: player.dataset.libassWasm,
    };
    var fontUrl = player.dataset.libassFontUrl;
    if (fontUrl) {
        options.availableFonts = { 'default': fontUrl };
        options.fallbackFont = 'default';
    }
    player.textRenderers.add(
        new LibASSTextRenderer(function () { return import('jassub'); }, options)
    );
});
