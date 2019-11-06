use crate::{
    image::{ImageBuf, ImageSourceBuf, TagVariants},
    misc::walk_parent_directories,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use xdg::BaseDirectories;

pub struct Config {
    pub image:        Option<ImageBuf>,
    pub docker_flags: Option<Vec<String>>,
}

impl Config {
    /// Read a config from either a local or user config
    ///
    /// The local config takes precedence over the user config.
    /// If neither exists, a default config is returned.
    pub fn read() -> anyhow::Result<Self> { RawConfig::read().map(Self::from) }

    /// Write a config to the external configuration path
    pub fn write(&self) -> anyhow::Result<()> { RawConfig::from(self).write() }
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        let RawConfig { docker_flags, image, tag, variants } = raw;

        let variants = variants.iter().flatten().map(String::as_str).collect::<TagVariants>();

        let source = match (image, tag) {
            (Some(image), _) => ImageSourceBuf::Container(image.into()),
            (None, Some(tag)) => ImageSourceBuf::Tensorflow(tag.into()),
            (None, None) => return Config { docker_flags, image: None },
        };

        Config { docker_flags, image: Some(ImageBuf { variants, source }) }
    }
}

#[derive(Deserialize, Default, Serialize)]
struct RawConfig {
    pub image:        Option<String>,
    pub tag:          Option<String>,
    pub variants:     Option<Vec<String>>,
    pub docker_flags: Option<Vec<String>>,
}

impl RawConfig {
    pub fn read() -> anyhow::Result<Self> {
        let config_path = match local_path()? {
            Some(config_path) => config_path,
            None => {
                let config_path = user_path()?;

                if !config_path.exists() {
                    return Ok(Self::default());
                }

                config_path
            }
        };

        let data = fs::read_to_string(&config_path).with_context(|| {
            format!("failed to read configuration file at {}", config_path.display())
        })?;

        toml::from_str::<Self>(&*data).with_context(|| {
            format!("failed to parse TOML in configuration file at {}", config_path.display())
        })
    }

    pub fn write(&self) -> anyhow::Result<()> {
        let config_path = user_path()?;

        println!("writing to configuration file at {}", config_path.display());

        if !config_path.exists() {
            let parent = config_path.parent().expect("config path without parent directory");
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create the Tensorman configuration directory at {}",
                    parent.display()
                )
            })?;
        }

        let data = toml::to_string_pretty(self).expect("failed to serialize config");
        fs::write(&config_path, data).with_context(|| {
            format!(
                "failed to write settings to Tensorman configuration file at {}",
                config_path.display()
            )
        })
    }
}

impl<'a> From<&'a Config> for RawConfig {
    fn from(config: &'a Config) -> Self {
        let (image, tag, variants) = config.image.as_ref().map_or((None, None, None), |image| {
            let variants = if image.variants.is_empty() {
                None
            } else {
                Some(<Vec<String>>::from(image.variants))
            };

            let (image, tag) = match &image.source {
                ImageSourceBuf::Container(image) => (Some(String::from(&**image)), None),
                ImageSourceBuf::Tensorflow(tag) => (None, Some(String::from(&**tag))),
            };

            (image, tag, variants)
        });

        RawConfig { image, tag, variants, docker_flags: config.docker_flags.clone() }
    }
}

fn local_path() -> anyhow::Result<Option<PathBuf>> {
    std::env::current_dir()
        .context("failed to fetch the current working directory")
        .map(|dir| walk_parent_directories(&dir, "Tensorman.toml"))
}

fn user_path() -> anyhow::Result<PathBuf> {
    BaseDirectories::with_prefix("tensorman")
        .context("failed to find the XDG base directory for tensorman")?
        .place_config_file("config.toml")
        .context("failed to fetch the user-wide Tensorman config path")
}
