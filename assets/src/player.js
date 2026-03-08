// Import vidstack CSS — esbuild extracts these into player.css alongside player.js
import 'vidstack/player/styles/default/theme.css';
import 'vidstack/player/styles/default/layouts/video.css';

// Register <media-player>, <media-provider>, <media-video-layout>, <media-poster>, etc.
import 'vidstack/player';

// Expose LibASSTextRenderer globally so inline template scripts can use it.
// By the time DOMContentLoaded fires, this deferred script has already run.
import { LibASSTextRenderer } from 'vidstack';
window._LibASSTextRenderer = LibASSTextRenderer;
