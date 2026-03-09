module.exports = {
  content: [
    "templates/**/*.html",
    "assets/static/**/*.js",
  ],
  css: ["assets/static/style.css"],
  // Keep selectors that are dynamically added or used by third-party libraries
  safelist: {
    standard: [
      // Dynamically toggled via JS
      "sidebar-open",
      "sidebar-collapsed",
      "sidebar-animating",
      "sidebar-will-collapse",
      "fa-expand",
      "fa-compress",
      // data-bs-theme attribute selectors (theme switching)
      /^\[data-bs-theme/,
    ],
    deep: [
      // Quill editor classes (dynamically generated)
      /^ql-/,
      // Highlight.js (dynamically generated)
      /^hljs/,
      // Tom Select (dynamically generated)
      /^ts-/,
      // HTMX (dynamically added)
      /^htmx-/,
      // @videojs/html web component class names (injected by skin modules from CDN)
      /^media-/,
    ],
    greedy: [],
    // @videojs/html CSS custom properties (injected by skin modules from CDN)
    variables: [/^--media-/],
  },
  // Preserve CSS variables and keyframes
  variables: true,
  keyframes: true,
  fontFace: true,
};
