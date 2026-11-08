use crate::astrobox::psys_host_v4::{self as host, dialog, ui};
use crate::model::{
    CATEGORIES, Deletion, Project, category_rank, color_for, format_duration, format_time, now_ms,
    shift_date, sort_projects, today_key,
};
use crate::state::Page;
use crate::{state, sync};

const PAGE_PROJECTS: &str = "page:projects";
const PAGE_HISTORY: &str = "page:history";
const ADD_PROJECT: &str = "project:add";
const SYNC_NOW: &str = "sync:now";
const DATE_PREVIOUS: &str = "date:previous";
const DATE_NEXT: &str = "date:next";
const DATE_TODAY: &str = "date:today";

const COLOR_BG: &str = "#111113";
const COLOR_PANEL: &str = "#1C1C1E";
const COLOR_ROW: &str = "#28282B";
const COLOR_TEXT: &str = "#F5F5F7";
const COLOR_MUTED: &str = "#A1A1A6";
const COLOR_ACCENT: &str = "#55D6BE";
const COLOR_DANGER: &str = "#FF6B7A";

pub fn rerender() {
    let root = state::read_state(|state| state.root_element_id.clone());
    if let Some(root) = root {
        host::ui::render(&root, build_page());
    }
}

pub async fn handle_event(event_type: ui::Event, event_id: &str, _payload: &str) {
    if event_type != ui::Event::Click {
        return;
    }

    match event_id {
        PAGE_PROJECTS => state::with_state(|state| state.page = Page::Projects),
        PAGE_HISTORY => state::with_state(|state| state.page = Page::History),
        ADD_PROJECT => {
            add_project().await;
            return;
        }
        SYNC_NOW => {
            sync::sync_now().await;
            return;
        }
        DATE_PREVIOUS => state::with_state(|state| {
            state.selected_date = shift_date(&state.selected_date, -1);
        }),
        DATE_NEXT => state::with_state(|state| {
            let next = shift_date(&state.selected_date, 1);
            let today = today_key(state.timezone_offset_minutes);
            state.selected_date = if next > today { today } else { next };
        }),
        DATE_TODAY => state::with_state(|state| {
            state.selected_date = today_key(state.timezone_offset_minutes);
        }),
        _ if event_id.starts_with("rename:") => {
            rename_project(&event_id[7..]).await;
            return;
        }
        _ if event_id.starts_with("category:") => {
            change_category(&event_id[9..]).await;
            return;
        }
        _ if event_id.starts_with("delete:") => {
            delete_project(&event_id[7..]).await;
            return;
        }
        _ => return,
    }
    let _ = state::save();
    rerender();
}

async fn add_project() {
    let result = input_dialog("添加项目", "输入项目名称（最多 12 个字）").await;
    if result.clicked_btn_id != "save" {
        return;
    }
    let name: String = result.input_result.trim().chars().take(12).collect();
    if name.is_empty() {
        set_status("项目名称不能为空");
        return;
    }
    let Some(category) = category_dialog("选择项目分类").await else {
        return;
    };
    let now = now_ms();
    let project = Project {
        id: format!("p_phone_{}", now),
        name,
        category: category.clone(),
        color: color_for(&category).to_string(),
        updated_at: now,
    };
    state::with_state(|state| {
        state.projects.push(project.clone());
        sort_projects(&mut state.projects);
        state.status = "项目已保存在手机".to_string();
    });
    let _ = state::save();
    rerender();
    match sync::send_project(&project).await {
        Ok(()) => set_status("项目已添加并发送到手环"),
        Err(error) => set_status(&error),
    }
}

async fn rename_project(id: &str) {
    let project = state::read_state(|state| {
        state
            .projects
            .iter()
            .find(|project| project.id == id)
            .cloned()
    });
    let Some(mut project) = project else {
        return;
    };
    let result = input_dialog("项目改名", &format!("当前名称：{}", project.name)).await;
    if result.clicked_btn_id != "save" {
        return;
    }
    let name: String = result.input_result.trim().chars().take(12).collect();
    if name.is_empty() {
        set_status("项目名称不能为空");
        return;
    }
    project.name = name;
    project.updated_at = now_ms();
    upsert_and_send(project, "项目名称已更新").await;
}

