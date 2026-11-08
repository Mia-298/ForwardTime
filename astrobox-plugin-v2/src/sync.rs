use crate::astrobox::psys_host_v4::{device, interconnect, os, register};
use crate::model::{
    Deletion, Envelope, PACKAGE_NAME, PROTOCOL, Project, Snapshot, envelope, merge_snapshot,
};
use crate::{state, ui};

pub async fn refresh_connection(sync_after: bool) {
    let offset = os::timezone_offset_minutes().await;
    let devices = device::get_connected_device_list().await;
    let selected = devices.into_iter().next();

    match selected {
        Some(info) => {
            let registration =
                register::register_interconnect_recv(info.addr.clone(), PACKAGE_NAME.to_string())
                    .await;
            match registration {
                Ok(()) => {
                    state::with_state(|state| {
                        let old_today = crate::model::today_key(state.timezone_offset_minutes);
                        state.timezone_offset_minutes = offset;
                        if state.selected_date.is_empty() || state.selected_date == old_today {
                            state.selected_date = crate::model::today_key(offset);
                        }
                        state.device_addr = Some(info.addr);
                        state.device_name = Some(info.name);
                        state.status = "手环已连接".to_string();
                    });
                    if sync_after {
                        sync_connected().await;
                    } else {
                        ui::rerender();
                    }
                }
                Err(error) => {
                    state::with_state(|state| {
                        state.device_addr = None;
                        state.device_name = None;
                        state.status = format!("互联授权失败：{}", error);
                    });
                    ui::rerender();
                }
            }
        }
        None => {
            state::with_state(|state| {
                state.timezone_offset_minutes = offset;
                state.device_addr = None;
                state.device_name = None;
                state.status = "未检测到已连接的小米手环".to_string();
            });
            ui::rerender();
        }
    }
}

pub async fn sync_now() {
    if state::read_state(|state| state.device_addr.is_none()) {
        refresh_connection(false).await;
    }
    sync_connected().await;
}

async fn sync_connected() {
    let (addr, projects, deletions) = state::read_state(|state| {
        (
            state.device_addr.clone(),
            state.projects.clone(),
            state.deleted_projects.clone(),
        )
    });
    let Some(addr) = addr else {
        state::with_state(|state| {
            state.status = "未检测到已连接的小米手环".to_string();
        });
        ui::rerender();
        return;
    };

    state::with_state(|state| {
        state.syncing = true;
        state.status = "正在同步...".to_string();
    });
    ui::rerender();

    let mut error: Option<String> = None;
    for deletion in deletions {
        if let Err(send_error) = send(&addr, "project_delete", &deletion).await {
            error = Some(send_error);
            break;
        }
    }
    if error.is_none() {
        for project in projects {
            if let Err(send_error) = send(&addr, "project_upsert", &project).await {
                error = Some(send_error);
                break;
            }
        }
    }
    if error.is_none() {
        error = send(&addr, "sync_request", &serde_json::json!({}))
            .await
            .err();
    }

    state::with_state(|state| {
        state.syncing = false;
        state.status = match error {
            Some(error) => format!("同步失败：{}", error),
            None => "同步请求已发送".to_string(),
        };
    });
    ui::rerender();
}

pub async fn send_project(project: &Project) -> Result<(), String> {
    let addr = state::read_state(|state| state.device_addr.clone());
    let Some(addr) = addr else {
        return Err("手环未连接，项目已保存在手机".to_string());
    };
    send(&addr, "project_upsert", project).await
}

pub async fn send_deletion(deletion: &Deletion) -> Result<(), String> {
    let addr = state::read_state(|state| state.device_addr.clone());
    let Some(addr) = addr else {
        return Err("手环未连接，删除操作已保存在手机".to_string());
    };
    send(&addr, "project_delete", deletion).await
}

async fn send<T: serde::Serialize>(addr: &str, message_type: &str, data: &T) -> Result<(), String> {
    let payload = envelope(message_type, data)?;
    interconnect::send_qaic_message(addr.to_string(), PACKAGE_NAME.to_string(), payload).await
}

pub async fn handle_message(raw: &str) {
    let Ok(message) = serde_json::from_str::<Envelope>(raw) else {
        return;
    };
    if message.protocol != PROTOCOL {
        return;
    }

    match message.message_type.as_str() {
        "snapshot" => {
            let Ok(snapshot) = serde_json::from_value::<Snapshot>(message.data) else {
                return;
            };
            state::with_state(|state| {
                merge_snapshot(
                    &mut state.projects,
                    &mut state.deleted_projects,
                    &mut state.records,
                    snapshot,
                    state.timezone_offset_minutes,
                );
                state.status = format!(
                    "已同步 {} 个项目、{} 条记录",
                    state.projects.len(),
                    state.records.len()
                );
            });
            let _ = state::save();
            ui::rerender();
        }
        "project_deleted" => {
            if let Ok(deletion) = serde_json::from_value::<Deletion>(message.data) {
                state::with_state(|state| {
                    state.projects.retain(|project| project.id != deletion.id);
                    state.deleted_projects.retain(|item| item.id != deletion.id);
                    state.deleted_projects.push(deletion);
                    state.status = "项目删除已同步".to_string();
                });
                let _ = state::save();
                ui::rerender();
            }
        }
        _ => {}
    }
}
