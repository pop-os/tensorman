use crate::image::{ImageBuf, ImageSourceBuf, TagVariants};

use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, io, path::PathBuf};
use xdg::BaseDirectories;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to get the base directory")]
    BaseDirectory(#[source] xdg::BaseDirectoriesError),
    #[error("failed to get config path")]
    ConfigPath(#[source] io::Error),
    #[error("failed to read configuration file into memory")]
    ConfigRead(#[source] io::Error),
    #[error("failed to write serialized config to configuration file")]
    ConfigWrite(#[source] io::Error),
    #[error("failed to create the tensorman configuration directory")]
    CreateDir(#[source] io::Error),
    #[error("failed to deserialize the configuration file")]
    Deserialize(#[source] toml::de::Error),
}

pub struct Config {
    pub image: Option<ImageBuf>,
}

impl Config {
    pub fn read() -> Result<Self, Error> { RawConfig::read().and_then(Self::try_from) }

    pub fn write(&self) -> Result<(), Error> { RawConfig::from(self).write() }
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        let RawConfig { image, tag, variants } = raw;

        let variants = variants.iter().flatten().map(String::as_str).collect::<TagVariants>();

        let source = match (image, tag) {
            (Some(image), _) => ImageSourceBuf::Container(image.into()),
            (None, Some(tag)) => ImageSourceBuf::Tensorflow(tag.into()),
            (None, None) => return Ok(Config { image: None }),
        };

        Ok(Config { image: Some(ImageBuf { variants, source }) })
    }
}

#[derive(Deserialize, Serialize)]
struct RawConfig {
    pub image:    Option<String>,
    pub tag:      Option<String>,
    pub variants: Option<Vec<String>>,
}

impl RawConfig {
    pub fn read() -> Result<Self, Error> {
        let config_path = config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read_to_string(&config_path).map_err(Error::ConfigRead)?;

        toml::from_str::<Self>(&*data).map_err(Error::Deserialize)
    }

    pub fn write(&self) -> Result<(), Error> {
        let config_path = config_path()?;

        if !config_path.exists() {
            let parent = config_path.parent().expect("config path without parent directory");
            fs::create_dir_all(parent).map_err(Error::CreateDir)?;
        }

        let data = toml::to_string_pretty(self).expect("failed to serialize config");
        fs::write(config_path, data).map_err(Error::ConfigWrite)
    }
}

impl Default for RawConfig {
    fn default() -> Self { Self { image: None, tag: None, variants: None } }
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

        RawConfig { image, tag, variants }
    }
}

fn config_path() -> Result<PathBuf, Error> {
    BaseDirectories::with_prefix("tensorman")
        .map_err(Error::BaseDirectory)?
        .place_config_file("config.toml")
        .map_err(Error::ConfigPath)
}
