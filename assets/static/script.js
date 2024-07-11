function toggleSidebar() {
    var sidebar = document.getElementById("sidebar");
    if (sidebar.style.display === "none") {
        sidebar.style.setProperty('display', 'flex', 'important');
        sidebarbackground.style.setProperty('display','block');
    } else {
        sidebar.style.setProperty('display', 'none', 'important');
        sidebarbackground.style.setProperty('display','none');
    }
}
