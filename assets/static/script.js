function toggleSidebar() {
    var isMobile = window.matchMedia("(max-width: 1000px)").matches;
    if (isMobile) {
        document.getElementById("sidebar").classList.toggle("sidebar-open");
        document.getElementById("sidebarbackground").classList.toggle("sidebar-open");
    } else {
        document.body.classList.toggle("sidebar-collapsed");
    }
}

function navbarSearch(event) {
    event.preventDefault();
    var input = document.getElementById('searchInput');
    if (input && input.value.trim().length > 0) {
        window.location.href = '/search?q=' + encodeURIComponent(input.value.trim());
    }
    return false;
}

document.addEventListener('DOMContentLoaded', () => {
    const sidebarBg = document.getElementById("sidebarbackground");
    if (sidebarBg) {
        sidebarBg.addEventListener("click", function () {
            if (window.matchMedia("(max-width: 1000px)").matches) {
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
});

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function() {
    var d = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-bs-theme', d ? 'dark' : 'light');
});

function closeListModal(event) {
    if (event.target.id === 'listModalOverlay') {
        event.target.style.display = 'none';
    }
}

function togglePdfFullscreen() {
    var wrapper = document.getElementById('pdfViewerWrapper');
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

function toggleDocumentFullscreen() {
    var wrapper = document.getElementById('documentViewerWrapper');
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

document.addEventListener('fullscreenchange', function () {
    var pdfIcon = document.getElementById('pdfFullscreenIcon');
    var docIcon = document.getElementById('documentFullscreenIcon');
    var isFullscreen = !!document.fullscreenElement;
    [pdfIcon, docIcon].forEach(function(icon) {
        if (!icon) return;
        if (isFullscreen) {
            icon.classList.remove('fa-expand');
            icon.classList.add('fa-compress');
        } else {
            icon.classList.remove('fa-compress');
            icon.classList.add('fa-expand');
        }
    });
});

document.addEventListener('webkitfullscreenchange', function () {
    var pdfIcon = document.getElementById('pdfFullscreenIcon');
    var docIcon = document.getElementById('documentFullscreenIcon');
    var isFullscreen = !!document.webkitFullscreenElement;
    [pdfIcon, docIcon].forEach(function(icon) {
        if (!icon) return;
        if (isFullscreen) {
            icon.classList.remove('fa-expand');
            icon.classList.add('fa-compress');
        } else {
            icon.classList.remove('fa-compress');
            icon.classList.add('fa-expand');
        }
    });
});
