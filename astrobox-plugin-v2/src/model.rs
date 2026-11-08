use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use time::{Date, Duration, Month, OffsetDateTime, UtcOffset};

pub const PROTOCOL: &str = "forward-timer/v1";
pub const PACKAGE_NAME: &str = "run.abox.forwardtimer";
pub const CATEGORIES: [&str; 6] = ["工作", "阅读", "学习", "运动", "娱乐", "个人"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub category: String,
    pub color: String,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default = "default_record_name")]
    pub project_name: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub started_at: u64,
    #[serde(default)]
    pub ended_at: u64,
    #[serde(default)]
    pub duration_sec: u64,
    #[serde(default)]
    pub date_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deletion {
    pub id: String,
    pub deleted_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub deleted_projects: Vec<Deletion>,
    #[serde(default)]
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Envelope {
    pub protocol: String,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Serialize)]
struct OutgoingEnvelope<'a, T: Serialize> {
    protocol: &'static str,
    #[serde(rename = "type")]
    message_type: &'a str,
    #[serde(rename = "sentAt")]
    sent_at: u64,
    data: &'a T,
}

fn default_record_name() -> String {
    "未命名项目".to_string()
}

fn default_category() -> String {
    "个人".to_string()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn color_for(category: &str) -> &'static str {
    match category {
        "工作" => "#55D6BE",
        "阅读" => "#FFD166",
        "学习" => "#7AA2F7",
        "运动" => "#FF7A90",
        "娱乐" => "#C6A6FF",
        "个人" => "#FFB86C",
        _ => "#8E8E93",
    }
}

pub fn category_rank(category: &str) -> usize {
    CATEGORIES
        .iter()
        .position(|item| *item == category)
        .unwrap_or(CATEGORIES.len())
}

pub fn normalize_project(mut project: Project) -> Option<Project> {
    project.id = project.id.trim().to_string();
    project.name = project.name.trim().chars().take(12).collect();
    if project.id.is_empty() || project.name.is_empty() {
        return None;
    }
    if !CATEGORIES.contains(&project.category.as_str()) {
        project.category = "个人".to_string();
    }
    if project.color.trim().is_empty() {
        project.color = color_for(&project.category).to_string();
    }
    if project.updated_at == 0 {
        project.updated_at = now_ms();
    }
    Some(project)
}

pub fn normalize_record(mut record: Record, timezone_offset_minutes: i32) -> Option<Record> {
    record.id = record.id.trim().to_string();
    if record.id.is_empty() {
        return None;
    }
    record.project_name = record.project_name.trim().chars().take(24).collect();
    if record.project_name.is_empty() {
        record.project_name = default_record_name();
    }
    if !CATEGORIES.contains(&record.category.as_str()) {
        record.category = default_category();
    }
    if record.ended_at == 0 {
        record.ended_at = now_ms();
    }
    if record.date_key.len() != 10 {
        record.date_key = date_key(record.ended_at, timezone_offset_minutes);
    }
    Some(record)
}

pub fn merge_snapshot(
    local_projects: &mut Vec<Project>,
    local_deletions: &mut Vec<Deletion>,
    local_records: &mut Vec<Record>,
    snapshot: Snapshot,
    timezone_offset_minutes: i32,
) {
    let mut deletion_by_id: HashMap<String, u64> = HashMap::new();
    for deletion in local_deletions
        .iter()
        .chain(snapshot.deleted_projects.iter())
    {
        deletion_by_id
            .entry(deletion.id.clone())
            .and_modify(|timestamp| *timestamp = (*timestamp).max(deletion.deleted_at))
            .or_insert(deletion.deleted_at);
    }

    let mut project_by_id: HashMap<String, Project> = HashMap::new();
    for project in local_projects.iter().cloned().chain(snapshot.projects) {
        let Some(project) = normalize_project(project) else {
            continue;
        };
        if deletion_by_id
            .get(&project.id)
            .is_some_and(|deleted_at| *deleted_at >= project.updated_at)
        {
            continue;
        }
        project_by_id
            .entry(project.id.clone())
            .and_modify(|current| {
                if project.updated_at >= current.updated_at {
                    *current = project.clone();
                }
            })
            .or_insert(project);
    }

    let mut record_by_id: HashMap<String, Record> = HashMap::new();
    for record in local_records.iter().cloned().chain(snapshot.records) {
        if let Some(record) = normalize_record(record, timezone_offset_minutes) {
            record_by_id.insert(record.id.clone(), record);
        }
    }

    *local_projects = project_by_id.into_values().collect();
    sort_projects(local_projects);
    *local_deletions = deletion_by_id
        .into_iter()
        .map(|(id, deleted_at)| Deletion { id, deleted_at })
        .collect();
    local_deletions.sort_by_key(|item| item.deleted_at);
    if local_deletions.len() > 500 {
        local_deletions.drain(..local_deletions.len() - 500);
    }
    *local_records = record_by_id.into_values().collect();
    local_records.sort_by_key(|item| item.ended_at);
    if local_records.len() > 5000 {
        local_records.drain(..local_records.len() - 5000);
    }
}

pub fn sort_projects(projects: &mut [Project]) {
    projects.sort_by(|left, right| {
        category_rank(&left.category)
            .cmp(&category_rank(&right.category))
            .then_with(|| left.name.cmp(&right.name))
    });
}

pub fn envelope<T: Serialize>(message_type: &str, data: &T) -> Result<String, String> {
    serde_json::to_string(&OutgoingEnvelope {
        protocol: PROTOCOL,
        message_type,
        sent_at: now_ms(),
        data,
    })
    .map_err(|error| error.to_string())
}

pub fn date_key(timestamp_ms: u64, timezone_offset_minutes: i32) -> String {
    let seconds = (timestamp_ms / 1000).min(i64::MAX as u64) as i64;
    let offset = UtcOffset::from_whole_seconds(timezone_offset_minutes.saturating_mul(60))
        .unwrap_or(UtcOffset::UTC);
    let value = OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset)
        .date();
    format_date(value)
}

pub fn today_key(timezone_offset_minutes: i32) -> String {
    date_key(now_ms(), timezone_offset_minutes)
}

pub fn shift_date(value: &str, days: i64) -> String {
    parse_date(value)
        .and_then(|date| date.checked_add(Duration::days(days)))
        .map(format_date)
        .unwrap_or_else(|| value.to_string())
}

fn parse_date(value: &str) -> Option<Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

fn format_date(value: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year(),
        u8::from(value.month()),
        value.day()
    )
}

pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let remaining = seconds % 60;
    if hours > 0 {
        format!("{}小时{}分", hours, minutes)
    } else if minutes > 0 {
        format!("{}分{}秒", minutes, remaining)
    } else {
        format!("{}秒", remaining)
    }
}

pub fn format_time(timestamp_ms: u64, timezone_offset_minutes: i32) -> String {
    let seconds = (timestamp_ms / 1000).min(i64::MAX as u64) as i64;
    let offset = UtcOffset::from_whole_seconds(timezone_offset_minutes.saturating_mul(60))
        .unwrap_or(UtcOffset::UTC);
    let value = OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);
    format!("{:02}:{:02}", value.hour(), value.minute())
}
