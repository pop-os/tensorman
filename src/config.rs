use crate::image::{ImageBuf, TagVariants};

use nix::sys::socket::SockType::Raw;
use serde::Deserialize;
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
    #[error(display = "failed to parse the configuration file")]
    Parsing(#[error(cause)] toml::de::Error),
}

pub struct Config {
    pub image: Option<ImageBuf>,
}

impl Config {
    pub fn read() -> Result<Self, Error> { RawConfig::read().and_then(Self::try_from) }
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

#[derive(Deserialize)]
struct RawConfig {
    pub tag:      Option<String>,
    pub variants: Option<Vec<String>>,
}

impl Default for RawConfig {
    fn default() -> Self { Self { tag: None, variants: None } }
}

impl RawConfig {
    pub fn read() -> Result<Self, Error> {
        let config_path = config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read_to_string(&config_path).map_err(Error::ConfigRead)?;

        toml::from_str::<Self>(&*data).map_err(Error::Parsing)
    }
}

fn config_path() -> Result<PathBuf, Error> {
    BaseDirectories::with_prefix("tensorman")
        .map_err(Error::BaseDirectory)?
        .place_config_file("config.toml")
        .map_err(Error::ConfigPath)
}
