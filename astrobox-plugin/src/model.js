export const PROTOCOL = "forward-timer/v1";
export const PACKAGE_NAME = "run.abox.forwardtimer";
export const CATEGORIES = ["工作", "阅读", "学习", "运动", "娱乐", "个人", "自定义", "其他"];

export const CATEGORY_COLORS = {
  工作: "#55D6BE",
  阅读: "#FF9F1C",
  学习: "#FFD166",
  运动: "#FF6B6B",
  娱乐: "#8EC5FF",
  个人: "#C6A6FF",
  自定义: "#72DDF7",
  其他: "#C6A6FF"
};

export function emptyState() {
  return {
    version: 1,
    projects: [],
    deletedProjects: [],
    records: [],
    selectedDate: localDateKey(Date.now()),
    categoryFilter: "全部",
    lastSyncAt: 0
  };
}

export function normalizeState(value) {
  const base = emptyState();
  if (!value || typeof value !== "object") return base;
  return {
    version: 1,
    projects: Array.isArray(value.projects) ? sortProjects(value.projects) : [],
    deletedProjects: normalizeDeletedProjects(value.deletedProjects),
    records: Array.isArray(value.records) ? value.records.slice(-5000) : [],
    selectedDate: isDateKey(value.selectedDate) ? value.selectedDate : base.selectedDate,
    categoryFilter: ["全部"].concat(CATEGORIES).includes(value.categoryFilter)
      ? value.categoryFilter
      : "全部",
    lastSyncAt: Number(value.lastSyncAt) || 0
  };
}

export function localDateKey(timestamp) {
  const date = new Date(timestamp);
  return [
    date.getFullYear(),
    pad(date.getMonth() + 1),
    pad(date.getDate())
  ].join("-");
}

export function shiftDate(dateValue, offsetDays) {
  const parts = dateValue.split("-").map(Number);
  const next = new Date(parts[0], parts[1] - 1, parts[2] + offsetDays, 12, 0, 0, 0);
  return localDateKey(next.getTime());
}

export function isDateKey(value) {
  return typeof value === "string" && /^\d{4}-\d{2}-\d{2}$/.test(value);
}

export function categoryRank(category) {
  const index = CATEGORIES.indexOf(category);
  return index < 0 ? CATEGORIES.length : index;
}

export function sortProjects(projects) {
  return projects.slice().sort((left, right) => {
    const categoryDiff = categoryRank(left.category) - categoryRank(right.category);
    if (categoryDiff !== 0) return categoryDiff;
    return String(left.name || "").localeCompare(String(right.name || ""), "zh-CN");
  });
}

export function sortRecords(records) {
  return records.slice().sort((left, right) => {
    const categoryDiff = categoryRank(left.category) - categoryRank(right.category);
    if (categoryDiff !== 0) return categoryDiff;
    return Number(right.endedAt || 0) - Number(left.endedAt || 0);
  });
}

export function mergeSnapshot(state, snapshot, now = Date.now()) {
  const next = normalizeState(state);
  const incomingProjects = Array.isArray(snapshot && snapshot.projects) ? snapshot.projects : [];
  const incomingDeletedProjects = Array.isArray(snapshot && snapshot.deletedProjects)
    ? snapshot.deletedProjects
    : [];
  const incomingRecords = Array.isArray(snapshot && snapshot.records) ? snapshot.records : [];

  const deletedById = new Map(next.deletedProjects.map((item) => [item.id, item]));
  for (const deletion of incomingDeletedProjects) {
    if (!deletion || !deletion.id) continue;
    const normalized = normalizeDeletion(deletion);
    const current = deletedById.get(normalized.id);
    if (!current || normalized.deletedAt >= current.deletedAt) {
      deletedById.set(normalized.id, normalized);
    }
  }

  const projectsById = new Map(next.projects.map((project) => [project.id, project]));
  for (const project of incomingProjects) {
    if (!project || !project.id || !project.name) continue;
    const deletion = deletedById.get(String(project.id));
    if (deletion && deletion.deletedAt >= Number(project.updatedAt || 0)) continue;
    const current = projectsById.get(project.id);
    if (!current || Number(project.updatedAt || 0) >= Number(current.updatedAt || 0)) {
      projectsById.set(project.id, normalizeProject(project));
    }
  }
  for (const deletion of deletedById.values()) {
    const project = projectsById.get(deletion.id);
    if (project && deletion.deletedAt >= Number(project.updatedAt || 0)) {
      projectsById.delete(deletion.id);
    }
  }

  const recordsById = new Map(next.records.map((record) => [record.id, record]));
  for (const record of incomingRecords) {
    if (!record || !record.id || recordsById.has(record.id)) continue;
    recordsById.set(record.id, normalizeRecord(record));
  }

  next.projects = sortProjects(Array.from(projectsById.values()));
  next.deletedProjects = Array.from(deletedById.values())
    .sort((left, right) => left.deletedAt - right.deletedAt)
    .slice(-500);
  next.records = Array.from(recordsById.values())
    .sort((left, right) => Number(left.endedAt || 0) - Number(right.endedAt || 0))
    .slice(-5000);
  next.lastSyncAt = now;
  return next;
}

