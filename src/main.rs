use chrono::NaiveDateTime;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::Hash,
    str::FromStr,
};

use crate::screenshot::{percy, pixeleagle, ScreenshotData, ScreenshotState};

mod screenshot;
mod template;

#[derive(Debug, Clone, Serialize)]
struct Example {
    name: String,
    category: ExampleCategory,
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
    results: HashMap<String, HashMap<String, Kind>>,
    screenshots: HashMap<String, HashMap<String, (ImageUrl, ScreenshotState, SnapshotViewerUrl)>>,
    logs: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, Hash)]
struct ExampleCategory(String);

#[derive(Debug, Serialize, Clone, PartialEq, Eq, Hash)]
struct ImageUrl(String);

#[derive(Debug, Serialize, Clone, PartialEq, Eq, Hash)]
struct SnapshotViewerUrl(String);

#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
enum Platform {
    Linux,
    Macos,
    Windows,
    Mobile,
    Tag(String),
}

impl ToString for Platform {
    fn to_string(&self) -> String {
        match self {
            Platform::Linux => String::from("Linux"),
            Platform::Macos => String::from("macOS"),
            Platform::Windows => String::from("Windows"),
            Platform::Mobile => String::from("Mobile"),
            Platform::Tag(tag) => tag.clone(),
        }
    }
}

impl FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Linux" => Ok(Platform::Linux),
            "macOS" => Ok(Platform::Macos),
            "Windows" => Ok(Platform::Windows),
            "mobile" => Ok(Platform::Mobile),
            _ => Err(s.to_string()),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
enum Kind {
    Successes,
    Failures,
    NoScreenshots,
    Percy,
    PixelEagle,
}

impl FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "successes" => Ok(Kind::Successes),
            "failures" => Ok(Kind::Failures),
            "no_screenshots" => Ok(Kind::NoScreenshots),
            "percy" => Ok(Kind::Percy),
            "pixeleagle" => Ok(Kind::PixelEagle),
            _ => Err(s.to_string()),
        }
    }
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

    for (i, run_path) in folders.iter().take(15).enumerate() {
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
            let file = file.as_ref().unwrap();
            if file.file_type().unwrap().is_dir() {
                continue;
            }
            let path = file.path();
            let mut name = path.file_name().unwrap().to_str().unwrap().split('-');
            let platform = Platform::from_str(name.next().unwrap()).unwrap();
            let kind = Kind::from_str(name.next().unwrap()).unwrap();

            if [Kind::Successes, Kind::Failures, Kind::NoScreenshots].contains(&kind) {
                println!("  - {:?} / {:?}", kind, platform);
                fs::read_to_string(&path).unwrap().lines().for_each(|line| {
                    let mut line = line.split(" - ");
                    let mut details = line.next().unwrap().split('/');
                    let example = Example {
                        category: ExampleCategory(details.next().unwrap().to_string()),
                        name: details.next().unwrap().to_string(),
                        flaky: kind != Kind::Successes,
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
                        .insert(platform.to_string().clone(), kind.clone());
                });
            }
            if [Kind::Percy, Kind::PixelEagle].contains(&kind) {
                println!("  - {:?} / {:?}", kind, platform);
                let content = fs::read_to_string(&path).unwrap();
                let screenshots = match kind {
                    Kind::Percy => percy::read_results(content),
                    Kind::PixelEagle => pixeleagle::read_results(content),
                    _ => unreachable!(),
                };
                for ScreenshotData {
                    example,
                    screenshot,
                    mut changed,
                    tag,
                    diff_ratio,
                    snapshot_url,
                } in screenshots.into_iter()
                {
                    let (category, name) = if platform == Platform::Mobile {
                        if let Some(tag) = tag.as_ref() {
                            all_mobile_platforms.insert(tag.clone());
                        }
                        (ExampleCategory("Mobile".to_string()), example)
                    } else {
                        let mut split = example.split('.').next().unwrap().split('/');
                        (
                            ExampleCategory(split.next().unwrap().to_string()),
                            split.next().unwrap().to_string(),
                        )
                    };
                    let example = Example {
                        category,
                        name,
                        flaky: false,
                    };
                    if changed == ScreenshotState::Changed {
                        let previous = all_examples.take(&example).unwrap_or(example.clone());
                        all_examples.insert(Example {
                            flaky: true,
                            ..previous
                        });
                    }
                    if diff_ratio == 0.0 && changed == ScreenshotState::Changed {
                        println!(
                            "    - setting {} / {} ({:?}) as unchanged",
                            example.category.0, example.name, tag
                        );
                        changed = ScreenshotState::Similar;
                    }
                    let platform = tag
                        .clone()
                        .map(|tag| Platform::Tag(tag.clone()))
                        .unwrap_or_else(|| platform.clone())
                        .to_string();
                    // If there is a screenshot but no results, mark as success
                    run.results
                        .entry(example.name.clone())
                        .or_insert_with(HashMap::new)
                        .entry(platform.clone())
                        .or_insert_with(|| Kind::Successes);
                    // Keeping Percy results over PixelEagle for now
                    // TODO: remove
                    if let Some(existing_screenshots) = run.screenshots.get(&example.name) {
                        if existing_screenshots.contains_key(&platform) {
                            if kind == Kind::PixelEagle {
                                continue;
                            }
                        }
                    }
                    run.screenshots
                        .entry(example.name)
                        .or_insert_with(HashMap::new)
                        .insert(platform.clone(), (screenshot, changed, snapshot_url));
                }
            }
        }
        for rerun_platform in [Platform::Linux, Platform::Windows, Platform::Macos] {
            let rerun = run_path.join(format!("status-rerun-{:?}", rerun_platform));
            if rerun.exists() {
                println!("  - rerun {:?}", rerun_platform);
                for file in fs::read_dir(rerun.as_path()).unwrap() {
                    let path = file.as_ref().unwrap().path();
                    let kind = path.file_name().unwrap().to_str().unwrap();
                    if kind == "successes" {
                        println!("    - {} / {:?}", kind, rerun_platform);
                        fs::read_to_string(file.as_ref().unwrap().path())
                            .unwrap()
                            .lines()
                            .for_each(|line| {
                                let mut line = line.split(" - ");
                                let mut details = line.next().unwrap().split('/');
                                let example = Example {
                                    category: ExampleCategory(details.next().unwrap().to_string()),
                                    name: details.next().unwrap().to_string(),
                                    flaky: false,
                                };
                                run.results
                                    .entry(example.name)
                                    .or_insert_with(HashMap::new)
                                    .insert(rerun_platform.to_string(), Kind::NoScreenshots);
                            });
                    }
                    if kind.ends_with(".log") {
                        let example_name = kind.strip_suffix(".log").unwrap();
                        println!("    - log / {:?} ({})", rerun_platform, example_name);
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
                            .insert(rerun_platform.to_string(), log);
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
                .map(|platforms| platforms.values().any(|v| v == &Kind::Failures))
                .unwrap_or(false)
        });
        if !has_screenshot && !has_failures {
            example.flaky = false;
        }
        all_examples_cleaned.push(example);
    }

    all_examples_cleaned.sort_by_key(|a| format!("{}/{}", a.category.0, a.name));

    template::build_site(runs, all_examples_cleaned, all_mobile_platforms)
}
