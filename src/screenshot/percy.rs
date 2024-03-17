use std::{thread, time::Duration};

use serde::Deserialize;

use crate::{ImageUrl, SnapshotViewerUrl};

use super::{ScreenshotData, ScreenshotState};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Main {
    pub web_url: String,
    pub total_comparisons: u32,
    pub total_comparisons_diff: u32,
}

fn get_snapshots_with_retry(build_id: &str) -> SnapshotsData {
    let mut response = ureq::get(&format!(
        "https://percy.io/api/v1/builds/{}/snapshots",
        build_id
    ))
    .call();
    if response.is_err() {
        thread::sleep(Duration::from_secs(20));
        response = ureq::get(&format!(
            "https://percy.io/api/v1/builds/{}/snapshots",
            build_id
        ))
        .call();
    }

    let data = response.unwrap().into_json::<SnapshotsData>();

    if let Ok(data) = data {
        data
    } else {
        println!("error parsing Percy response: {:?}", data);
        SnapshotsData {
            data: vec![],
            included: vec![],
        }
    }
}

pub fn read_results(results: String) -> Vec<ScreenshotData> {
    // sleep to limit how hard Percy API are used
    thread::sleep(Duration::from_secs(1));

    let Ok(main) = serde_json::from_str::<Main>(&results) else {
        return vec![];
    };
    let build_id = main.web_url.split('/').last().unwrap();
    let data = get_snapshots_with_retry(build_id);

    snapshots_to_images(data, &main.web_url)
}

fn snapshots_to_images(snapshots: SnapshotsData, build_url: &str) -> Vec<ScreenshotData> {
    let mut images = Vec::new();
    for snapshot in snapshots.data {
        match snapshot {
            Snapshot::Snapshots {
                id,
                attributes,
                relationships,
                ..
            } => {
                let attributes = &attributes;
                for comparison_id in relationships
                    .comparisons
                    .data
                    .iter()
                    .map(|comp| comp.id.clone())
                {
                    let mut tag = None;
                    let (comparison_attributes, comparison_relationship) = snapshots
                        .included
                        .iter()
                        .find_map(|included| match included {
                            Snapshot::Comparisons {
                                id,
                                attributes,
                                relationships,
                            } if id == &comparison_id => Some((attributes, relationships)),
                            _ => None,
                        })
                        .unwrap();
                    if let Some(comparison_tag) =
                        comparison_relationship.comparison_tag.data.as_ref()
                    {
                        let comparison_tag_id = comparison_tag.id.clone();
                        let comparison_tag = snapshots
                            .included
                            .iter()
                            .find_map(|included| match included {
                                Snapshot::ComparisonTags { id, attributes }
                                    if id == &comparison_tag_id =>
                                {
                                    Some(attributes)
                                }
                                _ => None,
                            })
                            .unwrap();
                        tag = Some(format!(
                            "{} {} / {}",
                            comparison_tag.os_name, comparison_tag.os_version, comparison_tag.name
                        ))
                    }
                    let image_id = if attributes.review_state_reason == ReviewStateReason::NoDiffs {
                        let base_screenshot_id = comparison_relationship
                            .base_screenshot
                            .data
                            .as_ref()
                            .unwrap()
                            .id
                            .clone();
                        let base_screenshot = snapshots
                            .included
                            .iter()
                            .find_map(|included| match included {
                                Snapshot::Screenshots { id, relationships }
                                    if id == &base_screenshot_id =>
                                {
                                    Some(relationships)
                                }
                                _ => None,
                            })
                            .unwrap();
                        base_screenshot.image.data.as_ref().unwrap().id.clone()
                    } else if [
                        ReviewStateReason::UnreviewedComparisons,
                        ReviewStateReason::UserApproved,
                    ]
                    .contains(&attributes.review_state_reason)
                    {
                        let head_screenshot_id = comparison_relationship
                            .head_screenshot
                            .data
                            .as_ref()
                            .unwrap()
                            .id
                            .clone();
                        let head_screenshot = snapshots
                            .included
                            .iter()
                            .find_map(|included| match included {
                                Snapshot::Screenshots { id, relationships }
                                    if id == &head_screenshot_id =>
                                {
                                    Some(relationships)
                                }
                                _ => None,
                            })
                            .unwrap();
                        head_screenshot.image.data.as_ref().unwrap().id.clone()
                    } else {
                        "".to_string()
                    };
                    let image = snapshots
                        .included
                        .iter()
                        .find_map(|included| match included {
                            Snapshot::Images { id, attributes } if id == &image_id => {
                                Some(attributes)
                            }
                            _ => None,
                        })
                        .unwrap();

                    let snapshot_url = format!(
                        "{}/{}/{}",
                        build_url,
                        if attributes.review_state_reason == ReviewStateReason::NoDiffs {
                            "unchanged"
                        } else {
                            "changed"
                        },
                        id
                    );

                    images.push(ScreenshotData {
                        example: attributes.name.clone(),
                        screenshot: ImageUrl(image.url.clone()),
                        changed: (&attributes.review_state_reason).into(),
                        diff_ratio: comparison_attributes.diff_ratio.unwrap_or(9999.99),
                        tag,
                        snapshot_url: SnapshotViewerUrl(snapshot_url.to_owned()),
                    });
                }
            }
            _ => {}
        }
    }
    images
}

