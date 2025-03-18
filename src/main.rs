use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

mod package;
mod system;
mod utils;
mod version;

#[derive(Parser)]
#[command(author, version, about = "Modern package manager for Linux")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package
    Install {
        /// Name of the package to install
        name: String,
        /// Specific version to install
        #[arg(short, long)]
        version: Option<String>,
        /// Install as user package (not system-wide)
        #[arg(short, long)]
        user: bool,
    },
    /// Remove a package
    Remove {
        /// Name of the package to remove
        name: String,
        /// Specific version to remove, removes all versions if not specified
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Update packages
    Update {
        /// Specific package to update, updates all if not specified
        name: Option<String>,
    },
    /// List installed packages
    List {
        /// Show system packages only
        #[arg(long)]
        system: bool,
        /// Show user packages only
        #[arg(long)]
        user: bool,
    },
    /// Search for packages
    Search {
        /// Query to search for
        query: String,
    },
    /// Switch between versions of a package
    Switch {
        /// Package name
        name: String,
        /// Version to switch to
        version: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match &cli.command {
        Commands::Install { name, version, user } => {
            println!("{}{}{}{}",
                "Installing package ".green(),
                name.yellow().bold(),
                if let Some(v) = version { format!(" version {}", v.cyan()) } else { "".to_string() },
                if *user { " (user package)".to_string() } else { "".to_string() }
            );
            package::install(name, version.clone(), *user)
        }
        Commands::Remove { name, version } => {
            println!("{}{}{}",
                "Removing package ".green(),
                name.yellow().bold(),
                if let Some(v) = version { format!(" version {}", v.cyan()) } else { "".to_string() }
            );
            package::remove(name, version.clone())
        }
        Commands::Update { name } => {
            if let Some(package_name) = name {
                println!("{} {}", "Updating package".green(), package_name.yellow().bold());
                package::update(Some(package_name))
            } else {
                println!("{}", "Updating all packages".green());
                package::update(None)
            }
        }
        Commands::List { system, user } => {
            println!("{}", "Listing installed packages".green());
            package::list(*system, *user)
        }
        Commands::Search { query } => {
            println!("{} {}", "Searching for".green(), query.yellow());
            package::search(query)
        }
        Commands::Switch { name, version } => {
            println!("{} {} {}{}", 
                "Switching".green(), 
                name.yellow().bold(),
                "to version".green(),
                version.cyan()
            );
            package::switch(name, version)
        }
    }
}
