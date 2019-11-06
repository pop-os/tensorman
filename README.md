# Tensorflow Container Manager

Packaging Tensorflow for Linux distributions is notoriously difficult, if not impossible. Every release of Tensorflow is accommodated by a myriad of possible build configurations, which requires building many variants of Tensorflow for each Tensorflow release. To make matters worse, each new version of Tensorflow will depend on a wide number of shared dependencies which may not be supported on older versions of a Linux distribution that is still actively supported by the distribution maintainers.

To solve this problem, the Tensorflow project provides official Docker container builds, which allows Tensorflow to operate in an isolated environment that is contained from the rest of the system. This virtual environment can operate independent of the base system, allowing you to use any version of Tensorflow on any version of a Linux distribution that supports the Docker runtime.

However, configuring and managing Docker containers for Tensorflow using the `docker` command line is currently tedious, and managing multiple versions for different projects is even moreso. To solve this problem for our users, we have developed `tensorman` as a convenient tool to manage the installation and execution of Tensorflow Docker containers. It condenses the command-line soup into a set of simple commands that are easy to memorize.

## Comparison to Docker Command

Take the following Docker invocation as an example:

```
docker run -u $UID:$UID -v $PWD:/project -w /project \
    --runtime=nvidia --it --rm tensorflow/tensorflow:latest-gpu \
    python ./script.py
```

This designates for the latest version of Tensorflow with GPU support to be used, mounting the working directory to `/project`, launching the container with the current user account, and and executing `script.py` with the Python binary in the container. With `tensorman`, we can achieve the same with:

```
tensorman run --gpu python -- ./script.py
```

Which defaults to the latest version, and whose version and tag variants can be set as defaults per-run, per-project, or user-wide.

## Installing/Updating Containers

By default, docker will automatically install a container when running a container that it is not already installed. However, if you would like to install a container beforehand, you may do so using the `pull` subcommand.

```
tensorman pull 1.14.0
tensorman pull latest
```

## Running commands in containers

The `run` subcommand allows you to execute a command from within the container. This could be the `bash` shell, for an interactive session inside the container, or the program / compiler which you wish to run.

```
# Default container version with Bash prompt
tensorman run bash

# Default container version with Python script
tensorman run python -- script.py

# Default container version with GPU support
tensorman run --gpu bash

# With GPU, Python3, and Juypyter support
tensorman run --gpu --python3 --jupyter bash
```

## Setting the container version

Taking inspiration from [rustup], there are methods to set the container version per-run, per-project, and per-user. The per-run version always takes priority over a per-project definition, which takes priority over the per-user configuration.

[rustup]: https://rustup.rs

### Setting per-run

If a version is specified following a `+` argument, `tensorman` will prefer this version.

```
tensorman +1.14.0 run --python3 --gpu bash
```

Custom images may be specified with a `=` argument.

```
tensorman =custom-image run --gpu bash
```

### Setting per-project

There are two files that can be used for configuring Tensorman locally: `tensorflow-toolchain`, and `Tensorman.toml`. These files will be automatically detected if they can be found in a parent directory.

#### tensorflow-toolchain

This file overrides the tensorflow image, defined either in `Tensorman.toml`, or the user-wide configuration file.

```
1.14.0 gpu python3
```

Or specifying a custom image:

```
=custom-image gpu
```

#### Tensorman.toml

This file supports additional configuration parameters, with a user-wide configuration located at `~/.config/tensorman/config.toml`, and a project-wide location at `Tensorman.toml`. One of the reasons in which you may want to use this file is to declare some additional Docker flags, with the `docker_flags` key.

Using a default tensorflow image:

```toml
docker_flags = [ '-p', '8080:8080' ]
tag = '2.0.0'
variants = ['gpu', 'python3']
```

Defining a custom image:

```toml
docker_flags = [ '-p', '8080:8080' ]
image = 'custom-image'
variants = ['gpu']
```

### Setting per-user

you can set a default version user-wide using the `default` subcommand. This version of Tensorflow will be launched whenever you use the `tensorman run` command.

```
tensorman default 1.14.0
tensorman default latest gpu python3
tensorman default nightly
```

> By default, `tensorman` will use `latest` as the default per-user version tag.

## Showing the active container version

If you would like to know which container will be used when launched from the current working directory, you can use the `show` command.

```
tensorman show
```

## Removing container images

Having many containers installed simultaneously on the same system can quickly use a lot of disk storage. If you find yourself in need of culling the containers installed on your system, you may do so with the `remove` command.

```
tensorman remove 1.14.0
tensorman remove latest
tensorman remove 481cb7ea88260404
tensorman remove custom-image
```

## Listing installed container images

To aid in discovering what containers are installed on the system, the `list` subcommand is available.

```
tensorman list
```

## Creating a custom image

In most projects, you will need to pull in more dependencies than the base Tensorflow image has. To do this, you will need to create the image by running a tensorflow container as root, installing and setting up the environment how you need it, and then saving those changes as a new custom image.

To do so, you will need to build the container in one terminal, and save it from another.

### Build new image

First launch a terminal where you will begin configuring the docker image:

```
tensorman run --gpu --python3 --root --name CONTAINER_NAME bash
```

Once you've made the changes needed, open another terminal and save it as a new image:

```
tensorman save CONTAINER_NAME IMAGE_NAME
```

### Running the custom image

You should then be able to specify that container with tensorman, like so:

```
tensorman =IMAGE_NAME run --gpu bash
```

> The `--python3` and `--jupyter` flags do nothing for custom containers, but `--gpu` is required to enable runtime support for the GPU.

### Removing the custom image

Images saved through tensorman are manageable through tensorman. Listing and removing works the same:

```
tensorman remove IMAGE_NAME
```

## License

Licensed under the GNU General Public License, Version 3.0, ([LICENSE](LICENSE) or https://www.gnu.org/licenses/gpl-3.0.en.html)

### Contribution

Any contribution intentionally submitted for inclusion in the work by you, shall be licensed under the GNU GPLv3.
