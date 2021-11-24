mod cross_compile;

use crate::Error::{MultipleCargoTargetsFound, NoCargoTargetsFound};
use cargo_metadata::MetadataCommand;
use cross_compile::CrossCompileError;
use flate2::write::GzEncoder;
use flate2::Compression;
use libcnb_data::buildpack::BuildpackToml;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus};
use tar::{EntryType, Header};

#[derive(Debug)]
pub enum Error {
    CouldNotFindBuildpackToml,
    CouldNotReadBuildpackToml(std::io::Error),
    BuildpackTomlDeserializationError(toml::de::Error),
    CargoMetadataError(cargo_metadata::Error),
    CouldNotFindBuildpackCargoPackage,
    CrossCompileError(CrossCompileError),
    CargoBuildUnsuccessful(ExitStatus),
    CargoBuildIoError(std::io::Error),
    NoCargoTargetsFound,
    MultipleCargoTargetsFound,
    CouldNotWriteBuildpackArchive(std::io::Error),
}

pub enum CargoProfile {
    Dev,
    Release,
}

pub fn package_buildpack(
    project_path: impl AsRef<Path>,
    cargo_profile: CargoProfile,
) -> Result<(), Error> {
    // Currently, this is the only supported target triple
    let target_triple = "x86_64-unknown-linux-musl";

    let mut cargo_args = vec!["build", "--target", target_triple];
    match cargo_profile {
        CargoProfile::Dev => {}
        CargoProfile::Release => cargo_args.push("--release"),
    }

    let cargo_build_exit_status = Command::new("cargo")
        .args(cargo_args)
        .envs(cross_compile::cross_compile_env(&target_triple).map_err(Error::CrossCompileError)?)
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(Error::CargoBuildIoError)?;

    if !cargo_build_exit_status.success() {
        return Err(Error::CargoBuildUnsuccessful(cargo_build_exit_status));
    }

    let buildpack_toml_path = project_path.as_ref().join("buildpack.toml");
    if !buildpack_toml_path.is_file() {
        return Err(Error::CouldNotFindBuildpackToml);
    }

    let buildpack_descriptor: BuildpackToml<Option<toml::Value>> =
        fs::read_to_string(&buildpack_toml_path)
            .map_err(Error::CouldNotReadBuildpackToml)
            .and_then(|file_contents| {
                toml::from_str(&file_contents).map_err(Error::BuildpackTomlDeserializationError)
            })?;

    let cargo_metadata = MetadataCommand::new()
        .manifest_path(project_path.as_ref().join("Cargo.toml"))
        .exec()
        .map_err(Error::CargoMetadataError)?;

    let buildpack_cargo_package = cargo_metadata
        .root_package()
        .ok_or(Error::CouldNotFindBuildpackCargoPackage)?;

    let target = match buildpack_cargo_package.targets.as_slice() {
        [] => Err(NoCargoTargetsFound),
        [single_target] => Ok(single_target),
        _ => Err(MultipleCargoTargetsFound),
    }?;

    let buildpack_binary_path = cargo_metadata
        .target_directory
        .join(&target_triple)
        .join(match cargo_profile {
            CargoProfile::Dev => "debug",
            CargoProfile::Release => "release",
        })
        .join(&target.name);

    package_buildpack_tarball(
        cargo_metadata.target_directory.join(format!(
            "{}_buildpack_{}.tar.gz",
            buildpack_descriptor.buildpack.id.replace("/", "_"),
            match cargo_profile {
                CargoProfile::Dev => "dev",
                CargoProfile::Release => "release",
            }
        )),
        &buildpack_toml_path,
        &buildpack_binary_path,
    )
    .map_err(Error::CouldNotWriteBuildpackArchive)
}

fn package_buildpack_tarball(
    destination_path: impl AsRef<Path>,
    buildpack_toml_path: impl AsRef<Path>,
    buildpack_binary_path: impl AsRef<Path>,
) -> std::io::Result<()> {
    let destination_file = fs::File::create(destination_path.as_ref())?;
    let mut buildpack_toml_file = fs::File::open(buildpack_toml_path.as_ref())?;
    let mut buildpack_binary_file = fs::File::open(buildpack_binary_path.as_ref())?;

    let mut tar_builder =
        tar::Builder::new(GzEncoder::new(destination_file, Compression::default()));

    tar_builder.append_file("buildpack.toml", &mut buildpack_toml_file)?;
    tar_builder.append_file("bin/build", &mut buildpack_binary_file)?;

    // Build a symlink header to link bin/detect to bin/build
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Symlink);
    header.set_path("bin/detect")?;
    header.set_link_name("build")?;
    header.set_size(0);
    header.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs(),
    );
    header.set_cksum();

    tar_builder.append(&header, &[][..])?;

    tar_builder.into_inner()?.finish()?.flush()
}