#[derive(Deserialize, Debug)]
struct SnapshotsData {
    data: Vec<Snapshot>,
    included: Vec<Snapshot>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
enum Snapshot {
    Snapshots {
        id: String,
        attributes: SnapshotAttributes,
        relationships: SnapshotRelationship,
    },
    Images {
        id: String,
        attributes: ImageAttributes,
    },
    Comparisons {
        id: String,
        attributes: ComparisonAttributes,
        relationships: ComparisonRelationship,
    },
    Screenshots {
        id: String,
        relationships: ScreenshotRelationship,
    },
    Builds,
    Browsers,
    BrowserFamilies,
    ComparisonTags {
        id: String,
        attributes: ComparisonTagAttributes,
    },
    IgnoredRegions,
    Projects,
    Users,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct RelationshipDatas {
    data: Vec<RelationshipData>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct RelationshipSingleData {
    data: Option<RelationshipData>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct RelationshipData {
    id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct SnapshotAttributes {
    name: String,
    review_state_reason: ReviewStateReason,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStateReason {
    NoDiffs,
    UnreviewedComparisons,
    UserApproved,
}

impl From<&ReviewStateReason> for ScreenshotState {
    fn from(reason: &ReviewStateReason) -> Self {
        match reason {
            ReviewStateReason::NoDiffs => ScreenshotState::Similar,
            _ => ScreenshotState::Changed,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct SnapshotRelationship {
    comparisons: RelationshipDatas,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct ImageAttributes {
    url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct ScreenshotRelationship {
    image: RelationshipSingleData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct ComparisonRelationship {
    base_screenshot: RelationshipSingleData,
    head_screenshot: RelationshipSingleData,
    comparison_tag: RelationshipSingleData,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct ComparisonAttributes {
    diff_ratio: Option<f32>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct ComparisonTagAttributes {
    name: String,
    os_name: String,
    os_version: String,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::screenshot::percy::SnapshotsData;

    use super::*;

    #[test]
    fn read_file_native() {
        let file = fs::read_to_string("src/screenshot/test-percy.json").unwrap();
        // dbg!(read_percy_results(file));
        let read = serde_json::from_str::<SnapshotsData>(&file).unwrap();
        dbg!(read.data.len());
        dbg!(read.included.len());
        dbg!(&read.data[0]);
        dbg!(snapshots_to_images(read, ""));
        // assert!(false);
    }

    #[test]
    fn read_file_mobile() {
        let file = fs::read_to_string("src/screenshot/test-percy-mobile.json").unwrap();
        // dbg!(read_percy_results(file));
        let read = serde_json::from_str::<SnapshotsData>(&file).unwrap();
        dbg!(read.data.len());
        dbg!(read.included.len());
        dbg!(&read.data[0]);
        dbg!(snapshots_to_images(read, ""));
        // assert!(false);
    }
}
