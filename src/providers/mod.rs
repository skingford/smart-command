//! Dynamic completion providers for real-time system state
//!
//! This module provides a plugin system for dynamic completions
//! such as git branches, docker containers, environment variables, etc.

#![allow(dead_code)]

pub mod docker;
pub mod env;
pub mod export;
pub mod git;
pub mod kubernetes;
pub mod make;
pub mod npm;
pub mod path;
pub mod process;
pub mod ssh;

use lru::LruCache;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Context passed to providers for completion
#[derive(Debug, Clone)]
pub struct ProviderContext {
    /// Current working directory
    pub cwd: PathBuf,
    /// Full command being typed
    pub command: String,
    /// Parsed arguments (excluding command name)
    pub args: Vec<String>,
    /// Current argument position (0-indexed)
    pub arg_position: usize,
    /// Partial input being completed
    pub partial_input: String,
    /// Previous arguments that might influence completion
    pub previous_args: Vec<String>,
}

impl ProviderContext {
    pub fn new(cwd: PathBuf, command: &str, args: Vec<String>, partial: &str) -> Self {
        let arg_position = args.len();
        Self {
            cwd,
            command: command.to_string(),
            args: args.clone(),
            arg_position,
            partial_input: partial.to_string(),
            previous_args: args,
        }
    }
}

/// A suggestion from a provider
#[derive(Debug, Clone)]
pub struct ProviderSuggestion {
    /// The completion value
    pub value: String,
    /// Description of this suggestion
    pub description: Option<String>,
    /// Category for grouping (e.g., "branch", "remote", "tag")
    pub category: Option<String>,
    /// Score for ranking (higher is better)
    pub score: i64,
    /// Whether to append whitespace after completion
    pub append_whitespace: bool,
}

impl ProviderSuggestion {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: None,
            category: None,
            score: 0,
            append_whitespace: true,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_category(mut self, cat: impl Into<String>) -> Self {
        self.category = Some(cat.into());
        self
    }

    pub fn with_score(mut self, score: i64) -> Self {
        self.score = score;
        self
    }

    pub fn no_whitespace(mut self) -> Self {
        self.append_whitespace = false;
        self
    }
}

/// Trait for completion providers
pub trait CompletionProvider: Send + Sync {
    /// Unique name of this provider
    fn name(&self) -> &str;

    /// Check if this provider can handle the given command context
    fn matches(&self, cmd: &str, arg_position: usize, context: &ProviderContext) -> bool;

    /// Generate completions for the given context
    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion>;

    /// Cache TTL for this provider's results (None = no caching)
    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    /// Priority when multiple providers match (higher = checked first)
    fn priority(&self) -> i32 {
        0
    }
}

/// Cached provider result
struct CachedResult {
    suggestions: Vec<ProviderSuggestion>,
    expires_at: Instant,
}

/// Provider registry that manages all completion providers
pub struct ProviderRegistry {
    providers: Vec<Box<dyn CompletionProvider>>,
    cache: Mutex<LruCache<String, CachedResult>>,
    enabled_providers: RwLock<Vec<String>>,
}

impl ProviderRegistry {
    /// Create a new registry with default providers
    pub fn new() -> Self {
        let mut registry = Self {
            providers: Vec::new(),
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
            enabled_providers: RwLock::new(vec![
                "git".to_string(),
                "docker".to_string(),
                "env".to_string(),
                "export".to_string(),
                "ssh".to_string(),
                "process".to_string(),
                "npm".to_string(),
                "path".to_string(),
                "make".to_string(),
                "k8s".to_string(),
            ]),
        };
        registry.register_default_providers();
        registry
    }

