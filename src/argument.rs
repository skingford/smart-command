//! Argument validation and type checking system
//!
//! This module provides validation for command arguments based on their types
//! defined in CommandSpec.

#![allow(dead_code)]

use crate::command_def::{ArgumentType, PathFilterConfig};
use regex::Regex;
use std::path::Path;

/// Result of validating an argument
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    /// Argument is valid
    Valid,
    /// Argument is invalid with reason
    Invalid(String),
    /// Argument is partially valid (user still typing)
    Incomplete(String),
}

/// Validates an argument value against its expected type
pub struct ArgumentValidator;

impl ArgumentValidator {
    /// Validate a value against an argument type
    pub fn validate(value: &str, arg_type: &ArgumentType) -> ValidationResult {
        if value.is_empty() {
            return ValidationResult::Incomplete("Value required".to_string());
        }

        match arg_type {
            ArgumentType::String => ValidationResult::Valid,

            ArgumentType::Number { min, max } => Self::validate_number(value, *min, *max),

            ArgumentType::Boolean => Self::validate_boolean(value),

            ArgumentType::Choice { values } => Self::validate_choice(value, values),

            ArgumentType::Pattern { regex } => Self::validate_pattern(value, regex),

            ArgumentType::Path { filter } => Self::validate_path(value, filter),

            ArgumentType::Provider { .. } => {
                // Provider-based types are validated by the provider itself
                ValidationResult::Valid
            }

            ArgumentType::Url => Self::validate_url(value),

            ArgumentType::Email => Self::validate_email(value),

            ArgumentType::Json => Self::validate_json(value),

            ArgumentType::Any => ValidationResult::Valid,
        }
    }

