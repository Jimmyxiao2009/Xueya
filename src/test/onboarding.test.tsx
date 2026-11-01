import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingGate } from "@/components/onboarding/OnboardingGate";
import { SettingsScreen } from "@/screens/SettingsScreen";
import { useCloudStore } from "@/stores/cloudStore";
import { useRouteStore } from "@/stores/routeStore";
import { useThemeStore } from "@/stores/themeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useNavStore } from "@/stores/uiStore";

vi.mock("@/lib/runtime", () => ({ isManagedBrowser: () => true }));
const account = { state: "authenticated" as const, loggedIn: true, userId: "new-user", email: "learner@example.com", syncEnabled: false, baseUrl: "http://localhost" };

beforeEach(() => {
  localStorage.clear();
  useThemeStore.setState({ theme: "light", chosen: false });
  useCloudStore.setState({ account: null, sessionState: "signedOut", loading: false, busy: false, error: null, load: vi.fn(async () => {}), register: vi.fn(async () => false), devices: [], usage: null, syncStatus: null, conflicts: [] });
  useRouteStore.setState({ routes: [], error: null, load: vi.fn(async () => {}) });
  useSettingsStore.setState({ providers: [], error: null, load: vi.fn(async () => {}) });
  useNavStore.setState({ view: "home" });
});

describe("first launch with real account boundaries", () => {
  it("restores a confirmed welcome step after a fresh page load", async () => {
    localStorage.setItem("verruckt.onboarding.confirmed", "1");
    useThemeStore.setState({ chosen: false });
    useCloudStore.setState({ account, sessionState: "authenticated" });
    render(<OnboardingGate><div>学习首页</div></OnboardingGate>);
    expect(await screen.findByText("学习首页")).toBeInTheDocument();
    expect(useNavStore.getState().view).toBe("create");
    expect(screen.queryByRole("button", { name: /就用这个/ })).not.toBeInTheDocument();
  });
  it("persists a selected theme and does not enter the app without authentication", async () => {
    render(<OnboardingGate><div>学习首页</div></OnboardingGate>);
    fireEvent.click(screen.getByRole("button", { name: /夜色书房/ }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("orbit.theme")).toBe("dark");
    fireEvent.click(screen.getByRole("button", { name: /就用这个/ }));
    await screen.findByLabelText("邮箱");
    expect(screen.queryByText("学习首页")).not.toBeInTheDocument();
  });

  it("blocks mismatched passwords, sends real credentials, and opens the real route form", async () => {
    useThemeStore.setState({ chosen: true });
    const register = vi.fn(async () => { useCloudStore.setState({ account, sessionState: "authenticated" }); return true; });
    useCloudStore.setState({ register });
    render(<OnboardingGate><div>学习首页</div></OnboardingGate>);
    fireEvent.change(await screen.findByLabelText("邮箱"), { target: { value: account.email } });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "secure-pass-123" } });
    fireEvent.change(screen.getByLabelText("确认密码"), { target: { value: "wrong-pass" } });
    fireEvent.click(screen.getByRole("button", { name: "创建账号，开始探索" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("不一致");
    expect(register).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("确认密码"), { target: { value: "secure-pass-123" } });
    fireEvent.click(screen.getByRole("button", { name: "创建账号，开始探索" }));
    await screen.findByText("学习首页");
    expect(register).toHaveBeenCalledWith(account.email, "secure-pass-123");
    expect(useNavStore.getState().view).toBe("create");
    expect(useRouteStore.getState().routes).toHaveLength(0);
    expect(localStorage.getItem("password")).toBeNull();
  });

  it("keeps route loading failures recoverable instead of inventing an empty path", async () => {
    useThemeStore.setState({ chosen: true });
    useCloudStore.setState({ account, sessionState: "authenticated" });
    useRouteStore.setState({ load: vi.fn(async () => { useRouteStore.setState({ error: "服务器暂不可用" }); }) });
    render(<OnboardingGate><div>学习首页</div></OnboardingGate>);
    expect(await screen.findByRole("alert")).toHaveTextContent("服务器暂不可用");
    expect(screen.queryByRole("button", { name: /添加第一条路径/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重新读取路径" })).toBeInTheDocument();
  });

  it("exits the learning workspace immediately after logout", async () => {
    useThemeStore.setState({ chosen: true });
    useCloudStore.setState({ account, sessionState: "authenticated" });
    render(<OnboardingGate><div>学习首页</div></OnboardingGate>);
    await screen.findByText("学习首页");
    await act(async () => { useCloudStore.setState({ account: null, sessionState: "signedOut" }); });
    await waitFor(() => expect(screen.queryByText("学习首页")).not.toBeInTheDocument());
    expect(screen.getByLabelText("邮箱")).toBeInTheDocument();
  });
});

it("managed browser settings expose account and theme but no server mutation controls", async () => {
  useCloudStore.setState({ account, sessionState: "authenticated" });
  render(<SettingsScreen onBack={() => {}} />);
  expect(await screen.findByRole("button", { name: "退出登录" })).toBeInTheDocument();
  expect(screen.getByRole("group", { name: "背景明暗" })).toBeInTheDocument();
  expect(screen.getByText(/密钥在服务器上加密保存/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存模型服务" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /保存搜索设置/ })).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Cloud 服务地址")).not.toBeInTheDocument();
  expect(screen.queryByText("离线模式")).not.toBeInTheDocument();
});
