pub mod artifact;
pub mod camera;
pub mod html;
pub mod manifest;
pub mod model;
pub mod pdf;
pub mod telemetry;
pub mod tools;

#[cfg(test)]
mod seed_tests {
    //! The sample project ships inside the Windows zip, so it has to stay openable: every
    //! reference it names must exist, and a new marking must not collide with an existing id.

    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    fn seed() -> crate::model::Project {
        let text = std::fs::read_to_string(root().join("seed/project.json")).expect("seed/project.json is committed");
        serde_json::from_str(&text).expect("seed/project.json parses")
    }

    #[test]
    fn every_reference_the_sample_names_is_in_the_repo() {
        let dir = root().join("seed");
        for artifact in &seed().artifacts {
            let rel = artifact.source.path().trim_start_matches("seed/");
            let full = dir.join(rel);
            assert!(full.is_file(), "{} points at {} which is missing", artifact.name, full.display());
        }
    }

    #[test]
    fn a_new_marking_gets_an_id_no_existing_marking_uses() {
        let project = seed();
        let mut ids: Vec<u32> = project.artifacts.iter().flat_map(|a| a.annotations.iter().map(|n| n.id)).collect();
        let highest = ids.iter().copied().max().unwrap_or(0);
        assert!(
            project.next_annotation > highest,
            "next_annotation is {} but marking {highest} exists, so a new marking would reuse an id",
            project.next_annotation
        );
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "two markings in the sample share an id");

        let next_artifact = project.artifacts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        assert_eq!(project.next_artifact, next_artifact, "next_artifact would reuse an existing reference id");
    }

    #[test]
    fn the_sample_plates_listing_matches_the_files_on_disk() {
        let text = std::fs::read_to_string(root().join("seed/plates.json")).expect("seed/plates.json is committed");
        let index: serde_json::Value = serde_json::from_str(&text).expect("seed/plates.json parses");
        let plates = index["plates"].as_array().expect("plates is an array");
        assert!(!plates.is_empty(), "the sample lists no plates");
        for plate in plates {
            let rel = plate.as_str().expect("plate paths are strings");
            assert!(root().join(rel).is_file(), "{rel} is listed but missing");
        }
    }
}
