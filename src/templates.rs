use handlebars::{handlebars_helper, Handlebars, JsonValue};
use std::path::Path;
use std::sync::Arc;

pub type Hbs = Arc<Handlebars<'static>>;

fn register_file(hb: &mut Handlebars<'static>, name: &str, path: &str) {
    if Path::new(path).exists() {
        hb.register_template_file(name, path)
            .unwrap_or_else(|e| panic!("Failed to register {name} from {path}: {e}"));
    } else {
        eprintln!("[templates] WARNING missing file: {path} (skipping register: {name})");
    }
}

pub fn build_handlebars() -> Hbs {
    let mut hb = Handlebars::new();

    handlebars_helper!(eq: |a: JsonValue, b: JsonValue| a == b);
    hb.register_helper("eq", Box::new(eq));

    register_file(&mut hb, "layouts/base", "templates/layouts/base.hbs");

    register_file(&mut hb, "pages/home", "templates/pages/home.hbs");
    register_file(&mut hb, "pages/not_found", "templates/pages/not_found.hbs");
    register_file(&mut hb, "pages/login", "templates/pages/login.hbs");
    register_file(&mut hb, "pages/register", "templates/pages/register.hbs");
    register_file(&mut hb, "pages/search", "templates/pages/search.hbs");
    register_file(&mut hb, "pages/details", "templates/pages/details.hbs");
    register_file(&mut hb, "pages/portfolio", "templates/pages/portfolio.hbs");
    register_file(&mut hb, "pages/alerts", "templates/pages/alerts.hbs");
    register_file(&mut hb, "pages/funds", "templates/pages/funds.hbs");
    register_file(&mut hb, "pages/settings", "templates/pages/settings.hbs");

    register_file(&mut hb, "partials/search_results", "templates/partials/search_results.hbs");
    register_file(&mut hb, "partials/quote", "templates/partials/quote.hbs");
    register_file(&mut hb, "partials/alerts_list", "templates/partials/alerts_list.hbs");
    register_file(&mut hb, "partials/watchlist_alerts", "templates/partials/watchlist_alerts.hbs");
    register_file(&mut hb, "partials/position_panel", "templates/partials/position_panel.hbs");
    register_file(&mut hb, "partials/portfolio_positions", "templates/partials/portfolio_positions.hbs");

    // ✅ NEW
    register_file(&mut hb, "partials/portfolio_position_card", "templates/partials/portfolio_position_card.hbs");

    register_file(&mut hb, "partials/funds_modal", "templates/partials/funds_modal.hbs");
    register_file(&mut hb, "partials/cash_badge", "templates/partials/cash_badge.hbs");
    // Settings
    register_file(&mut hb, "partials/change_email", "templates/partials/change_email.hbs");
    register_file(&mut hb, "partials/change_password", "templates/partials/change_password.hbs");
    register_file(&mut hb, "partials/orders_list", "templates/partials/orders_list.hbs");
    if Path::new("templates/partials/navbar.hbs").exists() {
        let navbar = std::fs::read_to_string("templates/partials/navbar.hbs")
            .expect("partials/navbar.hbs");
        hb.register_partial("navbar", navbar)
            .expect("register navbar partial");
    }
    if Path::new("templates/partials/footer.hbs").exists() {
        let footer = std::fs::read_to_string("templates/partials/footer.hbs")
            .expect("partials/footer.hbs");
        hb.register_partial("footer", footer)
            .expect("register footer partial");
    }

    Arc::new(hb)
}
