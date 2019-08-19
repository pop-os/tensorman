use crate::image::{ImageBuf, TagVariants};

use nix::sys::socket::SockType::Raw;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fs, io, path::PathBuf};
use xdg::BaseDirectories;

#[derive(Debug, Error)]
pub enum Error {
    #[error(display = "failed to get the base directory")]
    BaseDirectory(#[error(cause)] xdg::BaseDirectoriesError),
    #[error(display = "failed to get config path")]
    ConfigPath(#[error(cause)] io::Error),
    #[error(display = "failed to read configuration file into memory")]
    ConfigRead(#[error(cause)] io::Error),
    #[error(display = "failed to write serialized config to configuration file")]
    ConfigWrite(#[error(cause)] io::Error),
    #[error(display = "failed to create the tensorman configuration directory")]
    CreateDir(#[error(cause)] io::Error),
    #[error(display = "failed to deserialize the configuration file")]
    Deserialize(#[error(cause)] toml::de::Error),
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
        let RawConfig { tag, variants } = raw;
        let image = tag.map(|tag| {
            let variants = variants.iter().flatten().map(String::as_str).collect::<TagVariants>();
            ImageBuf { tag: tag.into(), variants }
        });

        Ok(Config { image })
    }
}

#[derive(Deserialize, Serialize)]
struct RawConfig {
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
    fn default() -> Self { Self { tag: None, variants: None } }
}

impl<'a> From<&'a Config> for RawConfig {
    fn from(config: &'a Config) -> Self {
        let (tag, variants) = config.image.as_ref().map_or((None, None), |image| {
            let variants = if image.variants.is_empty() {
                None
            } else {
                Some(<Vec<String>>::from(image.variants))
            };

            (Some(image.tag.clone().into()), variants)
        });

        RawConfig { tag, variants }
    }
}

fn config_path() -> Result<PathBuf, Error> {
    BaseDirectories::with_prefix("tensorman")
        .map_err(Error::BaseDirectory)?
        .place_config_file("config.toml")
        .map_err(Error::ConfigPath)
}