async fn change_category(id: &str) {
    let project = state::read_state(|state| {
        state
            .projects
            .iter()
            .find(|project| project.id == id)
            .cloned()
    });
    let Some(mut project) = project else {
        return;
    };
    let Some(category) = category_dialog("修改项目分类").await else {
        return;
    };
    project.category = category.clone();
    project.color = color_for(&category).to_string();
    project.updated_at = now_ms();
    upsert_and_send(project, "项目分类已更新").await;
}

async fn upsert_and_send(project: Project, success: &str) {
    state::with_state(|state| {
        if let Some(current) = state.projects.iter_mut().find(|item| item.id == project.id) {
            *current = project.clone();
        }
        sort_projects(&mut state.projects);
        state.status = "修改已保存在手机".to_string();
    });
    let _ = state::save();
    rerender();
    match sync::send_project(&project).await {
        Ok(()) => set_status(success),
        Err(error) => set_status(&error),
    }
}

async fn delete_project(id: &str) {
    let project = state::read_state(|state| {
        state
            .projects
            .iter()
            .find(|project| project.id == id)
            .cloned()
    });
    let Some(project) = project else {
        return;
    };
    let result = dialog::show_dialog(
        dialog::DialogType::Alert,
        dialog::DialogStyle::Website,
        dialog::DialogInfo {
            title: "删除项目".to_string(),
            content: format!("确定删除“{}”？历史记录不会被删除。", project.name),
            buttons: vec![
                dialog::DialogButton {
                    id: "cancel".to_string(),
                    primary: false,
                    content: "取消".to_string(),
                },
                dialog::DialogButton {
                    id: "delete".to_string(),
                    primary: true,
                    content: "删除".to_string(),
                },
            ],
        },
    )
    .await;
    if result.clicked_btn_id != "delete" {
        return;
    }
    let deletion = Deletion {
        id: project.id,
        deleted_at: now_ms(),
    };
    state::with_state(|state| {
        state.projects.retain(|item| item.id != deletion.id);
        state.deleted_projects.retain(|item| item.id != deletion.id);
        state.deleted_projects.push(deletion.clone());
        state.status = "删除操作已保存在手机".to_string();
    });
    let _ = state::save();
    rerender();
    match sync::send_deletion(&deletion).await {
        Ok(()) => set_status("项目已从手环删除"),
        Err(error) => set_status(&error),
    }
}

async fn input_dialog(title: &str, content: &str) -> dialog::DialogResult {
    dialog::show_dialog(
        dialog::DialogType::Input,
        dialog::DialogStyle::Website,
        dialog::DialogInfo {
            title: title.to_string(),
            content: content.to_string(),
            buttons: vec![
                dialog::DialogButton {
                    id: "cancel".to_string(),
                    primary: false,
                    content: "取消".to_string(),
                },
                dialog::DialogButton {
                    id: "save".to_string(),
                    primary: true,
                    content: "保存".to_string(),
                },
            ],
        },
    )
    .await
}

async fn category_dialog(title: &str) -> Option<String> {
    let mut buttons: Vec<dialog::DialogButton> = CATEGORIES
        .iter()
        .map(|category| dialog::DialogButton {
            id: category.to_string(),
            primary: *category == "工作",
            content: category.to_string(),
        })
        .collect();
    buttons.push(dialog::DialogButton {
        id: "cancel".to_string(),
        primary: false,
        content: "取消".to_string(),
    });
    let result = dialog::show_dialog(
        dialog::DialogType::Alert,
        dialog::DialogStyle::Website,
        dialog::DialogInfo {
            title: title.to_string(),
            content: "选择分类后会立即同步到手环".to_string(),
            buttons,
        },
    )
    .await;
    CATEGORIES
        .contains(&result.clicked_btn_id.as_str())
        .then_some(result.clicked_btn_id)
}

