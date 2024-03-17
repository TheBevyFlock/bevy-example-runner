use std::collections::HashSet;

use tera::{Context, Tera};

use crate::{Example, Run};

pub fn build_site(
    runs: Vec<Run>,
    all_examples: Vec<Example>,
    all_mobile_platforms: HashSet<String>,
) {
    let mut context = Context::new();
    context.insert("runs".to_string(), &runs);
    context.insert("all_examples".to_string(), &all_examples);
    context.insert("all_mobile_platforms".to_string(), &all_mobile_platforms);

    let mut tera = Tera::default();
    tera.add_raw_template(
        "macros.html",
        &std::fs::read_to_string("./templates/macros.html").unwrap(),
    )
    .unwrap();
    tera.add_raw_template(
        "index.html",
        &std::fs::read_to_string("./templates/index.html").unwrap(),
    )
    .unwrap();
    tera.add_raw_template(
        "about.html",
        &std::fs::read_to_string("./templates/about.html").unwrap(),
    )
    .unwrap();

    let rendered = tera.render("index.html", &context).unwrap();
    std::fs::write("./site/index.html", &rendered).unwrap();

    let rendered = tera.render("about.html", &context).unwrap();
    std::fs::write("./site/about.html", &rendered).unwrap();
}
