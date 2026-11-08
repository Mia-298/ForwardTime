# 正计时同步协议

手环快应用与 AstroBox 插件使用 JSON 文本消息，协议标识为 `forward-timer/v1`。

## 通用消息结构

```json
{
  "protocol": "forward-timer/v1",
  "type": "snapshot",
  "sentAt": 1794110400000,
  "data": {}
}
```

## 消息类型

| 类型 | 方向 | 用途 |
| --- | --- | --- |
| `sync_request` | 手机 -> 手环 | 请求手环发送当前项目和最近 1000 条记录 |
| `snapshot` | 手环 -> 手机 | 全量同步；手机按记录 ID 去重，项目按 `updatedAt` 合并，并传播删除标记 |
| `project_upsert` | 手机 -> 手环 | 新增或更新项目 |
| `project_delete` | 手机 -> 手环 | 删除项目；若该项目正在计时，会在本次计时完成后移除 |
| `project_deleted` | 手环 -> 手机 | 通知手机删除对应项目 |

## 数据模型

项目：

```json
{
  "id": "p_phone_1794110400000",
  "name": "客户会议",
  "category": "工作",
  "color": "#55D6BE",
  "updatedAt": 1794110400000
}
```

记录：

```json
{
  "id": "r_1794114000000_1234",
  "projectId": "p_phone_1794110400000",
  "projectName": "客户会议",
  "category": "工作",
  "startedAt": 1794110400000,
  "endedAt": 1794114000000,
  "durationSec": 3600,
  "dateKey": "2026-11-08"
}
```

记录中保留项目名称和类别快照，因此删除项目不会破坏历史数据。

删除项目时两端保存 `{ id, deletedAt }` 删除标记。断线重连后，时间不晚于删除标记的旧项目不会被全量同步恢复；同一 ID 只有以更晚的 `updatedAt` 重新创建时才会恢复。

