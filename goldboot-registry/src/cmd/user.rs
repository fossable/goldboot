//! `user add|remove|list|hash` — user account management.

use crate::{
    auth::hash_password,
    cmd::UserCommands,
    config::{Config, UserConfig},
};
use anyhow::{Context, Result, bail};
use dialoguer::{Password, theme::ColorfulTheme};
use std::{io::BufRead, path::Path};
use zeroize::Zeroize;

pub fn run(cmd: UserCommands) -> Result<()> {
    match cmd {
        UserCommands::Add {
            config,
            username,
            pull,
            push,
        } => add(&config, &username, pull, push),
        UserCommands::Remove { config, username } => remove(&config, &username),
        UserCommands::List { config } => list(&config),
        UserCommands::Hash { stdin } => hash_cmd(stdin),
    }
}

fn add(config_path: &Path, username: &str, pull: bool, push: bool) -> Result<()> {
    let mut cfg = if config_path.exists() {
        Config::load(config_path)?
    } else {
        Config::default()
    };
    if cfg.users.contains_key(username) {
        bail!("user '{}' already exists", username);
    }
    let mut password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Password for {username}"))
        .with_confirmation("Confirm", "Passwords do not match")
        .interact()?;
    let phc = hash_password(&password)?;
    password.zeroize();
    cfg.users.insert(
        username.to_string(),
        UserConfig {
            password_hash: phc,
            pull,
            push,
        },
    );
    cfg.save(config_path)
        .with_context(|| format!("save {}", config_path.display()))?;
    println!("Added user '{username}'");
    Ok(())
}

fn remove(config_path: &Path, username: &str) -> Result<()> {
    let mut cfg = Config::load(config_path)?;
    if cfg.users.remove(username).is_none() {
        bail!("user '{}' not found", username);
    }
    cfg.save(config_path)?;
    println!("Removed user '{username}'");
    Ok(())
}

fn list(config_path: &Path) -> Result<()> {
    let cfg = Config::load(config_path)?;
    if cfg.users.is_empty() {
        println!("(no users)");
        return Ok(());
    }
    let mut names: Vec<_> = cfg.users.iter().collect();
    names.sort_by_key(|(k, _)| (*k).clone());
    for (name, u) in names {
        let perms = match (u.pull, u.push) {
            (true, true) => "pull,push",
            (true, false) => "pull",
            (false, true) => "push",
            (false, false) => "none",
        };
        println!("{name}\t{perms}");
    }
    Ok(())
}

fn hash_cmd(stdin: bool) -> Result<()> {
    let mut password = if stdin {
        let mut buf = String::new();
        std::io::stdin().lock().read_line(&mut buf)?;
        buf.trim_end_matches(&['\r', '\n'][..]).to_string()
    } else {
        Password::with_theme(&ColorfulTheme::default())
            .with_prompt("Password")
            .with_confirmation("Confirm", "Passwords do not match")
            .interact()?
    };
    let phc = hash_password(&password)?;
    password.zeroize();
    println!("{phc}");
    Ok(())
}
