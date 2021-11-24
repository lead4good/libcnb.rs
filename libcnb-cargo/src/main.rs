use libcnb_cargo::Error;
use libcnb_cargo::{package_buildpack, CargoProfile};

fn main() {
    let current_dir = std::env::current_dir().unwrap();

    package_buildpack(&current_dir, CargoProfile::Dev).unwrap();

    /*match read_project(&current_dir) {
        Ok(_) => {}
        Err(Error::CouldNotFindBuildpackToml) => {}
        Err(Error::CouldNotReadBuildpackToml(err)) => {}
        Err(Error::BuildpackTomlDeserializationError(err)) => {}
        Err(Error::CargoMetadataError(err)) => {}
        Err(Error::CouldNotFindBuildpackCargoPackage) => {}
        Err(Error::CrossCompileError(err)) => {}
        Err(Error::CargoBuildUnsuccessful(exit_status)) => {}
    };*/
}
