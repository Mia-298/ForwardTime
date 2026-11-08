import AstroBox from "astrobox-plugin-sdk";
import {
  PACKAGE_NAME,
  PROTOCOL,
  CATEGORIES,
  CATEGORY_COLORS,
  emptyState,
  normalizeState,
  localDateKey,
  shiftDate,
  mergeSnapshot,
  upsertLocalProject,
  deleteLocalProject,
  recordsForSelection,
  summarize,
  formatDuration,
  formatTime,
  parseIncoming,
  buildEnvelope
} from "./model.js";

let state = emptyState();
let projectNameDraft = "";
let projectCategoryDraft = "工作";
let deleteProjectId = "";
let statusText = "等待同步";
let ui = [];

const syncId = AstroBox.native.regNativeFun(syncNow);
const previousDateId = AstroBox.native.regNativeFun(showPreviousDate);
const nextDateId = AstroBox.native.regNativeFun(showNextDate);
const todayId = AstroBox.native.regNativeFun(showToday);
const categoryFilterId = AstroBox.native.regNativeFun(changeCategoryFilter);
const projectNameId = AstroBox.native.regNativeFun(changeProjectName);
const projectCategoryId = AstroBox.native.regNativeFun(changeProjectCategory);
const addProjectId = AstroBox.native.regNativeFun(addProject);
const deleteProjectSelectId = AstroBox.native.regNativeFun(changeDeleteProject);
const deleteProjectIdCallback = AstroBox.native.regNativeFun(deleteProject);

AstroBox.lifecycle.onLoad(() => {
  loadConfig();
  registerListeners();
  render();
  syncNow();
});

function loadConfig() {
  try {
    state = normalizeState(AstroBox.config.readConfig());
  } catch (error) {
    state = emptyState();
  }
}

function saveConfig() {
  try {
    AstroBox.config.writeConfig(state);
  } catch (error) {
    statusText = "本地保存失败";
  }
}

function registerListeners() {
  const onInterconnect = (payload) => handleIncoming(payload);
  const onDevice = () => syncNow();

  for (const eventName of ["interconnect", "interconnect-message"]) {
    try {
      AstroBox.event.addEventListener(eventName, onInterconnect);
    } catch (error) {}
  }
  for (const eventName of ["device", "device_action", "device-action"]) {
    try {
      AstroBox.event.addEventListener(eventName, onDevice);
    } catch (error) {}
  }
}

async function syncNow() {
  statusText = "正在同步...";
  render();
  try {
    for (const deletion of state.deletedProjects) {
      await AstroBox.interconnect.sendQAICMessage(
        PACKAGE_NAME,
        buildEnvelope("project_delete", deletion)
      );
    }
    await AstroBox.interconnect.sendQAICMessage(
      PACKAGE_NAME,
      buildEnvelope("sync_request", {})
    );
    statusText = "同步请求已发送";
  } catch (error) {
    statusText = `同步失败：${error && error.message ? error.message : "请检查设备连接"}`;
  }
  render();
}

function handleIncoming(payload) {
  const message = parseIncoming(payload);
  if (!message || message.protocol !== PROTOCOL) return;

  if (message.type === "snapshot") {
    state = mergeSnapshot(state, message.data || {});
    statusText = `已同步 ${state.records.length} 条记录`;
    saveConfig();
    render();
    return;
  }

  if (message.type === "project_deleted" && message.data && message.data.id) {
    state = deleteLocalProject(state, message.data.id, message.data.deletedAt);
    statusText = "项目删除已同步";
    saveConfig();
    render();
  }
}

function showPreviousDate() {
  state.selectedDate = shiftDate(state.selectedDate, -1);
  saveConfig();
  render();
}

function showNextDate() {
  const next = shiftDate(state.selectedDate, 1);
  const today = localDateKey(Date.now());
  state.selectedDate = next > today ? today : next;
  saveConfig();
  render();
}

function showToday() {
  state.selectedDate = localDateKey(Date.now());
  saveConfig();
  render();
}

function changeCategoryFilter(value) {
  const options = selectedFirst(["全部"].concat(CATEGORIES), state.categoryFilter);
  const normalized = normalizeDropdownValue(value, options);
  if (normalized) state.categoryFilter = normalized;
  saveConfig();
  render();
}

function changeProjectName(value) {
  projectNameDraft = String(value || "").trim().slice(0, 24);
}

function changeProjectCategory(value) {
  const options = selectedFirst(CATEGORIES, projectCategoryDraft);
  const normalized = normalizeDropdownValue(value, options);
  if (normalized) projectCategoryDraft = normalized;
}

async function addProject() {
  if (!projectNameDraft) {
    statusText = "请先输入项目名称";
    render();
    return;
  }

  const now = Date.now();
  const project = {
    id: `p_phone_${now}`,
    name: projectNameDraft,
    category: projectCategoryDraft,
    color: CATEGORY_COLORS[projectCategoryDraft],
    updatedAt: now
  };
  state = upsertLocalProject(state, project);
  projectNameDraft = "";
  deleteProjectId = project.id;
  saveConfig();
  render();

  try {
    await AstroBox.interconnect.sendQAICMessage(
      PACKAGE_NAME,
      buildEnvelope("project_upsert", project)
    );
    statusText = "项目已添加并发送到手环";
  } catch (error) {
    statusText = "项目已保存在手机，连接后请点同步";
  }
  render();
}

