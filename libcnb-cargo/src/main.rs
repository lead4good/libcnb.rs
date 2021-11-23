use libcnb_cargo::read_project;
use libcnb_cargo::Error;

fn main() {
    let current_dir = std::env::current_dir().unwrap();

    match read_project(&current_dir) {
        Ok(_) => {}
        Err(Error::CouldNotFindBuildpackToml) => {}
        Err(Error::CouldNotReadBuildpackToml(err)) => {}
        Err(Error::BuildpackTomlDeserializationError(err)) => {}
        Err(Error::CargoMetadataError(err)) => {}
        Err(Error::CouldNotFindBuildpackCargoPackage) => {}
        Err(Error::CrossCompileError(err)) => {}
    };
}
