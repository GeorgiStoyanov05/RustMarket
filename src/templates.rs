use handlebars::Handlebars;
use std::sync::Arc;

pub type Hbs = Arc<Handlebars<'static>>;

pub fn build_handlebars() -> Hbs {
    let mut hb = Handlebars::new();

    // Layout + pages
    hb.register_template_file("layouts/base", "templates/layouts/base.hbs")
        .expect("template layouts/base");

    hb.register_template_file("pages/home", "templates/pages/home.hbs")
        .expect("template pages/home");
    hb.register_template_file("pages/not_found", "templates/pages/not_found.hbs")
        .expect("template pages/not_found");
    hb.register_template_file("pages/login", "templates/pages/login.hbs")
        .expect("template pages/login");
    hb.register_template_file("pages/register", "templates/pages/register.hbs")
        .expect("template pages/register");
    hb.register_template_file("pages/search", "templates/pages/search.hbs")
        .expect("template pages/search");
    hb.register_template_file("pages/details", "templates/pages/details.hbs")
        .expect("template pages/details");

    // NEW pages
    hb.register_template_file("pages/portfolio", "templates/pages/portfolio.hbs")
        .expect("template pages/portfolio");
    hb.register_template_file("pages/alerts", "templates/pages/alerts.hbs")
        .expect("template pages/alerts");
    hb.register_template_file("pages/settings", "templates/pages/settings.hbs")
        .expect("template pages/settings");

    // Partial endpoints
    hb.register_template_file("partials/search_results", "templates/partials/search_results.hbs")
        .expect("template partials/search_results");

    hb.register_template_file("partials/alerts_list", "templates/partials/alerts_list.hbs")
        .expect("template partials/alerts_list");

    hb.register_template_file("partials/watchlist_alerts", "templates/partials/watchlist_alerts.hbs")
        .expect("template partials/watchlist_alerts");

    hb.register_template_file("partials/position_panel", "templates/partials/position_panel.hbs")
        .expect("template partials/position_panel");

    hb.register_template_file("partials/portfolio_positions", "templates/partials/portfolio_positions.hbs")
        .expect("template partials/portfolio_positions");
    let navbar = std::fs::read_to_string("templates/partials/navbar.hbs")
        .expect("partials/navbar.hbs");
    hb.register_partial("navbar", navbar).expect("register navbar partial");

    let footer = std::fs::read_to_string("templates/partials/footer.hbs")
        .expect("partials/footer.hbs");
    hb.register_partial("footer", footer).expect("register footer partial");

    Arc::new(hb)
}