function changeDeleteProject(value) {
  const ids = state.projects.map((project) => project.id);
  const labels = state.projects.map(projectLabel);
  const selectedLabel = normalizeDropdownValue(value, labels);
  const labelIndex = labels.indexOf(selectedLabel);
  if (labelIndex >= 0) deleteProjectId = ids[labelIndex];
}

async function deleteProject() {
  if (!deleteProjectId && state.projects.length > 0) {
    deleteProjectId = state.projects[0].id;
  }
  if (!deleteProjectId) {
    statusText = "没有可删除的项目";
    render();
    return;
  }

  const removedId = deleteProjectId;
  const deletedAt = Date.now();
  state = deleteLocalProject(state, removedId, deletedAt);
  deleteProjectId = state.projects.length > 0 ? state.projects[0].id : "";
  saveConfig();
  render();

  try {
    await AstroBox.interconnect.sendQAICMessage(
      PACKAGE_NAME,
      buildEnvelope("project_delete", { id: removedId, deletedAt })
    );
    statusText = "项目已从手机和手环删除";
  } catch (error) {
    statusText = "手机端已删除，连接后请再次同步删除";
  }
  render();
}

function render() {
  const records = recordsForSelection(state);
  const summary = summarize(records);
  const isToday = state.selectedDate === localDateKey(Date.now());
  const history = buildHistoryText(records);
  const projectLabels = state.projects.map(projectLabel);
  const categoryFilterOptions = selectedFirst(["全部"].concat(CATEGORIES), state.categoryFilter);
  const projectCategoryOptions = selectedFirst(CATEGORIES, projectCategoryDraft);

  if (deleteProjectId && !state.projects.some((item) => item.id === deleteProjectId)) {
    deleteProjectId = "";
  }
  if (!deleteProjectId && state.projects.length > 0) deleteProjectId = state.projects[0].id;

  ui = [
    textNode("hero", "正计时 · 小米手环 9"),
    textNode("status", `${statusText}${state.lastSyncAt ? ` · ${formatSyncTime(state.lastSyncAt)}` : ""}`),
    buttonNode("sync", "立即同步", syncId, true),
    textNode("dateTitle", `${isToday ? "今天" : state.selectedDate} · ${state.categoryFilter} · ${summary.count} 条 · ${formatDuration(summary.durationSec)}`),
    dropdownNode("categoryFilter", categoryFilterOptions, categoryFilterId),
    buttonNode("previousDate", "前一天", previousDateId, false),
    buttonNode("today", "回到今天", todayId, false),
    buttonNode("nextDate", "后一天", nextDateId, false),
    textNode("history", history),
    textNode("projectTitle", "新增计时项目"),
    inputNode("projectName", projectNameDraft, "输入项目名称", projectNameId),
    dropdownNode("projectCategory", projectCategoryOptions, projectCategoryId),
    buttonNode("addProject", "添加并同步到手环", addProjectId, true),
    textNode("deleteTitle", "删除计时项目"),
    dropdownNode("deleteProjectSelect", projectLabels.length > 0 ? projectLabels : ["暂无项目"], deleteProjectSelectId),
    buttonNode("deleteProject", "删除所选项目", deleteProjectIdCallback, false)
  ];

  AstroBox.ui.updatePluginSettingsUI(ui);
}

function buildHistoryText(records) {
  if (records.length === 0) return "当天没有符合条件的记录";
  let currentCategory = "";
  const lines = [];
  for (const record of records) {
    if (record.category !== currentCategory) {
      currentCategory = record.category;
      lines.push(`\n【${currentCategory}】`);
    }
    lines.push(`${formatTime(record.endedAt)}  ${record.projectName}  ${formatDuration(record.durationSec)}`);
  }
  return lines.join("\n").trim();
}

function projectLabel(project) {
  return project.name;
}

function selectedFirst(options, selected) {
  return [selected].concat(options.filter((item) => item !== selected));
}

function normalizeDropdownValue(value, options) {
  if (typeof value === "number" && options[value] !== undefined) return options[value];
  const text = String(value === undefined || value === null ? "" : value);
  if (options.includes(text)) return text;
  const index = Number(text);
  if (Number.isInteger(index) && options[index] !== undefined) return options[index];
  return "";
}

function formatSyncTime(timestamp) {
  const date = new Date(timestamp);
  return `同步于 ${formatTime(date.getTime())}`;
}

function textNode(id, value) {
  return {
    node_id: id,
    visibility: true,
    disabled: false,
    content: { type: "Text", value }
  };
}

function buttonNode(id, text, callbackId, primary) {
  return {
    node_id: id,
    visibility: true,
    disabled: false,
    content: {
      type: "Button",
      value: { primary, text, callback_fun_id: callbackId }
    }
  };
}

function inputNode(id, text, placeholder, callbackId) {
  return {
    node_id: id,
    visibility: true,
    disabled: false,
    content: {
      type: "Input",
      value: { text, placeholder, callback_fun_id: callbackId }
    }
  };
}

function dropdownNode(id, options, callbackId) {
  return {
    node_id: id,
    visibility: true,
    disabled: false,
    content: {
      type: "Dropdown",
      value: { options, callback_fun_id: callbackId }
    }
  };
}

