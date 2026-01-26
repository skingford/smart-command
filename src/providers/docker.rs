//! Docker completion providers
//!
//! Provides dynamic completions for docker images, containers, volumes, and networks.

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::process::Command;
use std::time::Duration;

/// Helper to run docker commands
fn docker_command(args: &[&str]) -> Option<String> {
    Command::new("docker")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

/// Check if docker is available
fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ============================================================================
// Docker Image Provider
// ============================================================================

/// Provides image name completions for docker run/pull/rmi
pub struct DockerImageProvider;

impl DockerImageProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_images(&self) -> Vec<(String, String, String)> {
        // Returns (repository:tag, image_id, size)
        docker_command(&[
            "images",
            "--format",
            "{{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Size}}",
        ])
        .map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 3 && !parts[0].contains("<none>") {
                        Some((
                            parts[0].to_string(),
                            parts[1].to_string(),
                            parts[2].to_string(),
                        ))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
    }
}

impl Default for DockerImageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for DockerImageProvider {
    fn name(&self) -> &str {
        "docker_image"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !docker_available() {
            return false;
        }

        if cmd == "docker" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

            // docker run <image>
            // docker pull <image>
            // docker push <image>
            // docker rmi <image>
            // docker history <image>
            // docker inspect <image>
            // docker tag <image>
            let image_commands = ["run", "pull", "push", "rmi", "history", "inspect", "tag"];

            if image_commands.contains(&subcommand) {
                // For 'run', image should be after all flags
                if subcommand == "run" {
                    // Check if we're past the flags
                    let last_arg = context.args.last().map(|s| s.as_str()).unwrap_or("");
                    if last_arg.starts_with('-') {
                        return false;
                    }
                }
                return true;
            }

            // docker image rm <image>
            if subcommand == "image" {
                let sub_subcommand = context.args.get(1).map(|s| s.as_str()).unwrap_or("");
                if ["rm", "inspect", "history", "tag", "push"].contains(&sub_subcommand) {
                    return true;
                }
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let images = self.get_images();
        let partial_lower = partial.to_lowercase();

        images
            .into_iter()
            .filter(|(name, _, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, id, size)| {
                ProviderSuggestion::new(&name)
                    .with_description(format!("{} ({})", id, size))
                    .with_category("image")
                    .with_score(100)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(10))
    }

    fn priority(&self) -> i32 {
        100
    }
}

// ============================================================================
// Docker Container Provider
// ============================================================================

/// Provides container name/id completions for docker exec/stop/rm/logs
pub struct DockerContainerProvider;

impl DockerContainerProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_containers(&self, all: bool) -> Vec<(String, String, String, bool)> {
        // Returns (name, id, image, is_running)
        let args = if all {
            vec![
                "ps",
                "-a",
                "--format",
                "{{.Names}}\t{{.ID}}\t{{.Image}}\t{{.Status}}",
            ]
        } else {
            vec![
                "ps",
                "--format",
                "{{.Names}}\t{{.ID}}\t{{.Image}}\t{{.Status}}",
            ]
        };

        docker_command(&args)
            .map(|output| {
                output
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() >= 4 {
                            let is_running = parts[3].starts_with("Up");
                            Some((
                                parts[0].to_string(),
                                parts[1].to_string(),
                                parts[2].to_string(),
                                is_running,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for DockerContainerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for DockerContainerProvider {
    fn name(&self) -> &str {
        "docker_container"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !docker_available() {
            return false;
        }

        if cmd == "docker" && arg_position >= 1 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

            // Commands that work with running containers
            let running_commands = ["exec", "attach", "top", "stats", "kill", "pause", "unpause"];

            // Commands that work with any container
            let any_commands = [
                "stop", "start", "restart", "rm", "logs", "inspect", "port", "rename", "cp",
                "commit", "diff", "export",
            ];

            if running_commands.contains(&subcommand) || any_commands.contains(&subcommand) {
                return true;
            }

            // docker container <subcommand> <container>
            if subcommand == "container" {
                let sub_subcommand = context.args.get(1).map(|s| s.as_str()).unwrap_or("");
                let container_commands = [
                    "attach", "commit", "cp", "diff", "exec", "export", "inspect", "kill", "logs",
                    "pause", "port", "rename", "restart", "rm", "start", "stats", "stop", "top",
                    "unpause", "wait",
                ];
                if container_commands.contains(&sub_subcommand) {
                    return true;
                }
            }
        }

        false
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");

        // For exec/attach/etc, only show running containers
        let running_only = ["exec", "attach", "top", "stats", "kill", "pause"].contains(&subcommand);
        let containers = self.get_containers(!running_only);

        let partial_lower = partial.to_lowercase();

        containers
            .into_iter()
            .filter(|(name, id, _, is_running)| {
                if running_only && !is_running {
                    return false;
                }
                name.to_lowercase().starts_with(&partial_lower)
                    || id.to_lowercase().starts_with(&partial_lower)
            })
            .flat_map(|(name, id, image, is_running)| {
                let status = if is_running { "running" } else { "stopped" };
                let score = if is_running { 100 } else { 50 };

                vec![
                    // Suggest by name
                    ProviderSuggestion::new(&name)
                        .with_description(format!("{} - {} ({})", image, id, status))
                        .with_category("container")
                        .with_score(score),
                    // Also suggest by ID
                    ProviderSuggestion::new(&id)
                        .with_description(format!("{} - {} ({})", name, image, status))
                        .with_category("container-id")
                        .with_score(score - 10),
                ]
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    fn priority(&self) -> i32 {
        90
    }
}

// ============================================================================
// Docker Volume Provider
// ============================================================================

/// Provides volume name completions for docker volume commands
pub struct DockerVolumeProvider;

impl DockerVolumeProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_volumes(&self) -> Vec<(String, String)> {
        // Returns (name, driver)
        docker_command(&["volume", "ls", "--format", "{{.Name}}\t{{.Driver}}"])
            .map(|output| {
                output
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() >= 2 {
                            Some((parts[0].to_string(), parts[1].to_string()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for DockerVolumeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for DockerVolumeProvider {
    fn name(&self) -> &str {
        "docker_volume"
    }

    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool {
        if !docker_available() {
            return false;
        }

        if cmd == "docker" && arg_position >= 2 {
            let subcommand = context.args.first().map(|s| s.as_str()).unwrap_or("");
            let sub_subcommand = context.args.get(1).map(|s| s.as_str()).unwrap_or("");

            // docker volume rm|inspect <volume>
            if subcommand == "volume" && ["rm", "inspect"].contains(&sub_subcommand) {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let volumes = self.get_volumes();
        let partial_lower = partial.to_lowercase();

        volumes
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&partial_lower))
            .map(|(name, driver)| {
                ProviderSuggestion::new(&name)
                    .with_description(format!("driver: {}", driver))
                    .with_category("volume")
                    .with_score(100)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(10))
    }

    fn priority(&self) -> i32 {
        80
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_image_provider() {
        let provider = DockerImageProvider::new();
        assert_eq!(provider.name(), "docker_image");
    }

    #[test]
    fn test_docker_container_provider() {
        let provider = DockerContainerProvider::new();
        assert_eq!(provider.name(), "docker_container");
    }

    #[test]
    fn test_docker_volume_provider() {
        let provider = DockerVolumeProvider::new();
        assert_eq!(provider.name(), "docker_volume");
    }
}
