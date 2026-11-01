# 学涯 / Orbit 比赛现场恢复

## A. Cloud / 网络故障

1. 先点击重试。
2. 仍失败：检查手机热点 / 网络。
3. 再失败：主动切换到 BYOK 或 Offline。
4. UI 必须明确显示当前模式；不要假装仍是 Cloud。

## B. Primary Course 异常

直接切换到 Backup Demo Course。

不要现场修数据库或重建路线。

## C. App 状态异常

1. 关闭 App。
2. 重新打开。
3. 依赖 Session Resume 继续。

不要现场清数据库。

## D. 账号失效

使用预先准备好的合法 Demo 账号重新登录。

不要把 token、密码写进仓库或文档。

## E. 最终原则

- 本地学习数据优先保留。
- Cloud 失败不静默降级。
- 演示失败时缩短路径，不要现场重构。
