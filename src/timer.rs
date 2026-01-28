//! Command Timer
//!
//! Tracks command execution time and provides statistics.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Command execution record
#[derive(Debug, Clone)]
pub struct CommandRecord {
    /// The command that was executed
    pub command: String,
    /// Execution duration
    pub duration: Duration,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Timestamp
    pub timestamp: Instant,
}

/// Command timer for tracking execution times
#[derive(Debug)]
pub struct CommandTimer {
    /// Current command start time
    start_time: Option<Instant>,
    /// Current command being tracked
    current_command: Option<String>,
    /// Recent command records (for statistics)
    records: VecDeque<CommandRecord>,
    /// Maximum records to keep
    max_records: usize,
    /// Threshold for showing time (in seconds)
    display_threshold: f64,
    /// Whether timing is enabled
    enabled: bool,
}

impl CommandTimer {
    /// Create a new command timer
    pub fn new() -> Self {
        Self {
            start_time: None,
            current_command: None,
            records: VecDeque::with_capacity(100),
            max_records: 100,
            display_threshold: 0.5, // Show timing for commands > 0.5s
            enabled: true,
        }
    }

    /// Set the display threshold (minimum seconds to show timing)
    pub fn with_threshold(mut self, seconds: f64) -> Self {
        self.display_threshold = seconds;
        self
    }

    /// Enable or disable timing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Start timing a command
    pub fn start(&mut self, command: &str) {
        if self.enabled {
            self.start_time = Some(Instant::now());
            self.current_command = Some(command.to_string());
        }
    }

    /// Stop timing and return the duration
    pub fn stop(&mut self, exit_code: Option<i32>) -> Option<Duration> {
        if !self.enabled {
            return None;
        }

        if let (Some(start), Some(command)) = (self.start_time.take(), self.current_command.take())
        {
            let duration = start.elapsed();

            // Record this command
            let record = CommandRecord {
                command,
                duration,
                exit_code,
                timestamp: start,
            };

            self.records.push_back(record);

            // Keep only max_records
            while self.records.len() > self.max_records {
                self.records.pop_front();
            }

            Some(duration)
        } else {
            None
        }
    }

    /// Get formatted duration string if above threshold
    pub fn format_duration(&self, duration: Duration) -> Option<String> {
        let secs = duration.as_secs_f64();

        if secs < self.display_threshold {
            return None;
        }

        let formatted = if secs < 1.0 {
            format!("{:.0}ms", secs * 1000.0)
        } else if secs < 60.0 {
            format!("{:.2}s", secs)
        } else if secs < 3600.0 {
            let mins = (secs / 60.0).floor();
            let remaining_secs = secs % 60.0;
            format!("{}m{:.1}s", mins as u32, remaining_secs)
        } else {
            let hours = (secs / 3600.0).floor();
            let remaining_mins = ((secs % 3600.0) / 60.0).floor();
            format!("{}h{}m", hours as u32, remaining_mins as u32)
        };

        Some(formatted)
    }

    /// Get average execution time for a command pattern
    pub fn average_time(&self, pattern: &str) -> Option<Duration> {
        let matching: Vec<_> = self
            .records
            .iter()
            .filter(|r| r.command.starts_with(pattern))
            .collect();

        if matching.is_empty() {
            return None;
        }

        let total: Duration = matching.iter().map(|r| r.duration).sum();
        Some(total / matching.len() as u32)
    }

    /// Get the slowest commands
    pub fn slowest(&self, count: usize) -> Vec<&CommandRecord> {
        let mut sorted: Vec<_> = self.records.iter().collect();
        sorted.sort_by(|a, b| b.duration.cmp(&a.duration));
        sorted.into_iter().take(count).collect()
    }

    /// Get statistics summary
    pub fn stats(&self) -> TimerStats {
        if self.records.is_empty() {
            return TimerStats::default();
        }

        let total_time: Duration = self.records.iter().map(|r| r.duration).sum();
        let avg_time = total_time / self.records.len() as u32;

        let max_record = self.records.iter().max_by_key(|r| r.duration);
        let min_record = self.records.iter().min_by_key(|r| r.duration);

        let failed_count = self
            .records
            .iter()
            .filter(|r| r.exit_code.map(|c| c != 0).unwrap_or(false))
            .count();

        TimerStats {
            total_commands: self.records.len(),
            total_time,
            average_time: avg_time,
            max_time: max_record.map(|r| r.duration).unwrap_or(Duration::ZERO),
            max_command: max_record.map(|r| r.command.clone()),
            min_time: min_record.map(|r| r.duration).unwrap_or(Duration::ZERO),
            failed_commands: failed_count,
        }
    }

