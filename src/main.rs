#[macro_use]
extern crate err_derive;
#[macro_use]
extern crate log;

mod config;
mod image;
mod info;
mod toolchain;

use self::{
    config::{Config, Error as ConfigError},
    image::{Image, ImageBuf, TagVariants},
    info::{iterate_image_info, Info},
};
use rs_docker::{image::Image as DockerImage, Docker};
use std::{env::args, io, process::exit};
use tabular::{Row, Table};

#[derive(Debug, Error)]
pub enum Error {
    #[error(display = "configuration error")]
    Configure(#[error(cause)] ConfigError),
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

    let config = Config::read().map_err(Error::Configure)?;
    let toolchain_override = toolchain::toolchain_override();

    let (mut tag, mut variants) =
        toolchain_override.as_ref().or_else(|| config.image.as_ref()).map_or_else(
            || ("latest", TagVariants::empty()),
            |image| (image.tag.as_ref(), image.variants),
        );

    let arguments: Vec<String> = args().skip(1).collect();
    let mut arguments = arguments.iter();

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

    let mut subcommand_args = Vec::new();

    let mut flagged_variants = TagVariants::empty();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => help(),
            "--" => break,
            "--gpu" => flagged_variants |= TagVariants::GPU,
            "--python3" => flagged_variants |= TagVariants::PY3,
            "--jupyter" => flagged_variants |= TagVariants::JUPYTER,
            argument => subcommand_args.push(argument),
        }
    }

    if !flagged_variants.is_empty() {
        variants = flagged_variants;
    }

    let mut image = Image { tag, variants };

    let result = match subcommand {
        "default" => {
            if subcommand_args.is_empty() {
                return Err(Error::RequiresArgument);
            }

            let tag: &str = subcommand_args[0];
            let variants = subcommand_args.into_iter().skip(1).collect::<TagVariants>();

            let new_config = Config { image: Some(ImageBuf { tag: Box::from(tag), variants }) };

            new_config.write().map_err(Error::Configure)?;
            Ok(())
        }
        "list" => {
            list(&mut docker)?;
            Ok(())
        }
        "pull" => {
            if let Some(tag) = subcommand_args.get(0) {
                image.tag = tag;
                image.variants = flagged_variants;
            }

            image.pull()
        }
        "remove" => {
            if subcommand_args.is_empty() {
                return Err(Error::RequiresArgument);
            }

            let image: &str = subcommand_args[0];
            remove(&mut docker, image)?;
            Ok(())
        }
        "run" => {
            if subcommand_args.is_empty() {
                return Err(Error::RequiresArgument);
            }

            let cmd: &str = subcommand_args[0];
            let args: Vec<&str> = subcommand_args.into_iter().skip(1).collect();

            image.run(cmd, if args.is_empty() { None } else { Some(&args) })
        }
        "show" => {
            if subcommand_args.is_empty() {
                println!("{}", image);
                Ok(())
            } else {
                unimplemented!()
            }
        }
        _ => help(),
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

const HELP: &str = "tensorman
    Tensorflow Docker image manager

USAGE:
    tensorman [+TAG] SUBCOMMAND [FLAGS...]

SUBCOMMANDS:
    default TAG [VARIANTS...]
        Defines the default tensorflow image to use when not specified

    list
        List tensorflow images installed on the system

    pull [TAG]
        Fetches and updates tensorflow images

    remove ID
        Removes an image either by its sha sum, or tag

    run COMMAND [-- ARGS...]
        Mounts an image and executes the given command.

        Use a shell as the COMMAND interactive sessions.

    show
        Show the active image that will be run

FLAGS:
    --gpu         Uses an image which supports GPU compute
    --python3     Uses an image which supports Python3
    --jupyter     Usages an image which has Jupyter preinstalled

    -h, --help    Display this information";

fn help() -> ! {
    println!("{}", HELP);
    exit(0);
}

fn main() {
    if let Err(why) = main_() {
        match why {
            Error::SubcommandRequired => help(),
            _ => {
                eprintln!("{}", why);
                exit(1)
            }
        }
    }
}
