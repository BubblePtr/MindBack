# Base UI 交互组件迁移设计

## 背景

MindBack 当前前端是 React + Vite + Tauri，主要界面集中在 `src/App.tsx`，按钮、设置表单、下拉选择和截图详情弹窗都由原生元素和全局 CSS 直接实现。此次迁移只处理交互 primitives，不把面板、网格、摘要列表等纯布局展示结构强行改成 Base UI。

## 目标

- 引入 `@base-ui/react` 作为交互组件底层。
- 建立 `src/components/ui/` 本地封装层，页面代码不直接散落 Base UI 导入。
- 迁移按钮、弹窗、字段、输入框、文本域和下拉选择。
- 保留现有视觉风格和全局 CSS 变量，优先降低行为风险。

## 非目标

- 不重做视觉设计。
- 不大规模拆分业务页面。
- 不修改 Tauri/Rust 后端行为。
- 不为纯展示组件引入 Base UI。

## 组件边界

- `Button`：封装 Base UI Button，提供 `variant` 和 `focusableWhenDisabled` 默认行为。
- `Dialog`：封装 Base UI Dialog，用于截图详情弹窗的焦点管理、Esc 关闭和背景关闭。
- `Field` / `Input` / `Textarea`：封装 Base UI Field 与 Control，统一设置页字段结构。
- `Select`：封装 Base UI Select，替换原生 `select`，保持受控值更新。

## 验证

- 运行 `bun run build` 验证 TypeScript 与 Vite 构建。
- 如构建通过，再用页面结构检查确认 `src/App.tsx` 不再直接使用交互类原生 `button/select/input/textarea` 作为主实现。
