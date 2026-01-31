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

    // Partials
    let navbar = std::fs::read_to_string("templates/partials/navbar.hbs")
        .expect("partials/navbar.hbs");
    hb.register_partial("navbar", navbar).expect("register navbar partial");

    let footer = std::fs::read_to_string("templates/partials/footer.hbs")
        .expect("partials/footer.hbs");
    hb.register_partial("footer", footer).expect("register footer partial");

    Arc::new(hb)
}
