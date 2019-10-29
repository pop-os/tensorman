use bollard::{
    container::{APIContainers, ListContainersOptions},
    image::{APIImages, ListImagesOptions},
    Docker,
};

use crate::{
    image::{Image, TagVariants},
    info::{iterate_image_info, Info},
};
use nix::unistd::geteuid;
use std::{env, io, process::Command};
use tabular::{Row, Table};
use tokio::runtime::Runtime as TokioRuntime;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to establish a connection to the Docker service")]
    Connection(#[source] failure::Compat<failure::Error>),
    #[error("a container with the name of {} already exists", _0)]
    ContainerExists(Box<str>),
    #[error("a container with the name of {} does not exist", _0)]
    ContainerNotFound(Box<str>),
    #[error("failed to fetch a list of containers")]
    Containers(#[source] failure::Compat<failure::Error>),
    #[error("failed to commit container {} as a new image named {}", container, repo)]
    Commit {
        container: Box<str>,
        repo: Box<str>,
        #[source]
        source: io::Error,
    },
    #[error("image to be committed already exists")]
    ImageAlreadyExists,
    #[error("failed to fetch list of docker images")]
    Images(#[source] failure::Compat<failure::Error>),
    #[error("image not found")]
    ImageNotFound,
    #[error("failed to remove docker image")]
    Remove(#[source] io::Error),
    #[error("failed to run container")]
    Run(#[from] RunError),
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("docker run command exited with a bad status")]
    BadStatus(#[source] io::Error),
    #[error("unable to get the current working directory")]
    CurrentDir(#[source] io::Error),
    #[error("unable to create tempfile for image to be run")]
    CreateTempFile(#[source] io::Error),
    #[error("unable to create local data directory for tensorman")]
    TensorDataDir(#[source] io::Error),
    #[error("unable to write container name to temporary file")]
    WriteContainerName(#[source] io::Error),
}

/// Runtime for executing Docker futures on.
pub struct Runtime {
    pub docker: Docker,
    pub tokio:  TokioRuntime,
}

impl Runtime {
    /// Creates a new runtime for interacting with Docker.
    pub fn new(docker_func: fn() -> Result<Docker, failure::Error>) -> Result<Self, Error> {
        Ok(Self {
            docker: docker_func().map_err(|failure| failure.compat()).map_err(Error::Connection)?,
            tokio:  TokioRuntime::new().unwrap(),
        })
    }

    /// Fetches a list of docker containers.
    pub fn containers(&mut self) -> Result<Vec<APIContainers>, Error> {
        let options = ListContainersOptions::<String> { all: true, ..Default::default() };
        self.tokio
            .block_on(self.docker.list_containers(Some(options)))
            .map_err(|failure| failure.compat())
            .map_err(Error::Containers)
    }

    /// Fetches a list of docker images.
    pub fn images(&mut self) -> Result<Vec<APIImages>, Error> {
        let options = ListImagesOptions::<String> { all: true, ..Default::default() };
        self.tokio
            .block_on(self.docker.list_images(Some(options)))
            .map_err(|failure| failure.compat())
            .map_err(Error::Images)
    }

    /// Displays docker images currently installed which are relevant to tensorman.
    pub fn list(&mut self) -> Result<(), Error> {
        let images = self.images()?;
        let mut table = Table::new("{:<}  {:<}  {:<}  {:<}");

        table.add_row(
            Row::new()
                .with_cell("REPOSITORY")
                .with_cell("TAG")
                .with_cell("IMAGE ID")
                .with_cell("SIZE"),
        );

        for info in iterate_image_info(images) {
            table.add_row(
                Row::new()
                    .with_cell(info.repo)
                    .with_cell(info.tag)
                    .with_cell(&info.image_id[..=14])
                    .with_cell(info.size),
            );
        }

        print!("{}", table);

        Ok(())
    }

    /// Removes a Docker image from the system.
    pub fn remove(&mut self, argument: &str) -> Result<(), Error> {
        let images = self.images()?;
        let mut found = false;
        for info in iterate_image_info(images) {
            if info.field_matches(argument) {
                found = true;
                docker_remove_image(&info).map_err(Error::Remove)?;
            }
        }

        if found {
            Ok(())
        } else {
            Err(Error::ImageNotFound)
        }
    }

    pub fn validate_image_does_not_exist(&mut self, image: &str) -> Result<(), Error> {
        let images = self.images()?;
        for info in iterate_image_info(images) {
            if info.repo.as_ref() == "tensorman" && info.tag.as_ref() == image {
                return Err(Error::ImageAlreadyExists);
            }
        }

        Ok(())
    }

    /// Runs a new container from a specified image and configurable parameters.
    pub fn run(
        &mut self,
        image: &Image,
        cmd: &str,
        name: Option<&str>,
        as_root: bool,
        args: Option<&[&str]>,
    ) -> Result<(), Error> {
        let pwd = env::current_dir().map_err(RunError::CurrentDir)?;
        let mut command = Command::new("docker");

        let user_: String;
        let user: &str = if as_root {
            "root"
        } else {
            user_ = format!("{0}:{0}", geteuid());
            &*user_
        };

        command.args(&["run", "-u", &user]);

        if let Some(name) = name {
            let name: &str = &*["tensorman-", name].concat();
            self.validate_container_uniqueness(name)?;
            command.arg("--name").arg(name);
        }

        if image.variants.contains(TagVariants::GPU) {
            command.arg("--gpus=all");
        }

        command.args(&[
            "-it",
            "--rm",
            "-v",
            &format!("{}:/project", pwd.display()),
            "-w",
            "/project",
            &String::from(image),
            cmd,
        ]);

        if let Some(args) = args {
            command.args(args);
        }

        eprintln!("{:?}", command);
        command.status().map_err(RunError::BadStatus)?;
        Ok(())
    }

    /// Saves an active container to a new image in the tensorman repository
    pub fn save(&mut self, container: &str, repo: &str) -> Result<(), Error> {
        let container: &str = &*["tensorman-", container].concat();
        self.validate_container_exists(container)?;
        self.validate_image_does_not_exist(repo)?;

        // TODO: This isn't working for some reason.
        // let options = CommitContainerOptions {
        //     author: "tensorman".into(),
        //     comment: "automated image creation by tensorman".into(),
        //     container: container.to_owned(),
        //     pause: true,
        //     repo: "tensorman",
        //     ..Default::default()
        // };

        // let config = ContainerConfig::<String> { ..Default::default() };
        // let future = self.docker.commit_container(options, config);

        // self.tokio.block_on(future).map_err(|failure| failure.compat()).map_err(|source| {
        //     Error::Commit { container: container.into(), repo: repo.into(), source }
        // })?;

        commit_command(container, repo).map_err(|source| Error::Commit {
            container: container.into(),
            repo: repo.into(),
            source,
        })?;

        Ok(())
    }

    /// Queries docker for a list of containers, and returns `Ok(true)` if container
    /// with a compatible name is found.
    fn container_exists(&mut self, name: &str) -> Result<bool, Error> {
        let contains_name =
            |vec: &[String]| vec.iter().filter(|s| !s.is_empty()).any(|e| &e[1..] == name);

        self.containers()
            .map(|cts| cts.into_iter().any(|container| contains_name(&container.names)))
    }

    /// Errors if a container with the `name` is not found.
    fn validate_container_exists(&mut self, name: &str) -> Result<(), Error> {
        if self.container_exists(name)? {
            Ok(())
        } else {
            Err(Error::ContainerNotFound(name.into()))
        }
    }

    /// Errors if a container with the `name` is found.
    fn validate_container_uniqueness(&mut self, name: &str) -> Result<(), Error> {
        if self.container_exists(name)? {
            Err(Error::ContainerExists(name.into()))
        } else {
            Ok(())
        }
    }
}

fn commit_command(container: &str, repo: &str) -> io::Result<()> {
    let image = ["tensorman:", repo].concat();
    Command::new("docker").args(&["commit", container, &image]).status().map(|_| ())
}

fn docker_remove_image(info: &Info) -> io::Result<()> {
    Command::new("docker").args(&["rmi", &info.image_id]).status().map(|_| ())
}