    /// Clear all records
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Default for CommandTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer statistics
#[derive(Debug, Default)]
pub struct TimerStats {
    pub total_commands: usize,
    pub total_time: Duration,
    pub average_time: Duration,
    pub max_time: Duration,
    pub max_command: Option<String>,
    pub min_time: Duration,
    pub failed_commands: usize,
}

impl std::fmt::Display for TimerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Command Statistics")?;
        writeln!(f, "─────────────────")?;
        writeln!(f, "Total commands: {}", self.total_commands)?;
        writeln!(f, "Total time: {:.2}s", self.total_time.as_secs_f64())?;
        writeln!(f, "Average time: {:.3}s", self.average_time.as_secs_f64())?;
        writeln!(f, "Max time: {:.3}s", self.max_time.as_secs_f64())?;
        if let Some(cmd) = &self.max_command {
            writeln!(f, "Slowest: {}", cmd)?;
        }
        writeln!(f, "Min time: {:.3}s", self.min_time.as_secs_f64())?;
        writeln!(f, "Failed: {}", self.failed_commands)?;
        Ok(())
    }
}

/// Handle timer-related commands
pub fn handle_timer_command(timer: &CommandTimer, cmd: &str, args: &[&str]) -> Option<String> {
    match cmd {
        "time" | "timer" => {
            if args.is_empty() || args[0] == "stats" {
                Some(timer.stats().to_string())
            } else if args[0] == "slow" || args[0] == "slowest" {
                let count = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
                let slowest = timer.slowest(count);

                if slowest.is_empty() {
                    Some("No command records yet.".to_string())
                } else {
                    let output: Vec<String> = slowest
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            let status = match r.exit_code {
                                Some(0) => "✓",
                                Some(_) => "✗",
                                None => "?",
                            };
                            format!(
                                "{}. {:.3}s {} {}",
                                i + 1,
                                r.duration.as_secs_f64(),
                                status,
                                r.command
                            )
                        })
                        .collect();
                    Some(format!("Slowest commands:\n{}", output.join("\n")))
                }
            } else if args[0] == "avg" {
                if let Some(pattern) = args.get(1) {
                    if let Some(avg) = timer.average_time(pattern) {
                        Some(format!(
                            "Average time for '{}': {:.3}s",
                            pattern,
                            avg.as_secs_f64()
                        ))
                    } else {
                        Some(format!("No records matching '{}'", pattern))
                    }
                } else {
                    Some("Usage: time avg <pattern>".to_string())
                }
            } else {
                Some("Usage: time [stats|slow|avg <pattern>]".to_string())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_timer_basic() {
        let mut timer = CommandTimer::new();

        timer.start("test command");
        thread::sleep(Duration::from_millis(10));
        let duration = timer.stop(Some(0));

        assert!(duration.is_some());
        assert!(duration.unwrap().as_millis() >= 10);
    }

    #[test]
    fn test_format_duration() {
        let timer = CommandTimer::new().with_threshold(0.0);

        assert_eq!(timer.format_duration(Duration::from_millis(100)), Some("100ms".to_string()));
        assert_eq!(timer.format_duration(Duration::from_secs_f64(1.5)), Some("1.50s".to_string()));
        assert_eq!(timer.format_duration(Duration::from_secs(90)), Some("1m30.0s".to_string()));
    }

    #[test]
    fn test_timer_disabled() {
        let mut timer = CommandTimer::new();
        timer.set_enabled(false);

        timer.start("test");
        let duration = timer.stop(Some(0));

        assert!(duration.is_none());
    }

    #[test]
    fn test_stats() {
        let mut timer = CommandTimer::new();

        for _ in 0..5 {
            timer.start("test");
            thread::sleep(Duration::from_millis(1));
            timer.stop(Some(0));
        }

        let stats = timer.stats();
        assert_eq!(stats.total_commands, 5);
        assert_eq!(stats.failed_commands, 0);
    }
}
