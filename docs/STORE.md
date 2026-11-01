# Windows 分发与 Microsoft Store 上架清单

当前已生成三种 Windows 分发产物：

| 产物 | 路径 | 用途 |
| --- | --- | --- |
| NSIS | `src-tauri/target/release/bundle/nsis/Orbit_0.1.0-rc.1_x64-setup.exe` | 普通用户下载安装 |
| MSI | `src-tauri/target/release/bundle/msi/Orbit_0.1.0_x64_en-US.msi` | 企业分发/组策略 |
| MSIX | `src-tauri/target/release/bundle/msix/Orbit_0.1.0.0_x64.msix` | Microsoft Store/旁加载 |

MSIX 由 `scripts/build-msix.ps1` 生成。脚本会：

1. 使用 `src-tauri/target/release/verruckt.exe`。
2. 从 `src-tauri/icons/128x128@2x.png` 生成 Store 所需尺寸 Logo。
3. 生成 `AppxManifest.xml`。
4. 调用 Windows SDK 的 `makeappx.exe` 打包并解包校验。
5. 若设置 `ORBIT_MSIX_PFX` 和 `ORBIT_MSIX_PFX_PASSWORD`，调用 `signtool.exe` 签名。

## 1. 当前状态

- MSIX 已成功打包并通过 `makeappx unpack` 校验。
- 当前 MSIX 未签名，Publisher 是占位值 `CN=Verruckt`。
- 该 MSIX 不能直接上架，必须先替换为 Partner Center 分配的真实 Publisher/Identity。
- 已用自签名证书验证 `scripts/build-msix.ps1` 的 `signtool` 签名分支：可成功签名，`Get-AuthenticodeSignature` 能读到证书 Subject。
- 已用临时自签名证书验证 `scripts/sign-windows.ps1 -ArtifactList`：NSIS/MSI/MSIX 三个副本均成功签名，`Get-AuthenticodeSignature` 能读到 `CN=Verruckt`；自签名根不受信任时状态为 `UnknownError`，属预期。
- 本机非管理员无法把测试证书加入 `LocalMachine\Root`，因此 `Add-AppxPackage` 安装会分别失败于：
  - 未签名：`0x80073D2C`（发布者不在未签名命名空间）
  - 自签名：`0x800B0109`（根证书不受信任）
- 这不是包本身的问题；使用受信任代码签名证书或 Partner Center 提交后可安装/上架。
- 另已验证开发者模式 loose layout：`scripts/test-msix-register.ps1` 可注册未签名布局、启动 `verruckt.exe`、确认进程运行并清理注册。说明 `AppxManifest.xml`、资源路径和可执行文件布局本身有效。
- NSIS 已做当前用户静默安装/启动/卸载实测：`scripts/test-nsis-install.ps1` 通过，安装目录为 `%LOCALAPPDATA%\Orbit`，卸载后目录清理。
- MSI 已构建；当前会话非管理员，静默安装返回 `1603` 且无残留。MSI 需要管理员权限或干净 Windows VM 复测。
- `scripts/check-store-release.ps1` 已加入发布包：会检查 Windows 包素材完整性，并扫描 `docs` 与 `store/site` 中的合规占位符；提交前应确保默认模式通过。


## 2. 合规材料

已准备模板：

- `docs/PRIVACY.md`：隐私政策
- `docs/TERMS.md`：使用条款
- `docs/SUPPORT.md`：支持说明

商店素材目录 `store/`：

- `store/Orbit_300x300.png` 等图标
- `store/screenshots/01-route.png`、`02-lesson.png`、`03-tutor.png`（1440x900）
- `store/site/`：可直接托管的隐私政策、使用条款和支持页面静态站点
- `store/README.md`：商店文案和素材清单

上架前必须替换其中的 `[支持邮箱]`、`[运营主体]`、`[隐私政策 URL]` 等占位信息，并托管为公网 HTTPS URL。

## 3. Microsoft Store 提交流程

1. 注册 Microsoft Partner Center 开发者账号。
2. 在 Partner Center 预留应用名称，记录：
   - `Package/Identity/Name`
   - `Package/Identity/Publisher`
   - `Package/Properties/PublisherDisplayName`
3. 通过环境变量注入 Partner Center 分配的身份信息，不要手改脚本：

```powershell
$env:ORBIT_MSIX_IDENTITY_NAME = 'PartnerCenter.Identity.Name'
$env:ORBIT_MSIX_PUBLISHER = 'CN=PartnerCenter Publisher'
$env:ORBIT_MSIX_PUBLISHER_DISPLAY_NAME = 'Publisher Display Name'
```

`Publisher` 必须与 Partner Center 完全一致。
4. 重新构建：

```powershell
npm run tauri build --bundles nsis,msi
pwsh -File scripts\build-msix.ps1
```

也可以直接传参：

```powershell
pwsh -File scripts\build-msix.ps1 `
  -IdentityName 'PartnerCenter.Identity.Name' `
  -Publisher 'CN=PartnerCenter Publisher' `
  -PublisherDisplayName 'Publisher Display Name'
```

5. 在 Partner Center 创建提交，上传：
   - `Orbit_0.1.0.0_x64.msix`
   - 至少 1 张应用截图
   - 商店列表文案、分类、年龄分级、隐私政策 URL、支持 URL
6. 提交认证，由 Microsoft Store 完成最终签名和发布。

## 4. 旁加载/企业签名

若要直接发给用户安装 MSIX，需要受信任的代码签名证书。证书 Subject 必须与 MSIX Manifest 的 `Publisher` 一致。

统一签名 NSIS/MSI/MSIX：

```powershell
$env:ORBIT_SIGN_PFX_PASSWORD = '***'
pwsh -File scripts\sign-windows.ps1 -Pfx C:\certs\orbit.pfx
```

也可以只签指定产物：

```powershell
pwsh -File scripts\sign-windows.ps1 `
  -Pfx C:\certs\orbit.pfx `
  -ArtifactList '.\dist\Orbit.exe,.\dist\Orbit.msi,.\dist\Orbit.msix'
```

`-ArtifactList` 使用逗号分隔，避免 `pwsh -File` 对数组参数的展开问题。

脚本行为：

1. 定位 Windows SDK 的 `signtool.exe`。
2. 对每个产物执行 SHA256 签名。
3. 默认使用 `http://timestamp.digicert.com` RFC3161 时间戳。
4. 使用 `Get-AuthenticodeSignature` 和 `signtool verify /pa` 复核。
5. 也支持 `-SkipTimestamp` / `-SkipVerify`，仅用于离线测试。

## 5. MSI/NSIS 签名

MSI/NSIS 同样通过 `scripts/sign-windows.ps1` 统一签名，不再分别手写 `signtool` 命令。签名后建议在干净 Windows VM 上复测安装和卸载。

## 6. 上架前检查

- 应用图标、Store Logo、Wide Logo、Splash Screen 已随 MSIX 打包。
- 已准备 300x300 以上的商店图标和至少 1366x768 截图。
- 已准备隐私政策、支持邮箱、官方网站。
- 已用 `scripts/test-msix-register.ps1` 验证未签名 loose MSIX 布局可注册并启动。
- 已用 `scripts/test-nsis-install.ps1` 在本机验证 NSIS 静默安装、启动和卸载。
- [ ] 待执行：在干净 Windows 虚拟机安装测试 NSIS/MSI/MSIX。
- [ ] 待确认：WebView2 Runtime 可用；离线环境需要评估随包部署 WebView2。
- [ ] 待替换：`ORBIT_PUBLIC_ORIGIN` 指向正式 HTTPS 域名。
