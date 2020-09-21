use anyhow::Context;
use bollard::{
    container::{ ListContainersOptions},
    image::{ListImagesOptions},
    models::{ContainerSummary, ImageSummary},
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

/// Runtime for executing Docker futures on.
pub struct Runtime {
    pub docker: Docker,
    pub tokio:  TokioRuntime,
}

impl Runtime {
    /// Creates a new runtime for interacting with Docker.
    pub fn new(docker_func: fn() -> Result<Docker, bollard::errors::Error>) -> anyhow::Result<Self> {
        Ok(Self {
            docker: docker_func()
                .context("failed to establish a connection to the Docker service")?,
            tokio:  TokioRuntime::new().context("failed to create tokio runtime")?,
        })
    }

    /// Fetches a list of docker containers.
    pub fn containers(&mut self) -> anyhow::Result<Vec<ContainerSummary>> {
        let options = ListContainersOptions::<String> { all: true, ..Default::default() };
        self.tokio
            .block_on(self.docker.list_containers(Some(options)))
            .context("failed to fetch list of containers from Docker service")
    }

    /// Fetches a list of docker images.
    pub fn images(&mut self) -> anyhow::Result<Vec<ImageSummary>> {
        let options = ListImagesOptions::<String> { all: true, ..Default::default() };
        self.tokio
            .block_on(self.docker.list_images(Some(options)))
            .context("failed to fetch list of images from Docker service")
    }

    /// Displays docker images currently installed which are relevant to tensorman.
    pub fn list(&mut self) -> anyhow::Result<()> {
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
    pub fn remove(&mut self, argument: &str, force: bool) -> anyhow::Result<()> {
        let images = self.images()?;
        let mut found = false;
        for info in iterate_image_info(images) {
            if info.field_matches(argument) {
                found = true;
                docker_remove_image(&info, force).context("failed to remove the docker image")?;
            }
        }

        if !found {
            return Err(anyhow!("image not found"));
        }

        Ok(())
    }

    /// Runs a new container from a specified image and configurable parameters.
    pub fn run(
        &mut self,
        image: &Image,
        cmd: &str,
        name: Option<&str>,
        port: Option<&str>,
        as_root: bool,
        args: Option<&[&str]>,
        docker_flags: Option<&[String]>,
    ) -> anyhow::Result<()> {
        let pwd = env::current_dir().context("unable to get the current working directory")?;

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
            ensure!(
                !self.container_exists(name)?,
                "an active container named {} already exists",
                name
            );
            command.arg("--name").arg(name);
        }

        if let Some(port) = port {
            command.arg("-p").arg(port);
        }

        if image.variants.contains(TagVariants::GPU) {
            command.arg("--gpus=all");
        }

        command.arg("-e").arg("HOME=/project");

        if let Some(args) = docker_flags {
            command.args(args);
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
        command.status().context("docker run command exited with a bad status")?;
        Ok(())
    }

    /// Saves an active container to a new image in the tensorman repository
    pub fn save(&mut self, container: &str, repo: &str) -> anyhow::Result<()> {
        let container: &str = &*["tensorman-", container].concat();

        ensure!(self.container_exists(container)?, "the container to be saved does not exist");

        let images = self.images()?;
        for info in iterate_image_info(images) {
            if info.repo.as_ref() == "tensorman" && info.tag.as_ref() == repo {
                return Err(anyhow!("image already exists with this name"));
            }
        }

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

        // self.tokio.block_on(future).map_err(|failure| failure.compat())

        commit_command(container, repo).context("failed to commit container")?;

        Ok(())
    }

    /// Queries docker for a list of containers, and returns `Ok(true)` if container
    /// with a compatible name is found.
    fn container_exists(&mut self, name: &str) -> anyhow::Result<bool> {
        let contains_name =
            |vec: &[String]| vec.iter().filter(|s| !s.is_empty()).any(|e| &e[1..] == name);

        self.containers()
            .map(|cts| cts.into_iter().any(|container| container.names.map_or(false, |v| contains_name(&v))))
    }
}

fn commit_command(container: &str, repo: &str) -> io::Result<()> {
    let image = ["tensorman:", repo].concat();
    Command::new("docker").args(&["commit", container, &image]).status().map(|_| ())
}

fn docker_remove_image(info: &Info, force: bool) -> io::Result<()> {
    let mut command = Command::new("docker");
    command.args(&["rmi", &info.image_id]);

    if force {
        command.arg("--force");
    }

    command.status().map(|_| ())
}
