import test from "node:test";
import assert from "node:assert/strict";
import {
  PROTOCOL,
  CATEGORIES,
  emptyState,
  deleteLocalProject,
  mergeSnapshot,
  recordsForSelection,
  parseIncoming,
  shiftDate,
  sortProjects,
  summarize
} from "../src/model.js";

test("mergeSnapshot deduplicates records and keeps newer projects", () => {
  const state = emptyState();
  state.projects = [{ id: "p1", name: "旧名称", category: "工作", updatedAt: 10 }];
  state.records = [{
    id: "r1",
    projectId: "p1",
    projectName: "旧名称",
    category: "工作",
    startedAt: 100,
    endedAt: 200,
    durationSec: 100,
    dateKey: "2026-11-08"
  }];

  const next = mergeSnapshot(state, {
    projects: [{ id: "p1", name: "新名称", category: "学习", updatedAt: 20 }],
    records: [state.records[0], {
      id: "r2",
      projectId: "p1",
      projectName: "新名称",
      category: "学习",
      startedAt: 300,
      endedAt: 500,
      durationSec: 200,
      dateKey: "2026-11-08"
    }]
  }, 999);

  assert.equal(next.projects[0].name, "新名称");
  assert.equal(next.records.length, 2);
  assert.equal(next.lastSyncAt, 999);
});

test("recordsForSelection filters by date and category then sorts categories", () => {
  const state = emptyState();
  state.selectedDate = "2026-11-08";
  state.categoryFilter = "全部";
  state.records = [
    { id: "r1", category: "运动", dateKey: "2026-11-08", endedAt: 500 },
    { id: "r2", category: "工作", dateKey: "2026-11-08", endedAt: 300 },
    { id: "r3", category: "学习", dateKey: "2026-11-07", endedAt: 600 }
  ];

  assert.deepEqual(recordsForSelection(state).map((item) => item.id), ["r2", "r1"]);
  state.categoryFilter = "运动";
  assert.deepEqual(recordsForSelection(state).map((item) => item.id), ["r1"]);
});

test("parseIncoming unwraps AstroBox event payloads", () => {
  const envelope = { protocol: PROTOCOL, type: "snapshot", data: {} };
  assert.deepEqual(parseIncoming({ data: JSON.stringify(envelope) }), envelope);
  assert.deepEqual(parseIncoming(JSON.stringify({ data: JSON.stringify(envelope) })), envelope);
  assert.equal(parseIncoming("not json"), null);
});

test("date shifting and summaries are deterministic", () => {
  assert.equal(shiftDate("2026-03-01", -1), "2026-02-28");
  assert.deepEqual(summarize([{ durationSec: 10 }, { durationSec: 20 }]), {
    count: 2,
    durationSec: 30
  });
});

test("project ordering follows category order", () => {
  const sorted = sortProjects([
    { id: "4", name: "电影", category: "娱乐" },
    { id: "3", name: "跑步", category: "运动" },
    { id: "1", name: "工作", category: "工作" },
    { id: "2", name: "阅读", category: "阅读" }
  ]);
  assert.deepEqual(CATEGORIES, ["工作", "阅读", "学习", "运动", "娱乐", "个人", "自定义", "其他"]);
  assert.deepEqual(sorted.map((item) => item.id), ["1", "2", "3", "4"]);
});

test("new project categories survive synchronization", () => {
  const next = mergeSnapshot(emptyState(), {
    projects: [
      { id: "read", name: "阅读", category: "阅读", updatedAt: 1 },
      { id: "fun", name: "娱乐", category: "娱乐", updatedAt: 1 },
      { id: "personal", name: "个人", category: "个人", updatedAt: 1 },
      { id: "custom", name: "冥想", category: "自定义", updatedAt: 1 }
    ]
  }, 2);

  assert.deepEqual(next.projects.map((item) => item.category), ["阅读", "娱乐", "个人", "自定义"]);
});

test("project deletion tombstones survive later stale snapshots", () => {
  let state = emptyState();
  state.projects = [{ id: "p1", name: "会议", category: "工作", updatedAt: 10 }];
  state = deleteLocalProject(state, "p1", 20);

  const stale = mergeSnapshot(state, {
    projects: [{ id: "p1", name: "会议", category: "工作", updatedAt: 10 }]
  }, 30);
  assert.equal(stale.projects.length, 0);
  assert.deepEqual(stale.deletedProjects, [{ id: "p1", deletedAt: 20 }]);

  const recreated = mergeSnapshot(stale, {
    projects: [{ id: "p1", name: "新会议", category: "工作", updatedAt: 40 }]
  }, 50);
  assert.equal(recreated.projects[0].name, "新会议");
});

test("incoming deletion tombstones remove a local project", () => {
  const state = emptyState();
  state.projects = [{ id: "p1", name: "阅读", category: "学习", updatedAt: 10 }];
  const next = mergeSnapshot(state, {
    deletedProjects: [{ id: "p1", deletedAt: 20 }]
  }, 30);
  assert.equal(next.projects.length, 0);
});
