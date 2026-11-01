import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { CloudAccountSection } from "@/components/settings/CloudAccountSection";
import { useCloudStore } from "@/stores/cloudStore";

for (const officialAi of [false, true]) {
  it(`does not treat zero Cloud credits as unlimited (officialAi=${officialAi})`, () => {
    useCloudStore.setState({
      load: vi.fn(async () => {}), sessionState: "authenticated", usage: null, devices: [], conflicts: [],
      account: {state:"authenticated",loggedIn:true,email:"quota-test@example.com",plan:"free",syncEnabled:false,baseUrl:"http://localhost",entitlements:{officialAi,cloudSync:false,maxDevices:1,monthlyAiCredits:0,storageBytes:0,devicesUsed:1,monthlyAiCreditsUsed:0,cloudStorageBytesUsed:0,plan:"free"}},
    });
    render(<CloudAccountSection />);
    expect(screen.queryByText(/不限/)).not.toBeInTheDocument();
    expect(screen.getByText(officialAi ? "0 / 0" : "未开通官方 AI")).toBeInTheDocument();
    expect(screen.getByRole("button",{name:/^学涯云/})).toBeDisabled();
  });
}
