use serde::Deserialize;
use std::path::Path;

fn default_ticket_pattern() -> String {
    r"\b[A-Z]{2,10}-\d{2,6}\b".to_string()
}

fn default_ticket_exempt() -> Vec<String> {
    // Collisions confirmed against real infra codebases during calibration:
    // security-advisory IDs and tool-suppression directives all match a
    // generic TICKET-123 shape but aren't project-tracker references.
    vec![
        r"\b(CVE|CWE|GHSA|AVD|RFC|ISO|SHA)-\d+\b".to_string(),
        r"trivy:ignore".to_string(),
        r"checkov:skip".to_string(),
        r"tfsec:ignore".to_string(),
        r"noqa".to_string(),
        r"nosec".to_string(),
        r"pylint:\s*disable".to_string(),
        r"eslint-disable".to_string(),
    ]
}

fn default_history_phrases() -> Vec<String> {
    vec![
        "was missing".into(),
        "used to be".into(),
        "previously was".into(),
        "previously did".into(),
        "previously had".into(),
        "before this fix".into(),
        "before this change".into(),
        "before this commit".into(),
        "before this pr".into(),
        "never actually".into(),
        "have never".into(),
        "root-caused".into(),
        "root caused".into(),
        "this fix".into(),
        "this bug".into(),
        "the bug was".into(),
        "was broken".into(),
        "no longer".into(),
        "now correctly".into(),
        "silently no-ops".into(),
        "silently noops".into(),
        "silently fails".into(),
        "silently skips".into(),
        "silently swallows".into(),
    ]
}

fn default_max_lines() -> usize {
    6
}

fn default_max_ratio() -> f64 {
    2.0
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TicketIdConfig {
    pub enabled: bool,
    pub pattern: String,
    pub exempt: Vec<String>,
}

impl Default for TicketIdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pattern: default_ticket_pattern(),
            exempt: default_ticket_exempt(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HistoryNarrationConfig {
    pub enabled: bool,
    pub phrases: Vec<String>,
}

impl Default for HistoryNarrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            phrases: default_history_phrases(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CrossFileRefConfig {
    pub enabled: bool,
}

impl Default for CrossFileRefConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct VerboseCommentConfig {
    pub enabled: bool,
    pub max_lines: usize,
    pub max_ratio: f64,
}

impl Default for VerboseCommentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: default_max_lines(),
            max_ratio: default_max_ratio(),
        }
    }
}

fn default_exclude() -> Vec<String> {
    vec![]
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub exclude: Vec<String>,
    #[serde(rename = "ticket_id")]
    pub ticket_id: TicketIdConfig,
    #[serde(rename = "history_narration")]
    pub history_narration: HistoryNarrationConfig,
    #[serde(rename = "cross_file_ref")]
    pub cross_file_ref: CrossFileRefConfig,
    #[serde(rename = "verbose_comment")]
    pub verbose_comment: VerboseCommentConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclude: default_exclude(),
            ticket_id: TicketIdConfig::default(),
            history_narration: HistoryNarrationConfig::default(),
            cross_file_ref: CrossFileRefConfig::default(),
            verbose_comment: VerboseCommentConfig::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    windbag: Config,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(path)?;
        let raw: RawConfig = toml::from_str(&text)?;
        Ok(raw.windbag)
    }
}
