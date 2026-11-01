# Microsoft Store 列表素材

## 应用信息占位

- 应用名称：Orbit
- 分类：Education
- 简短描述：把好奇心变成一条不断生长的学习路径。
- 完整描述：Orbit 把资料整理成一条会生长的学习路线：来源 → 综合 → 路线 → 学习 → 提问 → 诊断 → 调整 → 复习。支持真实账号、AI 路线、节点课程、导师问答、测验与学习进度。
- 关键词：学习, AI, 学习路线, 知识管理, 教育, 自学
- 支持 URL：`[支持 URL]`
- 隐私政策 URL：`[隐私政策 URL]`
- 版权：Copyright © 2026 Verruckt

## 图标素材

由 `scripts/build-store-assets.ps1` 生成：

- `Orbit_300x300.png`
- `Orbit_150x150.png`
- `Orbit_71x71.png`

## 截图

当前已生成 1440x900 桌面截图：

- `screenshots/01-route.png`：学习路线首页
- `screenshots/02-lesson.png`：节点课程内容
- `screenshots/03-tutor.png`：真实 AI 导师问答

上架前建议补充：

- 至少 4 张桌面截图，尺寸 1366x768 或更大。
- 一条展示注册/登录流程的截图。
- 一条展示资料导入的截图。
- 一条展示测验/进度反馈的截图。
- 如需移动端商店，补 1080x1920 竖屏截图。

## 合规静态站点

`store/site/` 可直接部署到任意静态托管或 Nginx/Caddy：

- `index.html`
- `privacy.html`
- `terms.html`
- `support.html`
- `assets/site.css`

部署为 HTTPS 后，把 `privacy.html` 和 `support.html` 的 URL 填入 Partner Center。
上架前必须替换模板中的 `[运营主体]`、`[支持邮箱]`、`[支持 URL]`、`[适用法律/地区]`。

## 生成命令

```powershell
pwsh -File scripts\build-store-assets.ps1
```

截图需要通过 Chrome CDP 在已登录状态下生成，参考 `.orbit-server/e2e/store-shots.txt`。
