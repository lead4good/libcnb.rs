use cargo_metadata::MetadataCommand;
use flate2::write::GzEncoder;
use flate2::Compression;
use libcnb_data::buildpack::BuildpackToml;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::{Command, ExitStatus};
use which::which;

#[derive(Debug)]
pub enum Error {
    CouldNotFindBuildpackToml,
    CouldNotReadBuildpackToml(std::io::Error),
    BuildpackTomlDeserializationError(toml::de::Error),
    CargoMetadataError(cargo_metadata::Error),
    CouldNotFindBuildpackCargoPackage,
    CrossCompileError(CrossCompileError),
    CargoBuildUnsuccessful(ExitStatus),
}

pub enum CargoProfile {
    Dev,
    Release,
}

pub fn read_project(project_path: impl AsRef<Path>) -> Result<(), Error> {
    // Currently, this is the only supported target triple
    let target_triple = "x86_64-unknown-linux-musl";
    let cargo_profile = CargoProfile::Dev;

    let mut cargo_args = vec!["build", "--target", target_triple];
    match cargo_profile {
        CargoProfile::Dev => {}
        CargoProfile::Release => cargo_args.push("--release"),
    }

    let cargo_build_exit_status = Command::new("cargo")
        .args(cargo_args)
        .envs(cross_compile_env().map_err(Error::CrossCompileError)?)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

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

    let target = buildpack_cargo_package.targets.first().unwrap();

    let buildpack_binary_path = cargo_metadata
        .target_directory
        .join(&target_triple)
        .join(match cargo_profile {
            CargoProfile::Dev => "debug",
            CargoProfile::Release => "release",
        })
        .join(&target.name);

    let temporary_buildpack_dir = tempfile::tempdir().unwrap();

    write_buildpack(
        temporary_buildpack_dir.path(),
        buildpack_toml_path,
        buildpack_binary_path,
    )
    .unwrap();

    package_tarball(
        temporary_buildpack_dir.path(),
        &mut File::create(cargo_metadata.target_directory.join(format!(
            "{}_buildpack_{}.tar.gz",
            buildpack_descriptor.buildpack.id,
            match cargo_profile {
                CargoProfile::Dev => "dev",
                CargoProfile::Release => "release",
            }
        )))
        .unwrap(),
    )
    .unwrap();

    Ok(())
}

#[derive(Debug)]
pub enum CrossCompileError {
    CouldNotFindLinkerBinary(String),
    CouldNotFindCCBinary(String),
}

fn cross_compile_env() -> Result<Vec<(OsString, OsString)>, CrossCompileError> {
    let env = if cfg!(target_os = "macos") {
        let ld_binary_name = "x86_64-linux-musl-ld";
        let cc_binary_name = "x86_64-linux-musl-gcc";

        vec![
            (
                OsString::from("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER"),
                which(ld_binary_name)
                    .map_err(|_| {
                        CrossCompileError::CouldNotFindLinkerBinary(String::from(ld_binary_name))
                    })?
                    .into_os_string(),
            ),
            (
                OsString::from("CC_x86_64_unknown_linux_musl"),
                which(cc_binary_name)
                    .map_err(|_| {
                        CrossCompileError::CouldNotFindCCBinary(String::from(cc_binary_name))
                    })?
                    .into_os_string(),
            ),
        ]
    } else {
        vec![]
    };

    Ok(env)
}

fn write_buildpack(
    target_dir_path: impl AsRef<Path>,
    buildpack_toml_path: impl AsRef<Path>,
    buildpack_binary_path: impl AsRef<Path>,
) -> std::io::Result<()> {
    let bin_dir_path = target_dir_path.as_ref().join("bin");
    let detect_path = bin_dir_path.join("detect");
    let build_path = bin_dir_path.join("build");

    fs::create_dir_all(&bin_dir_path)?;

    fs::copy(
        &buildpack_toml_path,
        target_dir_path.as_ref().join("buildpack.toml"),
    )?;

    fs::copy(&buildpack_binary_path, &build_path)?;
    std::os::unix::fs::symlink(&build_path, &detect_path).unwrap();

    Ok(())
}

fn package_tarball(
    directory: impl AsRef<Path>,
    destination_file: &mut File,
) -> std::io::Result<()> {
    tar::Builder::new(GzEncoder::new(destination_file, Compression::default()))
        .append_dir_all(".", directory.as_ref())
}