    /// Register all default providers
    fn register_default_providers(&mut self) {
        // Git providers
        self.register(Box::new(git::GitBranchProvider::new()));
        self.register(Box::new(git::GitRemoteProvider::new()));
        self.register(Box::new(git::GitTagProvider::new()));
        self.register(Box::new(git::GitStashProvider::new()));
        self.register(Box::new(git::GitFileProvider::new()));

        // Docker providers
        self.register(Box::new(docker::DockerImageProvider::new()));
        self.register(Box::new(docker::DockerContainerProvider::new()));
        self.register(Box::new(docker::DockerVolumeProvider::new()));

        // System providers
        self.register(Box::new(env::EnvVarProvider::new()));
        self.register(Box::new(export::ExportProvider::new()));
        self.register(Box::new(process::ProcessProvider::new()));
        self.register(Box::new(ssh::SshHostProvider::new()));

        // Package providers
        self.register(Box::new(npm::NpmPackageProvider::new()));

        // Build tool providers
        self.register(Box::new(make::MakeTargetProvider::new()));

        // Kubernetes providers
        self.register(Box::new(kubernetes::KubernetesResourceProvider::pods()));
        self.register(Box::new(kubernetes::KubernetesResourceProvider::services()));
        self.register(Box::new(kubernetes::KubernetesResourceProvider::deployments()));
        self.register(Box::new(kubernetes::KubernetesResourceProvider::namespaces()));
        self.register(Box::new(kubernetes::KubernetesResourceProvider::configmaps()));
        self.register(Box::new(kubernetes::KubernetesResourceProvider::secrets()));
        self.register(Box::new(kubernetes::KubernetesContextProvider::new()));
        self.register(Box::new(kubernetes::KubernetesNamespaceProvider::new()));

        // Path provider (enhanced)
        self.register(Box::new(path::PathProvider::new()));
    }

    /// Register a custom provider
    pub fn register(&mut self, provider: Box<dyn CompletionProvider>) {
        self.providers.push(provider);
        // Sort by priority (higher first)
        self.providers.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// Enable/disable specific providers
    pub fn set_enabled(&self, providers: Vec<String>) {
        *self.enabled_providers.write().unwrap() = providers;
    }

    /// Get completions from all matching providers
    pub fn complete(&self, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let enabled = self.enabled_providers.read().unwrap();
        let mut all_suggestions = Vec::new();

        for provider in &self.providers {
            // Check if provider is enabled
            let provider_name = provider.name();
            let provider_category = provider_name.split('_').next().unwrap_or(provider_name);
            if !enabled.iter().any(|e| provider_category.starts_with(e)) {
                continue;
            }

            // Check if provider matches this context
            if !provider.matches(&context.command, context.arg_position, context) {
                continue;
            }

            // Check cache first
            let cache_key = format!(
                "{}:{}:{}:{}",
                provider.name(),
                context.command,
                context.arg_position,
                context.partial_input
            );

            let mut cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                if cached.expires_at > Instant::now() {
                    all_suggestions.extend(cached.suggestions.clone());
                    continue;
                }
            }
            drop(cache);

            // Get fresh completions
            let suggestions = provider.complete(&context.partial_input, context);

            // Cache if provider supports it
            if let Some(ttl) = provider.cache_ttl() {
                let mut cache = self.cache.lock().unwrap();
                cache.put(
                    cache_key,
                    CachedResult {
                        suggestions: suggestions.clone(),
                        expires_at: Instant::now() + ttl,
                    },
                );
            }

            all_suggestions.extend(suggestions);
        }

        // Sort by score
        all_suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        all_suggestions
    }

    /// Clear all cached results
    pub fn clear_cache(&self) {
        self.cache.lock().unwrap().clear();
    }

    /// Invalidate cache for a specific provider
    pub fn invalidate(&self, provider_name: &str) {
        let mut cache = self.cache.lock().unwrap();
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter(|(k, _)| k.starts_with(provider_name))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys_to_remove {
            cache.pop(&key);
        }
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global provider registry instance
pub static PROVIDER_REGISTRY: Lazy<Arc<ProviderRegistry>> =
    Lazy::new(|| Arc::new(ProviderRegistry::new()));

/// Get the global provider registry
pub fn registry() -> Arc<ProviderRegistry> {
    PROVIDER_REGISTRY.clone()
}

/// Provider configuration for YAML definitions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderConfig {
    /// Provider name (e.g., "git_branch", "docker_container")
    pub name: String,
    /// Additional configuration for the provider
    #[serde(default)]
    pub config: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_context() {
        let ctx = ProviderContext::new(
            PathBuf::from("/home/user"),
            "git",
            vec!["checkout".to_string()],
            "feat",
        );
        assert_eq!(ctx.command, "git");
        assert_eq!(ctx.arg_position, 1);
        assert_eq!(ctx.partial_input, "feat");
    }

    #[test]
    fn test_provider_suggestion() {
        let suggestion = ProviderSuggestion::new("main")
            .with_description("Main branch")
            .with_category("branch")
            .with_score(100);

        assert_eq!(suggestion.value, "main");
        assert_eq!(suggestion.description, Some("Main branch".to_string()));
        assert_eq!(suggestion.category, Some("branch".to_string()));
        assert_eq!(suggestion.score, 100);
    }

    #[test]
    fn test_registry_creation() {
        let registry = ProviderRegistry::new();
        assert!(!registry.providers.is_empty());
    }
}
