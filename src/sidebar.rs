#[derive(Template)]
#[template(path = "pages/component-sidebar.html")]
struct SidebarComponentTemplate {
    config: Config,
    active_item: String,
    current_user: Option<User>,
    locale: RequestLocale,
}
fn generate_sidebar(
    config: &Config,
    active_item: String,
    current_user: Option<User>,
    locale: RequestLocale,
) -> String {
    let template = SidebarComponentTemplate {
        config: config.to_owned(),
        active_item,
        current_user,
        locale,
    };
    template.render().unwrap()
}