fn set_status(message: &str) {
    state::with_state(|state| state.status = message.to_string());
    rerender();
}

fn build_page() -> ui::Element {
    let page = state::read_state(|state| state.page);
    let mut content = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg(COLOR_BG)
        .padding(16)
        .gap(14)
        .child(build_header())
        .child(build_tabs());

    content = match page {
        Page::Projects => content.child(build_projects()),
        Page::History => content.child(build_history()),
    };

    ui::Element::new(ui::ElementType::ScrollArea, None)
        .width_full()
        .height_full()
        .bg(COLOR_BG)
        .child(content)
}

fn build_header() -> ui::Element {
    let (device, status, syncing) = state::read_state(|state| {
        (
            state
                .device_name
                .clone()
                .unwrap_or_else(|| "未连接手环".to_string()),
            state.status.clone(),
            state.syncing,
        )
    });
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(5)
        .child(
            ui::Element::new(ui::ElementType::P, Some("正计时配置"))
                .size(25)
                .text_color(COLOR_TEXT),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(device.as_str()))
                .size(13)
                .text_color(COLOR_ACCENT),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(status.as_str()))
                .size(12)
                .text_color(COLOR_MUTED),
        )
        .child(action_button(
            if syncing {
                "同步中..."
            } else {
                "立即同步"
            },
            SYNC_NOW,
            COLOR_PANEL,
            COLOR_TEXT,
        ))
}

fn build_tabs() -> ui::Element {
    let page = state::read_state(|state| state.page);
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .width_full()
        .gap(8)
        .child(tab_button(
            "项目管理",
            PAGE_PROJECTS,
            page == Page::Projects,
        ))
        .child(tab_button("历史记录", PAGE_HISTORY, page == Page::History))
}

fn tab_button(label: &str, event: &str, selected: bool) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(label))
        .flex_grow(1.0)
        .padding(11)
        .radius(8)
        .bg(if selected { COLOR_ACCENT } else { COLOR_PANEL })
        .text_color(if selected { "#111113" } else { COLOR_TEXT })
        .on(ui::Event::Click, event)
}

fn build_projects() -> ui::Element {
    let projects = state::read_state(|state| state.projects.clone());
    let mut section = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .child(action_button(
            "＋ 添加项目",
            ADD_PROJECT,
            COLOR_ACCENT,
            "#111113",
        ))
        .child(
            ui::Element::new(
                ui::ElementType::P,
                Some(format!("当前共 {} 个项目", projects.len()).as_str()),
            )
            .size(13)
            .text_color(COLOR_MUTED),
        );

    if projects.is_empty() {
        return section.child(empty_panel(
            "还没有项目",
            "连接手环并点“立即同步”，或在手机上添加项目",
        ));
    }

    for category in CATEGORIES {
        let matching: Vec<Project> = projects
            .iter()
            .filter(|project| project.category == category)
            .cloned()
            .collect();
        if matching.is_empty() {
            continue;
        }
        section = section.child(section_title(category, matching.len()));
        for project in matching {
            section = section.child(project_row(&project));
        }
    }
    section
}

fn project_row(project: &Project) -> ui::Element {
    let rename = format!("rename:{}", project.id);
    let category = format!("category:{}", project.id);
    let delete = format!("delete:{}", project.id);
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .width_full()
        .align_center()
        .gap(6)
        .padding(10)
        .radius(8)
        .bg(COLOR_ROW)
        .child(
            ui::Element::new(ui::ElementType::P, Some(project.name.as_str()))
                .flex_grow(1.0)
                .size(16)
                .text_color(COLOR_TEXT),
        )
        .child(small_button("改名", &rename, COLOR_PANEL, COLOR_TEXT))
        .child(small_button("分类", &category, COLOR_PANEL, COLOR_TEXT))
        .child(small_button("删", &delete, "#3A1D22", COLOR_DANGER))
}

