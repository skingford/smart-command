//! Smart Validator for Real-time Syntax Checking
//!
//! Validates command input before execution, checking for:
//! - Unclosed quotes (single and double)
//! - Unclosed brackets/braces/parentheses
//! - Incomplete pipes and redirects
//! - Backslash line continuations

use reedline::{ValidationResult, Validator};

/// Smart validator that checks for common syntax errors
pub struct SmartValidator;

impl SmartValidator {
    pub fn new() -> Self {
        Self
    }

    /// Check if quotes are balanced
    fn check_quotes(&self, line: &str) -> bool {
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut prev_char = ' ';

        for c in line.chars() {
            // Skip escaped characters
            if prev_char == '\\' {
                prev_char = c;
                continue;
            }

            match c {
                '\'' if !in_double_quote => in_single_quote = !in_single_quote,
                '"' if !in_single_quote => in_double_quote = !in_double_quote,
                _ => {}
            }
            prev_char = c;
        }

        !in_single_quote && !in_double_quote
    }

    /// Check if brackets are balanced
    fn check_brackets(&self, line: &str) -> bool {
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut prev_char = ' ';

        let mut parens = 0i32; // ()
        let mut braces = 0i32; // {}
        let mut brackets = 0i32; // []

        for c in line.chars() {
            // Skip escaped characters
            if prev_char == '\\' {
                prev_char = c;
                continue;
            }

            // Track quotes
            match c {
                '\'' if !in_double_quote => in_single_quote = !in_single_quote,
                '"' if !in_single_quote => in_double_quote = !in_double_quote,
                _ => {}
            }

            // Only count brackets outside quotes
            if !in_single_quote && !in_double_quote {
                match c {
                    '(' => parens += 1,
                    ')' => parens -= 1,
                    '{' => braces += 1,
                    '}' => braces -= 1,
                    '[' => brackets += 1,
                    ']' => brackets -= 1,
                    _ => {}
                }

                // Negative count means closing without opening
                if parens < 0 || braces < 0 || brackets < 0 {
                    return false;
                }
            }

            prev_char = c;
        }

        parens == 0 && braces == 0 && brackets == 0
    }

    /// Check if line ends with an incomplete operator
    fn check_incomplete_operator(&self, line: &str) -> bool {
        let trimmed = line.trim_end();

        if trimmed.is_empty() {
            return true;
        }

        // Check for line continuation
        if trimmed.ends_with('\\') {
            return false;
        }

        // Check for incomplete pipe or logical operators
        if trimmed.ends_with('|') || trimmed.ends_with("&&") || trimmed.ends_with("||") {
            return false;
        }

        // Check for incomplete redirect (but allow >> as a valid ending for heredoc start)
        // Single > or < at end is incomplete
        let last_char = trimmed.chars().last().unwrap_or(' ');
        if last_char == '>' || last_char == '<' {
            // But >> or << might be start of heredoc, allow for now
            let chars: Vec<char> = trimmed.chars().collect();
            if chars.len() >= 2 {
                let second_last = chars[chars.len() - 2];
                if second_last != '>' && second_last != '<' {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl Default for SmartValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator for SmartValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        // Check for unclosed quotes
        if !self.check_quotes(line) {
            return ValidationResult::Incomplete;
        }

        // Check for unclosed brackets
        if !self.check_brackets(line) {
            return ValidationResult::Incomplete;
        }

        // Check for incomplete operators
        if !self.check_incomplete_operator(line) {
            return ValidationResult::Incomplete;
        }

        ValidationResult::Complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balanced_quotes() {
        let validator = SmartValidator::new();

        assert!(validator.check_quotes("echo 'hello'"));
        assert!(validator.check_quotes("echo \"hello\""));
        assert!(validator.check_quotes("echo 'hello \"world\"'"));
        assert!(validator.check_quotes("echo \"hello 'world'\""));

        assert!(!validator.check_quotes("echo 'hello"));
        assert!(!validator.check_quotes("echo \"hello"));
        assert!(!validator.check_quotes("echo 'hello\""));
    }

    #[test]
    fn test_escaped_quotes() {
        let validator = SmartValidator::new();

        assert!(validator.check_quotes("echo \"hello \\\"world\\\"\""));
        assert!(validator.check_quotes("echo 'hello \\'world\\''"));
    }

    #[test]
    fn test_balanced_brackets() {
        let validator = SmartValidator::new();

        assert!(validator.check_brackets("echo $(pwd)"));
        assert!(validator.check_brackets("arr=(1 2 3)"));
        assert!(validator.check_brackets("echo ${VAR}"));
        assert!(validator.check_brackets("echo [test]"));

        assert!(!validator.check_brackets("echo $(pwd"));
        assert!(!validator.check_brackets("arr=(1 2 3"));
        assert!(!validator.check_brackets("echo ${VAR"));
    }

    #[test]
    fn test_incomplete_operators() {
        let validator = SmartValidator::new();

        assert!(validator.check_incomplete_operator("echo hello"));
        assert!(validator.check_incomplete_operator("ls -la"));

        assert!(!validator.check_incomplete_operator("echo hello |"));
        assert!(!validator.check_incomplete_operator("echo hello &&"));
        assert!(!validator.check_incomplete_operator("echo hello ||"));
        assert!(!validator.check_incomplete_operator("echo hello \\"));
    }

    #[test]
    fn test_full_validation() {
        let validator = SmartValidator::new();

        // Complete commands
        assert!(matches!(
            validator.validate("echo hello"),
            ValidationResult::Complete
        ));
        assert!(matches!(
            validator.validate("ls -la | grep foo"),
            ValidationResult::Complete
        ));

        // Incomplete commands
        assert!(matches!(
            validator.validate("echo 'hello"),
            ValidationResult::Incomplete
        ));
        assert!(matches!(
            validator.validate("echo hello |"),
            ValidationResult::Incomplete
        ));
        assert!(matches!(
            validator.validate("echo $(pwd"),
            ValidationResult::Incomplete
        ));
    }
}
