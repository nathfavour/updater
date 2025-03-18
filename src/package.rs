use anyhow::{Context, Result};
use colored::*;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::system::{self, PackageManager};
use crate::utils;
use crate::version;

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub versions: HashMap<String, PackageVersion>,
    pub active_version: Option<String>,
    pub system: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageVersion {
    pub install_path: PathBuf,
    pub install_date: String,
    pub bin_paths: Vec<PathBuf>,
    pub package_manager: Option<String>,
}

pub fn get_package_db_path() -> PathBuf {
    let data_dir = dirs::data_dir().expect("Could not determine data directory");
    let updater_dir = data_dir.join("updater");
    fs::create_dir_all(&updater_dir).expect("Failed to create data directory");
    updater_dir.join("packages.json")
}

pub fn load_packages() -> Result<HashMap<String, Package>> {
    let db_path = get_package_db_path();
    if !db_path.exists() {
        return Ok(HashMap::new());
    }

    let data = fs::read_to_string(&db_path).context("Failed to read package database")?;
    let packages: HashMap<String, Package> = serde_json::from_str(&data).context("Failed to parse package database")?;
    Ok(packages)
}

pub fn save_packages(packages: &HashMap<String, Package>) -> Result<()> {
    let db_path = get_package_db_path();
    let data = serde_json::to_string_pretty(packages).context("Failed to serialize package database")?;
    fs::write(&db_path, data).context("Failed to write package database")?;
    Ok(())
}

pub fn install(name: &str, version: Option<String>, user: bool) -> Result<()> {
    let mut packages = load_packages()?;
    
    // Determine the appropriate package manager for the system
    let package_manager = system::detect_package_manager()?;
    let version_to_install = version.clone().unwrap_or_else(|| "latest".to_string());
    
    println!("Using package manager: {}", package_manager.get_name().cyan());
    
    // Define installation path based on user/system preference
    let base_install_path = if user {
        dirs::home_dir().unwrap().join(".local/share/updater/packages")
    } else {
        PathBuf::from("/opt/updater/packages")
    };
    
    let install_dir = base_install_path.join(name).join(&version_to_install);
    fs::create_dir_all(&install_dir)?;
    
    // Use the appropriate package manager to install
    let bin_paths = package_manager.install(name, version.as_deref(), &install_dir, user)?;
    
    // Update package database
    let package = packages.entry(name.to_string())
        .or_insert_with(|| Package {
            name: name.to_string(),
            versions: HashMap::new(),
            active_version: None,
            system: !user,
        });
    
    let now = chrono::Local::now().to_rfc3339();
    let package_version = PackageVersion {
        install_path: install_dir.clone(),
        install_date: now,
        bin_paths,
        package_manager: Some(package_manager.get_name().to_string()),
    };
    
    package.versions.insert(version_to_install.clone(), package_version);
    
    // If this is the first version or no active version, make it active
    if package.active_version.is_none() {
        package.active_version = Some(version_to_install);
    }
    
    save_packages(&packages)?;
    println!("{} {}", "Successfully installed".green(), name.yellow().bold());
    
    Ok(())
}

pub fn remove(name: &str, version: Option<String>) -> Result<()> {
    let mut packages = load_packages()?;
    
    if let Some(package) = packages.get_mut(name) {
        match version {
            Some(ver) => {
                if let Some(pkg_version) = package.versions.remove(&ver) {
                    // Remove the package files
                    fs::remove_dir_all(pkg_version.install_path)?;
                    
                    // If we removed the active version, set active to None
                    if package.active_version.as_ref() == Some(&ver) {
                        package.active_version = None;
                        
                        // Try to set another version as active if available
                        if let Some((next_ver, _)) = package.versions.iter().next() {
                            package.active_version = Some(next_ver.clone());
                            println!("{} {} {}", 
                                "Set".green(), 
                                next_ver.cyan(), 
                                "as the active version".green());
                        }
                    }
                    
                    println!("{} {} {}", 
                        "Removed version".green(), 
                        ver.yellow(), 
                        "of package".green());
                } else {
                    println!("{} {}", 
                        "Version not found:".red(), 
                        ver.yellow());
                    return Ok(());
                }
            },
            None => {
                // Remove all versions of the package
                for (_, pkg_version) in &package.versions {
                    if pkg_version.install_path.exists() {
                        fs::remove_dir_all(&pkg_version.install_path)?;
                    }
                }
                packages.remove(name);
                println!("{} {}", "Removed package".green(), name.yellow().bold());
            }
        }
        
        save_packages(&packages)?;
    } else {
        println!("{} {}", "Package not found:".red(), name.yellow());
    }
    
    Ok(())
}

