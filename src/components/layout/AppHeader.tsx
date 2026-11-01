import { Icon } from "@/components/ui/Icon";
import { HeaderBlur, SoftBlur } from "@/components/ui/SoftBlur";

interface HeaderProps {
  /** The route title shown in the home header. */
  title: string;
  /** Eyebrow above the title. */
  eyebrow?: string;
  onOpenLayer: () => void;
  /** Play the title's slide-in animation again (route switch). */
  titleKey: string;
  direction: number;
  reduced: boolean;
}

/**
 * The home header *is* the route switcher: it unfolds into the acrylic
 * management layer rather than navigating away.
 */
export function AppHeader({
  title,
  eyebrow = "我的学习",
  onOpenLayer,
  titleKey,
  direction,
  reduced,
}: HeaderProps) {
  return (
    <header className="app-header">
      <HeaderBlur />
      <button
        className="subject-switch"
        onClick={onOpenLayer}
        aria-haspopup="dialog"
        aria-label={`${title}，展开路线管理`}
      >
        <span>
          <span className="eyebrow">{eyebrow}</span>
          <span
            className="subject-name"
            key={titleKey}
            ref={(node) => {
              if (!node || reduced || !node.animate) return;
              node.animate(
                [
                  { opacity: 0, transform: `translateX(${direction * 10}px)` },
                  { opacity: 1, transform: "translateX(0)" },
                ],
                { duration: 210, easing: "cubic-bezier(.2,.8,.2,1)" },
              );
            }}
          >
            {title}
          </span>
        </span>
        <Icon name="chevron" />
      </button>
      <span className="app-wordmark" aria-label="学涯">学涯</span>
    </header>
  );
}

/** The compact header used by every secondary screen. */
export function CompactHeader({
  onHome,
  trailing = "学习空间",
}: {
  onHome: () => void;
  trailing?: string;
}) {
  return (
    <header className="compact-top">
      <SoftBlur />
      <button className="brand" onClick={onHome} aria-label="回到学习路线">
        学涯
      </button>
      <span className="prototype">{trailing}</span>
    </header>
  );
}