fn build_history() -> ui::Element {
    let (selected_date, timezone_offset, mut records) = state::read_state(|state| {
        (
            state.selected_date.clone(),
            state.timezone_offset_minutes,
            state
                .records
                .iter()
                .filter(|record| record.date_key == state.selected_date)
                .cloned()
                .collect::<Vec<_>>(),
        )
    });
    records.sort_by(|left, right| {
        category_rank(&left.category)
            .cmp(&category_rank(&right.category))
            .then_with(|| right.ended_at.cmp(&left.ended_at))
    });
    let total_seconds: u64 = records.iter().map(|record| record.duration_sec).sum();

    let mut section = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .width_full()
                .gap(8)
                .child(small_button(
                    "‹ 前一天",
                    DATE_PREVIOUS,
                    COLOR_PANEL,
                    COLOR_TEXT,
                ))
                .child(small_button("今天", DATE_TODAY, COLOR_ACCENT, "#111113"))
                .child(small_button("后一天 ›", DATE_NEXT, COLOR_PANEL, COLOR_TEXT)),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(selected_date.as_str()))
                .size(20)
                .text_color(COLOR_TEXT),
        )
        .child(
            ui::Element::new(
                ui::ElementType::P,
                Some(
                    format!(
                        "{} 条记录 · {}",
                        records.len(),
                        format_duration(total_seconds)
                    )
                    .as_str(),
                ),
            )
            .size(13)
            .text_color(COLOR_MUTED),
        );

    if records.is_empty() {
        return section.child(empty_panel("当天没有记录", "完成计时并同步后会显示在这里"));
    }

    let mut current_category = String::new();
    for record in records {
        if record.category != current_category {
            current_category = record.category.clone();
            let count = state::read_state(|state| {
                state
                    .records
                    .iter()
                    .filter(|item| {
                        item.date_key == selected_date && item.category == current_category
                    })
                    .count()
            });
            section = section.child(section_title(&current_category, count));
        }
        let meta = format!(
            "{} · {}",
            format_duration(record.duration_sec),
            format_time(record.ended_at, timezone_offset)
        );
        section = section.child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .width_full()
                .align_center()
                .gap(8)
                .padding(11)
                .radius(8)
                .bg(COLOR_ROW)
                .child(
                    ui::Element::new(ui::ElementType::P, Some(record.project_name.as_str()))
                        .flex_grow(1.0)
                        .size(15)
                        .text_color(COLOR_TEXT),
                )
                .child(
                    ui::Element::new(ui::ElementType::P, Some(meta.as_str()))
                        .size(12)
                        .text_color(COLOR_MUTED),
                ),
        );
    }
    section
}

fn section_title(category: &str, count: usize) -> ui::Element {
    ui::Element::new(
        ui::ElementType::P,
        Some(format!("{}  {}", category, count).as_str()),
    )
    .size(13)
    .text_color(color_for(category))
    .margin_top(8)
}

fn empty_panel(title: &str, description: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(6)
        .padding(16)
        .radius(8)
        .bg(COLOR_PANEL)
        .child(
            ui::Element::new(ui::ElementType::P, Some(title))
                .size(16)
                .text_color(COLOR_TEXT),
        )
        .child(
            ui::Element::new(ui::ElementType::P, Some(description))
                .size(13)
                .text_color(COLOR_MUTED),
        )
}

fn action_button(label: &str, event: &str, background: &str, text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(label))
        .width_full()
        .padding(12)
        .radius(8)
        .bg(background)
        .text_color(text)
        .on(ui::Event::Click, event)
}

fn small_button(label: &str, event: &str, background: &str, text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(label))
        .padding(8)
        .radius(7)
        .bg(background)
        .text_color(text)
        .on(ui::Event::Click, event)
}
