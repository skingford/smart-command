//! Pipeline and Redirect Awareness
//!
//! Parses command pipelines and provides context-aware suggestions
//! based on pipe operators, redirects, and command chaining.

#![allow(dead_code)]

/// Pipeline operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineOperator {
    /// `|` - pipe stdout to next command
    Pipe,
    /// `&&` - run next command if this one succeeds
    And,
    /// `||` - run next command if this one fails
    Or,
    /// `;` - run next command regardless
    Sequence,
    /// `>` - redirect stdout to file (overwrite)
    RedirectOut,
    /// `>>` - redirect stdout to file (append)
    RedirectAppend,
    /// `<` - redirect file to stdin
    RedirectIn,
    /// `2>` - redirect stderr to file
    RedirectErr,
    /// `2>&1` - redirect stderr to stdout
    RedirectErrToOut,
    /// `&` - run in background
    Background,
}

impl PipelineOperator {
    /// Parse an operator from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "|" => Some(Self::Pipe),
            "&&" => Some(Self::And),
            "||" => Some(Self::Or),
            ";" => Some(Self::Sequence),
            ">" => Some(Self::RedirectOut),
            ">>" => Some(Self::RedirectAppend),
            "<" => Some(Self::RedirectIn),
            "2>" => Some(Self::RedirectErr),
            "2>&1" => Some(Self::RedirectErrToOut),
            "&" => Some(Self::Background),
            _ => None,
        }
    }

    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pipe => "|",
            Self::And => "&&",
            Self::Or => "||",
            Self::Sequence => ";",
            Self::RedirectOut => ">",
            Self::RedirectAppend => ">>",
            Self::RedirectIn => "<",
            Self::RedirectErr => "2>",
            Self::RedirectErrToOut => "2>&1",
            Self::Background => "&",
        }
    }
}

/// A segment of a pipeline
#[derive(Debug, Clone)]
pub struct PipelineSegment {
    /// The command name
    pub command: String,
    /// Arguments to the command
    pub args: Vec<String>,
    /// Operator after this segment (if any)
    pub operator_after: Option<PipelineOperator>,
}

/// A parsed pipeline
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// All segments in the pipeline
    pub segments: Vec<PipelineSegment>,
    /// Index of the current segment (being typed)
    pub current_segment: usize,
    /// Whether currently typing an operator
    pub in_operator: bool,
}

impl Pipeline {
    /// Parse a command line into a pipeline
    pub fn parse(line: &str) -> Self {
        let mut segments = Vec::new();
        let mut current_command = String::new();
        let mut current_args = Vec::new();
        let mut chars = line.chars().peekable();
        let mut in_quotes = None;
        let mut current_token = String::new();

        while let Some(c) = chars.next() {
            // Handle quotes
            if c == '"' || c == '\'' {
                if in_quotes == Some(c) {
                    in_quotes = None;
                } else if in_quotes.is_none() {
                    in_quotes = Some(c);
                }
                current_token.push(c);
                continue;
            }

            // Inside quotes, everything is literal
            if in_quotes.is_some() {
                current_token.push(c);
                continue;
            }

            // Check for operators
            let maybe_operator = match c {
                '|' => {
                    if chars.peek() == Some(&'|') {
                        chars.next();
                        Some(PipelineOperator::Or)
                    } else {
                        Some(PipelineOperator::Pipe)
                    }
                }
                '&' => {
                    if chars.peek() == Some(&'&') {
                        chars.next();
                        Some(PipelineOperator::And)
                    } else {
                        Some(PipelineOperator::Background)
                    }
                }
                ';' => Some(PipelineOperator::Sequence),
                '>' => {
                    if chars.peek() == Some(&'>') {
                        chars.next();
                        Some(PipelineOperator::RedirectAppend)
                    } else {
                        Some(PipelineOperator::RedirectOut)
                    }
                }
                '<' => Some(PipelineOperator::RedirectIn),
                '2' if chars.peek() == Some(&'>') => {
                    chars.next();
                    if chars.peek() == Some(&'&') {
                        chars.next();
                        if chars.peek() == Some(&'1') {
                            chars.next();
                            Some(PipelineOperator::RedirectErrToOut)
                        } else {
                            Some(PipelineOperator::RedirectErr)
                        }
                    } else {
                        Some(PipelineOperator::RedirectErr)
                    }
                }
                _ => None,
            };

            if let Some(op) = maybe_operator {
                // Finish current token
                if !current_token.is_empty() {
                    if current_command.is_empty() {
                        current_command = current_token.clone();
                    } else {
                        current_args.push(current_token.clone());
                    }
                    current_token.clear();
                }

                // Save current segment
                if !current_command.is_empty() {
                    segments.push(PipelineSegment {
                        command: current_command.clone(),
                        args: current_args.clone(),
                        operator_after: Some(op),
                    });
                    current_command.clear();
                    current_args.clear();
                }
                continue;
            }

            // Handle whitespace
            if c.is_whitespace() {
                if !current_token.is_empty() {
                    if current_command.is_empty() {
                        current_command = current_token.clone();
                    } else {
                        current_args.push(current_token.clone());
                    }
                    current_token.clear();
                }
                continue;
            }

            current_token.push(c);
        }

        // Handle remaining token
        if !current_token.is_empty() {
            if current_command.is_empty() {
                current_command = current_token;
            } else {
                current_args.push(current_token);
            }
        }

        // Add final segment
        if !current_command.is_empty() || !current_args.is_empty() {
            segments.push(PipelineSegment {
                command: current_command,
                args: current_args,
                operator_after: None,
            });
        }

        let current_segment = if segments.is_empty() {
            0
        } else {
            segments.len() - 1
        };

        // Check if we're in an operator position
        let in_operator = line.trim_end().ends_with('|')
            || line.trim_end().ends_with('&')
            || line.trim_end().ends_with(';')
            || line.trim_end().ends_with('>')
            || line.trim_end().ends_with('<');

        Pipeline {
            segments,
            current_segment,
            in_operator,
        }
    }

