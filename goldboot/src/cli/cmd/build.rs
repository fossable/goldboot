use crate::builder::Builder;
use crate::builder::config::ConfigPath;
use std::process::ExitCode;
use tracing::{debug, error};
use validator::Validate;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd.clone() {
        super::Commands::Build {
            read_password,
            path,
            name,
            ..
        } => {
            let config_path = match ConfigPath::from_dir(&path) {
                Ok(Some(p)) => {
                    debug!("Loading config from {}", p);
                    p
                }
                Ok(None) => {
                    error!(
                        "No goldboot.ron or <name>.goldboot.ron found in {}",
                        &path
                    );
                    return ExitCode::FAILURE;
                }
                Err(e) => {
                    error!("Failed to locate config file: {e}");
                    return ExitCode::FAILURE;
                }
            };

            let elements = match config_path.load() {
                Ok(c) => {
                    debug!("Loaded: {:#?}", &c);
                    c
                }
                Err(error) => {
                    error!("Failed to load config: {:?}", error);
                    return ExitCode::FAILURE;
                }
            };

            let resolved_name = match (name, config_path.inferred_name()) {
                (Some(cli), _) => cli,
                (None, Some(inferred)) => inferred.to_string(),
                (None, None) => {
                    error!(
                        "Image name required: pass --name, or rename `goldboot.ron` to `<name>.goldboot.ron`"
                    );
                    return ExitCode::FAILURE;
                }
            };

            if let Err(e) = goldboot_image::validate_ref_segment(&resolved_name) {
                error!("Invalid image name '{resolved_name}': {e}");
                return ExitCode::FAILURE;
            }

            let mut builder = Builder::new(resolved_name, elements, config_path.context_dir());

            if read_password {
                print!("Enter password: ");
                let mut password = String::new();
                std::io::stdin().read_line(&mut password).unwrap();
                // config.password = Some(password);
            } else if let Ok(_password) = std::env::var("GOLDBOOT_PASSWORD") {
                unsafe {
                    std::env::set_var("GOLDBOOT_PASSWORD", "");
                }
                // config.password = Some(password);
            }

            match builder.validate() {
                Err(err) => {
                    error!(error = ?err, "Failed to validate config file");
                    return ExitCode::FAILURE;
                }
                _ => debug!("Validated config file"),
            };

            match builder.run(cmd) {
                Err(err) => {
                    error!(error = ?err, "Failed to build image");
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            }
        }
        _ => panic!(),
    }
}
