use chrono::NaiveDateTime;
use percy::read_percy_results;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::Hash,
    thread,
    time::Duration,
};
use tera::{Context, Tera};

use crate::percy::ScreenshotData;

mod percy;

#[derive(Debug, Clone, Serialize)]
struct Example {
    name: String,
    category: String,
    flaky: bool,
}

impl PartialEq for Example {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.category == other.category
    }
}
impl Eq for Example {}
impl Hash for Example {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.category.hash(state);
    }
}

#[derive(Debug, Serialize, Default)]
struct Run {
    date: String,
    commit: String,
    results: HashMap<String, HashMap<String, String>>,
    screenshots: HashMap<String, HashMap<String, (String, String, String)>>,
    logs: HashMap<String, HashMap<String, String>>,
}

fn main() {
    let paths = fs::read_dir(std::env::args().nth(1).as_deref().unwrap()).unwrap();

    let _ = fs::create_dir("./site");

    let mut all_examples = HashSet::new();
    let mut runs = vec![];
    let mut all_mobile_platforms = HashSet::new();

    let mut folders = paths
        .filter_map(|dir| dir.map(|d| d.path()).ok())
        .collect::<Vec<_>>();
    folders.sort();
    folders.reverse();

    for (i, run_path) in folders.iter().take(40).enumerate() {
        let file_name = run_path.file_name().unwrap().to_str().unwrap();
        if file_name.starts_with(".") {
            continue;
        }
        println!("Processing {:?} ({})", run_path, i);
        let mut split = file_name.split('-');
        let mut run = Run {
            date: NaiveDateTime::parse_from_str(split.next().unwrap(), "%Y%m%d%H%M")
                .unwrap()
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            commit: split.next().unwrap().to_string(),
            ..Default::default()
        };

        for file in fs::read_dir(run_path).unwrap() {
            let path = file.as_ref().unwrap().path();
            let mut name = path.file_name().unwrap().to_str().unwrap().split('-');
            let platform = name.next().unwrap();
            let kind = name.next().unwrap();

            if ["successes", "failures", "no_screenshots"].contains(&kind) {
                println!("  - {} / {}", kind, platform);
                fs::read_to_string(file.as_ref().unwrap().path())
                    .unwrap()
                    .lines()
                    .for_each(|line| {
                        let mut line = line.split(" - ");
                        let mut details = line.next().unwrap().split('/');
                        let example = Example {
                            category: details.next().unwrap().to_string(),
                            name: details.next().unwrap().to_string(),
                            flaky: kind != "successes",
                        };
                        let previous = all_examples.take(&example);
                        all_examples.insert(Example {
                            flaky: previous.map(|ex: Example| ex.flaky).unwrap_or(false)
                                || example.flaky,
                            ..example.clone()
                        });
                        run.results
                            .entry(example.name)
                            .or_insert_with(HashMap::new)
                            .insert(platform.to_string(), kind.to_string());
                    });
            }
            if kind == "percy" {
                println!("  - {} / {}", kind, platform);
                let screenshots =
                    read_percy_results(fs::read_to_string(file.as_ref().unwrap().path()).unwrap());
                // sleep to limit how hard Percy API are used
                thread::sleep(Duration::from_secs(3));
                for ScreenshotData {
                    example,
                    screenshot,
                    mut changed,
                    tag,
                    diff_ratio,
                    snapshot_url,
                } in screenshots.into_iter()
                {
                    let (category, name) = if platform == "mobile" {
                        if let Some(tag) = tag.as_ref() {
                            all_mobile_platforms.insert(tag.clone());
                        }
                        ("Mobile".to_string(), example)
                    } else {
                        let mut split = example.split('.').next().unwrap().split('/');
                        (
                            split.next().unwrap().to_string(),
                            split.next().unwrap().to_string(),
                        )
                    };
                    let example = Example {
                        category,
                        name,
                        flaky: false,
                    };
                    if changed != "no_diffs" {
                        let previous = all_examples.take(&example).unwrap_or(example.clone());
                        all_examples.insert(Example {
                            flaky: true,
                            ..previous
                        });
                    }
                    if diff_ratio == 0.0 && changed != "no_diffs" {
                        println!(
                            "    - setting {} / {} ({:?}) as unchanged",
                            example.category, example.name, tag
                        );
                        changed = "no_diffs".to_string();
                    }
                    // If there is a screenshot but no results, mark as success
                    run.results
                        .entry(example.name.clone())
                        .or_insert_with(HashMap::new)
                        .entry(tag.clone().unwrap_or_else(|| platform.to_string()))
                        .or_insert_with(|| "successes".to_string());
                    run.screenshots
                        .entry(example.name)
                        .or_insert_with(HashMap::new)
                        .insert(
                            tag.unwrap_or_else(|| platform.to_string()),
                            (screenshot, changed, snapshot_url),
                        );
                }
            }
        }
        for platform in ["Windows", "Linux", "macOS"] {
            let rerun = run_path.join(format!("status-rerun-{}", platform));
            if rerun.exists() {
                println!("  - rerun {}", platform);
                for file in fs::read_dir(rerun.as_path()).unwrap() {
                    let path = file.as_ref().unwrap().path();
                    let kind = path.file_name().unwrap().to_str().unwrap();
                    if kind == "successes" {
                        println!("    - {} / {}", kind, platform);
                        fs::read_to_string(file.as_ref().unwrap().path())
                            .unwrap()
                            .lines()
                            .for_each(|line| {
                                let mut line = line.split(" - ");
                                let mut details = line.next().unwrap().split('/');
                                let example = Example {
                                    category: details.next().unwrap().to_string(),
                                    name: details.next().unwrap().to_string(),
                                    flaky: false,
                                };
                                run.results
                                    .entry(example.name)
                                    .or_insert_with(HashMap::new)
                                    .insert(platform.to_string(), "no_screenshots".to_string());
                            });
                    }
                    if kind.ends_with(".log") {
                        let example_name = kind.strip_suffix(".log").unwrap();
                        println!("    - log / {} ({})", platform, example_name);
                        let mut log = fs::read_to_string(file.as_ref().unwrap().path()).unwrap();
                        log = log.replace("[0m", "");
                        log = log.replace("[1m", "");
                        log = log.replace("[2m", "");
                        log = log.replace("[31m", "");
                        log = log.replace("[32m", "");
                        log = log.replace("[33m", "");
                        run.logs
                            .entry(example_name.to_string())
                            .or_insert_with(HashMap::new)
                            .insert(platform.to_string(), log);
                    }
                }
            }
        }
        runs.push(run);
    }

    let mut all_examples_cleaned = Vec::new();
    // examples that never have screenshot are not flaky
    for mut example in all_examples.drain() {
        let has_screenshot = runs
            .iter()
            .any(|run| run.screenshots.get(&example.name).is_some());
        let has_failures = runs.iter().any(|run| {
            run.results
                .get(&example.name)
                .map(|platforms| platforms.values().any(|v| v == "failures"))
                .unwrap_or(false)
        });
        if !has_screenshot && !has_failures {
            example.flaky = false;
        }
        all_examples_cleaned.push(example);
    }

    all_examples_cleaned.sort_by_key(|a| format!("{}/{}", a.category, a.name));

    let mut context = Context::new();
    context.insert("runs".to_string(), &runs);
    context.insert("all_examples".to_string(), &all_examples_cleaned);
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