    /// Get the current segment being typed
    pub fn current(&self) -> Option<&PipelineSegment> {
        self.segments.get(self.current_segment)
    }

    /// Get the previous segment (for context)
    pub fn previous(&self) -> Option<&PipelineSegment> {
        if self.current_segment > 0 {
            self.segments.get(self.current_segment - 1)
        } else {
            None
        }
    }

    /// Check if we're after a pipe operator
    pub fn is_after_pipe(&self) -> bool {
        self.previous()
            .and_then(|s| s.operator_after.as_ref())
            .map(|o| *o == PipelineOperator::Pipe)
            .unwrap_or(false)
    }

    /// Check if we're at a redirect position
    pub fn is_at_redirect(&self) -> bool {
        self.previous()
            .and_then(|s| s.operator_after.as_ref())
            .map(|o| {
                matches!(
                    o,
                    PipelineOperator::RedirectOut
                        | PipelineOperator::RedirectAppend
                        | PipelineOperator::RedirectIn
                        | PipelineOperator::RedirectErr
                )
            })
            .unwrap_or(false)
    }
}

/// Suggestions for different contexts
pub struct PipelineSuggestions;

impl PipelineSuggestions {
    /// Commands that commonly read from stdin (for after pipe)
    pub fn stdin_commands() -> &'static [&'static str] {
        &[
            "grep", "awk", "sed", "sort", "uniq", "head", "tail", "wc", "cut", "tr", "xargs",
            "less", "more", "cat", "tee", "rev", "nl", "paste", "column", "fold", "expand",
            "unexpand", "od", "xxd", "base64", "jq", "yq",
        ]
    }

    /// Get suggestions based on the previous command in a pipe
    pub fn for_pipe_after(previous_cmd: &str) -> Vec<(&'static str, &'static str)> {
        let mut suggestions = Vec::new();

        // Common pipe chains
        match previous_cmd {
            "ls" => {
                suggestions.push(("grep", "Filter files by pattern"));
                suggestions.push(("wc -l", "Count files"));
                suggestions.push(("head", "Show first files"));
                suggestions.push(("tail", "Show last files"));
                suggestions.push(("sort", "Sort files"));
            }
            "cat" | "less" | "more" => {
                suggestions.push(("grep", "Search for pattern"));
                suggestions.push(("wc -l", "Count lines"));
                suggestions.push(("head", "Show first lines"));
                suggestions.push(("tail", "Show last lines"));
                suggestions.push(("sort", "Sort lines"));
                suggestions.push(("uniq", "Remove duplicates"));
            }
            "grep" => {
                suggestions.push(("wc -l", "Count matches"));
                suggestions.push(("head", "Show first matches"));
                suggestions.push(("sort", "Sort matches"));
                suggestions.push(("cut", "Extract fields"));
                suggestions.push(("awk", "Process matches"));
            }
            "ps" => {
                suggestions.push(("grep", "Filter processes"));
                suggestions.push(("awk", "Extract columns"));
                suggestions.push(("head", "Show top processes"));
                suggestions.push(("sort", "Sort processes"));
            }
            "find" => {
                suggestions.push(("xargs", "Execute on results"));
                suggestions.push(("wc -l", "Count results"));
                suggestions.push(("head", "Show first results"));
            }
            "curl" | "wget" => {
                suggestions.push(("jq", "Parse JSON"));
                suggestions.push(("grep", "Search output"));
                suggestions.push(("head", "Show first lines"));
            }
            "docker" => {
                suggestions.push(("grep", "Filter output"));
                suggestions.push(("awk", "Extract columns"));
                suggestions.push(("head", "Show first items"));
            }
            "git" => {
                suggestions.push(("grep", "Filter output"));
                suggestions.push(("head", "Show first items"));
                suggestions.push(("wc -l", "Count items"));
            }
            _ => {
                // Generic suggestions for any pipe
                suggestions.push(("grep", "Filter by pattern"));
                suggestions.push(("wc -l", "Count lines"));
                suggestions.push(("head", "Show first lines"));
                suggestions.push(("tail", "Show last lines"));
                suggestions.push(("sort", "Sort output"));
            }
        }

        suggestions
    }

    /// Get follow-up command suggestions based on the previous command
    pub fn follow_up_commands(previous_cmd: &str) -> Vec<(&'static str, &'static str)> {
        let mut suggestions = Vec::new();

        match previous_cmd {
            "git add" => {
                suggestions.push(("git commit", "Commit the staged changes"));
                suggestions.push(("git status", "Check status"));
                suggestions.push(("git diff --cached", "See staged changes"));
            }
            "git commit" => {
                suggestions.push(("git push", "Push to remote"));
                suggestions.push(("git log --oneline -5", "View recent commits"));
            }
            "git pull" => {
                suggestions.push(("git log --oneline -5", "View new commits"));
                suggestions.push(("git diff HEAD~5", "See recent changes"));
            }
            "git checkout" | "git switch" => {
                suggestions.push(("git pull", "Update branch"));
                suggestions.push(("git status", "Check status"));
            }
            "npm install" | "yarn add" | "pnpm add" => {
                suggestions.push(("npm start", "Start the application"));
                suggestions.push(("npm run dev", "Start in development mode"));
                suggestions.push(("npm test", "Run tests"));
            }
            "cargo build" => {
                suggestions.push(("cargo run", "Run the application"));
                suggestions.push(("cargo test", "Run tests"));
            }
            "cargo test" => {
                suggestions.push(("cargo build --release", "Build for release"));
                suggestions.push(("cargo run", "Run the application"));
            }
            "docker build" => {
                suggestions.push(("docker run", "Run the built image"));
                suggestions.push(("docker push", "Push to registry"));
            }
            "make" => {
                suggestions.push(("make install", "Install the built artifacts"));
                suggestions.push(("make test", "Run tests"));
                suggestions.push(("make clean", "Clean build artifacts"));
            }
            _ => {}
        }

        suggestions
    }
}

