//! The data files, compiled into the program.

macro_rules! files {
    ($($name:literal),* $(,)?) => {
        pub const FILES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../../assets/data/", $name)))),*
        ];
    };
}

files!(
    "biomes.ron",
    "canon.ron",
    "enemies.ron",
    "items.ron",
    "lexicon.ron",
    "recipes.ron",
    "skills.ron",
);

pub fn file(name: &str) -> Option<&'static str> {
    FILES.iter().find(|f| f.0 == name).map(|f| f.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_data_file_is_built_in_and_loads() {
        let dir = crate::data::dev_assets_dir().join("data");
        let on_disk: BTreeSet<String> = std::fs::read_dir(dir)
            .expect("data folder")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        let built: BTreeSet<String> = FILES.iter().map(|f| f.0.to_string()).collect();
        assert_eq!(built, on_disk);
        crate::data::GameData::built_in().expect("built-in data loads");
    }
}