export function upsertLocalProject(state, project) {
  const next = normalizeState(state);
  const normalized = normalizeProject(project);
  const deletion = next.deletedProjects.find((item) => item.id === normalized.id);
  if (deletion && deletion.deletedAt >= normalized.updatedAt) return next;
  next.deletedProjects = next.deletedProjects.filter((item) => item.id !== normalized.id);
  const index = next.projects.findIndex((item) => item.id === normalized.id);
  if (index >= 0) next.projects[index] = normalized;
  else next.projects.push(normalized);
  next.projects = sortProjects(next.projects);
  return next;
}

export function deleteLocalProject(state, projectId, deletedAt = Date.now()) {
  const next = normalizeState(state);
  const id = String(projectId || "");
  if (!id) return next;
  next.projects = next.projects.filter((project) => project.id !== id);
  const existing = next.deletedProjects.find((item) => item.id === id);
  const timestamp = Math.max(Number(deletedAt) || Date.now(), existing ? existing.deletedAt : 0);
  next.deletedProjects = next.deletedProjects
    .filter((item) => item.id !== id)
    .concat([{ id, deletedAt: timestamp }])
    .sort((left, right) => left.deletedAt - right.deletedAt)
    .slice(-500);
  return next;
}

export function recordsForSelection(state) {
  const normalized = normalizeState(state);
  return sortRecords(normalized.records.filter((record) => {
    if (record.dateKey !== normalized.selectedDate) return false;
    return normalized.categoryFilter === "全部" || record.category === normalized.categoryFilter;
  }));
}

export function summarize(records) {
  return records.reduce((result, record) => {
    result.count += 1;
    result.durationSec += Number(record.durationSec || 0);
    return result;
  }, { count: 0, durationSec: 0 });
}

export function formatDuration(secondsValue) {
  const seconds = Math.max(0, Number(secondsValue) || 0);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remain = seconds % 60;
  if (hours > 0) return `${hours}小时${minutes}分`;
  if (minutes > 0) return `${minutes}分${remain > 0 ? `${remain}秒` : ""}`;
  return `${remain}秒`;
}

export function formatTime(timestamp) {
  const date = new Date(timestamp);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function parseIncoming(payload) {
  let value = payload;
  for (let depth = 0; depth < 4; depth += 1) {
    if (typeof value === "string") {
      try {
        value = JSON.parse(value);
        continue;
      } catch (error) {
        return null;
      }
    }
    if (value && value.protocol === PROTOCOL) return value;
    if (value && typeof value === "object" && value.data !== undefined) {
      value = value.data;
      continue;
    }
    break;
  }
  return value && value.protocol === PROTOCOL ? value : null;
}

export function buildEnvelope(type, data) {
  return JSON.stringify({ protocol: PROTOCOL, type, sentAt: Date.now(), data });
}

function normalizeProject(project) {
  const category = CATEGORIES.includes(project.category) ? project.category : "其他";
  return {
    id: String(project.id),
    name: String(project.name).trim().slice(0, 24),
    category,
    color: project.color || CATEGORY_COLORS[category],
    updatedAt: Number(project.updatedAt) || Date.now()
  };
}

function normalizeRecord(record) {
  const category = CATEGORIES.includes(record.category) ? record.category : "其他";
  const endedAt = Number(record.endedAt) || Date.now();
  return {
    id: String(record.id),
    projectId: String(record.projectId || ""),
    projectName: String(record.projectName || "未命名项目").slice(0, 24),
    category,
    startedAt: Number(record.startedAt) || endedAt,
    endedAt,
    durationSec: Math.max(0, Number(record.durationSec) || 0),
    dateKey: isDateKey(record.dateKey) ? record.dateKey : localDateKey(endedAt)
  };
}

function normalizeDeletedProjects(value) {
  if (!Array.isArray(value)) return [];
  const byId = new Map();
  for (const item of value) {
    if (!item || !item.id) continue;
    const normalized = normalizeDeletion(item);
    const current = byId.get(normalized.id);
    if (!current || normalized.deletedAt >= current.deletedAt) {
      byId.set(normalized.id, normalized);
    }
  }
  return Array.from(byId.values())
    .sort((left, right) => left.deletedAt - right.deletedAt)
    .slice(-500);
}

function normalizeDeletion(value) {
  return {
    id: String(value.id),
    deletedAt: Number(value.deletedAt) || Date.now()
  };
}

function pad(value) {
  return String(value).padStart(2, "0");
}