    fn validate_number(value: &str, min: Option<i64>, max: Option<i64>) -> ValidationResult {
        match value.parse::<i64>() {
            Ok(num) => {
                if let Some(min_val) = min {
                    if num < min_val {
                        return ValidationResult::Invalid(format!(
                            "Value {} is less than minimum {}",
                            num, min_val
                        ));
                    }
                }
                if let Some(max_val) = max {
                    if num > max_val {
                        return ValidationResult::Invalid(format!(
                            "Value {} is greater than maximum {}",
                            num, max_val
                        ));
                    }
                }
                ValidationResult::Valid
            }
            Err(_) => {
                // Check if it might be a partial number
                if value.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    ValidationResult::Incomplete("Enter a number".to_string())
                } else {
                    ValidationResult::Invalid("Not a valid number".to_string())
                }
            }
        }
    }

    fn validate_boolean(value: &str) -> ValidationResult {
        let lower = value.to_lowercase();
        match lower.as_str() {
            "true" | "false" | "yes" | "no" | "1" | "0" | "on" | "off" => ValidationResult::Valid,
            "t" | "f" | "y" | "n" | "tr" | "tru" | "fa" | "fal" | "fals" | "ye" | "o" => {
                ValidationResult::Incomplete("Complete the boolean value".to_string())
            }
            _ => ValidationResult::Invalid(
                "Expected: true, false, yes, no, 1, 0, on, off".to_string(),
            ),
        }
    }

    fn validate_choice(value: &str, choices: &[String]) -> ValidationResult {
        let lower = value.to_lowercase();

        // Exact match
        if choices.iter().any(|c| c.to_lowercase() == lower) {
            return ValidationResult::Valid;
        }

        // Partial match (user still typing)
        let partial_matches: Vec<_> = choices
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&lower))
            .collect();

        if !partial_matches.is_empty() {
            return ValidationResult::Incomplete(format!(
                "Did you mean: {}?",
                partial_matches
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        ValidationResult::Invalid(format!(
            "Expected one of: {}",
            choices.join(", ")
        ))
    }

    fn validate_pattern(value: &str, pattern: &str) -> ValidationResult {
        match Regex::new(pattern) {
            Ok(re) => {
                if re.is_match(value) {
                    ValidationResult::Valid
                } else {
                    ValidationResult::Invalid(format!("Does not match pattern: {}", pattern))
                }
            }
            Err(_) => {
                // Invalid regex pattern in definition
                ValidationResult::Valid
            }
        }
    }

    fn validate_path(value: &str, filter: &PathFilterConfig) -> ValidationResult {
        let path = Path::new(value);

        // Check if path exists
        if !path.exists() {
            // Path doesn't exist yet, but might be a partial path
            // Check if parent exists
            if let Some(parent) = path.parent() {
                if parent.exists() || parent.to_string_lossy().is_empty() {
                    return ValidationResult::Incomplete("Path does not exist yet".to_string());
                }
            }
            return ValidationResult::Invalid("Path does not exist".to_string());
        }

        // Check files_only / dirs_only
        if filter.files_only && !path.is_file() {
            return ValidationResult::Invalid("Expected a file, not a directory".to_string());
        }
        if filter.dirs_only && !path.is_dir() {
            return ValidationResult::Invalid("Expected a directory, not a file".to_string());
        }

        // Check extension filter
        if path.is_file() {
            if let Some(ref exts) = filter.extensions {
                let path_ext = path
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                if !exts.iter().any(|e| e == &path_ext) {
                    return ValidationResult::Invalid(format!(
                        "Expected file with extension: {}",
                        exts.join(", ")
                    ));
                }
            }
        }

        // Check hidden file filter
        if !filter.include_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    return ValidationResult::Invalid("Hidden files not allowed".to_string());
                }
            }
        }

        ValidationResult::Valid
    }

    fn validate_url(value: &str) -> ValidationResult {
        // Simple URL validation
        if value.starts_with("http://")
            || value.starts_with("https://")
            || value.starts_with("ftp://")
        {
            // Check for basic URL structure
            let without_scheme = value
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .trim_start_matches("ftp://");

            if without_scheme.is_empty() {
                return ValidationResult::Incomplete("Enter the hostname".to_string());
            }

            if without_scheme.contains('.') || without_scheme.contains(':') {
                return ValidationResult::Valid;
            }

            return ValidationResult::Incomplete("Enter complete URL".to_string());
        }

        // Might be typing the scheme
        if "http://".starts_with(value)
            || "https://".starts_with(value)
            || "ftp://".starts_with(value)
        {
            return ValidationResult::Incomplete("Complete the URL scheme".to_string());
        }

        ValidationResult::Invalid("Expected URL (http://, https://, ftp://)".to_string())
    }

    fn validate_email(value: &str) -> ValidationResult {
        if value.contains('@') {
            let parts: Vec<&str> = value.split('@').collect();
            if parts.len() == 2 {
                let local = parts[0];
                let domain = parts[1];

                if local.is_empty() {
                    return ValidationResult::Invalid("Missing local part before @".to_string());
                }

                if domain.is_empty() {
                    return ValidationResult::Incomplete("Enter domain after @".to_string());
                }

                if domain.contains('.') && domain.len() > 3 {
                    return ValidationResult::Valid;
                }

                return ValidationResult::Incomplete("Complete the domain".to_string());
            }
            return ValidationResult::Invalid("Invalid email format".to_string());
        }

        // No @ yet, might still be typing
        if !value.is_empty() {
            return ValidationResult::Incomplete("Enter @ and domain".to_string());
        }

        ValidationResult::Invalid("Expected email address".to_string())
    }

    fn validate_json(value: &str) -> ValidationResult {
        match serde_json::from_str::<serde_json::Value>(value) {
            Ok(_) => ValidationResult::Valid,
            Err(e) => {
                // Check if it looks like incomplete JSON
                let trimmed = value.trim();
                if trimmed.starts_with('{') && !trimmed.ends_with('}') {
                    return ValidationResult::Incomplete("Close the JSON object with }".to_string());
                }
                if trimmed.starts_with('[') && !trimmed.ends_with(']') {
                    return ValidationResult::Incomplete("Close the JSON array with ]".to_string());
                }
                if trimmed.starts_with('"') && !trimmed.ends_with('"') {
                    return ValidationResult::Incomplete("Close the string with \"".to_string());
                }

                ValidationResult::Invalid(format!("Invalid JSON: {}", e))
            }
        }
    }

    /// Get a hint for the expected format of an argument type
    pub fn get_hint(arg_type: &ArgumentType) -> String {
        match arg_type {
            ArgumentType::String => "<string>".to_string(),
            ArgumentType::Number { min, max } => match (min, max) {
                (Some(min), Some(max)) => format!("<number {}-{}>", min, max),
                (Some(min), None) => format!("<number >= {}>", min),
                (None, Some(max)) => format!("<number <= {}>", max),
                (None, None) => "<number>".to_string(),
            },
            ArgumentType::Boolean => "<true|false>".to_string(),
            ArgumentType::Choice { values } => {
                if values.len() <= 4 {
                    format!("<{}>", values.join("|"))
                } else {
                    format!("<{}|...>", values[..3].join("|"))
                }
            }
            ArgumentType::Pattern { regex } => format!("<pattern: {}>", regex),
            ArgumentType::Path { filter } => {
                if filter.dirs_only {
                    "<directory>".to_string()
                } else if filter.files_only {
                    if let Some(ref exts) = filter.extensions {
                        format!("<file {}>", exts.join("|"))
                    } else {
                        "<file>".to_string()
                    }
                } else {
                    "<path>".to_string()
                }
            }
            ArgumentType::Provider { name } => format!("<{}>", name),
            ArgumentType::Url => "<url>".to_string(),
            ArgumentType::Email => "<email>".to_string(),
            ArgumentType::Json => "<json>".to_string(),
            ArgumentType::Any => "<value>".to_string(),
        }
    }
}

