#[macro_use]
extern crate err_derive;
#[macro_use]
extern crate log;

mod image;
mod info;

use self::{
    image::{Image, TagVariants},
    info::{iterate_image_info, Info},
};
use rs_docker::{image::Image as DockerImage, Docker};
use std::{env::args, io, process::exit};
use tabular::{Row, Table};

#[derive(Debug, Error)]
pub enum Error {
    #[error(display = "failed to establish a connection to the Docker service")]
    DockerConnection(#[error(cause)] io::Error),
    #[error(display = "failed to fetch list of docker containers")]
    DockerContainers(#[error(cause)] io::Error),
    #[error(display = "failed to remove docker image")]
    DockerRemove(#[error(cause)] io::Error),
    #[error(display = "subcommand requires at least one argument")]
    RequiresArgument,
    #[error(display = "subcommand failed")]
    Subcommand(#[error(cause)] io::Error),
    #[error(display = "missing subcommand argument")]
    SubcommandRequired,
}

fn main_() -> Result<(), Error> {
    let mut docker =
        Docker::connect("unix:///var/run/docker.sock").map_err(Error::DockerConnection)?;

    let arguments: Vec<String> = args().skip(1).collect();
    let mut arguments = arguments.iter();
    let mut tag = "latest";

    // Allow the first argument, if it begins with `+`, to override the tag.
    let mut subcommand = arguments.next().and_then(|argument| {
        if argument.starts_with('+') {
            tag = &argument[1..];
            arguments.next().map(String::as_str)
        } else {
            Some(argument.as_str())
        }
    });

    let subcommand = subcommand.take().ok_or(Error::SubcommandRequired)?;

    let mut variants = TagVariants::empty();
    let mut subcommand_args = Vec::new();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--" => break,
            "--gpu" => variants |= TagVariants::GPU,
            "--py3" => variants |= TagVariants::PY3,
            "--jupyter" => variants |= TagVariants::JUPYTER,
            argument => subcommand_args.push(argument),
        }
    }

    let image = Image { tag, variants };

    let result = match subcommand {
        "default" => unimplemented!(),
        "list" => {
            list(&mut docker)?;
            Ok(())
        }
        "pull" => image.pull(),
        "remove" => {
            if subcommand_args.len() != 1 {
                return Err(Error::RequiresArgument);
            }

            let image: &str = subcommand_args[0];
            remove(&mut docker, image)?;
            Ok(())
        }
        "run" => {
            if subcommand_args.len() != 1 {
                return Err(Error::RequiresArgument);
            }

            let cmd: &str = subcommand_args[0];
            let args: Vec<&str> = arguments.map(|x| x.as_str()).collect();

            image.run(cmd, if args.is_empty() { None } else { Some(&args) })
        }
        _ => panic!("unknown subcommand: {}", subcommand),
    };

    result.map_err(Error::Subcommand)
}

fn remove(docker: &mut Docker, argument: &str) -> Result<(), Error> {
    let images = get_images(docker)?;
    for info in iterate_image_info(images) {
        if info.field_matches(argument) {
            docker_remove_image(&info).map_err(Error::DockerRemove)?;
        }
    }

    Ok(())
}

fn list(docker: &mut Docker) -> Result<(), Error> {
    let images = get_images(docker)?;
    let mut table = Table::new("{:<}  {:<}  {:<}  {:<}");

    table.add_row(
        Row::new().with_cell("REPOSITORY").with_cell("TAG").with_cell("IMAGE ID").with_cell("SIZE"),
    );

    for mut info in iterate_image_info(images) {
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

fn docker_remove_image(info: &Info) -> io::Result<()> {
    use std::process::Command;

    Command::new("docker").args(&["rmi", &info.image_id]).status().map(|_| ())
}

fn get_images(docker: &mut Docker) -> Result<Vec<DockerImage>, Error> {
    docker.get_images(true).map_err(Error::DockerContainers)
}

fn main() {
    if let Err(why) = main_() {
        eprintln!("{}", why);
        exit(1)
    }
}
