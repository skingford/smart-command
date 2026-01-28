//! Kubernetes completion provider
//!
//! Provides completion for kubectl commands including:
//! - Pod names
//! - Service names
//! - Deployment names
//! - Namespace names
//! - Context names

use super::{CompletionProvider, ProviderContext, ProviderSuggestion};
use std::process::Command;
use std::time::Duration;

/// Provider for Kubernetes resource names
pub struct KubernetesResourceProvider {
    resource_type: &'static str,
}

impl KubernetesResourceProvider {
    pub fn pods() -> Self {
        Self {
            resource_type: "pods",
        }
    }

    pub fn services() -> Self {
        Self {
            resource_type: "services",
        }
    }

    pub fn deployments() -> Self {
        Self {
            resource_type: "deployments",
        }
    }

    pub fn namespaces() -> Self {
        Self {
            resource_type: "namespaces",
        }
    }

    pub fn configmaps() -> Self {
        Self {
            resource_type: "configmaps",
        }
    }

    pub fn secrets() -> Self {
        Self {
            resource_type: "secrets",
        }
    }

    /// Get resources of the specified type
    fn get_resources(&self, namespace: Option<&str>) -> Vec<(String, Option<String>)> {
        let mut cmd = Command::new("kubectl");
        cmd.args(["get", self.resource_type, "-o", "name", "--no-headers"]);

        if let Some(ns) = namespace {
            cmd.args(["-n", ns]);
        }

        match cmd.output() {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| {
                        // Output format: type/name
                        line.split('/').last().map(|name| (name.to_string(), None))
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Check if kubectl is available
    fn kubectl_available() -> bool {
        Command::new("kubectl")
            .arg("version")
            .arg("--client")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Extract namespace from args if -n or --namespace is specified
    fn extract_namespace(args: &[String]) -> Option<String> {
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if arg == "-n" || arg == "--namespace" {
                return iter.next().cloned();
            }
            if arg.starts_with("-n=") {
                return Some(arg.trim_start_matches("-n=").to_string());
            }
            if arg.starts_with("--namespace=") {
                return Some(arg.trim_start_matches("--namespace=").to_string());
            }
        }
        None
    }
}

impl CompletionProvider for KubernetesResourceProvider {
    fn name(&self) -> &str {
        match self.resource_type {
            "pods" => "k8s_pods",
            "services" => "k8s_services",
            "deployments" => "k8s_deployments",
            "namespaces" => "k8s_namespaces",
            "configmaps" => "k8s_configmaps",
            "secrets" => "k8s_secrets",
            _ => "k8s_resource",
        }
    }

    fn matches(&self, cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        if cmd != "kubectl" && cmd != "k" {
            return false;
        }

        // Check if kubectl is available
        if !Self::kubectl_available() {
            return false;
        }

        // Match based on subcommand and resource type
        let args = &context.args;
        if args.is_empty() {
            return false;
        }

        let subcommand = &args[0];

        // Commands that work with resources
        let resource_commands = [
            "get", "describe", "delete", "edit", "logs", "exec", "port-forward", "cp", "attach",
            "scale", "rollout", "label", "annotate", "patch",
        ];

        if !resource_commands.contains(&subcommand.as_str()) {
            return false;
        }

        // For 'logs', 'exec', 'port-forward', 'attach' - only pods
        if ["logs", "exec", "port-forward", "attach", "cp"].contains(&subcommand.as_str()) {
            return self.resource_type == "pods";
        }

        // Check if resource type is specified in args
        if args.len() >= 2 {
            let resource_arg = &args[1];
            return self.matches_resource_type(resource_arg);
        }

        false
    }

    fn complete(&self, partial: &str, context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let namespace = Self::extract_namespace(&context.args);
        let resources = self.get_resources(namespace.as_deref());

        resources
            .into_iter()
            .filter(|(name, _)| {
                partial.is_empty() || name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|(name, _)| {
                ProviderSuggestion::new(&name)
                    .with_category(self.resource_type)
                    .with_score(50)
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        // Short cache - k8s resources change frequently
        Some(Duration::from_secs(5))
    }

    fn priority(&self) -> i32 {
        15
    }
}

impl KubernetesResourceProvider {
    fn matches_resource_type(&self, arg: &str) -> bool {
        let arg_lower = arg.to_lowercase();
        match self.resource_type {
            "pods" => {
                ["pod", "pods", "po"].contains(&arg_lower.as_str())
            }
            "services" => {
                ["service", "services", "svc"].contains(&arg_lower.as_str())
            }
            "deployments" => {
                ["deployment", "deployments", "deploy"].contains(&arg_lower.as_str())
            }
            "namespaces" => {
                ["namespace", "namespaces", "ns"].contains(&arg_lower.as_str())
            }
            "configmaps" => {
                ["configmap", "configmaps", "cm"].contains(&arg_lower.as_str())
            }
            "secrets" => {
                ["secret", "secrets"].contains(&arg_lower.as_str())
            }
            _ => false,
        }
    }
}

/// Provider for Kubernetes contexts
pub struct KubernetesContextProvider;

impl KubernetesContextProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_contexts() -> Vec<(String, bool)> {
        let output = Command::new("kubectl")
            .args(["config", "get-contexts", "-o", "name"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                // Get current context
                let current = Command::new("kubectl")
                    .args(["config", "current-context"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                        } else {
                            None
                        }
                    });

                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|line| {
                        let name = line.trim().to_string();
                        let is_current = current.as_ref().map(|c| c == &name).unwrap_or(false);
                        (name, is_current)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

impl Default for KubernetesContextProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for KubernetesContextProvider {
    fn name(&self) -> &str {
        "k8s_context"
    }

    fn matches(&self, cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        if cmd != "kubectl" && cmd != "k" {
            return false;
        }

        // Match for 'config use-context' or '--context' flag
        let args = &context.args;

        // Check for 'config use-context'
        if args.len() >= 2 && args[0] == "config" && args[1] == "use-context" {
            return true;
        }

        // Check for '--context' flag
        if let Some(prev) = args.last() {
            if prev == "--context" {
                return true;
            }
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let contexts = Self::get_contexts();

        contexts
            .into_iter()
            .filter(|(name, _)| {
                partial.is_empty() || name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|(name, is_current)| {
                let mut suggestion = ProviderSuggestion::new(&name).with_category("context");

                if is_current {
                    suggestion = suggestion.with_description("current").with_score(100);
                } else {
                    suggestion = suggestion.with_score(50);
                }

                suggestion
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        15
    }
}

/// Provider for Kubernetes namespaces (for -n flag)
pub struct KubernetesNamespaceProvider;

impl KubernetesNamespaceProvider {
    pub fn new() -> Self {
        Self
    }

    fn get_namespaces() -> Vec<String> {
        let output = Command::new("kubectl")
            .args(["get", "namespaces", "-o", "name", "--no-headers"])
            .output();

        match output {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split('/').last().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl Default for KubernetesNamespaceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider for KubernetesNamespaceProvider {
    fn name(&self) -> &str {
        "k8s_namespace"
    }

    fn matches(&self, cmd: &str, _arg_position: usize, context: &ProviderContext) -> bool {
        if cmd != "kubectl" && cmd != "k" {
            return false;
        }

        // Match when previous arg is -n or --namespace
        if let Some(prev) = context.args.last() {
            return prev == "-n" || prev == "--namespace";
        }

        false
    }

    fn complete(&self, partial: &str, _context: &ProviderContext) -> Vec<ProviderSuggestion> {
        let namespaces = Self::get_namespaces();

        namespaces
            .into_iter()
            .filter(|name| {
                partial.is_empty() || name.to_lowercase().starts_with(&partial.to_lowercase())
            })
            .map(|name| {
                let mut suggestion =
                    ProviderSuggestion::new(&name).with_category("namespace");

                // Boost common namespaces
                if name == "default" || name == "kube-system" {
                    suggestion = suggestion.with_score(100);
                } else {
                    suggestion = suggestion.with_score(50);
                }

                suggestion
            })
            .collect()
    }

    fn cache_ttl(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    fn priority(&self) -> i32 {
        20 // Higher priority for namespace completion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_namespace() {
        let args = vec![
            "get".to_string(),
            "pods".to_string(),
            "-n".to_string(),
            "kube-system".to_string(),
        ];
        let ns = KubernetesResourceProvider::extract_namespace(&args);
        assert_eq!(ns, Some("kube-system".to_string()));

        let args2 = vec![
            "get".to_string(),
            "pods".to_string(),
            "--namespace=default".to_string(),
        ];
        let ns2 = KubernetesResourceProvider::extract_namespace(&args2);
        assert_eq!(ns2, Some("default".to_string()));
    }

    #[test]
    fn test_matches_resource_type() {
        let pods_provider = KubernetesResourceProvider::pods();
        assert!(pods_provider.matches_resource_type("pod"));
        assert!(pods_provider.matches_resource_type("pods"));
        assert!(pods_provider.matches_resource_type("po"));
        assert!(!pods_provider.matches_resource_type("service"));

        let svc_provider = KubernetesResourceProvider::services();
        assert!(svc_provider.matches_resource_type("service"));
        assert!(svc_provider.matches_resource_type("svc"));
    }
}
