use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

use crate::model::{Deletion, Project, Record, sort_projects, today_key};

const STATE_FILE: &str = "data/forward_timer_state.json";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Projects,
    History,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredState {
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    deleted_projects: Vec<Deletion>,
    #[serde(default)]
    records: Vec<Record>,
    #[serde(default)]
    selected_date: String,
}

pub struct AppState {
    pub root_element_id: Option<String>,
    pub page: Page,
    pub projects: Vec<Project>,
    pub deleted_projects: Vec<Deletion>,
    pub records: Vec<Record>,
    pub selected_date: String,
    pub timezone_offset_minutes: i32,
    pub device_addr: Option<String>,
    pub device_name: Option<String>,
    pub status: String,
    pub syncing: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            root_element_id: None,
            page: Page::Projects,
            projects: Vec::new(),
            deleted_projects: Vec::new(),
            records: Vec::new(),
            selected_date: String::new(),
            timezone_offset_minutes: 0,
            device_addr: None,
            device_name: None,
            status: "等待连接手环".to_string(),
            syncing: false,
        }
    }
}

static STATE: OnceLock<RwLock<AppState>> = OnceLock::new();

pub fn state() -> &'static RwLock<AppState> {
    STATE.get_or_init(|| RwLock::new(AppState::default()))
}

pub fn with_state<T>(callback: impl FnOnce(&mut AppState) -> T) -> T {
    let mut guard = state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    callback(&mut guard)
}

pub fn read_state<T>(callback: impl FnOnce(&AppState) -> T) -> T {
    let guard = state()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    callback(&guard)
}

pub fn load() {
    let stored = std::fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|raw| serde_json::from_str::<StoredState>(&raw).ok());
    with_state(|state| {
        if let Some(stored) = stored {
            state.projects = stored.projects;
            sort_projects(&mut state.projects);
            state.deleted_projects = stored.deleted_projects;
            state.records = stored.records;
            state.selected_date = stored.selected_date;
        }
        if state.selected_date.is_empty() {
            state.selected_date = today_key(state.timezone_offset_minutes);
        }
    });
}

pub fn save() -> Result<(), String> {
    let stored = read_state(|state| StoredState {
        projects: state.projects.clone(),
        deleted_projects: state.deleted_projects.clone(),
        records: state.records.clone(),
        selected_date: state.selected_date.clone(),
    });
    if let Some(parent) = std::path::Path::new(STATE_FILE).parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
    std::fs::write(STATE_FILE, json).map_err(|error| error.to_string())
}
