//! NPM package completion provider
//!
//! Provides completions for npm package names with local caching.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Local package cache
static PACKAGE_CACHE: Mutex<Option<PackageCache>> = Mutex::new(None);

struct PackageCache {
    packages: HashMap<String, Vec<String>>,
    fetched_at: Instant,
}

/// Provides npm/yarn/pnpm package name completions
pub struct NpmPackageProvider;

impl NpmPackageProvider {
    pub fn new() -> Self {
        Self
    }

    /// Get locally installed packages from node_modules
    fn get_local_packages(&self, cwd: &PathBuf) -> Vec<String> {
        let node_modules = cwd.join("node_modules");
        if !node_modules.exists() {
            return vec![];
        }

        let mut packages = Vec::new();

        // Read direct children (regular packages)
        if let Ok(entries) = fs::read_dir(&node_modules) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files and non-packages
                if name.starts_with('.') || name.starts_with('_') {
                    continue;
                }

                // Handle scoped packages (@org/package)
                if name.starts_with('@') {
                    let scope_path = node_modules.join(&name);
                    if let Ok(scope_entries) = fs::read_dir(&scope_path) {
                        for scope_entry in scope_entries.filter_map(|e| e.ok()) {
                            let pkg_name = scope_entry.file_name().to_string_lossy().to_string();
                            if !pkg_name.starts_with('.') {
                                packages.push(format!("{}/{}", name, pkg_name));
                            }
                        }
                    }
                } else {
                    packages.push(name);
                }
            }
        }

        packages.sort();
        packages
    }

    /// Get packages from package.json dependencies
    fn get_package_json_deps(&self, cwd: &PathBuf) -> Vec<String> {
        let package_json = cwd.join("package.json");
        if !package_json.exists() {
            return vec![];
        }

        let Ok(content) = fs::read_to_string(&package_json) else {
            return vec![];
        };

        let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
            return vec![];
        };

        let mut packages = Vec::new();

        for dep_key in ["dependencies", "devDependencies", "peerDependencies"] {
            if let Some(deps) = json.get(dep_key).and_then(|v| v.as_object()) {
                for name in deps.keys() {
                    packages.push(name.clone());
                }
            }
        }

        packages.sort();
        packages.dedup();
        packages
    }

    /// Get globally installed packages
    fn get_global_packages(&self) -> Vec<String> {
        use std::process::Command;

        let output = Command::new("npm")
            .args(["list", "-g", "--depth=0", "--json"])
            .output()
            .ok();

        output
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|json| {
                json.get("dependencies")
                    .and_then(|deps| deps.as_object())
                    .map(|deps| deps.keys().cloned().collect())
            })
            .unwrap_or_default()
    }

    /// Get popular packages (commonly used, hardcoded list)
    fn get_popular_packages(&self) -> Vec<&'static str> {
        vec![
            "react",
            "react-dom",
            "vue",
            "angular",
            "svelte",
            "express",
            "fastify",
            "koa",
            "next",
            "nuxt",
            "typescript",
            "webpack",
            "vite",
            "esbuild",
            "rollup",
            "parcel",
            "eslint",
            "prettier",
            "jest",
            "vitest",
            "mocha",
            "chai",
            "lodash",
            "axios",
            "moment",
            "dayjs",
            "date-fns",
            "uuid",
            "nanoid",
            "zod",
            "yup",
            "joi",
            "dotenv",
            "cors",
            "helmet",
            "mongoose",
            "prisma",
            "drizzle-orm",
            "sequelize",
            "typeorm",
            "socket.io",
            "ws",
            "redis",
            "ioredis",
            "bull",
            "bullmq",
            "nodemailer",
            "sharp",
            "jimp",
            "puppeteer",
            "playwright",
            "cheerio",
            "commander",
            "yargs",
            "inquirer",
            "ora",
            "chalk",
            "picocolors",
            "glob",
            "fast-glob",
            "chokidar",
            "nodemon",
            "concurrently",
            "cross-env",
            "rimraf",
            "fs-extra",
        ]
    }
}

impl Default for NpmPackageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for NpmPackageProvider {
    fn name(&self) -> &str {
        "npm_package"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if arg_position < 1 {
            return false;
        }

        let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

        // npm install <package>
        // npm uninstall <package>
        // npm info <package>
        // npm view <package>
        // yarn add <package>
        // yarn remove <package>
        // pnpm add <package>
        // pnpm remove <package>

        match cmd {
            "npm" => {
                ["install", "i", "uninstall", "un", "remove", "info", "view", "update"]
                    .contains(&subcommand)
            }
            "yarn" => ["add", "remove", "info", "upgrade"].contains(&subcommand),
            "pnpm" => ["add", "remove", "info", "update"].contains(&subcommand),
            _ => false,
        }
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let mut suggestions = Vec::new();
        let partial_lower = partial.to_lowercase();

        let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

        // For uninstall/remove, only suggest installed packages
        let is_remove = ["uninstall", "un", "remove"].contains(&subcommand);

        if is_remove {
            // Only local packages
            let local = self.get_package_json_deps(&context.cwd);
            for pkg in local {
                if pkg.to_lowercase().starts_with(&partial_lower) {
                    suggestions.push(
                        ProviderSuggestion::new(&pkg)
                            .with_description("installed")
                            .with_category("local")
                            .with_score(100),
                    );
                }
            }
        } else {
            // For install, suggest popular packages and local ones
            let local = self.get_local_packages(&context.cwd);

            // Local packages (already installed, might want to update)
            for pkg in &local {
                if pkg.to_lowercase().starts_with(&partial_lower) {
                    suggestions.push(
                        ProviderSuggestion::new(pkg)
                            .with_description("installed")
                            .with_category("local")
                            .with_score(80),
                    );
                }
            }

            // Popular packages
            for pkg in self.get_popular_packages() {
                if pkg.to_lowercase().starts_with(&partial_lower)
                    && !local.iter().any(|l| l == pkg)
                {
                    suggestions.push(
                        ProviderSuggestion::new(pkg)
                            .with_description("popular")
                            .with_category("popular")
                            .with_score(50),
                    );
                }
            }
        }

        suggestions
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npm_package_provider_matches() {
        let provider = NpmPackageProvider::new();

        let ctx = ProviderContext::new(PathBuf::from("."), "npm", vec!["install".to_string()], "");
        assert!(provider.matches("npm", 1, &ctx));

        let ctx2 = ProviderContext::new(PathBuf::from("."), "yarn", vec!["add".to_string()], "");
        assert!(provider.matches("yarn", 1, &ctx2));
    }

    #[test]
    fn test_popular_packages() {
        let provider = NpmPackageProvider::new();
        let popular = provider.get_popular_packages();
        assert!(popular.contains(&"react"));
        assert!(popular.contains(&"typescript"));
    }
}