/// Validates a complete command line against its spec
pub struct CommandValidator;

impl CommandValidator {
    /// Validate all arguments in a command
    pub fn validate_command(
        args: &[String],
        arg_specs: &[crate::command_def::ArgumentSpec],
    ) -> Vec<(usize, ValidationResult)> {
        let mut results = Vec::new();

        for (idx, value) in args.iter().enumerate() {
            // Find the argument spec for this position
            let spec = arg_specs.iter().find(|s| {
                s.position == Some(idx) || (s.variadic && s.position.map(|p| idx >= p).unwrap_or(false))
            });

            if let Some(spec) = spec {
                // Get the type from either provider shorthand or arg_type
                let arg_type = if spec.provider.is_some() {
                    &ArgumentType::Any // Provider-validated
                } else {
                    &spec.arg_type
                };

                let result = ArgumentValidator::validate(value, arg_type);
                if result != ValidationResult::Valid {
                    results.push((idx, result));
                }
            }
        }

        // Check for required arguments
        for spec in arg_specs {
            if spec.required {
                if let Some(pos) = spec.position {
                    if args.get(pos).map(|s| s.is_empty()).unwrap_or(true) {
                        let name = spec.name.as_deref().unwrap_or("argument");
                        results.push((
                            pos,
                            ValidationResult::Invalid(format!("Required: {}", name)),
                        ));
                    }
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_number() {
        assert_eq!(
            ArgumentValidator::validate("42", &ArgumentType::Number { min: None, max: None }),
            ValidationResult::Valid
        );

        assert_eq!(
            ArgumentValidator::validate("5", &ArgumentType::Number { min: Some(1), max: Some(10) }),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate("100", &ArgumentType::Number { min: None, max: Some(50) }),
            ValidationResult::Invalid(_)
        ));

        assert!(matches!(
            ArgumentValidator::validate("abc", &ArgumentType::Number { min: None, max: None }),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_boolean() {
        assert_eq!(
            ArgumentValidator::validate("true", &ArgumentType::Boolean),
            ValidationResult::Valid
        );
        assert_eq!(
            ArgumentValidator::validate("yes", &ArgumentType::Boolean),
            ValidationResult::Valid
        );
        assert_eq!(
            ArgumentValidator::validate("1", &ArgumentType::Boolean),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate("maybe", &ArgumentType::Boolean),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_choice() {
        let choices = vec!["red".to_string(), "green".to_string(), "blue".to_string()];

        assert_eq!(
            ArgumentValidator::validate("red", &ArgumentType::Choice { values: choices.clone() }),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate("re", &ArgumentType::Choice { values: choices.clone() }),
            ValidationResult::Incomplete(_)
        ));

        assert!(matches!(
            ArgumentValidator::validate("yellow", &ArgumentType::Choice { values: choices }),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_email() {
        assert_eq!(
            ArgumentValidator::validate("user@example.com", &ArgumentType::Email),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate("user@", &ArgumentType::Email),
            ValidationResult::Incomplete(_)
        ));

        assert!(matches!(
            ArgumentValidator::validate("user", &ArgumentType::Email),
            ValidationResult::Incomplete(_)
        ));
    }

    #[test]
    fn test_validate_url() {
        assert_eq!(
            ArgumentValidator::validate("https://example.com", &ArgumentType::Url),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate("https://", &ArgumentType::Url),
            ValidationResult::Incomplete(_)
        ));

        assert!(matches!(
            ArgumentValidator::validate("example.com", &ArgumentType::Url),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_validate_json() {
        assert_eq!(
            ArgumentValidator::validate(r#"{"key": "value"}"#, &ArgumentType::Json),
            ValidationResult::Valid
        );

        assert_eq!(
            ArgumentValidator::validate("[1, 2, 3]", &ArgumentType::Json),
            ValidationResult::Valid
        );

        assert!(matches!(
            ArgumentValidator::validate(r#"{"key":"#, &ArgumentType::Json),
            ValidationResult::Incomplete(_)
        ));
    }

    #[test]
    fn test_get_hint() {
        assert_eq!(
            ArgumentValidator::get_hint(&ArgumentType::Boolean),
            "<true|false>"
        );

        assert_eq!(
            ArgumentValidator::get_hint(&ArgumentType::Number { min: Some(0), max: Some(100) }),
            "<number 0-100>"
        );

        let choices = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            ArgumentValidator::get_hint(&ArgumentType::Choice { values: choices }),
            "<a|b>"
        );
    }
}