pub fn update(name: Option<&str>) -> Result<()> {
    let mut packages = load_packages()?;
    
    match name {
        Some(package_name) => {
            if let Some(package) = packages.get(package_name) {
                if let Some(active_version) = &package.active_version {
                    if let Some(version_info) = package.versions.get(active_version) {
                        if let Some(pm_name) = &version_info.package_manager {
                            let pm = system::get_package_manager_by_name(pm_name)?;
                            pm.update(package_name, Some(active_version), &version_info.install_path, !package.system)?;
                            println!("{} {}", "Updated package".green(), package_name.yellow().bold());
                        }
                    }
                }
            } else {
                println!("{} {}", "Package not found:".red(), package_name.yellow());
            }
        },
        None => {
            // Update all packages
            for (name, package) in &packages {
                if let Some(active_version) = &package.active_version {
                    if let Some(version_info) = package.versions.get(active_version) {
                        if let Some(pm_name) = &version_info.package_manager {
                            let pm = system::get_package_manager_by_name(pm_name)?;
                            match pm.update(name, Some(active_version), &version_info.install_path, !package.system) {
                                Ok(_) => println!("{} {}", "Updated package".green(), name.yellow()),
                                Err(e) => println!("{} {}: {}", "Failed to update".red(), name.yellow(), e),
                            }
                        }
                    }
                }
            }
        }
    }
    
    save_packages(&packages)?;
    Ok(())
}

pub fn list(system_only: bool, user_only: bool) -> Result<()> {
    let packages = load_packages()?;
    
    if packages.is_empty() {
        println!("{}", "No packages installed".yellow());
        return Ok(());
    }
    
    let mut count = 0;
    for (name, package) in packages {
        // Filter based on package type
        if (system_only && !package.system) || (user_only && package.system) {
            continue;
        }
        
        count += 1;
        let pkg_type = if package.system { "system" } else { "user" };
        println!("{} {} ({})", name.green().bold(), pkg_type.cyan(), package.versions.len().to_string().yellow());
        
        for (version, pkg_version) in &package.versions {
            let active_marker = if Some(version) == package.active_version.as_ref() {
                "* ".green().bold()
            } else {
                "  ".normal()
            };
            
            println!("{}v{} - installed on {}", 
                active_marker,
                version.cyan(),
                pkg_version.install_date.yellow());
        }
        println!();
    }
    
    if count == 0 {
        if system_only {
            println!("{}", "No system packages installed".yellow());
        } else if user_only {
            println!("{}", "No user packages installed".yellow());
        }
    }
    
    Ok(())
}

pub fn search(query: &str) -> Result<()> {
    // Get available package managers
    let package_managers = system::get_available_package_managers()?;
    let mut found = false;
    
    for pm in package_managers {
        println!("{} {}", "Searching with".green(), pm.get_name().cyan());
        let results = pm.search(query)?;
        
        if !results.is_empty() {
            found = true;
            for result in results {
                println!("{} - {} [{}]", 
                    result.name.green().bold(),
                    result.description.normal(),
                    pm.get_name().cyan());
            }
        }
    }
    
    if !found {
        println!("{} {}", "No packages found matching:".yellow(), query);
    }
    
    Ok(())
}

pub fn switch(name: &str, version: &str) -> Result<()> {
    let mut packages = load_packages()?;
    
    if let Some(package) = packages.get_mut(name) {
        if package.versions.contains_key(version) {
            package.active_version = Some(version.to_string());
            save_packages(&packages)?;
            println!("{} {} {} {}", 
                "Switched".green(), 
                name.yellow().bold(),
                "to version".green(),
                version.cyan());
        } else {
            println!("{} {} {}", 
                "Version".red(), 
                version.yellow(),
                "not found for package".red());
        }
    } else {
        println!("{} {}", 
            "Package not found:".red(), 
            name.yellow());
    }
    
    Ok(())
}
