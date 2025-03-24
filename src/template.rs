use std::collections::{HashMap, HashSet};

use serde::Serialize;
use tera::{Context, Tera};

use crate::{screenshot::ScreenshotState, Example, ImageUrl, Kind, Run, SnapshotViewerUrl};

#[derive(Debug, Serialize, Default)]
struct StringRun {
    date: String,
    commit: String,
    results: HashMap<String, HashMap<String, Kind>>,
    screenshots: HashMap<String, HashMap<String, (ImageUrl, ScreenshotState, SnapshotViewerUrl)>>,
    logs: HashMap<String, HashMap<String, String>>,
}

impl From<Run> for StringRun {
    fn from(value: Run) -> Self {
        StringRun {
            date: value.date.clone(),
            commit: value.commit.clone(),
            results: value
                .results
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                    )
                })
                .collect(),
            screenshots: value
                .screenshots
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                    )
                })
                .collect(),

            logs: value.logs.clone(),
        }
    }
}

pub fn build_site(
    runs: Vec<Run>,
    all_examples: Vec<Example>,
    all_mobile_platforms: HashSet<String>,
) {
    let runs: Vec<StringRun> = runs.into_iter().map(|r| r.into()).collect();
    let mut context = Context::new();
    context.insert("runs".to_string(), &runs);
    context.insert("all_examples".to_string(), &all_examples);
    context.insert("all_mobile_platforms".to_string(), &all_mobile_platforms);

    let mut tera = Tera::default();
    tera.add_raw_template(
        "icons.html",
        &std::fs::read_to_string("./templates/icons.html").unwrap(),
    )
    .unwrap();
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
