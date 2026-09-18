import type { AppErrorShape } from "@/api";

/**
 * Turn stable backend cloud error codes into short Orbit-style Chinese copy.
 * Components must never render raw HTTP status text or provider payloads.
 */
export function cloudErrorText(error: AppErrorShape): string {
  switch (error.code) {
    case "CLOUD_AUTH_ERROR":
      return "账号或登录状态已失效，请重新登录。";
    case "CLOUD_SESSION_EXPIRED":
      return "登录状态已过期，请重新登录。";
    case "CLOUD_ENTITLEMENT_REQUIRED":
      return "当前会员方案不包含这项云能力。";
    case "CLOUD_QUOTA_EXCEEDED":
      return "本月官方 AI 额度已用完，可切换到自定义 API 或离线模式。";
    case "CLOUD_STORAGE_QUOTA_EXCEEDED":
      return "云存储空间已满，暂时无法上传新的学习资料。";
    case "CLOUD_SYNC_CONFLICT":
      return "发现同步冲突，已保留本地副本，不会覆盖你的内容。";
    case "CLOUD_UNAVAILABLE":
      return "Verruckt Cloud 暂时不可用，本地学习不受影响。";
    case "NETWORK_ERROR":
      return "网络连接失败，稍后会自动重试同步。";
    default:
      return error.message || "操作失败，请稍后再试。";
  }
}
