use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// Detected project configuration from auto-detection
#[derive(Debug, Clone)]
pub struct DetectedProject {
    /// App name derived from config file or directory name
    pub name: String,
    /// Command to run (e.g., "npm", "cargo", "go")
    pub command: String,
    /// Arguments to pass to the command
    pub args: Vec<String>,
    /// Default port for this framework (if known)
    pub default_port: Option<u16>,
    /// Language/framework type for logging
    pub framework: String,
}

/// Try to detect project type from config files in the given directory.
/// Returns None if no recognizable project is found.
pub fn detect_project(dir: &Path) -> Result<Option<DetectedProject>> {
    // Try each config file in order of preference
    if let Some(project) = try_package_json(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_cargo_toml(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_go_mod(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_pyproject_toml(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_gemfile(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_mix_exs(dir)? {
        return Ok(Some(project));
    }
    if let Some(project) = try_composer_json(dir)? {
        return Ok(Some(project));
    }

    Ok(None)
}

/// Get the directory name as a fallback app name
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app")
        .to_string()
}

/// Try to detect Node.js project from package.json
fn try_package_json(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("package.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let pkg: PackageJson = serde_json::from_str(&content)?;

    let name = pkg.name.clone().unwrap_or_else(|| dir_name(dir));

    // Determine the dev command based on what's available
    let (command, args, default_port) = detect_node_command(dir, &pkg);

    Ok(Some(DetectedProject {
        name,
        command,
        args,
        default_port,
        framework: "Node.js".to_string(),
    }))
}

/// Detect the appropriate Node.js dev command
fn detect_node_command(dir: &Path, pkg: &PackageJson) -> (String, Vec<String>, Option<u16>) {
    let scripts = pkg.scripts.as_ref();

    // Check for common dev tools in devDependencies or dependencies
    let has_dev_dep = |name: &str| -> bool {
        pkg.dev_dependencies
            .as_ref()
            .map_or(false, |d| d.contains_key(name))
            || pkg.dependencies
                .as_ref()
                .map_or(false, |d| d.contains_key(name))
    };

    // Check for lock files to determine package manager
    let has_file = |name: &str| dir.join(name).exists();

    // Priority: check for scripts first, then detect from dependencies
    if let Some(scripts) = scripts {
        if scripts.contains_key("dev") {
            // Use the package manager's dev script
            let (pm, pm_args) = if has_file("pnpm-lock.yaml") {
                ("pnpm", vec!["run".to_string(), "dev".to_string()])
            } else if has_file("yarn.lock") {
                ("yarn", vec!["dev".to_string()])
            } else if has_file("bun.lockb") || has_file("bun.lock") {
                ("bun", vec!["run".to_string(), "dev".to_string()])
            } else {
                ("npm", vec!["run".to_string(), "dev".to_string()])
            };
            return (pm.to_string(), pm_args, Some(5173));
        }
        if scripts.contains_key("start") {
            let (pm, pm_args) = if has_file("pnpm-lock.yaml") {
                ("pnpm", vec!["start".to_string()])
            } else if has_file("yarn.lock") {
                ("yarn", vec!["start".to_string()])
            } else if has_file("bun.lockb") || has_file("bun.lock") {
                ("bun", vec!["start".to_string()])
            } else {
                ("npm", vec!["start".to_string()])
            };
            return (pm.to_string(), pm_args, Some(3000));
        }
    }

    // Detect from dependencies
    if has_dev_dep("vite") || has_dev_dep("astro") {
        let (pm, pm_args) = if has_file("pnpm-lock.yaml") {
            ("pnpm", vec!["run".to_string(), "dev".to_string()])
        } else if has_file("yarn.lock") {
            ("yarn", vec!["dev".to_string()])
        } else {
            ("npm", vec!["run".to_string(), "dev".to_string()])
        };
        return (pm.to_string(), pm_args, Some(5173));
    }

    if has_dev_dep("next") {
        return ("npx".to_string(), vec!["next".to_string(), "dev".to_string()], Some(3000));
    }

    if has_dev_dep("nuxt") || has_dev_dep("nuxt3") {
        return ("npx".to_string(), vec!["nuxt".to_string(), "dev".to_string()], Some(3000));
    }

    if has_dev_dep("react-scripts") {
        return ("npm".to_string(), vec!["start".to_string()], Some(3000));
    }

    if has_dev_dep("angular-cli") || has_dev_dep("@angular/cli") {
        return ("npx".to_string(), vec!["ng".to_string(), "serve".to_string()], Some(4200));
    }

    // Fallback: try npm start or just node
    if has_file("node_modules") {
        return ("npm".to_string(), vec!["start".to_string()], Some(3000));
    }

    // Last resort: use node directly
    ("node".to_string(), vec![".".to_string()], Some(3000))
}

/// Try to detect Rust project from Cargo.toml
fn try_cargo_toml(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cargo: CargoToml = toml::from_str(&content)?;

    let name = cargo.package
        .as_ref()
        .and_then(|p| p.name.clone())
        .unwrap_or_else(|| dir_name(dir));

    // Check if it's a web framework
    let is_web = cargo.dependencies.as_ref().map_or(false, |deps| {
        deps.contains_key("actix-web")
            || deps.contains_key("axum")
            || deps.contains_key("warp")
            || deps.contains_key("rocket")
            || deps.contains_key("hyper")
            || deps.contains_key("poem")
            || deps.contains_key("salvo")
    });

    if is_web {
        Ok(Some(DetectedProject {
            name,
            command: "cargo".to_string(),
            args: vec!["run".to_string()],
            default_port: Some(8080),
            framework: "Rust".to_string(),
        }))
    } else {
        // Not a web project, still allow running
        Ok(Some(DetectedProject {
            name,
            command: "cargo".to_string(),
            args: vec!["run".to_string()],
            default_port: None,
            framework: "Rust".to_string(),
        }))
    }
}

/// Try to detect Go project from go.mod
fn try_go_mod(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("go.mod");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let module_name = content
        .lines()
        .find(|l| l.starts_with("module "))
        .map(|l| l.trim_start_matches("module ").trim().to_string())
        .unwrap_or_else(|| dir_name(dir));

    // Extract just the last part of the module name for the app name
    let name = module_name
        .rsplit('/')
        .next()
        .unwrap_or(&module_name)
        .to_string();

    // Check for common Go web frameworks
    let has_web_dep = content.contains("gin-gonic")
        || content.contains("gorilla/mux")
        || content.contains("go-chi")
        || content.contains("echo")
        || content.contains("fiber")
        || content.contains("labstack");

    if has_web_dep {
        Ok(Some(DetectedProject {
            name,
            command: "go".to_string(),
            args: vec!["run".to_string(), ".".to_string()],
            default_port: Some(8080),
            framework: "Go".to_string(),
        }))
    } else {
        Ok(Some(DetectedProject {
            name,
            command: "go".to_string(),
            args: vec!["run".to_string(), ".".to_string()],
            default_port: None,
            framework: "Go".to_string(),
        }))
    }
}

/// Try to detect Python project from pyproject.toml
fn try_pyproject_toml(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("pyproject.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let pyproject: PyProjectToml = toml::from_str(&content)?;

    let name = pyproject
        .project
        .as_ref()
        .and_then(|p| p.name.clone())
        .unwrap_or_else(|| dir_name(dir));

    // Check for common Python web frameworks in dependencies
    let is_web = pyproject
        .project
        .as_ref()
        .and_then(|p| p.dependencies.as_ref())
        .map_or(false, |deps| {
            deps.iter().any(|d| {
                d.starts_with("django")
                    || d.starts_with("flask")
                    || d.starts_with("fastapi")
                    || d.starts_with("starlette")
                    || d.starts_with("uvicorn")
            })
        });

    if is_web {
        // Try to detect the appropriate command
        if pyproject
            .project
            .as_ref()
            .and_then(|p| p.dependencies.as_ref())
            .map_or(false, |deps| deps.iter().any(|d| d.starts_with("fastapi") || d.starts_with("uvicorn")))
        {
            return Ok(Some(DetectedProject {
                name,
                command: "uvicorn".to_string(),
                args: vec!["main:app".to_string(), "--reload".to_string()],
                default_port: Some(8000),
                framework: "Python".to_string(),
            }));
        }

        if pyproject
            .project
            .as_ref()
            .and_then(|p| p.dependencies.as_ref())
            .map_or(false, |deps| deps.iter().any(|d| d.starts_with("django")))
        {
            return Ok(Some(DetectedProject {
                name,
                command: "python".to_string(),
                args: vec!["manage.py".to_string(), "runserver".to_string()],
                default_port: Some(8000),
                framework: "Python".to_string(),
            }));
        }

        if pyproject
            .project
            .as_ref()
            .and_then(|p| p.dependencies.as_ref())
            .map_or(false, |deps| deps.iter().any(|d| d.starts_with("flask")))
        {
            return Ok(Some(DetectedProject {
                name,
                command: "flask".to_string(),
                args: vec!["run".to_string()],
                default_port: Some(5000),
                framework: "Python".to_string(),
            }));
        }
    }

    // Generic Python project
    Ok(Some(DetectedProject {
        name,
        command: "python".to_string(),
        args: vec!["-m".to_string(), "http.server".to_string()],
        default_port: Some(8000),
        framework: "Python".to_string(),
    }))
}

/// Try to detect Ruby project from Gemfile
fn try_gemfile(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("Gemfile");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let name = dir_name(dir);

    // Check for Rails
    if content.contains("rails") {
        return Ok(Some(DetectedProject {
            name,
            command: "bundle".to_string(),
            args: vec!["exec".to_string(), "rails".to_string(), "server".to_string()],
            default_port: Some(3000),
            framework: "Ruby on Rails".to_string(),
        }));
    }

    // Check for Sinatra
    if content.contains("sinatra") {
        return Ok(Some(DetectedProject {
            name,
            command: "bundle".to_string(),
            args: vec!["exec".to_string(), "ruby".to_string(), "app.rb".to_string()],
            default_port: Some(4567),
            framework: "Ruby (Sinatra)".to_string(),
        }));
    }

    // Generic Ruby project
    Ok(Some(DetectedProject {
        name,
        command: "bundle".to_string(),
        args: vec!["exec".to_string(), "rackup".to_string()],
        default_port: Some(9292),
        framework: "Ruby".to_string(),
    }))
}

/// Try to detect Elixir project from mix.exs
fn try_mix_exs(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("mix.exs");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let name = dir_name(dir);

    // Check for Phoenix
    if content.contains("phoenix") {
        return Ok(Some(DetectedProject {
            name,
            command: "mix".to_string(),
            args: vec!["phx.server".to_string()],
            default_port: Some(4000),
            framework: "Elixir (Phoenix)".to_string(),
        }));
    }

    // Generic Elixir project
    Ok(Some(DetectedProject {
        name,
        command: "mix".to_string(),
        args: vec!["run".to_string()],
        default_port: None,
        framework: "Elixir".to_string(),
    }))
}

/// Try to detect PHP project from composer.json
fn try_composer_json(dir: &Path) -> Result<Option<DetectedProject>> {
    let path = dir.join("composer.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let composer: ComposerJson = serde_json::from_str(&content)?;
    let name = composer.name
        .map(|n| n.rsplit('/').next().unwrap_or(&n).to_string())
        .unwrap_or_else(|| dir_name(dir));

    // Check for Laravel
    if let Some(require) = &composer.require {
        if require.contains_key("laravel/framework") {
            return Ok(Some(DetectedProject {
                name,
                command: "php".to_string(),
                args: vec!["artisan".to_string(), "serve".to_string()],
                default_port: Some(8000),
                framework: "PHP (Laravel)".to_string(),
            }));
        }
    }

    // Generic PHP project - use built-in server
    Ok(Some(DetectedProject {
        name,
        command: "php".to_string(),
        args: vec![
            "-S".to_string(),
            "127.0.0.1:8000".to_string(),
            "-t".to_string(),
            "public".to_string(),
        ],
        default_port: Some(8000),
        framework: "PHP".to_string(),
    }))
}

// Serde structs for parsing config files

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: Option<String>,
    scripts: Option<std::collections::HashMap<String, String>>,
    dependencies: Option<std::collections::HashMap<String, String>>,
    dev_dependencies: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: Option<CargoPackage>,
    dependencies: Option<std::collections::HashMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PyProjectToml {
    project: Option<PyProject>,
}

#[derive(Debug, Deserialize)]
struct PyProject {
    name: Option<String>,
    dependencies: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ComposerJson {
    name: Option<String>,
    require: Option<std::collections::HashMap<String, String>>,
}
