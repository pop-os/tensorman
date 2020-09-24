use super::runtime::DockerImage;

#[derive(Debug)]
pub struct Info {
    pub repo:     Box<str>,
    pub tag:      Box<str>,
    pub image_id: Box<str>,
    pub size:     Box<str>,
}

impl Info {
    /// Check if any of the string fields matches the `needle`.
    pub fn field_matches(&self, needle: &str) -> bool {
        match self.repo.as_ref() {
            "tensorman" => self.tag.as_ref() == needle,
            "tensorflow/tensorflow" => {
                self.tag.as_ref() == needle || self.image_id.starts_with(needle)
            }
            _ => false,
        }
    }
}

pub fn iterate_image_info(images: Vec<DockerImage>) -> impl Iterator<Item = Info> {
    fn valid_repo(repo: &str) -> bool {
        repo == "tensorflow/tensorflow" || repo == "tensorman"
    }

    images
        .into_iter()
        .filter(|image| valid_repo(&image.Repository))
        .map(|image| {
            Info {
                repo: image.Repository.into(),
                tag: image.Tag.into(),
                image_id: image.ID.into(),
                size: image.Size.into(),
            }
        })
}