/// Pipeline templates for quick expansion
#[derive(Debug, Clone)]
pub struct PipelineTemplate {
    /// Trigger pattern (e.g., "||count")
    pub trigger: String,
    /// Expansion (e.g., "| wc -l")
    pub expansion: String,
    /// Description
    pub description: String,
}

impl PipelineTemplate {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                trigger: "||count".to_string(),
                expansion: "| wc -l".to_string(),
                description: "Count lines".to_string(),
            },
            Self {
                trigger: "||sort".to_string(),
                expansion: "| sort | uniq".to_string(),
                description: "Sort and deduplicate".to_string(),
            },
            Self {
                trigger: "||head".to_string(),
                expansion: "| head -20".to_string(),
                description: "Show first 20 lines".to_string(),
            },
            Self {
                trigger: "||tail".to_string(),
                expansion: "| tail -20".to_string(),
                description: "Show last 20 lines".to_string(),
            },
            Self {
                trigger: "||grep".to_string(),
                expansion: "| grep ".to_string(),
                description: "Filter with grep".to_string(),
            },
            Self {
                trigger: "||json".to_string(),
                expansion: "| jq '.'".to_string(),
                description: "Pretty print JSON".to_string(),
            },
            Self {
                trigger: "||less".to_string(),
                expansion: "| less".to_string(),
                description: "Page through output".to_string(),
            },
            Self {
                trigger: "||clip".to_string(),
                expansion: "| pbcopy".to_string(), // macOS
                description: "Copy to clipboard".to_string(),
            },
            Self {
                trigger: "||save".to_string(),
                expansion: "> output.txt".to_string(),
                description: "Save to file".to_string(),
            },
            Self {
                trigger: "||xargs".to_string(),
                expansion: "| xargs ".to_string(),
                description: "Execute on each line".to_string(),
            },
        ]
    }

    /// Find a matching template for the given trigger
    pub fn find(trigger: &str) -> Option<Self> {
        Self::defaults().into_iter().find(|t| t.trigger == trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pipeline() {
        let pipeline = Pipeline::parse("ls -la | grep foo");
        assert_eq!(pipeline.segments.len(), 2);
        assert_eq!(pipeline.segments[0].command, "ls");
        assert_eq!(pipeline.segments[0].args, vec!["-la"]);
        assert_eq!(
            pipeline.segments[0].operator_after,
            Some(PipelineOperator::Pipe)
        );
        assert_eq!(pipeline.segments[1].command, "grep");
        assert_eq!(pipeline.segments[1].args, vec!["foo"]);
    }

    #[test]
    fn test_complex_pipeline() {
        let pipeline = Pipeline::parse("cat file.txt | grep pattern | wc -l > count.txt");
        assert_eq!(pipeline.segments.len(), 4);
        assert_eq!(pipeline.segments[0].command, "cat");
        assert_eq!(pipeline.segments[1].command, "grep");
        assert_eq!(pipeline.segments[2].command, "wc");
        assert_eq!(
            pipeline.segments[2].operator_after,
            Some(PipelineOperator::RedirectOut)
        );
    }

    #[test]
    fn test_and_or_operators() {
        let pipeline = Pipeline::parse("make && make install || echo failed");
        assert_eq!(pipeline.segments.len(), 3);
        assert_eq!(
            pipeline.segments[0].operator_after,
            Some(PipelineOperator::And)
        );
        assert_eq!(
            pipeline.segments[1].operator_after,
            Some(PipelineOperator::Or)
        );
    }

    #[test]
    fn test_quoted_strings() {
        let pipeline = Pipeline::parse("echo \"hello | world\" | grep hello");
        assert_eq!(pipeline.segments.len(), 2);
        assert_eq!(pipeline.segments[0].args, vec!["\"hello | world\""]);
    }

    #[test]
    fn test_is_after_pipe() {
        let pipeline = Pipeline::parse("ls | ");
        assert!(pipeline.is_after_pipe() || pipeline.in_operator);
    }

    #[test]
    fn test_stdin_commands() {
        let commands = PipelineSuggestions::stdin_commands();
        assert!(commands.contains(&"grep"));
        assert!(commands.contains(&"awk"));
        assert!(commands.contains(&"sort"));
    }

    #[test]
    fn test_pipe_suggestions() {
        let suggestions = PipelineSuggestions::for_pipe_after("ls");
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|(cmd, _)| *cmd == "grep"));
    }

    #[test]
    fn test_follow_up_commands() {
        let suggestions = PipelineSuggestions::follow_up_commands("git add");
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|(cmd, _)| *cmd == "git commit"));
    }

    #[test]
    fn test_pipeline_templates() {
        let template = PipelineTemplate::find("||count");
        assert!(template.is_some());
        assert_eq!(template.unwrap().expansion, "| wc -l");
    }
}
