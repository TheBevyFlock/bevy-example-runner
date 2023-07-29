use chrono::NaiveDateTime;
use percy::read_percy_results;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs, thread,
    time::Duration,
};
use tera::{Context, Tera};

mod percy;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize)]
struct Example {
    name: String,
    category: String,
}

#[derive(Debug, Serialize, Default)]
struct Run {
    date: String,
    commit: String,
    results: HashMap<String, HashMap<String, String>>,
    screenshots: HashMap<String, HashMap<String, String>>,
}

fn main() {
    let paths = fs::read_dir(std::env::args().nth(1).as_deref().unwrap()).unwrap();

    let mut all_examples = HashSet::new();
    let mut runs = vec![];

    let mut folders = paths
        .filter_map(|dir| dir.map(|d| d.path()).ok())
        .collect::<Vec<_>>();
    folders.sort();
    folders.reverse();

    for path in folders {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.starts_with(".") {
            continue;
        }
        println!("Processing {:?}", path);
        let mut split = file_name.split('-');
        let mut run = Run {
            date: NaiveDateTime::parse_from_str(split.next().unwrap(), "%Y%m%d%H%M")
                .unwrap()
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            commit: split.next().unwrap().to_string(),
            ..Default::default()
        };

        for file in fs::read_dir(path).unwrap() {
            let path = file.as_ref().unwrap().path();
            let mut name = path.file_name().unwrap().to_str().unwrap().split('-');
            let platform = name.next().unwrap();
            let kind = name.next().unwrap();

            if ["successes", "failures", "no_screenshots"].contains(&kind) {
                fs::read_to_string(file.as_ref().unwrap().path())
                    .unwrap()
                    .lines()
                    .for_each(|line| {
                        let mut line = line.split(" - ");
                        let mut details = line.next().unwrap().split('/');
                        let example = Example {
                            category: details.next().unwrap().to_string(),
                            name: details.next().unwrap().to_string(),
                        };
                        all_examples.insert(example.clone());
                        run.results
                            .entry(example.name)
                            .or_insert_with(HashMap::new)
                            .insert(platform.to_string(), kind.to_string());
                    });
            }
            if kind == "percy" {
                let screenshots =
                    read_percy_results(fs::read_to_string(file.as_ref().unwrap().path()).unwrap());
                thread::sleep(Duration::from_secs(10));
                for (example, screenshot) in screenshots.into_iter() {
                    run.screenshots
                        .entry(
                            example
                                .split('/')
                                .last()
                                .unwrap()
                                .to_string()
                                .split('.')
                                .next()
                                .unwrap()
                                .to_string(),
                        )
                        .or_insert_with(HashMap::new)
                        .insert(platform.to_string(), screenshot);
                }
            }
        }
        runs.push(run);
    }

    let mut sorted_examples = all_examples.iter().collect::<Vec<_>>();
    sorted_examples.sort_by_key(|a| format!("{}/{}", a.category, a.name));

    let mut context = Context::new();
    context.insert("runs".to_string(), &runs);
    context.insert("all_examples".to_string(), &sorted_examples);

    let mut tera = Tera::default();
    tera.add_raw_template(
        "index.html",
        &std::fs::read_to_string("./templates/index.html").unwrap(),
    )
    .unwrap();
    let rendered = tera.render("index.html", &context).unwrap();

    std::fs::write("./index.html", &rendered).unwrap();
}
