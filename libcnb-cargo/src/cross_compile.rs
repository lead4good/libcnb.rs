use std::ffi::OsString;
use which::which;

#[derive(Debug)]
pub enum CrossCompileError {
    CouldNotFindLinkerBinary(String),
    CouldNotFindCCBinary(String),
    UnsupportedTargetTriple(String),
}

pub fn cross_compile_env(
    target_triple: impl AsRef<str>,
) -> Result<Vec<(OsString, OsString)>, CrossCompileError> {
    match target_triple.as_ref() {
        "x86_64-unknown-linux-musl" => {
            let env = if cfg!(target_os = "macos") {
                let ld_binary_name = "x86_64-linux-musl-ld";
                let cc_binary_name = "x86_64-linux-musl-gcc";

                vec![
                    (
                        OsString::from("CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER"),
                        which(ld_binary_name)
                            .map_err(|_| {
                                CrossCompileError::CouldNotFindLinkerBinary(String::from(
                                    ld_binary_name,
                                ))
                            })?
                            .into_os_string(),
                    ),
                    (
                        OsString::from("CC_x86_64_unknown_linux_musl"),
                        which(cc_binary_name)
                            .map_err(|_| {
                                CrossCompileError::CouldNotFindCCBinary(String::from(
                                    cc_binary_name,
                                ))
                            })?
                            .into_os_string(),
                    ),
                ]
            } else {
                vec![]
            };

            Ok(env)
        }
        target_triple => Err(CrossCompileError::UnsupportedTargetTriple(String::from(
            target_triple,
        ))),
    }
}
