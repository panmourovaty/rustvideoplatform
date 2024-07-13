function toggleSidebar() {
    const sidebar = document.getElementById("sidebar");
    const displayStyle = sidebar.style.display === "none" ? 'flex' : 'none';
    sidebar.style.setProperty('display', displayStyle, 'important');
    document.getElementById("sidebarbackground").style.display = displayStyle;
}

document.addEventListener('DOMContentLoaded', () => {
    const searchInput = document.getElementById('searchInput');
    const suggestionsList = document.getElementById('suggestions');

    document.getElementById('suggestions').addEventListener('click', (e) => {
        if (e.target.tagName === 'LI') {
            searchInput.value = e.target.textContent;
            suggestionsList.innerHTML = '';
        }
    });

    searchInput.addEventListener('focus', () => {
        if (suggestionsList.children.length > 0) {
            suggestionsList.style.display = '';
        }
    });

    searchInput.addEventListener('blur', () => {
        setTimeout(() => {
            suggestionsList.style.display = 'none';
        }, 200);
    });

    document.body.addEventListener('htmx:afterSwap', (event) => {
        const target = event.detail.target;
        if (target.id === 'suggestions' && document.getElementById('searchInput') === document.activeElement) {
            target.style.display = target.children.length > 0 ? '' : 'none';
        }
    });
});

function applyColorScheme() {
    const isDarkMode = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    const htmlElement = document.documentElement;

    if (isDarkMode) {
        htmlElement.setAttribute('data-bs-theme', 'dark');
    } else {
        htmlElement.setAttribute('data-bs-theme', 'light');
    }
}
applyColorScheme();

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', event => {
    applyColorScheme();
});