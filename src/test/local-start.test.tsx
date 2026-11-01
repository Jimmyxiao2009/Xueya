import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { OnboardingGate } from "@/components/onboarding/OnboardingGate";
import { useCloudStore } from "@/stores/cloudStore";
import { useThemeStore } from "@/stores/themeStore";

vi.mock("@/lib/runtime", () => ({ isManagedBrowser: () => false }));
beforeEach(() => {
  localStorage.clear();
  useThemeStore.setState({ chosen: false });
  useCloudStore.setState({ account: null, sessionState: "signedOut", load: vi.fn(() => new Promise<void>(() => {})) });
});
it("enters native local core while Cloud restoration never responds and survives remount", () => {
  const first = render(<OnboardingGate><div>本地学习</div></OnboardingGate>);
  fireEvent.click(screen.getByRole("button", { name: /仅在本设备开始/ }));
  expect(screen.getByLabelText("想学的主题")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "返回路线管理" }));
  expect(screen.getByText("本地学习")).toBeInTheDocument();
  first.unmount();
  render(<OnboardingGate><div>本地学习</div></OnboardingGate>);
  expect(screen.getByText("本地学习")).toBeInTheDocument();
});
