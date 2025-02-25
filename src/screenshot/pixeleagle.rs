use serde::Deserialize;

use crate::{ImageUrl, SnapshotViewerUrl};

use super::{ScreenshotData, ScreenshotState};

#[derive(Deserialize)]
struct ComparisonTarget {
    project_id: String,
    from: u32,
    to: u32,
}

#[derive(Deserialize, Debug)]
struct Comparison {
    project_id: String,
    from: u32,
    to: u32,
    // missing: Vec<Screenshot>,
    new: Vec<Screenshot>,
    diff: Vec<Screenshot>,
    unchanged: Vec<Screenshot>,
}

#[derive(Deserialize, Debug)]
struct Screenshot {
    name: String,
    hash: String,
    // previous_hash: Option<String>,
    diff: Option<Difference>,
}

#[derive(Deserialize, Debug)]
enum Difference {
    Unknown,
    Processing,
    Done(f32),
}

pub fn read_results(results: String) -> Vec<ScreenshotData> {
    let Ok(target) = serde_json::from_str::<ComparisonTarget>(&results) else {
        return vec![];
    };

    let screenshots = ureq::get(&format!(
        "https://pixel-eagle.com/{}/runs/{}/compare/{}",
        target.project_id, target.from, target.to
    ))
    .call()
    .unwrap()
    .into_json::<Comparison>()
    .unwrap();

    comparison_to_screenshot_data(screenshots)
}

fn comparison_to_screenshot_data(comparison: Comparison) -> Vec<ScreenshotData> {
    let mut result = vec![];

    for screenshot in comparison.new {
        result.push(ScreenshotData {
            example: screenshot.name.clone(),
            screenshot: ImageUrl(format!(
                "https://pixel-eagle.com/files/{}/screenshot/{}",
                comparison.project_id.clone(),
                screenshot.hash.clone()
            )),
            changed: ScreenshotState::Changed,
            tag: None,
            diff_ratio: 0.0,
            snapshot_url: SnapshotViewerUrl(format!(
                "https://pixel-eagle.com/project/{}/run/{}/compare/{}?screenshot={}",
                comparison.project_id, comparison.from, comparison.to, screenshot.name
            )),
        });
    }

    for screenshot in comparison.unchanged {
        result.push(ScreenshotData {
            example: screenshot.name.clone(),
            screenshot: ImageUrl(format!(
                "https://pixel-eagle.com/{}/screenshot/{}",
                comparison.project_id.clone(),
                screenshot.hash.clone()
            )),
            changed: ScreenshotState::Similar,
            tag: None,
            diff_ratio: 0.0,
            snapshot_url: SnapshotViewerUrl(format!(
                "https://pixel-eagle.com/project/{}/run/{}/compare/{}?screenshot={}",
                comparison.project_id, comparison.from, comparison.to, screenshot.name
            )),
        });
    }

    for screenshot in comparison.diff {
        result.push(ScreenshotData {
            example: screenshot.name.clone(),
            screenshot: ImageUrl(format!(
                "https://pixel-eagle.com/{}/screenshot/{}",
                comparison.project_id.clone(),
                screenshot.hash.clone()
            )),
            changed: ScreenshotState::Changed,
            tag: None,
            diff_ratio: screenshot
                .diff
                .map(|diff| match diff {
                    Difference::Done(ratio) => ratio,
                    _ => 1.0,
                })
                .unwrap(),
            snapshot_url: SnapshotViewerUrl(format!(
                "https://pixel-eagle.com/project/{}/run/{}/compare/{}?screenshot={}",
                comparison.project_id, comparison.from, comparison.to, screenshot.name
            )),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn read_file_native() {
        let file = fs::read_to_string("src/screenshot/test-pixeleagle.json").unwrap();
        let read = serde_json::from_str::<Comparison>(&file).unwrap();
        // dbg!(read.diff);
        dbg!(comparison_to_screenshot_data(read));
        // assert!(false);
    }
}
