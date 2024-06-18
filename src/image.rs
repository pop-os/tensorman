use std::{
    fmt::{self, Display},
    io,
    process::Command,
};

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct TagVariants: u8 {
        const GPU = 1;
        const PY3 = 1 << 1;
        const JUPYTER = 1 << 2;
    }
}

impl<'a> std::iter::FromIterator<&'a str> for TagVariants {
    fn from_iter<I>(iterator: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut variants = TagVariants::empty();
        for variant in iterator {
            match variant {
                "gpu" => variants |= TagVariants::GPU,
                "python3" => variants |= TagVariants::PY3,
                "jupyter" => variants |= TagVariants::JUPYTER,
                _ => (),
            }
        }

        variants
    }
}

impl From<TagVariants> for Vec<String> {
    fn from(variants: TagVariants) -> Self {
        let mut vector = Vec::new();

        if variants.contains(TagVariants::GPU) {
            vector.push("gpu".to_string());
        }

        if variants.contains(TagVariants::PY3) {
            vector.push("python3".to_string());
        }

        if variants.contains(TagVariants::JUPYTER) {
            vector.push("jupyter".to_string());
        }

        vector
    }
}

#[derive(Debug)]
pub struct ImageBuf {
    pub variants: TagVariants,
    pub source:   ImageSourceBuf,
}

/// A description of a Tensorflow Docker image, identified by its tag and tag variants.
#[derive(Debug)]
pub struct Image<'a> {
    pub variants: TagVariants,
    pub source:   ImageSource<'a>,
}

#[derive(Debug)]
pub enum ImageSourceBuf {
    Container(Box<str>),
    Tensorflow(Box<str>),
}

#[derive(Debug)]
pub enum ImageSource<'a> {
    Container(&'a str),
    Tensorflow(&'a str),
}

impl<'a> Image<'a> {
    pub fn pull(&self, docker_cmd: &str) -> io::Result<()> {
        let mut command = Command::new(docker_cmd);
        command.args(&["pull", &String::from(self)]);
        eprintln!("{:?}", command);
        command.status().map(|_| ())
    }
}

impl<'a> From<&Image<'a>> for String {
    fn from(image: &Image<'a>) -> Self {
        match image.source {
            ImageSource::Container(container) => ["tensorman:", container].concat(),
            ImageSource::Tensorflow(tag) => {
                let mut buffer = ["tensorflow/tensorflow:", tag].concat();

                if !image.variants.is_empty() {
                    if image.variants.contains(TagVariants::GPU) {
                        buffer.push_str("-gpu");
                    }

                    if image.variants.contains(TagVariants::PY3) {
                        buffer.push_str("-py3");
                    }

                    if image.variants.contains(TagVariants::JUPYTER) {
                        buffer.push_str("-jupyter");
                    }
                }

                buffer
            }
        }
    }
}

impl<'a> Display for Image<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.source {
            ImageSource::Container(container) => f.write_str(container),
            ImageSource::Tensorflow(tag) => {
                f.write_str("tensorflow/tensorflow:")?;
                f.write_str(tag)?;

                if !self.variants.is_empty() {
                    if self.variants.contains(TagVariants::GPU) {
                        f.write_str("-gpu")?;
                    }

                    if self.variants.contains(TagVariants::PY3) {
                        f.write_str("-py3")?;
                    }

                    if self.variants.contains(TagVariants::JUPYTER) {
                        f.write_str("-jupyter")?;
                    }
                }

                Ok(())
            }
        }
    }
}
