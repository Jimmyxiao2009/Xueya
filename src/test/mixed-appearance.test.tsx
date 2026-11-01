import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { RoadmapScreen } from "@/components/roadmap/RoadmapScreen";
import { ThemeChoices } from "@/components/onboarding/OnboardingGate";
import { parseTheme, THEME_KEY, useThemeStore } from "@/stores/themeStore";
import type { LearningRoute, RouteDetail } from "@/api";

const route: LearningRoute = { id:"r", topic:"几何", title:"几何", description:"", status:"active", kind:"math", currentNodeId:"n1", seeded:false, nodeCount:2, completedCount:1, progress:.5, metadataJson:"{}", createdAt:"2026-09-20" };
const detail: RouteDetail = { course:route, dependencies:[], nodes:["completed", "available"].map((status,i)=>({id:`n${i}`,courseId:"r",title:`概念${i}`,description:"",objective:"",orderHint:i,estimatedMinutes:10,status,kind:"lesson",confidence:1,generatedBy:"test",metadataJson:"{}"})) };

for (const background of ["lighthouse","buoy"] as const) for (const icons of ["lighthouse","buoy"] as const) for (const scheme of ["light","dark"] as const) {
  it(`${background} background + ${icons} icons + ${scheme} preserves learning actions`,()=>{
    act(()=>{ useThemeStore.getState().chooseMotif(background); useThemeStore.getState().chooseIcons(icons); useThemeStore.getState().choose(scheme); });
    const open = vi.fn();
    const {container} = render(<RoadmapScreen route={route} detail={detail} onOpenNode={open} />);
    expect(!!container.querySelector('.knowledge-sea')).toBe(background === "lighthouse");
    expect(container.querySelectorAll('.node-hit .beacon')).toHaveLength(icons === "lighthouse" ? 2 : 0);
    expect(container.querySelectorAll('.node-hit .node-buoy')).toHaveLength(icons === "buoy" ? 2 : 0);
    const trigger = screen.getByRole('button',{name:'打开概念1'});
    fireEvent.click(trigger);
    expect(open).toHaveBeenCalledWith('n1',trigger);
    expect(parseTheme(localStorage.getItem(THEME_KEY))).toEqual({visualMotif:background,iconMotif:icons,colorScheme:scheme});
  });
}

it('switches icons without resetting background or follow-system',()=>{
  act(()=>{useThemeStore.getState().chooseMotif('lighthouse');useThemeStore.getState().chooseIcons('lighthouse');useThemeStore.getState().choose('system');});
  render(<ThemeChoices />);
  fireEvent.click(screen.getByRole('button',{name:'浮标图标'}));
  expect(screen.getByRole('button',{name:'灯塔雾海'})).toHaveAttribute('aria-pressed','true');
  expect(screen.getByRole('button',{name:'跟随系统'})).toHaveAttribute('aria-pressed','true');
  expect(document.documentElement.dataset.icons).toBe('buoy');
});
