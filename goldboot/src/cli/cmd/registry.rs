use std::process::ExitCode;
use tracing::info;

use crate::{
    library::ImageLibrary,
    registry::{Client, ImageRef},
};

pub(crate) fn resolve_auth(
    username: Option<String>,
    password: Option<String>,
) -> Result<Option<(String, String)>, &'static str> {
    match (username, password) {
        (Some(u), Some(p)) => Ok(Some((u, p))),
        (None, None) => Ok(None),
        (Some(_), None) => Err("--username given without --password"),
        (None, Some(_)) => Err("--password given without --username"),
    }
}

/// Pull `<host>/<name>:<tag>` from a remote registry into the local
/// library at the corresponding `<host>/<name>/<tag>.gb` path.
pub fn pull(reference: String, username: Option<String>, password: Option<String>) -> ExitCode {
    let r = match ImageRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid reference: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(remote_host) = r.host.clone() else {
        eprintln!("pull requires a host: <host>/<name>:<tag>");
        return ExitCode::FAILURE;
    };
    let Some(tag) = r.tag.clone() else {
        eprintln!(
            "Tag is required for pull (no server-side 'latest' alias). Use {reference}:<tag>."
        );
        return ExitCode::FAILURE;
    };

    let auth = match resolve_auth(username, password) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let client = match Client::new(&remote_host, auth) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Bad registry address: {e}");
            return ExitCode::FAILURE;
        }
    };

    let library = ImageLibrary::open();
    let tmp = library.temporary();
    info!("Pulling {reference}");
    if let Err(e) = client.pull_to_file(&r.name, &tag, &tmp) {
        eprintln!("Pull failed: {e}");
        let _ = std::fs::remove_file(&tmp);
        return ExitCode::FAILURE;
    }
    if let Err(e) = library.add_built(&tmp, &r) {
        eprintln!("Failed to add image to library: {e}");
        let _ = std::fs::remove_file(&tmp);
        return ExitCode::FAILURE;
    }
    println!("Pulled {reference}");
    ExitCode::SUCCESS
}

/// Push a local image to a remote registry. The destination reference is
/// `<dest_host>/<name>[:<tag>]`. When `:<tag>` is omitted, the image's
/// in-header tag is used. The image is sourced from the local library —
/// first under `local/<name>/<tag>`, then under `<dest_host>/<name>/<tag>`
/// in case the image has already been promoted.
pub fn push(reference: String, username: Option<String>, password: Option<String>) -> ExitCode {
    let dest = match ImageRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid reference: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(remote_host) = dest.host.clone() else {
        eprintln!("push requires a host: <host>/<name>[:<tag>]");
        return ExitCode::FAILURE;
    };
    let dest_host_bare = dest
        .host_bare()
        .expect("host present — checked above")
        .to_string();
    let auth = match resolve_auth(username, password) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let library = ImageLibrary::open();

    // Find the source image. Prefer the hostless (locally-built) copy;
    // fall back to the destination host in case the image was already
    // promoted by an earlier push.
    let local_lookup = ImageRef {
        host: None,
        name: dest.name.clone(),
        tag: dest.tag.clone(),
    };
    let (current_host, image) = match library.find_by_ref(&local_lookup) {
        Ok(h) => (None, h),
        Err(_) => match library.find_by_ref(&dest) {
            Ok(h) => (Some(dest_host_bare.clone()), h),
            Err(e) => {
                eprintln!("No local image matching {reference}: {e}");
                return ExitCode::FAILURE;
            }
        },
    };
    let resolved_tag = dest
        .tag
        .clone()
        .unwrap_or_else(|| image.primary_header.tag_str());

    let file_len = match std::fs::metadata(&image.path) {
        Ok(m) => m.len(),
        Err(e) => {
            eprintln!("Failed to stat image: {e}");
            return ExitCode::FAILURE;
        }
    };
    let file = match std::fs::File::open(&image.path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open image: {e}");
            return ExitCode::FAILURE;
        }
    };

    let client = match Client::new(&remote_host, auth) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Bad registry address: {e}");
            return ExitCode::FAILURE;
        }
    };

    info!("Pushing {} ({} bytes)", image.path.display(), file_len);
    if let Err(e) = client.push_image(&dest.name, &resolved_tag, file, file_len) {
        eprintln!("Push failed: {e}");
        return ExitCode::FAILURE;
    }

    // Reflect the push in the local library by moving the file under the
    // destination host's bucket (if it wasn't already there).
    if current_host.as_deref() != Some(&dest_host_bare) {
        let from = ImageRef {
            host: current_host,
            name: dest.name.clone(),
            tag: Some(resolved_tag.clone()),
        };
        let to = ImageRef {
            host: Some(dest_host_bare.clone()),
            name: dest.name.clone(),
            tag: Some(resolved_tag.clone()),
        };
        if let Err(e) = library.move_ref(&from, &to) {
            eprintln!("Warning: pushed successfully but failed to relocate local copy: {e}");
        }
    }

    let printed = ImageRef::new(&dest.name)
        .with_host(&dest_host_bare)
        .with_tag(&resolved_tag);
    println!("Pushed {}", printed);
    ExitCode::SUCCESS
}
